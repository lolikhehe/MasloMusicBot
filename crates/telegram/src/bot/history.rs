//! Listening history (`/history`) and the genre dynamics chart (`/dynamics`).
//!
//! Both read data that the delivery path already writes to Postgres: `listens`
//! (one metadata row per sent track) and `user_genre_events` (timestamped genre
//! counts). No audio, cover or lyrics bytes are ever reconstructed here.

use crate::state::AppState;
use common::domain::Lang;
use common::error::HandlerResult;
use common::i18n;
use common::time;
use providers::lastfm::StatsPeriod;
use std::collections::HashMap;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, InputFile, ParseMode};
use tracing::warn;

/// Accept exactly zero or one period token.
fn parse_period(arg: Option<&str>, default: StatsPeriod) -> Option<StatsPeriod> {
    let tokens: Vec<&str> = arg.unwrap_or("").split_whitespace().collect();
    if tokens.len() > 1 {
        return None;
    }
    match tokens.first() {
        None => Some(default),
        Some(value) => StatsPeriod::parse(value),
    }
}

fn since_for(period: StatsPeriod) -> i64 {
    time::unix_now() - period.window_secs().unwrap_or(365 * 86_400)
}

pub async fn handle_history(
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
    let Some(period) = parse_period(arg, StatsPeriod::Month) else {
        bot.send_message(msg.chat.id, i18n::history_usage(lang))
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    };
    let since = since_for(period);
    let rows = state.db.list_listens(user_id, since, 15).await?;
    let mut text = i18n::history_title(lang, period.label(lang));
    if rows.is_empty() {
        text.push_str(i18n::history_empty(lang));
    } else {
        for row in &rows {
            text.push_str(&i18n::history_item(
                &row.title,
                row.performer.as_deref(),
                &time::utc_stamp(row.created_at),
            ));
        }
        let total = state.db.count_listens(user_id, since).await?;
        if total > rows.len() as i64 {
            text.push_str(&i18n::history_more(lang, total - rows.len() as i64));
        }
    }
    text.push_str(&format!("\n— {}", common::version::DEV_CHANNEL_HANDLE));
    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::Html)
        .await?;
    Ok(())
}

pub async fn handle_dynamics(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    arg: Option<&str>,
    lang: Lang,
) -> HandlerResult {
    let Some(from) = msg.from.as_ref() else {
        return Ok(());
    };
    let Some(period) = parse_period(arg, StatsPeriod::Month) else {
        bot.send_message(msg.chat.id, i18n::dynamics_usage(lang))
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    };
    send_dynamics(state, bot, msg.chat.id, from.id.0 as i64, lang, period).await
}

fn dynamics_keyboard(period: StatsPeriod, lang: Lang) -> InlineKeyboardMarkup {
    let row: Vec<InlineKeyboardButton> = [
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
            format!("dyn:p:{}", candidate.as_str()),
        )
    })
    .collect();
    InlineKeyboardMarkup::new(vec![row])
}

fn display_genre(value: &str) -> String {
    if value.eq_ignore_ascii_case("r&b") {
        return "R&B".to_owned();
    }
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render the dynamics chart for one user. Shared by the command and the
/// period keyboard callback.
pub async fn send_dynamics(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    user_id: i64,
    lang: Lang,
    period: StatsPeriod,
) -> HandlerResult {
    let since = since_for(period);
    let window = time::unix_now().saturating_sub(since).max(1);
    // Aim for ~12 buckets, never finer than one hour.
    let bucket = (window / 12).max(3_600);
    let cells = state.db.genre_dynamics(user_id, since, bucket).await?;
    if cells.is_empty() {
        bot.send_message(chat, i18n::dynamics_empty(lang)).await?;
        return Ok(());
    }

    // Index the sparse cells once. The old version rescanned the whole list for
    // every (genre, bucket) pair — O(genres × buckets × events) on every render.
    let mut totals: HashMap<&str, i64> = HashMap::new();
    let mut values_by: HashMap<(&str, i64), i64> = HashMap::new();
    for cell in &cells {
        *totals.entry(cell.genre.as_str()).or_default() += cell.listens;
        *values_by
            .entry((cell.genre.as_str(), cell.bucket))
            .or_default() += cell.listens;
    }
    let mut top: Vec<&str> = totals.keys().copied().collect();
    top.sort_by(|left, right| {
        totals[right]
            .cmp(&totals[left])
            .then_with(|| left.cmp(right))
    });
    top.truncate(8);

    let first = cells.iter().map(|cell| cell.bucket).min().unwrap_or(since);
    let last = cells.iter().map(|cell| cell.bucket).max().unwrap_or(since);
    let mut ticks = Vec::new();
    let mut cursor = first;
    while cursor <= last && ticks.len() < 24 {
        ticks.push(cursor);
        cursor += bucket;
    }
    let labels: Vec<String> = ticks
        .iter()
        .map(|tick| time::utc_stamp(*tick).get(5..10).unwrap_or("?").to_owned())
        .collect();

    let mut series = Vec::with_capacity(top.len());
    for (index, genre) in top.iter().enumerate() {
        let values: Vec<u64> = ticks
            .iter()
            .map(|tick| values_by.get(&(*genre, *tick)).copied().unwrap_or(0).max(0) as u64)
            .collect();
        series.push(collage::LineSeries {
            label: display_genre(genre),
            color: collage::SERIES_COLORS[index % collage::SERIES_COLORS.len()],
            values,
        });
    }

    let title = i18n::dynamics_title(lang);
    let subtitle = i18n::dynamics_subtitle(lang, cells.len(), labels.len(), period.label(lang));
    let path = state
        .config
        .tmp_dir
        .join(format!("dynamics-{}.png", uuid::Uuid::new_v4().simple()));
    let render_path = path.clone();
    let render_subtitle = subtitle.clone();
    let rendered = tokio::task::spawn_blocking(move || {
        collage::render_lines(title, &render_subtitle, &labels, &series, &render_path)
    })
    .await;
    match rendered {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            warn!(%error, "genre dynamics render failed");
            bot.send_message(chat, i18n::dynamics_failed(lang)).await?;
            return Ok(());
        }
        Err(error) => {
            warn!(%error, "genre dynamics render task failed");
            bot.send_message(chat, i18n::dynamics_failed(lang)).await?;
            return Ok(());
        }
    }
    let result = bot
        .send_photo(chat, InputFile::file(&path))
        .caption(subtitle)
        .reply_markup(dynamics_keyboard(period, lang))
        .await;
    let _ = tokio::fs::remove_file(&path).await;
    if let Err(error) = result {
        warn!(%error, "genre dynamics upload failed");
    }
    Ok(())
}

/// Resolve the `dyn:p:<period>` keyboard callback from the chart caption.
pub async fn handle_dynamics_callback(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    period: StatsPeriod,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let user_id = query.from.id.0 as i64;
    let user = state.db.ensure_user(user_id, Lang::En).await?;
    send_dynamics(state, bot, message.chat.id, user_id, user.lang, period).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_known_periods() {
        assert_eq!(
            parse_period(None, StatsPeriod::Month),
            Some(StatsPeriod::Month)
        );
        assert_eq!(
            parse_period(Some("7d"), StatsPeriod::Month),
            Some(StatsPeriod::Week)
        );
        assert_eq!(
            parse_period(Some("all"), StatsPeriod::Month),
            Some(StatsPeriod::Overall)
        );
        assert_eq!(parse_period(Some("nope"), StatsPeriod::Month), None);
        assert_eq!(parse_period(Some("7d all"), StatsPeriod::Month), None);
    }
}
