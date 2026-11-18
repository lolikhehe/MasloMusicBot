//! Opt-in personal digests and the Postgres-backed planner.
//!
//! Every replica runs the same one-minute sweep. A due row is claimed with a
//! compare-and-swap on `next_send_at`, so only the replica that wins the CAS
//! sends the message. Quiet hours postpone the row without resetting the
//! schedule; a Telegram delivery failure disables the plan instead of retrying
//! forever (the usual cause is a blocked/unknown user).

use crate::state::AppState;
use common::domain::Lang;
use common::error::HandlerResult;
use common::i18n;
use common::time;
use teloxide::prelude::*;
use teloxide::types::{ChatId, ParseMode};
use tokio::time::MissedTickBehavior;
use tracing::warn;

const SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
const DAILY_SECS: i64 = 86_400;
const WEEKLY_SECS: i64 = 7 * 86_400;
const SEND_HOUR_UTC: i64 = 9;

/// Next 09:00 UTC strictly after `now` (next Monday for a weekly plan).
/// 1970-01-01 was a Thursday, hence the `+3` when mapping to 0 = Monday.
pub fn next_send_time(now: i64, weekly: bool) -> i64 {
    let day = now.div_euclid(86_400);
    let seconds = now.rem_euclid(86_400);
    let mut target_day = day + i64::from(seconds >= SEND_HOUR_UTC * 3_600);
    if weekly {
        let weekday = (target_day + 3).rem_euclid(7); // 0 = Monday
        target_day += (7 - weekday) % 7;
    }
    target_day * 86_400 + SEND_HOUR_UTC * 3_600
}

fn parse_quiet(value: &str) -> Option<(i16, i16)> {
    let (start, end) = value.split_once('-')?;
    let start = start.parse::<i16>().ok()?;
    let end = end.parse::<i16>().ok()?;
    ((0..24).contains(&start) && (0..24).contains(&end)).then_some((start, end))
}

fn quiet_from_schedule(
    schedule: Option<&common::domain::DigestSchedule>,
) -> (i16, i16) {
    schedule
        .map(|row| (row.quiet_start_utc, row.quiet_end_utc))
        .unwrap_or((0, 0))
}

pub async fn handle_digest(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    arg: Option<&str>,
    lang: Lang,
) -> HandlerResult {
    let Some(from) = msg.from.as_ref() else {
        return Ok(());
    };
    let user_id = from.id.0 as i64;
    let tokens: Vec<&str> = arg.unwrap_or("").split_whitespace().collect();
    let now = time::unix_now();
    let current = state.db.get_digest_schedule(user_id).await?;

    match tokens.as_slice() {
        [] | ["status"] => {
            let text = match current {
                Some(row) => i18n::digest_status(
                    lang,
                    i18n::digest_period_label(lang, row.period == "daily"),
                    &time::utc_stamp(row.next_send_at),
                    &i18n::digest_window_label(
                        lang,
                        row.quiet_start_utc,
                        row.quiet_end_utc,
                    ),
                    row.enabled,
                ),
                None => i18n::digest_not_enabled(lang).to_owned(),
            };
            bot.send_message(msg.chat.id, text)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        ["off"] => {
            state.db.disable_digest(user_id).await?;
            bot.send_message(msg.chat.id, i18n::digest_disabled(lang))
                .await?;
        }
        ["daily"] | ["weekly"] => {
            let weekly = tokens[0] == "weekly";
            let (quiet_start, quiet_end) = quiet_from_schedule(current.as_ref());
            let next = next_send_time(now, weekly);
            state
                .db
                .set_digest_schedule(
                    user_id,
                    if weekly { "weekly" } else { "daily" },
                    true,
                    next,
                    quiet_start,
                    quiet_end,
                )
                .await?;
            let text = i18n::digest_enabled(
                lang,
                i18n::digest_period_label(lang, !weekly),
                &time::utc_stamp(next),
            );
            bot.send_message(msg.chat.id, text)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        ["quiet", value] => {
            let (quiet_start, quiet_end) = if value.eq_ignore_ascii_case("off") {
                (0, 0)
            } else if let Some(window) = parse_quiet(value) {
                window
            } else {
                bot.send_message(msg.chat.id, i18n::digest_usage(lang))
                    .parse_mode(ParseMode::Html)
                    .await?;
                return Ok(());
            };
            let (period, enabled, next) = current
                .as_ref()
                .map(|row| (row.period.clone(), row.enabled, row.next_send_at))
                .unwrap_or_else(|| ("daily".to_owned(), true, next_send_time(now, false)));
            state
                .db
                .set_digest_schedule(
                    user_id,
                    &period,
                    enabled,
                    next,
                    quiet_start,
                    quiet_end,
                )
                .await?;
            let text = i18n::digest_quiet_set(
                lang,
                &i18n::digest_window_label(lang, quiet_start, quiet_end),
            );
            bot.send_message(msg.chat.id, text)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        _ => {
            bot.send_message(msg.chat.id, i18n::digest_usage(lang))
                .parse_mode(ParseMode::Html)
                .await?;
        }
    }
    Ok(())
}

/// Spawn the detached one-minute planner. Safe to call once per process; with
/// several replicas the database CAS is the actual lock.
pub fn spawn(bot: Bot, state: AppState) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(error) = sweep(&bot, &state).await {
                warn!(%error, "digest sweep failed");
            }
        }
    });
}

