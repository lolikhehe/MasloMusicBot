//! All user-facing strings live here. English is the fallback language.

use crate::domain::{Lang, Provider, Stats};
use crate::error::DownloadError;
use crate::version::DEV_CHANNEL_HANDLE;

fn pick(lang: Lang, en: &'static str, uk: &'static str) -> &'static str {
    match lang {
        Lang::Uk => uk,
        Lang::En => en,
        other => crate::locales::lookup(other.code(), en).unwrap_or(en),
    }
}

/// Translated `format!` template. Translations keep the same named
/// placeholders as the English source.
fn template(lang: Lang, en: &'static str) -> &'static str {
    pick(lang, en, en)
}

/// Like `format!`, but takes the translated template for `lang`. Accepts
/// named args (`name = value`) and positional `{}` args.
macro_rules! lformat {
    ($lang:expr, $tpl:literal $(, $key:ident = $val:expr)* $(,)?) => {{
        let mut result = $crate::i18n::template($lang, $tpl).to_owned();
        $(
            result = result.replace(
                concat!("{", stringify!($key), "}"),
                &$val.to_string(),
            );
        )*
        result
    }};
    ($lang:expr, $tpl:literal, $($val:expr),+ $(,)?) => {{
        let mut result = $crate::i18n::template($lang, $tpl).to_owned();
        $(
            result = result.replacen("{}", &$val.to_string(), 1);
        )*
        result
    }};
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
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality (kbps or FLAC lossless), source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)\n/import [list] — import a playlist; with no argument the next message (file or Artist – Title lines) is used\n/profile or /lastfmstats [period] — scrobble statistics (enable Last.fm stats in /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — PNG chart of your most played items\n/genres [5..20] — genre chart from your downloads (enable the analyzer in /settings)\n/lyrics [Artist — Title] — song lyrics from public sources (no argument: last track; or reply to an audio)\n/changelog — show the release notes\n/me — your own counters and favourite genres\n/queue — recent download jobs\n/cancel &lt;id&gt; — cancel an active download\n/stop — cancel all your active actions at once\n/convert auto|m4a|mp3|flac|opus — output format; reply to a track to convert that one\n/normalize on|off — ffmpeg loudnorm pass\n/playlist &lt;name&gt; — search public playlists\n/fav — your saved ⭐ tracks (tap ☆ under a track to save it)\n/similar — reply to one of my tracks: up to 10 similar tracks\n/artist &lt;name&gt; — top tracks of an artist\n/cover — reply to one of my tracks: full-size album cover\n/groupmode on|off — admin: allow the bot in this group\n/history [period] — your recent listening history\n/dynamics [period] — genre dynamics over time chart\n/digest daily|weekly|off — personal digest to your private chat\n/digest quiet HH-HH|off — quiet hours in UTC",
        "<b>Як користуватися</b>\n\n• Приватний чат: надішліть посилання або <code>Виконавець — Назва</code>.\n• Інлайн: <code>@this_bot запит</code> у будь-якому чаті. Порожній запит з підключеним Last.fm запропонує те, що грає зараз.\n• Альбоми та плейлисти завантажуються по одному треку з повідомленням про прогрес. Знайти альбом за назвою: <code>/alb &lt;назва&gt;</code>.\n• Повторні запити віддаються з кешу.\n\n<b>Команди</b>\n/start — про бота\n/help — ця довідка\n/settings — мова, якість (kbps або FLAC lossless), джерело і теги\n/alb — знайти альбом за назвою й завантажити\n/lastfm — підключити Last.fm (порожній inline-запит запропонує те, що грає зараз)\n/import [список] — імпорт плейлиста; без аргументу наступне повідомлення (файл або рядки «Виконавець – Назва») вважається списком\n/profile або /lastfmstats [період] — статистика скроблів (увімкни в /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [період] — PNG-колаж найбільш прослуханих\n/genres [5..20] — графік жанрів із твоїх завантажень (увімкни аналізатор у /settings)\n/lyrics [Виконавець — Назва] — текст пісні з публічних джерел (без аргументу: останній трек; або відповідь на аудіо)\n/changelog — показати список змін\n/me — твоя статистика та улюблені жанри\n/queue — останні завантаження\n/cancel &lt;id&gt; — скасувати активне завантаження\n/stop — зупинити всі твої активні дії одразу\n/convert auto|m4a|mp3|flac|opus — формат; відповідь на трек конвертує саме його\n/normalize on|off — вирівнювання гучності ffmpeg\n/playlist &lt;назва&gt; — пошук публічних плейлистів\n/fav — твої збережені ⭐ треки (тапни ☆ під треком, щоб зберегти)\n/similar — відповідь на мій трек: до 10 схожих треків\n/artist &lt;ім'я&gt; — топ треків виконавця\n/cover — відповідь на мій трек: обкладинка альбома у повному розмірі\n/groupmode on|off — адмін: дозволити бота в цій групі\n/history [період] — остання історія прослуховувань\n/dynamics [період] — графік динаміки жанрів за часом\n/digest daily|weekly|off — персональний дайджест у приватні повідомлення\n/digest quiet HH-HH|off — тихі години за UTC",
    );
    if is_admin {
        format!(
            "{base}\n\n<b>Admin</b>\n/stats — counters\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;key or url&gt;\n/broadcast &lt;text&gt; — one-off notice to all known users"
        )
    } else {
        base.to_owned()
    }
}

