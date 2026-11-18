//! Last.fm scrobble statistics (`/profile`, `/lastfmstats`) and PNG collages
//! (`/collage`). All data comes from the public Last.fm API with the operator's
//! key; users only provide their public nickname via `/lastfm`.

use crate::bot::handlers::escape_html;
use crate::state::AppState;
use common::domain::{Lang, UserRecord};
use common::error::HandlerResult;
use common::i18n;
use providers::external::ArtworkKind;
use providers::http;
use providers::lastfm::{LastFmClient, ScrobbleItem, StatsKind, StatsPeriod};
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, InputFile, ParseMode};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::warn;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Hard ceiling for one collage cover. Last.fm/Deezer artwork is far smaller;
/// the cap only stops a misbehaving host from buffering an unbounded body.
const MAX_COVER_BYTES: u64 = 4 * 1024 * 1024;
/// Simultaneous artwork resolutions/downloads per collage.
const COVER_PARALLELISM: usize = 6;

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

/// First level of the `/profile` menu: short labels that fit a phone screen.
/// A 12-button grid with names like "Треки 10×10" gets truncated by Telegram.
fn collage_kind_keyboard(lang: Lang, period: StatsPeriod) -> InlineKeyboardMarkup {
    let (tracks, albums, artists) = if lang == Lang::Uk {
        ("🎵 Треки", "💿 Альбоми", "👤 Виконавці")
    } else {
        ("🎵 Tracks", "💿 Albums", "👤 Artists")
    };
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(tracks, format!("lf:m:tracks:{}", period.as_str())),
        InlineKeyboardButton::callback(albums, format!("lf:m:albums:{}", period.as_str())),
        InlineKeyboardButton::callback(artists, format!("lf:m:artists:{}", period.as_str())),
    ]])
}

/// Second level: grid sizes for the chosen kind, plus a back button.
fn collage_grid_keyboard(lang: Lang, kind: StatsKind, period: StatsPeriod) -> InlineKeyboardMarkup {
    let back = if lang == Lang::Uk {
        "« Назад"
    } else {
        "« Back"
    };
    InlineKeyboardMarkup::new(vec![
        [3u8, 5, 8, 10]
            .into_iter()
            .map(|grid| {
                InlineKeyboardButton::callback(
                    format!("{grid}×{grid}"),
                    format!("lf:c:{}:{grid}:{}", kind.as_str(), period.as_str()),
                )
            })
            .collect(),
        vec![InlineKeyboardButton::callback(
            back,
            format!("lf:b:{}", period.as_str()),
        )],
    ])
}

