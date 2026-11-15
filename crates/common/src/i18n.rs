//! All user-facing strings live here. English is the fallback language.

use crate::domain::{Lang, Stats};
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
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat.\n• Albums and playlists are downloaded one track at a time with a progress message.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language and audio quality",
        "<b>Як користуватися</b>\n\n• Приватний чат: надішліть посилання або <code>Виконавець — Назва</code>.\n• Інлайн: <code>@this_bot запит</code> у будь-якому чаті.\n• Альбоми та плейлисти завантажуються по одному треку з повідомленням про прогрес.\n• Повторні запити віддаються з кешу.\n\n<b>Команди</b>\n/start — про бота\n/help — ця довідка\n/settings — мова та якість аудіо",
    );
    if is_admin {
        format!(
            "{base}\n\n<b>Admin</b>\n/stats — counters\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;key or url&gt;"
        )
    } else {
        base.to_owned()
    }
}

pub fn settings_title(lang: Lang, quality: u32, language: Lang) -> String {
    let title = pick(lang, "<b>Settings</b>", "<b>Налаштування</b>");
    let q = pick(lang, "Quality", "Якість");
    let l = pick(lang, "Language", "Мова");
    format!(
        "{title}\n\n{q}: <b>{quality} kbps</b>\n{l}: <b>{}</b>",
        language_name(language)
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
        }
    }
}