/// `/lyrics` help text. The command accepts an explicit query, the last sent
/// track, or a reply to an audio message.
pub fn lyrics_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "📝 Usage: <code>/lyrics Artist — Title</code>\nOr send <code>/lyrics</code> after a track, or reply to an audio message with <code>/lyrics</code>.",
        "📝 Використання: <code>/lyrics Виконавець — Назва</code>\nАбо надішліть <code>/lyrics</code> після треку, або відповідайте <code>/lyrics</code> на аудіоповідомлення.",
    )
}

pub fn lyrics_searching(lang: Lang) -> &'static str {
    pick(lang, "🔎 Looking up lyrics…", "🔎 Шукаю текст пісні…")
}

pub fn lyrics_header(lang: Lang, source: &str, synced: bool) -> String {
    let title = pick(lang, "<b>Lyrics</b>", "<b>Текст пісні</b>");
    let kind = if synced {
        pick(lang, "timestamped", "з таймкодами")
    } else {
        pick(lang, "plain text", "звичайний текст")
    };
    format!("📝 {title} · {source} · {kind}\n\n")
}

pub fn lyrics_footer(lang: Lang) -> String {
    let _ = lang;
    format!("\n\n— {DEV_CHANNEL_HANDLE}")
}

pub fn lyrics_not_found(lang: Lang) -> &'static str {
    pick(
        lang,
        "😕 Lyrics not found in the public sources (LRCLIB, lyrics.ovh).",
        "😕 Текст не знайдено у публічних джерелах (LRCLIB, lyrics.ovh).",
    )
}

pub fn lyrics_no_last(lang: Lang) -> &'static str {
    pick(
        lang,
        "Send a track first, reply to an audio message, or pass a query: <code>/lyrics Artist — Title</code>.",
        "Спочатку надішліть трек, відповідайте на аудіоповідомлення або вкажіть запит: <code>/lyrics Виконавець — Назва</code>.",
    )
}

pub fn lyrics_lrc_button(lang: Lang) -> &'static str {
    pick(lang, "📄 Download .lrc", "📄 Завантажити .lrc")
}

pub fn lyrics_other_source_button(lang: Lang) -> &'static str {
    pick(lang, "🔄 Other source", "🔄 Інше джерело")
}

pub fn lyrics_lrc_sent(lang: Lang) -> &'static str {
    pick(
        lang,
        "📄 LRC file saved. Drop it next to the audio in your player.",
        "📄 Файл LRC збережено. Покладіть його поруч з аудіо у плеєрі.",
    )
}

pub fn lyrics_no_other_source(lang: Lang) -> &'static str {
    pick(
        lang,
        "😕 The other public source has no lyrics for this track.",
        "😕 Інше публічне джерело не має тексту для цього треку.",
    )
}

pub fn changelog_title(lang: Lang) -> &'static str {
    pick(lang, "<b>What's new</b>\n\n", "<b>Що нового</b>\n\n")
}

pub fn user_stats(lang: Lang, tracks: i64, listens: i64, genres: i64, seconds: i64) -> String {
    let hours = seconds as f64 / 3600.0;
    let title = pick(lang, "<b>Your stats</b>", "<b>Ваша статистика</b>");
    let counted = pick(lang, "Tracks counted", "Треків враховано");
    let listens_label = pick(lang, "Listens", "Прослуховувань");
    let genres_label = pick(lang, "Genres", "Жанрів");
    let time_label = pick(lang, "Audio time", "Час аудіо");
    let genres_hint = pick(lang, "top: /genres", "топ: /genres");
    format!(
        "{title}\n\n{counted}: <b>{tracks}</b>\n{listens_label}: <b>{listens}</b>\n{genres_label}: <b>{genres}</b> ({genres_hint})\n{time_label}: <b>{hours:.1} h</b>\n\n{}",
        crate::version::DEV_CHANNEL_HANDLE
    )
}

pub fn job_already_running(lang: Lang) -> &'static str {
    pick(
        lang,
        "⏳ This download is already being processed by another instance.",
        "⏳ Це завантаження вже обробляється іншим екземпляром бота.",
    )
}

pub fn job_cancelled(lang: Lang) -> &'static str {
    pick(lang, "🛑 Download cancelled.", "🛑 Завантаження скасовано.")
}

pub fn queue_title(lang: Lang) -> &'static str {
    pick(
        lang,
        "<b>Recent downloads</b>\n\n",
        "<b>Останні завантаження</b>\n\n",
    )
}

pub fn queue_empty(lang: Lang) -> &'static str {
    pick(lang, "No downloads yet.", "Завантажень ще немає.")
}

pub fn queue_item(id: &str, kind: &str, status: &str, cancelling: bool) -> String {
    let mark = if cancelling {
        " · 🛑 cancelling"
    } else {
        ""
    };
    format!("<code>{id}</code> · {kind} · {status}{mark}\n")
}

pub fn cancel_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: <code>/cancel &lt;job id from /queue&gt;</code>",
        "Використання: <code>/cancel &lt;id з /queue&gt;</code>",
    )
}

pub fn cancel_result(lang: Lang, ok: bool) -> &'static str {
    match ok {
        true => pick(
            lang,
            "🛑 Cancellation requested.",
            "🛑 Скасування запрошено.",
        ),
        false => pick(
            lang,
            "No active job with that id.",
            "Активного завдання з таким id немає.",
        ),
    }
}

