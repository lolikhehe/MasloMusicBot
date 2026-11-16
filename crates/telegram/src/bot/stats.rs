//! Last.fm scrobble statistics (`/profile`, `/lastfmstats`) and PNG collages
//! (`/collage`). All data comes from the public Last.fm API with the operator's
//! key; users only provide their public nickname via `/lastfm`.

use crate::bot::handlers::escape_html;
use crate::state::AppState;
use common::domain::Lang;
use common::error::HandlerResult;
use common::i18n;
use providers::lastfm::{LastFmClient, ScrobbleItem, StatsKind, StatsPeriod};
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, InputFile, ParseMode};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::warn;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

struct Context {
    client: LastFmClient,
    nick: String,
    lang: Lang,
}

async fn context(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    lang: Lang,
) -> Result<Option<Context>, BoxError> {
    let user_id = msg
        .from
        .as_ref()
        .map(|user| user.id.0 as i64)
        .unwrap_or(msg.chat.id.0);
    let user = state.db.ensure_user(user_id, lang).await?;
    if !user.lastfm_stats {
        bot.send_message(msg.chat.id, i18n::lastfm_stats_disabled(lang))
            .await?;
        return Ok(None);
    }
    let Some(nick) = user.lastfm_user.filter(|value| !value.is_empty()) else {
        bot.send_message(msg.chat.id, i18n::lastfm_not_connected(lang))
            .await?;
        return Ok(None);
    };
    let Some(client) = state.lastfm.clone() else {
        bot.send_message(msg.chat.id, i18n::lastfm_disabled(lang))
            .await?;
        return Ok(None);
    };
    Ok(Some(Context { client, nick, lang }))
}

fn collage_keyboard(lang: Lang, period: StatsPeriod) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for (label_en, label_uk, kind) in [
        ("Tracks", "Треки", StatsKind::Tracks),
        ("Albums", "Альбоми", StatsKind::Albums),
        ("Artists", "Виконавці", StatsKind::Artists),
    ] {
        let label = if lang == Lang::Uk { label_uk } else { label_en };
        rows.push(
            [3u8, 5, 8, 10]
                .into_iter()
                .map(|grid| {
                    InlineKeyboardButton::callback(
                        format!("{label} {grid}×{grid}"),
                        format!("lf:c:{}:{grid}:{}", kind.as_str(), period.as_str()),
                    )
                })
                .collect(),
        );
    }
    InlineKeyboardMarkup::new(rows)
}

pub async fn handle_profile(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    arg: Option<&str>,
    lang: Lang,
) -> HandlerResult {
    let Some(context) = context(state, bot, msg, lang).await? else {
        return Ok(());
    };
    let token = arg
        .and_then(|value| value.split_whitespace().next())
        .unwrap_or("");
    let period = StatsPeriod::parse(token).unwrap_or(StatsPeriod::ThreeMonths);
    let summary = match context.client.profile(&context.nick).await {
        Ok(summary) => summary,
        Err(error) => {
            warn!(%error, "last.fm profile lookup failed");
            bot.send_message(msg.chat.id, i18n::collage_failed(lang))
                .await?;
            return Ok(());
        }
    };
    let top = context
        .client
        .top_items(&context.nick, period, StatsKind::Tracks, 5)
        .await
        .unwrap_or_default();
    let top_lines: Vec<String> = top
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let title = escape_html(&item.name);
            let artist = item
                .secondary
                .as_deref()
                .map(escape_html)
                .unwrap_or_default();
            format!(
                "{}. {}{} · <b>{}</b>",
                index + 1,
                title,
                if artist.is_empty() {
                    String::new()
                } else {
                    format!(" — {artist}")
                },
                item.playcount
            )
        })
        .collect();
    let text = i18n::profile_summary(
        lang,
        &summary.name,
        summary.playcount,
        summary.tracks,
        summary.albums,
        summary.artists,
        summary.country.as_deref(),
        period.label(lang),
        &top_lines,
    );
    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(collage_keyboard(lang, period))
        .await?;
    Ok(())
}

fn parse_grid(value: &str) -> Option<u8> {
    let (left, right) = value.split_once(['x', 'X', '×'])?;
    let left = left.parse::<u8>().ok()?;
    let right = right.parse::<u8>().ok()?;
    if left == right && (3..=10).contains(&left) {
        Some(left)
    } else {
        None
    }
}

