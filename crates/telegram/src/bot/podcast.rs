//! Podcast subscriptions: bounded RSS/Atom polling and episode delivery.
//!
//! A feed URL is attacker-controlled, so validation goes through `FeedClient`
//! (bounded body, fixed error strings) and every user-supplied title is escaped
//! before it reaches Telegram's HTML parser. Subscription state lives in
//! Postgres: the `claim_feed` CAS makes a poll run on exactly one replica.

use crate::bot::handlers::escape_html;
use crate::bot::send::{send_html, truncate};
use crate::state::{AppState, selection};
use common::domain::{
    DownloadTarget, FeedDue, FeedEpisode, Lang, Provider, TrackKey, TrackMeta, TrackRequest,
};
use common::error::{AppResult, HandlerResult};
use common::i18n;
use common::time;
use providers::feed::{Episode, FeedError, episode_guid_fallback, fnv1a_hex, url_is_public};
use std::collections::HashSet;
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{ChatId, InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use tracing::warn;

const MAX_FEEDS_PER_USER: i64 = 10;
const MAX_FEEDS_GLOBAL: i64 = 500;
const DEFAULT_INTERVAL_SECS: i64 = 3_600;
const MAX_EPISODES_PER_PUSH: usize = 5;
const MAX_FEED_FAILS: i32 = 5;
const FEED_LEASE_SECS: i64 = 900;
const POLL_BATCH: i64 = 20;
const SUBSCRIBER_PACING: Duration = Duration::from_millis(80);

/// `/pod` command entry point: list, delete by index, or subscribe to a URL.
pub async fn handle_podcast(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    lang: Lang,
    args: Option<String>,
) -> HandlerResult {
    let Some(from) = msg.from.as_ref() else {
        return Ok(());
    };
    let user_id = from.id.0 as i64;
    let chat_id = msg.chat.id.0;
    let args = args.unwrap_or_default();
    let tokens: Vec<&str> = args.split_whitespace().collect();

    match tokens.as_slice() {
        [] | ["list"] => {
            let subs = state.db.list_subs(user_id, chat_id).await?;
            if subs.is_empty() {
                send_html(bot, msg.chat.id, i18n::podcast_empty(lang)).await?;
            } else {
                let mut text = i18n::podcast_list_title(lang).to_owned();
                for (index, sub) in subs.iter().enumerate() {
                    text.push_str(&format!("\n[{}] {}", index + 1, escape_html(&sub.title)));
                }
                send_html(bot, msg.chat.id, text).await?;
            }
        }
        ["del", index] => {
            let subs = state.db.list_subs(user_id, chat_id).await?;
            let picked = index
                .parse::<usize>()
                .ok()
                .and_then(|number| number.checked_sub(1))
                .and_then(|position| subs.get(position));
            match picked {
                Some(sub) => {
                    state.db.unsubscribe(user_id, chat_id, sub.feed_id).await?;
                    state.db.delete_orphan_feed(sub.feed_id).await?;
                    send_html(
                        bot,
                        msg.chat.id,
                        i18n::podcast_deleted(lang, &escape_html(&sub.title)),
                    )
                    .await?;
                }
                None => {
                    send_html(bot, msg.chat.id, i18n::podcast_bad_index(lang)).await?;
                }
            }
        }
        [raw_url] if raw_url.starts_with("http://") || raw_url.starts_with("https://") => {
            subscribe(state, bot, msg, lang, user_id, chat_id, raw_url).await?;
        }
        _ => {
            send_html(bot, msg.chat.id, i18n::podcast_usage(lang)).await?;
        }
    }
    Ok(())
}

/// Validate the URL, fetch its current snapshot, enforce both caps and seed
/// every existing episode as already seen (no burst of pushes on subscribe).
async fn subscribe(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    lang: Lang,
    user_id: i64,
    chat_id: i64,
    raw_url: &str,
) -> HandlerResult {
    let Some(client) = state.feed.as_ref() else {
        send_html(bot, msg.chat.id, i18n::podcast_invalid(lang)).await?;
        return Ok(());
    };
    let Ok(url) = reqwest::Url::parse(raw_url) else {
        send_html(bot, msg.chat.id, i18n::podcast_invalid(lang)).await?;
        return Ok(());
    };
    if !url_is_public(url.as_str()) {
        send_html(bot, msg.chat.id, i18n::podcast_invalid(lang)).await?;
        return Ok(());
    }
    let url = url.as_str();

    let snapshot = match client.fetch(url, None, None).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            // Never log the URL: it may carry a token in the path or query.
            warn!(%error, "feed validation failed");
            send_html(bot, msg.chat.id, i18n::podcast_invalid(lang)).await?;
            return Ok(());
        }
    };

    if state.db.count_user_feeds(user_id).await? >= MAX_FEEDS_PER_USER {
        send_html(bot, msg.chat.id, i18n::podcast_limit_user(lang)).await?;
        return Ok(());
    }

    let now = time::unix_now();
    let before = state.db.count_feeds().await?;
    let feed_id = state.db.insert_feed(url, &snapshot.title, now).await?;
    let after = state.db.count_feeds().await?;
    if after > before && after > MAX_FEEDS_GLOBAL {
        state.db.delete_orphan_feed(feed_id).await?;
        send_html(bot, msg.chat.id, i18n::podcast_limit_global(lang)).await?;
        return Ok(());
    }
    if !state.db.subscribe(user_id, chat_id, feed_id).await? {
        state.db.delete_orphan_feed(feed_id).await?;
        send_html(bot, msg.chat.id, i18n::podcast_already(lang)).await?;
        return Ok(());
    }

    let episodes: Vec<FeedEpisode> = snapshot
        .episodes
        .iter()
        .filter_map(|episode| {
            if !url_is_public(&episode.url) {
                warn!(feed_id, guid = %episode_guid(episode), "skipping unsafe enclosure URL");
                return None;
            }
            Some(FeedEpisode {
                guid: episode_guid(episode),
                title: episode.title.clone(),
                url: episode.url.clone(),
                published_at: episode.published,
            })
        })
        .collect();
    state.db.insert_episodes(feed_id, &episodes).await?;
    send_html(
        bot,
        msg.chat.id,
        i18n::podcast_subscribed(lang, &escape_html(&snapshot.title)),
    )
    .await?;
    Ok(())
}