pub fn convert_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: <code>/convert auto|m4a|mp3|flac|opus</code> — applies to your next downloads. Reply to a track with <code>/convert &lt;format&gt;</code> to convert that track.",
        "Використання: <code>/convert auto|m4a|mp3|flac|opus</code> — застосується до наступних завантажень. Відповідайте на трек командою <code>/convert &lt;формат&gt;</code>, щоб конвертувати саме його.",
    )
}

pub fn convert_reply_no_track(lang: Lang) -> &'static str {
    pick(
        lang,
        "This message is not a downloaded track I can convert.",
        "Це повідомлення не є завантаженим треком, який я можу конвертувати.",
    )
}

pub fn convert_reply_done(lang: Lang, format: &str) -> String {
    let label = pick(lang, "Converted to", "Конвертовано в");
    format!("<b>{label}: {format}</b>")
}

pub fn convert_reply_failed(lang: Lang, error: &str) -> String {
    let label = pick(lang, "Conversion failed", "Не вдалося конвертувати");
    format!("<b>{label}</b>: {error}")
}

pub fn stop_result(lang: Lang, jobs: u64, prompts: u64) -> String {
    let text = pick(
        lang,
        "Stopped: {jobs} download(s), {prompts} pending action(s).",
        "Зупинено: завантажень — {jobs}, очікуваних дій — {prompts}.",
    );
    text.replace("{jobs}", &jobs.to_string())
        .replace("{prompts}", &prompts.to_string())
}

pub fn convert_saved(lang: Lang, format: &str, normalize: bool) -> String {
    let label = pick(lang, "Output format", "Формат");
    let norm = pick(lang, "on", "увімк.");
    let plain = pick(lang, "off", "вимк.");
    format!(
        "{label}: <b>{format}</b>\nloudnorm: <b>{}</b>",
        if normalize { norm } else { plain }
    )
}

pub fn normalize_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: <code>/normalize on|off</code> (ffmpeg loudnorm, opt-in).",
        "Використання: <code>/normalize on|off</code> (ffmpeg loudnorm, за бажанням).",
    )
}

pub fn groupmode_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Admin only. Usage: <code>/groupmode on|off</code>",
        "Тільки для адмінів. Використання: <code>/groupmode on|off</code>",
    )
}

pub fn groupmode_result(lang: Lang, enabled: bool) -> &'static str {
    match enabled {
        true => pick(
            lang,
            "✅ Group mode enabled.",
            "✅ Груповий режим увімкнено.",
        ),
        false => pick(
            lang,
            "🛑 Group mode disabled.",
            "🛑 Груповий режим вимкнено.",
        ),
    }
}

pub fn playlist_searching(lang: Lang) -> &'static str {
    pick(
        lang,
        "🔎 Searching public playlists…",
        "🔎 Шукаю публічні плейлисти…",
    )
}

pub fn playlist_results(lang: Lang, query: &str, count: usize) -> String {
    let label = pick(lang, "Playlists", "Плейлисти");
    format!("{label} «{query}» · {count}")
}

pub fn no_playlists_found(lang: Lang) -> &'static str {
    pick(
        lang,
        "😕 No public playlists found.",
        "😕 Публічних плейлистів не знайдено.",
    )
}

pub fn playlist_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: <code>/playlist &lt;name&gt;</code> — search public playlists (Deezer). Apple Music links already work as album URLs.",
        "Використання: <code>/playlist &lt;назва&gt;</code> — пошук публічних плейлистів (Deezer). Посилання Apple Music уже працюють як альбоми.",
    )
}

pub fn broadcast_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: <code>/broadcast &lt;text&gt;</code>",
        "Використання: <code>/broadcast &lt;текст&gt;</code>",
    )
}

pub fn broadcast_result(lang: Lang, sent: usize, failed: usize) -> String {
    let label = pick(lang, "Broadcast complete", "Розсилка завершена");
    format!("{label}: {sent} ✓ / {failed} ✗")
}

pub fn genre_chart_usage_period(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>",
        "Використання: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>",
    )
}

#[allow(clippy::too_many_arguments)]
pub fn settings_title(
    lang: Lang,
    quality: u32,
    language: Lang,
    search_source: Provider,
    tags_enabled: bool,
    lastfm_user: Option<&str>,
    hq_cover: bool,
    lastfm_stats: bool,
    genre_analysis: bool,
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
    let stats_state = if lastfm_stats {
        pick(lang, "on", "увімк.")
    } else {
        pick(lang, "off", "вимк.")
    };
    let sl = pick(lang, "Last.fm stats", "Статистика Last.fm");
    let ga_state = if genre_analysis {
        pick(lang, "on", "увімк.")
    } else {
        pick(lang, "off", "вимк.")
    };
    let gal = pick(lang, "Genre analyzer", "Аналізатор жанрів");
    let q_display = quality_display(lang, quality);
    format!(
        "{title}\n\n{q}: <b>{q_display}</b>\n{l}: <b>{}</b>\n{s}: <b>{}</b>\n{t}: <b>{}</b>\n{f}: <b>{lf}</b>\n{hql}: <b>{hq}</b>\n{sl}: <b>{stats_state}</b>\n{gal}: <b>{ga_state}</b>",
        language_name(language),
        search_source.display_name(),
        state
    )
}

pub fn tags_label(lang: Lang) -> &'static str {
    pick(lang, "🏷 Tags: album + genre", "🏷 Теги: альбом і жанри")
}

pub fn genre_analysis_label(lang: Lang) -> &'static str {
    pick(lang, "🎛 Genre analyzer", "🎛 Аналізатор жанрів")
}