/// Edit the existing `/profile` message in place when navigating the menu.
/// `kind = None` rebuilds the first level (the back button).
pub async fn handle_collage_menu_callback(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    kind: Option<StatsKind>,
    period: StatsPeriod,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let user = state
        .db
        .ensure_user(query.from.id.0 as i64, Lang::En)
        .await?;
    let markup = match kind {
        Some(kind) => collage_grid_keyboard(user.lang, kind, period),
        None => collage_kind_keyboard(user.lang, period),
    };
    bot.edit_message_reply_markup(message.chat.id, message.id)
        .reply_markup(markup)
        .await?;
    Ok(())
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
    // Both calls hit the same API host; running them together halves the wait.
    let (summary, top) = tokio::join!(
        context.client.profile(&context.nick),
        context
            .client
            .top_items(&context.nick, period, StatsKind::Tracks, 5),
    );
    let summary = match summary {
        Ok(summary) => summary,
        Err(error) => {
            warn!(%error, "last.fm profile lookup failed");
            bot.send_message(msg.chat.id, i18n::collage_failed(lang))
                .await?;
            return Ok(());
        }
    };
    let top = top.unwrap_or_default();
    let top_lines: Vec<String> = top
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let title = escape_html(&item.name);
            match item.secondary.as_deref().map(escape_html) {
                Some(artist) if !artist.is_empty() => format!(
                    "{}. {title} — {artist} · <b>{}</b>",
                    index + 1,
                    item.playcount
                ),
                _ => format!("{}. {title} · <b>{}</b>", index + 1, item.playcount),
            }
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
        .reply_markup(collage_kind_keyboard(lang, period))
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
    // Last.fm artwork is used when real; its grey-star placeholder is filtered
    // out in the client, in which case the public Deezer catalog is asked for
    // the actual cover. Unique queries and unique URLs are each resolved once,
    // so a 10x10 "top albums" chart with repeated records does far less work
    // than before, and every duplicate cell shares one `Arc<[u8]>`.
    let artwork_kind = match kind {
        StatsKind::Tracks => ArtworkKind::Track,
        StatsKind::Albums => ArtworkKind::Album,
        StatsKind::Artists => ArtworkKind::Artist,
    };
    let mut urls: Vec<Option<String>> = items.iter().map(|item| item.image_url.clone()).collect();
    let query_for = |item: &ScrobbleItem| match kind {
        StatsKind::Artists => item.name.clone(),
        StatsKind::Tracks | StatsKind::Albums => format!(
            "{} {}",
            item.secondary.as_deref().unwrap_or_default(),
            item.name
        ),
    };

    let mut pending: HashMap<String, (String, Vec<usize>)> = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        if urls[index].is_some() {
            continue;
        }
        let query = query_for(item);
        let cache_key = format!("{}:{}", artwork_kind.as_str(), query.trim().to_lowercase());
        pending
            .entry(cache_key)
            .or_insert_with(|| (query, Vec::new()))
            .1
            .push(index);
    }

    let mut resolves = JoinSet::new();
    let resolve_semaphore = Arc::new(Semaphore::new(COVER_PARALLELISM));
    for (cache_key, (query, indexes)) in pending {
        if let Some(cached) = state.artwork_cache.get(&cache_key).await {
            for index in indexes {
                urls[index] = cached.clone();
            }
            continue;
        }
        let state = state.clone();
        let semaphore = resolve_semaphore.clone();
        resolves.spawn(async move {
            let _permit = semaphore.acquire().await.ok()?;
            let resolved = state.manager.artwork_url(artwork_kind, &query).await;
            state
                .artwork_cache
                .insert(cache_key, resolved.clone())
                .await;
            Some((indexes, resolved))
        });
    }
    while let Some(result) = resolves.join_next().await {
        if let Ok(Some((indexes, resolved))) = result {
            for index in indexes {
                urls[index] = resolved.clone();
            }
        }
    }

    let mut by_url: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, url) in urls.into_iter().enumerate() {
        if let Some(url) = url {
            by_url.entry(url).or_default().push(index);
        }
    }
    let mut covers: Vec<Option<Arc<[u8]>>> = vec![None; items.len()];
    let mut downloads = JoinSet::new();
    let download_semaphore = Arc::new(Semaphore::new(COVER_PARALLELISM));
    for (url, indexes) in by_url {
        let client = state.http.clone();
        let semaphore = download_semaphore.clone();
        downloads.spawn(async move {
            let _permit = semaphore.acquire().await.ok()?;
            let bytes = http::fetch_bytes(&client, &url, MAX_COVER_BYTES).await?;
            // Empty/1x1 responses are not useful covers; let the cell fall back
            // to the plain dark placeholder.
            (bytes.len() > 512).then_some((indexes, bytes))
        });
    }
    while let Some(result) = downloads.join_next().await {
        if let Ok(Some((indexes, bytes))) = result {
            let shared: Arc<[u8]> = bytes.into();
            for index in indexes {
                covers[index] = Some(shared.clone());
            }
        }
    }

    let cells: Vec<collage::CollageItem> = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| collage::CollageItem {
            name: item.name,
            secondary: item.secondary,
            playcount: item.playcount,
            cover: covers[index].take(),
        })
        .collect();
    let label = match (kind, context.lang) {
        (
            StatsKind::Tracks,
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be,
        ) => "Top tracks",
        (StatsKind::Tracks, Lang::Uk) => "Топ треків",
        (
            StatsKind::Albums,
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be,
        ) => "Top albums",
        (StatsKind::Albums, Lang::Uk) => "Топ альбомів",
        (
            StatsKind::Artists,
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be,
        ) => "Top artists",
        (StatsKind::Artists, Lang::Uk) => "Топ виконавців",
    };
    let title = format!("{label} · {}", period.label(context.lang));
    let path = state
        .config
        .tmp_dir
        .join(format!("collage-{}.png", uuid::Uuid::new_v4().simple()));
    // Decoding 100 covers and encoding a multi-megapixel PNG is CPU work;
    // keeping it off the async workers matters when several charts overlap.
    let render_path = path.clone();
    let rendered =
        tokio::task::spawn_blocking(move || collage::render(&title, &cells, grid, &render_path))
            .await;
    match rendered {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            warn!(%error, "collage render failed");
            bot.send_message(chat, i18n::collage_failed(context.lang))
                .await?;
            return Ok(());
        }
        Err(error) => {
            warn!(%error, "collage render task failed");
            bot.send_message(chat, i18n::collage_failed(context.lang))
                .await?;
            return Ok(());
        }
    }
    let result = bot.send_photo(chat, InputFile::file(&path)).await;
    let _ = tokio::fs::remove_file(&path).await;
    if let Err(error) = result {
        warn!(%error, "collage upload failed");
    }
    Ok(())
}

