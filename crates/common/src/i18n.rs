//! All user-facing strings live here. English is the fallback language.

use crate::domain::{Lang, Provider, Stats};
use crate::error::DownloadError;

fn pick(lang: Lang, en: &'static str, uk: &'static str) -> &'static str {
    match lang {
        Lang::En => en,
        Lang::Uk => uk,
    }
}

pub fn language_name(lang: Lang) -> &'static str {
    pick(lang, "English", "Українська")
}

pub fn disclaimer(lang: Lang) -> &'static str {
    pick(
        lang,
        "⚖️ <b>Use responsibly.</b> Download only music you own, that is licensed under a free licence, or that you are otherwise allowed to copy. You are responsible for how you use this bot. The bot does not bypass DRM or paid access, and it stores only metadata plus Telegram file identifiers.",
        "⚖️ <b>Користуйтеся відповідально.</b> Завантажуйте лише музику, якою володієте, яка має вільну ліцензію або яку вам дозволено копіювати. Ви несете відповідальність за використання бота. Бот не обходить DRM чи платний доступ і зберігає лише метадані та ідентифікатори файлів Telegram.",
    )
}

pub fn start(lang: Lang) -> String {
    let intro = pick(
        lang,
        "🎵 <b>Music Bot</b>\n\nSend a track link, an album link, or just <code>Artist — Title</code>. I will find the track and send it as audio with tags and cover.\n\nYou can also use me inline: type <code>@this_bot song</code> in any chat.",
        "🎵 <b>Music Bot</b>\n\nНадішліть посилання на трек, альбом або просто <code>Виконавець — Назва</code>. Я знайду трек і надішлю його як аудіо з тегами та обкладинкою.\n\nТакож можна користуватися ботом інлайн: напишіть <code>@this_bot пісня</code> у будь-якому чаті.",
    );
    format!("{intro}\n\n{}", disclaimer(lang))
}

pub fn help(lang: Lang, is_admin: bool) -> String {
    let base = pick(
        lang,
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality, source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)",
        "<b>Як користуватися</b>\n\n• Приватний чат: надішліть посилання або <code>Виконавець — Назва</code>.\n• Інлайн: <code>@this_bot запит</code> у будь-якому чаті. Порожній запит з підключеним Last.fm запропонує те, що грає зараз.\n• Альбоми та плейлисти завантажуються по одному треку з повідомленням про прогрес. Знайти альбом за назвою: <code>/alb &lt;назва&gt;</code>.\n• Повторні запити віддаються з кешу.\n\n<b>Команди</b>\n/start — про бота\n/help — ця довідка\n/settings — мова, якість, джерело і теги\n/alb — знайти альбом за назвою й завантажити\n/lastfm — підключити Last.fm (порожній inline-запит запропонує те, що грає зараз)",
    );
    if is_admin {
        format!(
            "{base}\n\n<b>Admin</b>\n/stats — counters\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;key or url&gt;"
        )
    } else {
        base.to_owned()
    }
}

pub fn settings_title(
    lang: Lang,
    quality: u32,
    language: Lang,
    search_source: Provider,
    tags_enabled: bool,
    lastfm_user: Option<&str>,
    hq_cover: bool,
) -> String {
    let title = pick(lang, "<b>Settings</b>", "<b>Налаштування</b>");
    let q = pick(lang, "Quality", "Якість");
    let l = pick(lang, "Language", "Мова");
    let s = pick(lang, "Search source (inline)", "Джерело пошуку (inline)");
    let t = pick(lang, "Tags (album + genres)", "Теги (альбом + жанри)");
    let f = pick(lang, "Last.fm", "Last.fm");
    let state = if tags_enabled {
        pick(lang, "on", "увімк.")
    } else {
        pick(lang, "off", "вимк.")
    };
    let lf = lastfm_user.unwrap_or(pick(
        lang,
        "not connected (setup: /lastfm)",
        "не підключено (налаштування: /lastfm)",
    ));
    let hq = if hq_cover {
        pick(lang, "on", "увімк.")
    } else {
        pick(lang, "off", "вимк.")
    };
    let hql = pick(lang, "HQ cover (1000px)", "Обкладинка HQ (1000px)");
    format!(
        "{title}\n\n{q}: <b>{quality} kbps</b>\n{l}: <b>{}</b>\n{s}: <b>{}</b>\n{t}: <b>{}</b>\n{f}: <b>{lf}</b>\n{hql}: <b>{hq}</b>",
        language_name(language),
        search_source.display_name(),
        state
    )
}