pub fn album_info_label(lang: Lang) -> &'static str {
    pick(lang, "ℹ️ Album & artist info", "ℹ️ Про альбом і виконавця")
}

pub fn additional_menu_button(lang: Lang) -> &'static str {
    pick(lang, "⚙ Advanced", "⚙ Додаткове")
}

/// Title of the advanced settings submenu opened by "⚙ Advanced".
pub fn additional_title(lang: Lang) -> &'static str {
    pick(
        lang,
        "<b>Advanced settings</b>",
        "<b>Додаткові налаштування</b>",
    )
}

pub fn fav_toggled(lang: Lang, added: bool) -> &'static str {
    if added {
        pick(lang, "⭐ Added to favorites.", "⭐ Додано в обрані.")
    } else {
        pick(lang, "Removed from favorites.", "Прибрано з обраних.")
    }
}

pub fn favorites_title(lang: Lang) -> &'static str {
    pick(lang, "⭐ Your favorites", "⭐ Обрані треки")
}

pub fn favorites_empty(lang: Lang) -> &'static str {
    pick(
        lang,
        "Nothing saved yet — tap ☆ under any track.",
        "Поки нічого — натисни ☆ під треком.",
    )
}

pub fn search_more(lang: Lang) -> &'static str {
    pick(lang, "More results ▸", "Ще результати ▸")
}

pub fn similar_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Reply to one of my tracks with /similar.",
        "Надішліть /similar у відповідь на мій трек.",
    )
}

pub fn similar_none(lang: Lang) -> &'static str {
    pick(
        lang,
        "No similar tracks found.",
        "Схожих треків не знайшов.",
    )
}

pub fn similar_title(lang: Lang, track: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(lang, "🎧 Similar to «{track}»", track = track),
        Lang::Uk => format!("🎧 Схоже на «{track}»"),
    }
}

pub fn artist_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: /artist <name>",
        "Використання: /artist <ім'я>",
    )
}

pub fn artist_none(lang: Lang) -> &'static str {
    pick(
        lang,
        "Artist not found in the catalog.",
        "Виконавця не знайшов у каталозі.",
    )
}

pub fn artist_title(lang: Lang, name: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(lang, "🎤 Top tracks of {name}", name = name),
        Lang::Uk => format!("🎤 Топ треків {name}"),
    }
}

pub fn cover_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Reply to one of my tracks with /cover.",
        "Надішліть /cover у відповідь на мій трек.",
    )
}

pub fn cover_none(lang: Lang) -> &'static str {
    pick(
        lang,
        "This track has no cover.",
        "У цього трека нема обкладинки.",
    )
}

pub fn genre_chart_label(lang: Lang) -> &'static str {
    pick(lang, "📊 Genre chart", "📊 Графік жанрів")
}

pub fn genre_chart_disabled(lang: Lang) -> &'static str {
    pick(
        lang,
        "🎛 Enable the genre analyzer in /settings first.",
        "🎛 Спочатку увімкни аналізатор жанрів у /settings.",
    )
}

pub fn genre_chart_empty(lang: Lang) -> &'static str {
    pick(
        lang,
        "No genre data yet — download a few tracks with the analyzer enabled.",
        "Ще немає даних — скачай кілька треків з увімкненим аналізатором.",
    )
}

pub fn genre_chart_failed(lang: Lang) -> &'static str {
    pick(
        lang,
        "Could not build the genre chart.",
        "Не вдалося побудувати графік жанрів.",
    )
}

pub fn genre_chart_title(lang: Lang) -> &'static str {
    pick(lang, "Your genres", "Твої жанри")
}

pub fn genre_chart_subtitle(lang: Lang, listens: i64, genres: usize) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "{listens} listens · {genres} genres",
            listens = listens,
            genres = genres
        ),
        Lang::Uk => format!("{listens} прослуховувань · {genres} жанрів"),
    }
}

pub fn genre_chart_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: /genres [5..20] — number of bars.",
        "Використання: /genres [5..20] — кількість стовпців.",
    )
}

pub fn lastfm_stats_label(lang: Lang) -> &'static str {
    pick(
        lang,
        "📊 Last.fm stats + collages",
        "📊 Статистика Last.fm + колажі",
    )
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
            Lang::En | Lang::Pl | Lang::De | Lang::Cn | Lang::Jp | Lang::Fr | Lang::Nl | Lang::Cz | Lang::Be => lformat!(lang, "🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off", nick = nick),
            Lang::Uk => format!("🎵 Last.fm підключено як <b>{nick}</b>. Тепер в inline-режимі порожній запит запропонує те, що грає зараз.\nВідключити: /lastfm off"),
        },
        None => match lang {
            Lang::En | Lang::Pl | Lang::De | Lang::Cn | Lang::Jp | Lang::Fr | Lang::Nl | Lang::Cz | Lang::Be => template(lang, "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now.").to_string(),
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
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public.",
            nick = nick
        ),
        Lang::Uk => format!(
            "✅ Підключено як <b>{nick}</b>, але останніх треків ще немає. Переконайся, що плеєр скроблить на Last.fm, а профіль — публічний."
        ),
    }
}

pub fn lastfm_connected_track(lang: Lang, nick: &str, track: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it.",
            nick = nick,
            track = track
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
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(lang, "▶ Now playing: {track}", track = track),
        Lang::Uk => format!("▶ Зараз грає: {track}"),
    }
}