fn genre_keyboard(bars: u8, period: StatsPeriod, lang: Lang) -> InlineKeyboardMarkup {
    let row: Vec<InlineKeyboardButton> = [5u8, 8, 10, 12, 15, 20]
        .into_iter()
        .map(|value| {
            let check = if value == bars { "✓ " } else { "" };
            InlineKeyboardButton::callback(
                format!("{check}{value}"),
                format!("gn:c:{value}:{}", period.as_str()),
            )
        })
        .collect();
    let periods: Vec<InlineKeyboardButton> = [
        StatsPeriod::Week,
        StatsPeriod::Month,
        StatsPeriod::ThreeMonths,
        StatsPeriod::SixMonths,
        StatsPeriod::Year,
        StatsPeriod::Overall,
    ]
    .into_iter()
    .map(|candidate| {
        let check = if candidate == period { "✓ " } else { "" };
        InlineKeyboardButton::callback(
            format!("{check}{}", candidate.label(lang)),
            format!("gn:p:{}:{bars}", candidate.as_str()),
        )
    })
    .collect();
    InlineKeyboardMarkup::new(vec![row, periods])
}

fn genre_display(value: &str) -> String {
    if value.eq_ignore_ascii_case("r&b") {
        return "R&B".to_owned();
    }
    value
        .split_whitespace()
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

pub async fn handle_genres(
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
    let user = state.db.ensure_user(user_id, lang).await?;
    if !user.genre_analysis {
        bot.send_message(msg.chat.id, i18n::genre_chart_disabled(lang))
            .await?;
        return Ok(());
    }
    let tokens: Vec<&str> = arg.unwrap_or("").split_whitespace().collect();
    let invalid = tokens
        .iter()
        .any(|value| value.parse::<u8>().is_err() && StatsPeriod::parse(value).is_none());
    if invalid || tokens.len() > 2 {
        bot.send_message(msg.chat.id, i18n::genre_chart_usage_period(lang))
            .await?;
        return Ok(());
    }
    let bars = tokens
        .iter()
        .find_map(|value| value.parse::<u8>().ok())
        .unwrap_or(10)
        .clamp(5, 20);
    let period = tokens
        .iter()
        .find_map(|value| StatsPeriod::parse(value))
        .unwrap_or(StatsPeriod::Overall);
    send_genre_chart(state, bot, msg.chat.id, user_id, &user, bars, period).await
}

async fn send_genre_chart(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    user_id: i64,
    user: &UserRecord,
    bars: u8,
    period: StatsPeriod,
) -> HandlerResult {
    let lang = user.lang;
    if !user.genre_analysis {
        bot.send_message(chat, i18n::genre_chart_disabled(lang))
            .await?;
        return Ok(());
    }
    // `Overall` has no window: use epoch, not "now" (which filters out every
    // recorded listen and made /genres always report "no data").
    let since = period.window_secs().map_or(0, |window| unix_now() - window);
    let rows = state
        .db
        .top_genres_since(user_id, usize::from(bars), since)
        .await?;
    if rows.is_empty() {
        bot.send_message(chat, i18n::genre_chart_empty(lang))
            .await?;
        return Ok(());
    }
    let total_tracks = state.db.genre_track_count_since(user_id, since).await?;
    let items: Vec<collage::BarItem> = rows
        .iter()
        .map(|(genre, listens)| collage::BarItem {
            label: genre_display(genre),
            value: (*listens).max(1) as u64,
        })
        .collect();
    let title = i18n::genre_chart_title(lang);
    let subtitle = format!(
        "{} · {}",
        i18n::genre_chart_subtitle(lang, total_tracks, items.len()),
        period.label(lang)
    );
    let path = state
        .config
        .tmp_dir
        .join(format!("genres-{}.png", uuid::Uuid::new_v4().simple()));
    let render_path = path.clone();
    let title_for_render = title.to_owned();
    let subtitle_for_render = subtitle.clone();
    let rendered = tokio::task::spawn_blocking(move || {
        collage::render_bars(
            &title_for_render,
            &subtitle_for_render,
            &items,
            &render_path,
        )
    })
    .await;
    match rendered {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            warn!(%error, "genre chart render failed");
            bot.send_message(chat, i18n::genre_chart_failed(lang))
                .await?;
            return Ok(());
        }
        Err(error) => {
            warn!(%error, "genre chart render task failed");
            bot.send_message(chat, i18n::genre_chart_failed(lang))
                .await?;
            return Ok(());
        }
    }
    let result = bot
        .send_photo(chat, InputFile::file(&path))
        .caption(subtitle)
        .reply_markup(genre_keyboard(bars, period, lang))
        .await;
    let _ = tokio::fs::remove_file(&path).await;
    if let Err(error) = result {
        warn!(%error, "genre chart upload failed");
    }
    Ok(())
}