/// Canonical GUID used both when seeding and when diffing, so entries without
/// an `<id>` are recognised on later polls.
fn episode_guid(episode: &Episode) -> String {
    let guid = episode.guid.trim();
    if guid.is_empty() {
        episode_guid_fallback(&episode.title, &episode.url)
    } else {
        guid.to_owned()
    }
}

/// Host-only fallback for a gone feed: the path/query may hold a token, so it
/// never reaches a chat message.
fn feed_host(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .unwrap_or_else(|| "feed".to_owned())
}

/// Exponential backoff with a one-day ceiling; `fail_count` is clamped so a
/// corrupt negative value cannot produce a negative shift.
pub fn backoff_next(interval_secs: i64, fail_count: i32) -> i64 {
    let shift = fail_count.clamp(0, 5) as u32;
    interval_secs.saturating_mul(1i64 << shift).min(86_400)
}

fn error_code(error: &FeedError) -> String {
    match error {
        FeedError::NotModified => "not modified".to_owned(),
        FeedError::Gone => "gone".to_owned(),
        FeedError::Http(status) => format!("http {status}"),
        FeedError::Parse => "parse".to_owned(),
        FeedError::TooLarge => "too large".to_owned(),
        FeedError::Network => "network".to_owned(),
    }
}

/// Poll the due feeds claimed by this replica. Every failure is contained
/// here — the detached sweep task must never panic.
pub async fn poll_feeds(state: &AppState, bot: &Bot) {
    let now = time::unix_now();
    let due = match state.db.due_feeds(now, POLL_BATCH).await {
        Ok(due) => due,
        Err(error) => {
            warn!(%error, "feed due scan failed");
            return;
        }
    };
    for feed in due {
        let lease = now.saturating_add(FEED_LEASE_SECS);
        let claimed = match state.db.claim_feed(feed.id, feed.next_poll_at, lease).await {
            Ok(claimed) => claimed,
            Err(error) => {
                warn!(%error, feed_id = feed.id, "feed claim failed");
                continue;
            }
        };
        if !claimed {
            continue;
        }
        if let Err(error) = poll_one(state, bot, &feed, lease, now).await {
            warn!(%error, feed_id = feed.id, "feed poll failed");
        }
        metrics::counter!("music_bot_feeds_polled_total").increment(1);
    }
}

