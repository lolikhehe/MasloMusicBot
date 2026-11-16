//! One-shot release-notes broadcast.
//!
//! Called once at startup, after Telegram connectivity is confirmed. The
//! current `VERSION` is compared against the `announced_version` row in `meta`;
//! when it differs, every known (non-banned) user gets a short PM with the
//! changelog and the row is updated, so a restart with the same build never
//! messages anyone twice. Delivery is best-effort: one blocked/unreachable user
//! cannot stop the rest, and the version is still recorded after the sweep.

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

pub async fn announce_update(bot: &Bot, state: &AppState) -> AppResult<()> {
    if state.db.get_meta(ANNOUNCED_VERSION_KEY).await? == Some(VERSION.to_owned()) {
        return Ok(());
    }

    let users = state.db.active_user_ids().await?;
    let mut sent = 0usize;
    let mut failed = 0usize;
    for (user_id, lang) in users {
        let text = common::i18n::update_notice(lang, PATCH_NOTES);
        match send::send_html(bot, ChatId(user_id), text).await {
            Ok(_) => sent += 1,
            Err(error) => {
                failed += 1;
                warn!(user_id, %error, "could not deliver update notice");
            }
        }
        tokio::time::sleep(SEND_DELAY).await;
    }

    state.db.set_meta(ANNOUNCED_VERSION_KEY, VERSION).await?;
    info!(
        version = VERSION,
        sent, failed, "update notice broadcast complete"
    );
    Ok(())
}