pub fn quality_lossless_label(lang: Lang) -> &'static str {
    pick(lang, "FLAC (lossless)", "FLAC (без втрати)")
}

/// Quality shown in /settings: "FLAC (lossless)" for the lossless sentinel,
/// otherwise "N kbps".
pub fn quality_display(lang: Lang, quality: u32) -> String {
    if quality == 0 {
        match lang {
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be => template(lang, "FLAC (lossless)").to_owned(),
            Lang::Uk => "FLAC (без втрати)".to_owned(),
        }
    } else {
        format!("{quality} kbps")
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
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => template(lang, "🇬🇧 English"),
        Lang::Uk => "🇺🇦 Українська",
    }
}

pub fn language_menu_button(lang: Lang) -> &'static str {
    pick(lang, "🌐 Language", "🌐 Мова")
}

pub fn language_menu_title(lang: Lang) -> &'static str {
    pick(
        lang,
        "<b>Language</b>\nPick the interface language:",
        "<b>Мова</b>\nОбери мову інтерфейсу:",
    )
}

pub fn back_button(lang: Lang) -> &'static str {
    pick(lang, "« Back", "« Назад")
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
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "⏳ Too many requests. Limit: {per_minute} per minute.",
            per_minute = per_minute
        ),
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
                Lang::En
                | Lang::Pl
                | Lang::De
                | Lang::Cn
                | Lang::Jp
                | Lang::Fr
                | Lang::Nl
                | Lang::Cz
                | Lang::Be => lformat!(
                    lang,
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
                Lang::En
                | Lang::Pl
                | Lang::De
                | Lang::Cn
                | Lang::Jp
                | Lang::Fr
                | Lang::Nl
                | Lang::Cz
                | Lang::Be => lformat!(
                    lang,
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
                Lang::En
                | Lang::Pl
                | Lang::De
                | Lang::Cn
                | Lang::Jp
                | Lang::Fr
                | Lang::Nl
                | Lang::Cz
                | Lang::Be => lformat!(
                    lang,
                    "🚫 {} is disabled on this bot.",
                    provider.display_name()
                ),
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
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => {
            lformat!(
                lang,
                "💿 <b>{title}</b>\n{count} track(s). Download the whole collection?",
                title = title,
                count = count
            )
        }
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
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "💿 <b>{title}</b>\n{count} track(s) queued.",
            title = title,
            count = count
        ),
        Lang::Uk => format!("💿 <b>{title}</b>\nУ черзі треків: {count}."),
    }
}

pub fn album_progress(lang: Lang, current: usize, total: usize, title: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "💿 {current}/{total} — {title}",
            current = current,
            total = total,
            title = title
        ),
        Lang::Uk => format!("💿 {current}/{total} — {title}"),
    }
}

pub fn album_done(lang: Lang, ok: usize, failed: usize) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "✅ Album finished: {ok} sent, {failed} failed.",
            ok = ok,
            failed = failed
        ),
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

/// Shown after an album finishes when some tracks failed: names of the
/// missing tracks (already HTML-escaped) and how to retry.
pub fn album_failed_list(lang: Lang, titles: &[String], extra: usize) -> String {
    let list = titles.join(", ");
    let more = if extra > 0 {
        match lang {
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be => lformat!(lang, " and {extra} more", extra = extra),
            Lang::Uk => format!(" і ще {extra}"),
        }
    } else {
        String::new()
    };
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "⚠ Failed: {list}{more}.\nSend /alb again — the finished tracks will come from cache instantly.",
            list = list,
            more = more
        ),
        Lang::Uk => format!(
            "⚠ Не вдалося: {list}{more}.\nПовтори виклик /alb — успішні треки будуть взяті з кешу миттєво."
        ),
    }
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
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "<b>Albums for</b> «{query}»\nPick one:",
            query = query
        ),
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
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(lang, " · {count} tracks", count = count),
        Lang::Uk => format!(" · {count} треків"),
    }
}

pub fn stats(lang: Lang, stats: &Stats, uptime: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => {
            // Mixed positional + named placeholders: substitute by hand so the
            // English template stays byte-identical as the translation key.
            let mut result = template(
                lang,
                "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>",
            )
            .to_owned();
            result = result.replacen("{}", &stats.tracks.to_string(), 1);
            result = result.replacen("{}", &stats.users.to_string(), 1);
            result = result.replacen("{}", &stats.sends.to_string(), 1);
            result.replace("{uptime}", uptime)
        }
        Lang::Uk => format!(
            "<b>Статистика</b>\nТреків у кеші: <b>{}</b>\nКористувачів: <b>{}</b>\nУспішних надсилань: <b>{}</b>\nЧас роботи: <b>{uptime}</b>",
            stats.tracks, stats.users, stats.sends
        ),
    }
}

/// Short release-notes PM sent once per version at startup.
/// `notes` is plain text from PATCH_NOTES.md and is escaped here before HTML.
pub fn update_notice(lang: Lang, notes: &str) -> String {
    let notes = html_escape::encode_text(notes.trim());
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => {
            lformat!(
                lang,
                "🆕 <b>Bot update</b>\n\n{notes}\n\n💬 Developer channel: @maslougazIA",
                notes = notes
            )
        }
        Lang::Uk => {
            format!("🆕 <b>Оновлення бота</b>\n\n{notes}\n\n💬 Канал розробника: @maslougazIA")
        }
    }
}

