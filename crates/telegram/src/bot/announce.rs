//! Major-version release-notes broadcast.
//!
//! Called once at startup, after Telegram connectivity is confirmed. Patch and
//! minor releases are intentionally silent — users got spammed by every small
//! deploy. A notice goes out only when the integer major version increases
//! (2.x → 3.0), carrying `PATCH_NOTES.md` as one accumulated changelog.
//! The `announced_version` row in `meta` remembers the last announced build,
//! so a restart never messages anyone twice. Delivery is best-effort: one
//! blocked/unreachable user cannot stop the rest.

use crate::bot::send;
use crate::state::AppState;
use common::error::AppResult;
use common::version::{PATCH_NOTES, VERSION};
use std::time::Duration;
use teloxide::prelude::*;
use tracing::{info, warn};

const ANNOUNCED_VERSION_KEY: &str = "announced_version";
/// Stay below Telegram's group-wide message rate limits on large user bases.
const SEND_DELAY: Duration = Duration::from_millis(80);

/// Only the newest release block: the whole changelog outgrew Telegram's
/// 4096-char limit (announce PMs failed with "message is too long").
/// Shared with `/changelog`, which has the same size problem.
pub(crate) fn first_release_block(notes: &str) -> String {
    let first = notes.split("\n\n").next().unwrap_or(notes).trim();
    if first.chars().count() <= 3900 {
        return first.to_owned();
    }
    let mut cut: String = first.chars().take(3900).collect();
    cut.push('…');
    cut
}

fn major_of(version: &str) -> Option<u64> {
    version.trim().split('.').next()?.parse::<u64>().ok()
}

/// One notice per major version. `None` means a fresh installation, which gets
/// the current major's notes once. A rollback never re-announces.
fn notice_due(current: &str, announced: Option<&str>) -> bool {
    let Some(current_major) = major_of(current) else {
        return false;
    };
    match announced.and_then(major_of) {
        None => true,
        Some(previous_major) => current_major > previous_major,
    }
}

pub async fn announce_update(bot: &Bot, state: &AppState) -> AppResult<()> {
    let announced = state.db.get_meta(ANNOUNCED_VERSION_KEY).await?;
    if !notice_due(VERSION, announced.as_deref()) {
        return Ok(());
    }

    // The caller times this out: a truncated loop must not re-PM early users on restart.
    state.db.set_meta(ANNOUNCED_VERSION_KEY, VERSION).await?;

    let users = state.db.active_user_ids().await?;
    let notes = first_release_block(PATCH_NOTES);
    let mut sent = 0usize;
    let mut failed = 0usize;
    for (user_id, lang) in users {
        let text = common::i18n::update_notice(lang, &notes);
        match send::send_html(bot, ChatId(user_id), text).await {
            Ok(_) => sent += 1,
            Err(error) => {
                failed += 1;
                warn!(user_id, %error, "could not deliver update notice");
            }
        }
        tokio::time::sleep(SEND_DELAY).await;
    }

    info!(
        version = VERSION,
        sent, failed, "update notice broadcast complete"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_release_block_drops_history_and_caps_monsters() {
        let real = first_release_block(PATCH_NOTES);
        assert!(real.contains(VERSION), "newest block matches VERSION");
        assert!(
            !real.contains("\n\n"),
            "only one block, no accumulated history"
        );
        assert!(
            real.chars().count() <= 3901,
            "always fits a Telegram message"
        );

        let monster = "3.0.0 (2026-11-19) — x\n".to_owned() + &"• bullet\n".repeat(2000);
        let cut = first_release_block(&monster);
        assert!(cut.chars().count() <= 3901);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn parses_major_component() {
        assert_eq!(major_of("2.5.1"), Some(2));
        assert_eq!(major_of("3.0.0"), Some(3));
        assert_eq!(major_of("10.1"), Some(10));
        assert_eq!(major_of("garbage"), None);
    }

    #[test]
    fn announces_only_major_bumps() {
        assert!(notice_due("2.5.1", None), "fresh install gets one notice");
        assert!(!notice_due("2.6.0", Some("2.5.1")), "minor is silent");
        assert!(!notice_due("2.5.2", Some("2.5.1")), "patch is silent");
        assert!(notice_due("3.0.0", Some("2.5.1")), "major bump announces");
        assert!(
            !notice_due("2.6.0", Some("3.0.0")),
            "rollback must not announce"
        );
        assert!(!notice_due("bad", Some("2.5.1")));
    }
}