pub fn tags_label(lang: Lang) -> &'static str {
    pick(lang, "🏷 Tags: album + genre", "🏷 Теги: альбом і жанри")
}

pub fn lastfm_disabled(lang: Lang) -> &'static str {
    pick(
        lang,
        "🎵 Last.fm is not configured on this bot yet.",
        "🎵 Last.fm на цьому боті не налаштовано.",
    )
}

pub fn lastfm_status(lang: Lang, user: Option<&str>) -> String {
    match user {
        Some(nick) => match lang {
            Lang::En => format!("🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off"),
            Lang::Uk => format!("🎵 Last.fm підключено як <b>{nick}</b>. Тепер в inline-режимі порожній запит запропонує те, що грає зараз.\nВідключити: /lastfm off"),
        },
        None => match lang {
            Lang::En => "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now.".to_string(),
            Lang::Uk => "🎵 Last.fm: не підключено.\nПідключити: /lastfm &lt;ваш нікнейм&gt; — порожній inline-запит запропонує те, що грає зараз.".to_string(),
        },
    }
}

pub fn lastfm_disconnected(lang: Lang) -> &'static str {
    pick(lang, "🔌 Last.fm disconnected.", "🔌 Last.fm відключено.")
}

pub fn lastfm_not_found(lang: Lang) -> &'static str {
    pick(
        lang,
        "⚠️ Last.fm says there is no such user. Check the nickname spelling at last.fm.",
        "⚠️ Last.fm не знає такого користувача. Перевір написання нікнейму на last.fm.",
    )
}

pub fn lastfm_no_activity(lang: Lang, nick: &str) -> String {
    match lang {
        Lang::En => format!(
            "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public."
        ),
        Lang::Uk => format!(
            "✅ Підключено як <b>{nick}</b>, але останніх треків ще немає. Переконайся, що плеєр скроблить на Last.fm, а профіль — публічний."
        ),
    }
}

pub fn lastfm_connected_track(lang: Lang, nick: &str, track: &str) -> String {
    match lang {
        Lang::En => format!(
            "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it."
        ),
        Lang::Uk => format!(
            "✅ Підключено як <b>{nick}</b>. Зараз грає: <b>{track}</b>. Порожній inline-запит запропонує його."
        ),
    }
}

pub fn lastfm_invalid(lang: Lang) -> &'static str {
    pick(
        lang,
        "⚠️ Invalid Last.fm username (2–15 latin letters, digits, _ or -).",
        "⚠️ Некоректний нікнейм Last.fm (2–15 латинських літер, цифр, _ або -).",
    )
}

/// Placeholder for the "now playing" inline proposal message.
pub fn now_playing_inline(lang: Lang, track: &str) -> String {
    match lang {
        Lang::En => format!("▶ Now playing: {track}"),
        Lang::Uk => format!("▶ Зараз грає: {track}"),
    }
}

pub fn hq_cover_label(lang: Lang) -> &'static str {
    pick(lang, "🖼 HQ cover (1000px)", "🖼 Обкладинка HQ (1000px)")
}