pub fn purge_ok(lang: Lang, key: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(lang, "🗑 Purged <code>{key}</code> from cache.", key = key),
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
        (
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be,
            true,
        ) => lformat!(lang, "🚫 User {user_id} banned.", user_id = user_id),
        (
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be,
            false,
        ) => lformat!(lang, "✅ User {user_id} unbanned.", user_id = user_id),
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
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(lang, "✅ Sent: {title}", title = title),
        Lang::Uk => format!("✅ Надіслано: {title}"),
    }
}

pub fn search_results(lang: Lang, query: &str, count: usize) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "🔎 Results for “{query}” ({count}):",
            query = query,
            count = count
        ),
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

pub fn import_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Send /import followed by a playlist (Artist – Title lines), or just /import and then send the file or the list in the next message.",
        "Надішли /import і список треків («Виконавець – Назва»), або просто /import — і в наступному повідомленні надішли файл чи список.",
    )
}

pub fn import_waiting(lang: Lang) -> &'static str {
    pick(
        lang,
        "📥 Waiting for the playlist. Send a CSV/TXT/JSON file (Spotify, TuneMyMusic, Soundiiz export) or paste lines like <code>Artist – Title</code>. Send <code>cancel</code> to abort.",
        "📥 Чекаю плейлист. Надішли файл CSV/TXT/JSON (експорт Spotify, TuneMyMusic, Soundiiz) або встав рядки вигляду <code>Виконавець – Назва</code>. Надішли <code>cancel</code>, щоб скасувати.",
    )
}

pub fn import_cancelled(lang: Lang) -> &'static str {
    pick(
        lang,
        "🛑 Playlist import cancelled.",
        "🛑 Імпорт плейлиста скасовано.",
    )
}

pub fn import_progress(lang: Lang, count: usize) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(lang, "🔎 Matching {count} imported tracks…", count = count),
        Lang::Uk => format!("🔎 Шукаю {count} імпортованих треків…"),
    }
}

pub fn import_no_tracks(lang: Lang) -> &'static str {
    pick(
        lang,
        "No tracks found in that playlist.",
        "У цьому плейлисті не знайдено треків.",
    )
}

pub fn import_too_many(lang: Lang, max: usize) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "Too many tracks; import at most {max} at a time.",
            max = max
        ),
        Lang::Uk => format!("Занадто багато треків; імпортуй не більше {max} за раз."),
    }
}

pub fn import_failed(lang: Lang) -> &'static str {
    pick(
        lang,
        "Could not read that playlist file.",
        "Не вдалося прочитати файл плейлиста.",
    )
}

pub fn lastfm_stats_disabled(lang: Lang) -> &'static str {
    pick(
        lang,
        "📊 Enable Last.fm stats in /settings first.",
        "📊 Спочатку увімкни статистику Last.fm у /settings.",
    )
}

pub fn lastfm_not_connected(lang: Lang) -> &'static str {
    pick(
        lang,
        "Connect your public Last.fm nickname with /lastfm <nick> first.",
        "Підключи публічний нік Last.fm через /lastfm <нік>.",
    )
}

pub fn profile_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: /profile [7d|1m|3m|6m|1y|all]",
        "Використання: /profile [7d|1m|3m|6m|1y|all]",
    )
}

pub fn collage_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]",
        "Використання: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]",
    )
}

#[allow(clippy::too_many_arguments)]
pub fn profile_summary(
    lang: Lang,
    name: &str,
    playcount: u64,
    tracks: u64,
    albums: u64,
    artists: u64,
    country: Option<&str>,
    period_label: &str,
    top_lines: &[String],
) -> String {
    let name = html_escape::encode_text(name);
    let period = html_escape::encode_text(period_label);
    let mut body = match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "📊 <b>Last.fm: {name}</b>\nScrobbles (all time): <b>{playcount}</b>\nLibrary (all time): {tracks} tracks · {albums} albums · {artists} artists",
            name = name,
            playcount = playcount,
            tracks = tracks,
            albums = albums,
            artists = artists
        ),
        Lang::Uk => format!(
            "📊 <b>Last.fm: {name}</b>\nСкроблів (весь час): <b>{playcount}</b>\nБібліотека (весь час): {tracks} треків · {albums} альбомів · {artists} виконавців"
        ),
    };
    if let Some(country) = country.filter(|value| !value.is_empty()) {
        let country = html_escape::encode_text(country);
        let line = match lang {
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be => lformat!(lang, "\nCountry: {country}", country = country),
            Lang::Uk => format!("\nКраїна: {country}"),
        };
        body.push_str(&line);
    }
    if !top_lines.is_empty() {
        let header = match lang {
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be => lformat!(lang, "\n\n<b>Top tracks ({period}):</b>", period = period),
            Lang::Uk => format!("\n\n<b>Топ треків ({period}):</b>"),
        };
        body.push_str(&header);
        for line in top_lines {
            body.push_str("\n• ");
            body.push_str(line);
        }
    }
    body
}

pub fn collage_generating(lang: Lang) -> &'static str {
    pick(lang, "🎨 Generating collage…", "🎨 Generating колаж…")
}

pub fn collage_failed(lang: Lang) -> &'static str {
    pick(
        lang,
        "Could not build the collage (Last.fm returned no data).",
        "Не вдалося створити колаж (Last.fm повернув порожні дані).",
    )
}