/// Fetch one feed, reconcile its episodes and commit the new lease.
async fn poll_one(
    state: &AppState,
    bot: &Bot,
    feed: &FeedDue,
    lease: i64,
    now: i64,
) -> AppResult<()> {
    let Some(client) = state.feed.as_ref() else {
        warn!("feed client unavailable; skipping poll");
        return Ok(());
    };
    let interval = if feed.interval_secs > 0 {
        feed.interval_secs
    } else {
        DEFAULT_INTERVAL_SECS
    };
    let result = client
        .fetch(
            &feed.url,
            feed.etag.as_deref(),
            feed.last_modified.as_deref(),
        )
        .await;
    let snapshot = match result {
        Ok(snapshot) => snapshot,
        Err(FeedError::NotModified) => {
            // Empty title is a "keep the stored value" sentinel: finish_feed's
            // CASE WHEN '' THEN title preserves the existing title.
            state
                .db
                .finish_feed(
                    feed.id,
                    lease,
                    now.saturating_add(interval),
                    0,
                    None,
                    "",
                    feed.etag.as_deref(),
                    feed.last_modified.as_deref(),
                )
                .await?;
            return Ok(());
        }
        Err(error @ (FeedError::Gone | FeedError::Http(404) | FeedError::Http(410))) => {
            let fails = feed.fail_count.saturating_add(1);
            if fails >= MAX_FEED_FAILS {
                state.db.disable_feed(feed.id).await?;
                notify_gone(state, bot, feed).await;
                return Ok(());
            }
            return record_failure(
                state,
                feed,
                lease,
                now,
                interval,
                fails,
                &error_code(&error),
            )
            .await;
        }
        Err(error) => {
            let fails = feed.fail_count.saturating_add(1);
            return record_failure(
                state,
                feed,
                lease,
                now,
                interval,
                fails,
                &error_code(&error),
            )
            .await;
        }
    };

    let seen = state.db.episode_guids(feed.id).await?;
    let seen: HashSet<&str> = seen.iter().map(String::as_str).collect();
    let fresh: Vec<(String, &Episode)> = snapshot
        .episodes
        .iter()
        .filter_map(|episode| {
            let guid = episode_guid(episode);
            (!seen.contains(guid.as_str())).then_some((guid, episode))
        })
        .take(MAX_EPISODES_PER_PUSH)
        .collect();

    let episodes: Vec<FeedEpisode> = fresh
        .iter()
        .map(|(guid, episode)| FeedEpisode {
            guid: guid.clone(),
            title: episode.title.clone(),
            url: episode.url.clone(),
            published_at: episode.published,
        })
        .collect();
    state.db.insert_episodes(feed.id, &episodes).await?;
    state
        .db
        .finish_feed(
            feed.id,
            lease,
            now.saturating_add(interval),
            0,
            None,
            &snapshot.title,
            snapshot.etag.as_deref(),
            snapshot.last_modified.as_deref(),
        )
        .await?;
    metrics::counter!("music_bot_feed_episodes_total", "value" => episodes.len().to_string())
        .increment(1);
    if !fresh.is_empty() {
        deliver_episodes(state, bot, feed.id, &snapshot.title, &fresh).await;
    }
    Ok(())
}

/// Persist a failed poll and push the next attempt out exponentially.
async fn record_failure(
    state: &AppState,
    feed: &FeedDue,
    lease: i64,
    now: i64,
    interval: i64,
    fails: i32,
    code: &str,
) -> AppResult<()> {
    let next = now.saturating_add(backoff_next(interval, fails));
    // Empty title means "unchanged" — finish_feed keeps the stored title via
    // CASE WHEN '' THEN title.
    state
        .db
        .finish_feed(
            feed.id,
            lease,
            next,
            fails,
            Some(code),
            "",
            feed.etag.as_deref(),
            feed.last_modified.as_deref(),
        )
        .await?;
    metrics::counter!("music_bot_feed_polls_failed_total").increment(1);
    Ok(())
}

/// Tell subscribers a feed was disabled after repeated failures. Title comes
/// from the URL host because `FeedDue` carries no stored title.
async fn notify_gone(state: &AppState, bot: &Bot, feed: &FeedDue) {
    let host = escape_html(&feed_host(&feed.url));
    let subscribers = match state.db.subscribers_for_feed(feed.id).await {
        Ok(subscribers) => subscribers,
        Err(error) => {
            warn!(%error, feed_id = feed.id, "gone-feed subscriber lookup failed");
            return;
        }
    };
    for subscriber in subscribers {
        match bot
            .send_message(
                ChatId(subscriber.chat_id),
                i18n::podcast_gone(subscriber.language, &host),
            )
            .parse_mode(ParseMode::Html)
            .await
        {
            Ok(_) => {}
            Err(error) => {
                warn!(%error, user_id = subscriber.user_id, "gone-feed notice failed");
            }
        }
        tokio::time::sleep(SUBSCRIBER_PACING).await;
    }
}