/// Shown in /settings when the user's chosen source is outside the operator's
/// SEARCH_PROVIDERS allow-list.
pub fn search_source_blocked(lang: Lang) -> &'static str {
    pick(
        lang,
        "⚠️ Your chosen search source is blocked by the server config — the bot falls back to YouTube. Ask the operator to enable it (SEARCH_PROVIDERS).",
        "⚠️ Вибране джерело пошуку заблоковано налаштуванням сервера — бот шукатиме в YouTube. Попросіть оператора увімкнути його (SEARCH_PROVIDERS).",
    )
}

pub fn settings_saved(lang: Lang) -> &'static str {
    pick(lang, "✅ Settings saved.", "✅ Налаштування збережено.")
}

pub fn language_button(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "🇬🇧 English",
        Lang::Uk => "🇺🇦 Українська",
    }
}

pub fn downloading(lang: Lang) -> &'static str {
    pick(
        lang,
        "⬇️ Downloading… this usually takes a few seconds.",
        "⬇️ Завантажую… це зазвичай триває кілька секунд.",
    )
}

pub fn searching(lang: Lang) -> &'static str {
    pick(lang, "🔎 Searching…", "🔎 Шукаю…")
}

pub fn no_results(lang: Lang) -> &'static str {
    pick(
        lang,
        "Nothing found. Try a different spelling or paste a direct link.",
        "Нічого не знайдено. Спробуйте інше написання або вставте пряме посилання.",
    )
}

pub fn banned(lang: Lang) -> &'static str {
    pick(
        lang,
        "🚫 You are not allowed to use this bot.",
        "🚫 Вам заборонено користуватися цим ботом.",
    )
}

pub fn admin_only(lang: Lang) -> &'static str {
    pick(lang, "⛔ Admins only.", "⛔ Лише для адміністраторів.")
}

pub fn rate_limited(lang: Lang, per_minute: u32) -> String {
    match lang {
        Lang::En => format!("⏳ Too many requests. Limit: {per_minute} per minute."),
        Lang::Uk => format!("⏳ Забагато запитів. Ліміт: {per_minute} на хвилину."),
    }
}

pub fn private_only(lang: Lang) -> &'static str {
    pick(
        lang,
        "ℹ️ Send links and search queries in a private chat with me. Commands work here too.",
        "ℹ️ Надсилайте посилання та пошукові запити в приватному чаті. Команди працюють і тут.",
    )
}

pub fn error(lang: Lang, error: &DownloadError) -> String {
    let text = match error {
        DownloadError::Private => pick(
            lang,
            "🔒 This track is private or requires signing in, so I cannot download it.",
            "🔒 Цей трек приватний або потребує входу, тому я не можу його завантажити.",
        ),
        DownloadError::RegionLocked => pick(
            lang,
            "🌍 This track is region locked and is not available from the bot's location.",
            "🌍 Цей трек заблокований у регіоні й недоступний із місця роботи бота.",
        ),
        DownloadError::TooLong { limit, .. } => {
            return match lang {
                Lang::En => format!(
                    "⏱ The track is too long. Maximum duration is {} minutes.",
                    limit / 60
                ),
                Lang::Uk => format!(
                    "⏱ Трек задовгий. Максимальна тривалість — {} хвилин.",
                    limit / 60
                ),
            };
        }
        DownloadError::TooLarge { limit, .. } => {
            return match lang {
                Lang::En => format!(
                    "📦 The file is too large. Maximum size is {} MB.",
                    limit / 1_000_000
                ),
                Lang::Uk => format!(
                    "📦 Файл завеликий. Максимальний розмір — {} МБ.",
                    limit / 1_000_000
                ),
            };
        }
        DownloadError::NotFound => pick(
            lang,
            "🔍 I could not find that track on the available sources.",
            "🔍 Не вдалося знайти цей трек у доступних джерелах.",
        ),
        DownloadError::Unsupported(_) => pick(
            lang,
            "🤷 This link is not supported, or it needs a paid/DRM-protected source. Try a direct track link or search by name.",
            "🤷 Це посилання не підтримується або потребує платного/DRM-джерела. Спробуйте пряме посилання на трек чи пошук за назвою.",
        ),
        DownloadError::ProviderDisabled(provider) => {
            return match lang {
                Lang::En => format!("🚫 {} is disabled on this bot.", provider.display_name()),
                Lang::Uk => format!("🚫 {} вимкнено в цьому боті.", provider.display_name()),
            };
        }
        DownloadError::SourceUnavailable(_) => pick(
            lang,
            "📡 The source is temporarily unavailable. Please try again in a moment.",
            "📡 Джерело тимчасово недоступне. Спробуйте ще раз за мить.",
        ),
        DownloadError::Audio(_) => pick(
            lang,
            "🎧 I downloaded the track but could not prepare the audio file.",
            "🎧 Трек завантажено, але не вдалося підготувати аудіофайл.",
        ),
        DownloadError::Internal(_) => pick(
            lang,
            "⚠️ Something went wrong while preparing the track. Please try again.",
            "⚠️ Під час підготовки треку сталася помилка. Спробуйте ще раз.",
        ),
    };
    text.to_owned()
}