async fn sweep(bot: &Bot, state: &AppState) -> common::error::AppResult<()> {
    let now = time::unix_now();
    let due = state.db.due_digests(now, 50).await?;
    for entry in due {
        if time::in_quiet_window(
            time::utc_hour(now),
            entry.quiet_start_utc,
            entry.quiet_end_utc,
        ) {
            let _ = state
                .db
                .defer_digest(entry.user_id, entry.next_send_at, now + 3_600)
                .await;
            continue;
        }

        let weekly = entry.period == "weekly";
        let window = if weekly { WEEKLY_SECS } else { DAILY_SECS };
        let summary = state
            .db
            .digest_summary(entry.user_id, now.saturating_sub(window))
            .await?;
        let text = i18n::digest_message(
            entry.lang,
            i18n::digest_period_label(entry.lang, !weekly),
            summary.listens,
            summary.tracks,
            &summary.genres,
            &summary.top_tracks,
        );
        let next = next_send_time(now, weekly);
        match bot
            .send_message(ChatId(entry.user_id), text)
            .parse_mode(ParseMode::Html)
            .await
        {
            Ok(_) => {
                if state
                    .db
                    .claim_digest(entry.user_id, entry.next_send_at, next)
                    .await?
                {
                    metrics::counter!("music_bot_digests_sent_total").increment(1);
                }
            }
            Err(error) => {
                warn!(%error, user_id = entry.user_id, "digest delivery failed; disabling plan");
                let _ = state.db.disable_digest(entry.user_id).await;
                metrics::counter!("music_bot_digests_failed_total").increment(1);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_send_is_at_nine_utc() {
        // 2023-11-14 22:13 UTC -> next day 09:00 UTC.
        let now = 1_700_000_000;
        let next = next_send_time(now, false);
        assert_eq!(time::utc_stamp(next), "2023-11-15 09:00");
    }

    #[test]
    fn weekly_next_send_is_a_monday() {
        // 2023-11-14 is a Tuesday -> next Monday 2023-11-20.
        let now = 1_700_000_000;
        let next = next_send_time(now, true);
        assert_eq!(time::utc_stamp(next), "2023-11-20 09:00");
    }

    #[test]
    fn parses_quiet_windows() {
        assert_eq!(parse_quiet("23-7"), Some((23, 7)));
        assert_eq!(parse_quiet("9-18"), Some((9, 18)));
        assert_eq!(parse_quiet("24-7"), None);
        assert_eq!(parse_quiet("x-y"), None);
    }
}