/// Resolve the `gn:c:<bars>` callback from `/genres` and the settings button.
pub async fn handle_genre_chart_callback(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    bars: u8,
    period: StatsPeriod,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let user_id = query.from.id.0 as i64;
    let user = state.db.ensure_user(user_id, Lang::En).await?;
    send_genre_chart(
        state,
        bot,
        message.chat.id,
        user_id,
        &user,
        bars.clamp(5, 20),
        period,
    )
    .await
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
    let user = state.db.ensure_user(user_id, Lang::En).await?;
    let Some(nick) = user.lastfm_user.filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    let Some(client) = state.lastfm.clone() else {
        return Ok(());
    };
    let context = Context {
        client,
        nick,
        lang: user.lang,
    };
    send_collage(state, bot, message.chat.id, &context, kind, grid, period).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::types::InlineKeyboardButtonKind;

    #[test]
    fn profile_menu_labels_fit_and_callbacks_round_trip() {
        let kind_menu = collage_kind_keyboard(Lang::Uk, StatsPeriod::ThreeMonths);
        let row = &kind_menu.inline_keyboard[0];
        assert_eq!(row.len(), 3, "one button per stat kind");
        for button in row {
            assert!(
                button.text.chars().count() <= 12,
                "kind label gets truncated: {}",
                button.text
            );
            let InlineKeyboardButtonKind::CallbackData(data) = &button.kind else {
                panic!("kind button must be a callback");
            };
            let parts: Vec<&str> = data.strip_prefix("lf:m:").unwrap().split(':').collect();
            assert_eq!(parts.len(), 2);
            assert!(StatsKind::parse(parts[0]).is_some());
            assert!(StatsPeriod::parse(parts[1]).is_some());
        }

        let grid_menu =
            collage_grid_keyboard(Lang::Uk, StatsKind::Tracks, StatsPeriod::ThreeMonths);
        assert_eq!(grid_menu.inline_keyboard.len(), 2, "sizes row + back row");
        let sizes = &grid_menu.inline_keyboard[0];
        assert_eq!(sizes.len(), 4);
        for button in sizes {
            assert!(
                button.text.chars().count() <= 5,
                "grid label gets truncated: {}",
                button.text
            );
            let InlineKeyboardButtonKind::CallbackData(data) = &button.kind else {
                panic!("grid button must be a callback");
            };
            let parts: Vec<&str> = data.strip_prefix("lf:c:").unwrap().split(':').collect();
            assert_eq!(parts.len(), 3);
            assert!(
                parts[1]
                    .parse::<u8>()
                    .is_ok_and(|grid| (3..=10).contains(&grid))
            );
        }
        let back = &grid_menu.inline_keyboard[1][0];
        assert!(
            matches!(&back.kind, InlineKeyboardButtonKind::CallbackData(data) if data.starts_with("lf:b:")),
            "back button must return to the first level"
        );
    }
}