pub fn album_confirm(lang: Lang, title: &str, count: usize) -> String {
    match lang {
        Lang::En => format!("💿 <b>{title}</b>\n{count} track(s). Download the whole collection?"),
        Lang::Uk => format!("💿 <b>{title}</b>\n{count} треків. Завантажити весь збірник?"),
    }
}

pub fn album_download_all(lang: Lang) -> &'static str {
    pick(lang, "⬇ Download all", "⬇ Завантажити всі")
}

pub fn album_cancel(lang: Lang) -> &'static str {
    pick(lang, "Cancel", "Скасувати")
}

pub fn album_cancelled(lang: Lang) -> &'static str {
    pick(lang, "Cancelled.", "Скасовано.")
}

pub fn album_loading(lang: Lang) -> &'static str {
    pick(lang, "💿 Loading the album…", "💿 Завантажую альбом…")
}

pub fn album_started(lang: Lang, title: &str, count: usize) -> String {
    match lang {
        Lang::En => format!("💿 <b>{title}</b>\n{count} track(s) queued."),
        Lang::Uk => format!("💿 <b>{title}</b>\nУ черзі треків: {count}."),
    }
}

pub fn album_progress(lang: Lang, current: usize, total: usize, title: &str) -> String {
    match lang {
        Lang::En => format!("💿 {current}/{total} — {title}"),
        Lang::Uk => format!("💿 {current}/{total} — {title}"),
    }
}

pub fn album_done(lang: Lang, ok: usize, failed: usize) -> String {
    match lang {
        Lang::En => format!("✅ Album finished: {ok} sent, {failed} failed."),
        Lang::Uk => format!("✅ Альбом завершено: надіслано {ok}, з помилками {failed}."),
    }
}

pub fn album_empty(lang: Lang) -> &'static str {
    pick(
        lang,
        "This album or playlist has no downloadable tracks.",
        "У цьому альбомі або плейлисті немає треків для завантаження.",
    )
}

pub fn album_search_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: /alb &lt;album name&gt;\nExample: /alb Master of Puppets",
        "Використання: /alb &lt;назва альбому&gt;\nПриклад: /alb Master of Puppets",
    )
}

pub fn album_searching(lang: Lang) -> &'static str {
    pick(lang, "🔎 Searching albums…", "🔎 Шукаю альбоми…")
}

pub fn album_search_results(lang: Lang, query: &str) -> String {
    match lang {
        Lang::En => format!("<b>Albums for</b> «{query}»\nPick one:"),
        Lang::Uk => format!("<b>Альбоми за запитом</b> «{query}»\nОбери альбом:"),
    }
}

pub fn no_albums_found(lang: Lang) -> &'static str {
    pick(
        lang,
        "No albums found. Try a different artist or album title.",
        "Альбоми не знайдені. Спробуй уточнити артиста або назву.",
    )
}