/// Build one message + keyboard per subscriber (language differs), storing
/// each episode as a `q:` selection so the shared callback resolves it.
async fn deliver_episodes(
    state: &AppState,
    bot: &Bot,
    feed_id: i64,
    feed_title: &str,
    episodes: &[(String, &Episode)],
) {
    // Unsafe enclosures are already recorded as seen by the caller; skip only
    // the download button so a hostile feed cannot push internal URLs to the
    // worker. Log the guid, never the URL (it may carry a token).
    let episodes: Vec<(String, &Episode)> = episodes
        .iter()
        .filter(|(_, episode)| {
            let ok = url_is_public(&episode.url);
            if !ok {
                warn!(feed_id, guid = %episode_guid(episode), "skipping unsafe enclosure URL");
            }
            ok
        })
        .cloned()
        .collect();
    if episodes.is_empty() {
        return;
    }
    let requests: Vec<TrackRequest> = episodes
        .iter()
        .map(|(guid, episode)| TrackRequest {
            key: TrackKey::new(format!("podcast:{feed_id}:{}", fnv1a_hex(guid))),
            provider: Provider::Podcast,
            source_url: Some(episode.url.clone()),
            target: DownloadTarget::Url(episode.url.clone()),
            meta: Some(TrackMeta {
                title: episode.title.clone(),
                duration_secs: episode.duration_secs,
                ..Default::default()
            }),
        })
        .collect();
    let subscribers = match state.db.subscribers_for_feed(feed_id).await {
        Ok(subscribers) => subscribers,
        Err(error) => {
            warn!(%error, feed_id = feed_id, "episode subscriber lookup failed");
            return;
        }
    };
    let count = requests.len() as u32;
    for subscriber in subscribers {
        let mut text =
            i18n::podcast_new_title(subscriber.language, &escape_html(feed_title), count);
        let mut rows = Vec::with_capacity(requests.len());
        let mut selections = Vec::with_capacity(requests.len());
        for (index, (episode, request)) in episodes.iter().zip(&requests).enumerate() {
            let title = &episode.1.title;
            let escaped = escape_html(title);
            text.push_str(&format!("\n{}. {escaped}", index + 1));
            let id = state.new_selection_id();
            rows.push(vec![InlineKeyboardButton::callback(
                format!("⬇ {}", truncate(title, 60)),
                format!("q:{id}"),
            )]);
            selections.push((id, request));
        }
        if let Err(error) = state
            .put_selections(selection::TRACK, subscriber.user_id, &selections)
            .await
        {
            warn!(%error, user_id = subscriber.user_id, "could not store episode selections");
            continue;
        }
        match bot
            .send_message(ChatId(subscriber.chat_id), text)
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(rows))
            .await
        {
            Ok(_) => {}
            Err(error) => {
                warn!(%error, user_id = subscriber.user_id, "episode delivery failed");
            }
        }
        tokio::time::sleep(SUBSCRIBER_PACING).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_then_caps() {
        assert_eq!(backoff_next(60, 0), 60);
        assert_eq!(backoff_next(60, 1), 120);
        assert_eq!(backoff_next(60, 5), 1_920);
        assert_eq!(backoff_next(3_600, 9), 86_400);
        assert_eq!(backoff_next(3_600, -2), 3_600);
    }

    #[test]
    fn gone_notice_uses_host_not_full_url() {
        let url = "https://user:token@example.com/private/feed.xml?key=secret";
        assert_eq!(feed_host(url), "example.com");
        assert!(!feed_host(url).contains("token"));
        assert!(!feed_host(url).contains("private"));
    }

    #[test]
    fn canonical_guid_matches_seeded_value() {
        let episode = Episode {
            guid: String::new(),
            title: "Ep One".to_owned(),
            url: "https://x/ep1.mp3".to_owned(),
            published: None,
            duration_secs: None,
        };
        assert_eq!(episode_guid(&episode), "f1915d6ce8e9e3fb");
    }
}