/// Compact caption for inline-delivered audio. Telegram already renders the
/// track title and the artist (performer line), so the parenthesized line
/// carries the album when known, plus genre hashtags and the signature.
pub fn inline_caption(label: Option<&str>, hashtags: &[String], bot_username: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(name) = label.map(str::trim).filter(|name| !name.is_empty()) {
        lines.push(format!("({name})"));
    }
    if !hashtags.is_empty() {
        lines.push(hashtags.join(" "));
    }
    lines.push(format!("@{bot_username} | {DEV_CHANNEL_HANDLE}"));
    lines.join("\n")
}

pub fn downloading_track(lang: Lang, track: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(lang, "⏳ Downloading: {track}", track = track),
        Lang::Uk => format!("⏳ Завантажую: {track}"),
    }
}

// ---- Listening history, genre dynamics and digests (v2.5) ----------------

pub fn history_title(lang: Lang, period: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "<b>Listening history</b> · {period}\n\n",
            period = period
        ),
        Lang::Uk => format!("<b>Історія прослуховувань</b> · {period}\n\n"),
    }
}

pub fn history_empty(lang: Lang) -> &'static str {
    pick(
        lang,
        "Nothing in this period yet — send a track first.",
        "За цей період ще нічого немає — спочатку надішли трек.",
    )
}

pub fn history_item(title: &str, performer: Option<&str>, when: &str) -> String {
    let title = html_escape::encode_text(title);
    let name = match performer.map(str::trim).filter(|name| !name.is_empty()) {
        Some(performer) => format!("{} — ", html_escape::encode_text(performer)),
        None => String::new(),
    };
    format!("• {name}{title} · <code>{when}</code>\n")
}

pub fn history_more(lang: Lang, hidden: i64) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(lang, "\n… and {hidden} earlier\n", hidden = hidden),
        Lang::Uk => format!("\n… і ще {hidden} раніше\n"),
    }
}

pub fn history_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: <code>/history [7d|1m|3m|6m|1y|all]</code>",
        "Використання: <code>/history [7d|1m|3m|6m|1y|all]</code>",
    )
}

pub fn dynamics_title(lang: Lang) -> &'static str {
    pick(lang, "Genre dynamics", "Динаміка жанрів")
}

pub fn dynamics_subtitle(lang: Lang, cells: usize, buckets: usize, period: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "{cells} genre-points · {buckets} buckets · {period}",
            cells = cells,
            buckets = buckets,
            period = period
        ),
        Lang::Uk => format!("{cells} точок жанрів · {buckets} інтервалів · {period}"),
    }
}

pub fn dynamics_empty(lang: Lang) -> &'static str {
    pick(
        lang,
        "No timed genre data yet. Enable the analyzer in /settings and download a few tracks.",
        "Ще немає даних із часом. Увімкни аналізатор у /settings і скачай кілька треків.",
    )
}

pub fn dynamics_failed(lang: Lang) -> &'static str {
    pick(
        lang,
        "Could not build the dynamics chart.",
        "Не вдалося побудувати графік динаміки.",
    )
}

pub fn dynamics_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>",
        "Використання: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>",
    )
}

pub fn digest_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "Usage:\n<code>/digest daily|weekly</code> — enable (default 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — quiet hours in UTC\n<code>/digest status</code> — current schedule\n<code>/digest off</code> — disable",
        "Використання:\n<code>/digest daily|weekly</code> — увімкнути (09:00 за UTC)\n<code>/digest quiet HH-HH|off</code> — тихі години за UTC\n<code>/digest status</code> — поточний розклад\n<code>/digest off</code> — вимкнути",
    )
}

pub fn digest_enabled(lang: Lang, period: &str, next: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "✅ {period} digest enabled. Next: <b>{next}</b> UTC.",
            period = period,
            next = next
        ),
        Lang::Uk => format!("✅ {period}-дайджест увімкнено. Наступний: <b>{next}</b> UTC."),
    }
}

pub fn digest_disabled(lang: Lang) -> &'static str {
    pick(lang, "🛑 Digest disabled.", "🛑 Дайджест вимкнено.")
}

pub fn digest_quiet_set(lang: Lang, window: &str) -> String {
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "🌙 Quiet hours: <b>{window}</b> UTC.",
            window = window
        ),
        Lang::Uk => format!("🌙 Тихі години: <b>{window}</b> за UTC."),
    }
}

pub fn digest_status(lang: Lang, period: &str, next: &str, quiet: &str, enabled: bool) -> String {
    let state = if enabled {
        pick(lang, "on", "увімк.")
    } else {
        pick(lang, "off", "вимк.")
    };
    match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => lformat!(
            lang,
            "📅 Digest: <b>{state}</b>\nPeriod: {period}\nNext: {next} UTC\nQuiet hours: {quiet} UTC",
            state = state,
            period = period,
            next = next,
            quiet = quiet
        ),
        Lang::Uk => format!(
            "📅 Дайджест: <b>{state}</b>\nПеріод: {period}\nНаступний: {next} UTC\nТихі години: {quiet} UTC"
        ),
    }
}

pub fn digest_not_enabled(lang: Lang) -> &'static str {
    pick(
        lang,
        "No digest is enabled. Start with <code>/digest daily</code>.",
        "Дайджест не увімкнено. Почніть із <code>/digest daily</code>.",
    )
}

pub fn digest_window_label(lang: Lang, start: i16, end: i16) -> String {
    if start == end {
        return pick(lang, "none", "немає").to_owned();
    }
    format!("{start:02}:00–{end:02}:00")
}