/// Short count suffix for album candidate buttons.
pub fn album_tracks_count(lang: Lang, count: u32) -> String {
    match lang {
        Lang::En => format!(" · {count} tracks"),
        Lang::Uk => format!(" · {count} треків"),
    }
}

pub fn stats(lang: Lang, stats: &Stats, uptime: &str) -> String {
    match lang {
        Lang::En => format!(
            "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>",
            stats.tracks, stats.users, stats.sends
        ),
        Lang::Uk => format!(
            "<b>Статистика</b>\nТреків у кеші: <b>{}</b>\nКористувачів: <b>{}</b>\nУспішних надсилань: <b>{}</b>\nЧас роботи: <b>{uptime}</b>",
            stats.tracks, stats.users, stats.sends
        ),
    }
}

pub fn purge_ok(lang: Lang, key: &str) -> String {
    match lang {
        Lang::En => format!("🗑 Purged <code>{key}</code> from cache."),
        Lang::Uk => format!("🗑 Видалено <code>{key}</code> з кешу."),
    }
}

pub fn purge_missing(lang: Lang) -> &'static str {
    pick(
        lang,
        "That key is not in the cache.",
        "Такого ключа немає в кеші.",
    )
}

pub fn ban_ok(lang: Lang, user_id: i64, banned: bool) -> String {
    match (lang, banned) {
        (Lang::En, true) => format!("🚫 User {user_id} banned."),
        (Lang::En, false) => format!("✅ User {user_id} unbanned."),
        (Lang::Uk, true) => format!("🚫 Користувача {user_id} забанено."),
        (Lang::Uk, false) => format!("✅ Користувача {user_id} розбанено."),
    }
}

pub fn invalid_purge_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: /purge &lt;cache key or track URL&gt;",
        "Використання: /purge &lt;ключ кешу або посилання на трек&gt;",
    )
}

pub fn selection_expired(lang: Lang) -> &'static str {
    pick(
        lang,
        "⌛ This result has expired. Please search again.",
        "⌛ Цей результат застарів. Пошукайте ще раз.",
    )
}

pub fn sent_track(lang: Lang, title: &str) -> String {
    match lang {
        Lang::En => format!("✅ Sent: {title}"),
        Lang::Uk => format!("✅ Надіслано: {title}"),
    }
}

pub fn search_results(lang: Lang, query: &str, count: usize) -> String {
    match lang {
        Lang::En => format!("🔎 Results for “{query}” ({count}):"),
        Lang::Uk => format!("🔎 Результати для «{query}» ({count}):"),
    }
}

pub fn invalid_user_id(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: /ban &lt;numeric user id&gt;",
        "Використання: /ban &lt;числовий id користувача&gt;",
    )
}

pub fn open_bot(lang: Lang) -> &'static str {
    pick(lang, "Open bot", "Відкрити бота")
}

pub fn sent_to_private(lang: Lang) -> &'static str {
    pick(
        lang,
        "📩 Sent to your private messages with the bot.",
        "📩 Трек надіслано вам в особисті повідомлення з ботом.",
    )
}

pub fn preparing_inline(lang: Lang) -> &'static str {
    pick(lang, "⏳ Preparing the track…", "⏳ Готую трек…")
}

pub fn downloading_track(lang: Lang, track: &str) -> String {
    match lang {
        Lang::En => format!("⏳ Downloading: {track}"),
        Lang::Uk => format!("⏳ Завантажую: {track}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_languages_have_text() {
        for lang in [Lang::En, Lang::Uk] {
            assert!(!start(lang).is_empty());
            assert!(!help(lang, true).is_empty());
            assert!(!error(lang, &DownloadError::NotFound).is_empty());
            assert!(!stats(lang, &Stats::default(), "0s").is_empty());
            assert!(!sent_to_private(lang).is_empty());
        }
    }
}