pub async fn handle_collage(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    arg: Option<&str>,
    lang: Lang,
) -> HandlerResult {
    let Some(context) = context(state, bot, msg, lang).await? else {
        return Ok(());
    };
    let tokens: Vec<&str> = arg.unwrap_or("").split_whitespace().collect();
    let kind = tokens
        .first()
        .and_then(|value| StatsKind::parse(value))
        .unwrap_or(StatsKind::Tracks);
    let grid = tokens
        .get(1)
        .and_then(|value| parse_grid(value))
        .unwrap_or(5);
    let period = tokens
        .get(2)
        .and_then(|value| StatsPeriod::parse(value))
        .unwrap_or(StatsPeriod::ThreeMonths);
    if tokens.is_empty() || tokens.len() > 3 {
        bot.send_message(msg.chat.id, i18n::collage_usage(lang))
            .await?;
        return Ok(());
    }
    send_collage(state, bot, msg.chat.id, &context, kind, grid, period).await
}

/// Shared by the command and the `/profile` keyboard callback.
async fn send_collage(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    context: &Context,
    kind: StatsKind,
    grid: u8,
    period: StatsPeriod,
) -> HandlerResult {
    let items: Vec<ScrobbleItem> = context
        .client
        .top_items(
            &context.nick,
            period,
            kind,
            usize::from(grid) * usize::from(grid),
        )
        .await
        .unwrap_or_default();
    if items.is_empty() {
        bot.send_message(chat, i18n::collage_failed(context.lang))
            .await?;
        return Ok(());
    }
    // Fetch real artwork from the URLs Last.fm already returned. Bounded
    // concurrency keeps this gentle on the operator's network, and nothing is
    // cached on disk.
    let mut covers: Vec<Option<Vec<u8>>> = vec![None; items.len()];
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .unwrap_or_default();
    let semaphore = Arc::new(Semaphore::new(4));
    let mut tasks = JoinSet::new();
    for (index, item) in items.iter().enumerate() {
        if let Some(url) = item.image_url.clone() {
            let client = client.clone();
            let semaphore = semaphore.clone();
            tasks.spawn(async move {
                let _permit = semaphore.acquire().await.ok()?;
                let response = client.get(&url).send().await.ok()?;
                if !response.status().is_success() {
                    return None;
                }
                Some((index, response.bytes().await.ok()?.to_vec()))
            });
        }
    }
    while let Some(result) = tasks.join_next().await {
        if let Ok(Some((index, bytes))) = result {
            covers[index] = Some(bytes);
        }
    }
    let cells: Vec<collage::CollageItem> = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| collage::CollageItem {
            name: item.name,
            secondary: item.secondary,
            playcount: item.playcount,
            cover: covers.get(index).cloned().flatten(),
        })
        .collect();
    let label = match (kind, context.lang) {
        (StatsKind::Tracks, Lang::En) => "Top tracks",
        (StatsKind::Tracks, Lang::Uk) => "Топ треків",
        (StatsKind::Albums, Lang::En) => "Top albums",
        (StatsKind::Albums, Lang::Uk) => "Топ альбомів",
        (StatsKind::Artists, Lang::En) => "Top artists",
        (StatsKind::Artists, Lang::Uk) => "Топ виконавців",
    };
    let title = format!("{label} · {}", period.label(context.lang));
    let path = state
        .config
        .tmp_dir
        .join(format!("collage-{}.png", uuid::Uuid::new_v4().simple()));
    if let Err(error) = collage::render(&title, &cells, grid, &path) {
        warn!(%error, "collage render failed");
        bot.send_message(chat, i18n::collage_failed(context.lang))
            .await?;
        return Ok(());
    }
    let result = bot.send_photo(chat, InputFile::file(&path)).await;
    let _ = tokio::fs::remove_file(&path).await;
    if let Err(error) = result {
        warn!(%error, "collage upload failed");
    }
    Ok(())
}

/// Resolve the `lf:c:<kind>:<grid>:<period>` callback from `/profile`.
pub async fn handle_collage_callback(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    kind: StatsKind,
    grid: u8,
    period: StatsPeriod,
) -> HandlerResult {
    let user_id = query.from.id.0 as i64;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let lang = state.db.ensure_user(user_id, Lang::En).await?.lang;
    let user = state.db.ensure_user(user_id, lang).await?;
    let Some(nick) = user.lastfm_user.filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    let Some(client) = state.lastfm.clone() else {
        return Ok(());
    };
    let context = Context { client, nick, lang };
    send_collage(state, bot, message.chat.id, &context, kind, grid, period).await
}