pub fn digest_period_label(lang: Lang, daily: bool) -> &'static str {
    match (lang, daily) {
        (
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be,
            true,
        ) => template(lang, "daily"),
        (
            Lang::En
            | Lang::Pl
            | Lang::De
            | Lang::Cn
            | Lang::Jp
            | Lang::Fr
            | Lang::Nl
            | Lang::Cz
            | Lang::Be,
            false,
        ) => template(lang, "weekly"),
        (Lang::Uk, true) => "щоденний",
        (Lang::Uk, false) => "щотижневий",
    }
}

/// The digest PM itself. Names are escaped here because they come from metadata.
pub fn digest_message(
    lang: Lang,
    period_label: &str,
    listens: i64,
    tracks: i64,
    genres: &[(String, i64)],
    top_tracks: &[(String, Option<String>, i64)],
) -> String {
    let title = match lang {
        Lang::En
        | Lang::Pl
        | Lang::De
        | Lang::Cn
        | Lang::Jp
        | Lang::Fr
        | Lang::Nl
        | Lang::Cz
        | Lang::Be => template(lang, "📊 Your digest"),
        Lang::Uk => "📊 Твій дайджест",
    };
    let listens_label = pick(lang, "Listens", "Прослуховувань");
    let tracks_label = pick(lang, "Tracks", "Треків");
    let genres_label = pick(lang, "Genres", "Жанри");
    let top_label = pick(lang, "Top tracks", "Топ треків");
    let mut body = format!(
        "<b>{title}</b> · {period_label}\n\n{listens_label}: <b>{listens}</b>\n{tracks_label}: <b>{tracks}</b>"
    );
    if !genres.is_empty() {
        let tags: Vec<String> = genres
            .iter()
            .map(|(genre, count)| format!("{} <b>{count}</b>", html_escape::encode_text(genre)))
            .collect();
        body.push_str(&format!("\n{genres_label}: {}", tags.join(" · ")));
    }
    if !top_tracks.is_empty() {
        body.push_str(&format!("\n\n<b>{top_label}:</b>"));
        for (track, artist, count) in top_tracks {
            let name = match artist.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
                Some(artist) => format!("{} — ", html_escape::encode_text(artist)),
                None => String::new(),
            };
            let track = html_escape::encode_text(track);
            body.push_str(&format!("\n• {name}{track} · <b>{count}</b>"));
        }
    }
    body.push_str(&format!("\n\n— {DEV_CHANNEL_HANDLE}"));
    body
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
            assert!(!lyrics_usage(lang).is_empty());
            assert!(!lyrics_lrc_button(lang).is_empty());
            assert!(!lyrics_other_source_button(lang).is_empty());
            assert!(!lyrics_lrc_sent(lang).is_empty());
            assert!(!lyrics_no_other_source(lang).is_empty());
            assert!(!changelog_title(lang).is_empty());
            assert!(!broadcast_usage(lang).is_empty());
            assert!(!genre_chart_usage_period(lang).is_empty());
            assert!(!user_stats(lang, 1, 2, 3, 3600).is_empty());
            assert!(!convert_usage(lang).is_empty());
            assert!(!normalize_usage(lang).is_empty());
            assert!(!groupmode_usage(lang).is_empty());
            assert!(!playlist_usage(lang).is_empty());
            assert!(!playlist_searching(lang).is_empty());
            assert!(!playlist_results(lang, "x", 1).is_empty());
            assert!(!no_playlists_found(lang).is_empty());
            assert!(!history_title(lang, "all").is_empty());
            assert!(!history_empty(lang).is_empty());
            assert!(!history_item("Song", Some("Artist"), "2026-11-18 09:00").is_empty());
            assert!(!dynamics_title(lang).is_empty());
            assert!(!dynamics_empty(lang).is_empty());
            assert!(!digest_usage(lang).is_empty());
            assert!(!digest_disabled(lang).is_empty());
            assert!(!digest_not_enabled(lang).is_empty());
            assert!(!digest_status(lang, "daily", "2026-11-19 09:00", "none", true).is_empty());
            assert!(!digest_message(lang, "daily", 1, 1, &[], &[]).is_empty());
            assert!(!import_waiting(lang).is_empty());
            assert!(!import_cancelled(lang).is_empty());
        }
    }

    #[test]
    fn digest_message_escapes_metadata_and_credits_channel() {
        for lang in [Lang::En, Lang::Uk] {
            let text = digest_message(
                lang,
                "daily",
                3,
                2,
                &[("Rock & <Roll>".to_owned(), 3)],
                &[("A < B".to_owned(), Some("X&Y".to_owned()), 2)],
            );
            assert!(text.contains("&lt;Roll&gt;"), "{text}");
            assert!(text.contains("A &lt; B"), "{text}");
            assert!(text.contains("X&amp;Y"), "{text}");
            assert!(text.contains(DEV_CHANNEL_HANDLE));
        }
    }

    #[test]
    fn every_language_uses_the_developer_handle() {
        for lang in [Lang::En, Lang::Uk] {
            assert!(
                user_stats(lang, 1, 1, 1, 1).contains(DEV_CHANNEL_HANDLE),
                "user stats must carry the developer channel"
            );
            assert!(lyrics_footer(lang).contains(DEV_CHANNEL_HANDLE));
        }
    }

    #[test]
    fn help_has_no_unescaped_angle_brackets() {
        for lang in [Lang::En, Lang::Uk] {
            let help = help(lang, true);
            assert!(help.contains("&lt;tracks|albums|artists&gt;"));
            assert!(!help.contains("<tracks"));
            assert!(!help.contains("<nick>"));
        }
    }
}
