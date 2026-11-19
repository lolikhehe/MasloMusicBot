//! Belarusian translations. Keys are the English source strings from `i18n.rs`.
pub fn lookup(english: &str) -> Option<&'static str> {
    match english {
        "English" => Some("Беларуская"),
        "⚖️ <b>Use responsibly.</b> Download only music you own, that is licensed under a free licence, or that you are otherwise allowed to copy. You are responsible for how you use this bot. The bot does not bypass DRM or paid access, and it stores only metadata plus Telegram file identifiers." => {
            Some(
                "⚖️ <b>Карыстайцеся адказна.</b> Спампоўвайце толькі музыку, якой вы валодаеце, якая мае свабодную ліцэнзію або якую вам дазволена капіяваць. Вы нясеце адказнасць за выкарыстанне гэтага бота. Бот не абыходзіць DRM ці плацевы доступ і захоўвае толькі метаданыя ды ідэнтыфікатары файлаў Telegram.",
            )
        }
        "🎵 <b>Music Bot</b>\n\nSend a track link, an album link, or just <code>Artist — Title</code>. I will find the track and send it as audio with tags and cover.\n\nYou can also use me inline: type <code>@this_bot song</code> in any chat." => {
            Some(
                "🎵 <b>Music Bot</b>\n\nАдашліце спасылку на трэк, альбом або проста <code>Выканаўца — Назва</code>. Я знайду трэк і адашлю яго як аўдыё з тэгамі і вокладкай.\n\nТаксама можна карыстацца мной у інлайн-рэжыме: напішыце <code>@this_bot song</code> у любым чаце.",
            )
        }
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality (kbps or FLAC lossless), source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)\n/import [list] — import a playlist; with no argument the next message (file or Artist – Title lines) is used\n/profile or /lastfmstats [period] — scrobble statistics (enable Last.fm stats in /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — PNG chart of your most played items\n/genres [5..20] — genre chart from your downloads (enable the analyzer in /settings)\n/lyrics [Artist — Title] — song lyrics from public sources (no argument: last track; or reply to an audio)\n/changelog — show the release notes\n/me — your own counters and favourite genres\n/queue — recent download jobs\n/cancel &lt;id&gt; — cancel an active download\n/stop — cancel all your active actions at once\n/convert auto|m4a|mp3|flac|opus — output format; reply to a track to convert that one\n/normalize on|off — ffmpeg loudnorm pass\n/playlist &lt;name&gt; — search public playlists\n/fav — your saved ⭐ tracks (tap ☆ under a track to save it)\n/similar — reply to one of my tracks: up to 10 similar tracks\n/artist &lt;name&gt; — top tracks of an artist\n/cover — reply to one of my tracks: full-size album cover\n/albums — your saved albums\n/recognize — reply to an audio file: identify it by sound\n/groupmode on|off — admin: allow the bot in this group\n/history [period] — your recent listening history\n/dynamics [period] — genre dynamics over time chart\n/digest daily|weekly|off — personal digest to your private chat\n/digest quiet HH-HH|off — quiet hours in UTC\n/digest status — current digest schedule" => {
            Some(
                "<b>Як карыстацца</b>\n\n• Прыватны чат: адашліце спасылку або <code>Выканаўца — Назва</code>.\n• Інлайн: <code>@this_bot query</code> у любым чаце. Пусты запыт з падключаным Last.fm прапануе тое, што грае зараз.\n• Альбомы і плейлісты спампоўваюцца па аднаму трэку з паведамленнем аб прагрэсе. Выкарыстоўвайце <code>/alb &lt;назва&gt;</code>, каб знайсці альбом па назве.\n• Паўторныя запыты аддаюцца з кэшу.\n\n<b>Команды</b>\n/start — пра бота\n/help — гэта даведка\n/settings — мова, якасць (kbps або FLAC lossless), крыніца і тэгі\n/alb — знайсці альбом па назве і спампаваць яго\n/lastfm — падключыць Last.fm (пусты інлайн-запыт прапануе трэк, які грае зараз)\n/import [спіс] — імпарт плейліста; без аргументу выкарыстоўваецца наступнае паведамленне (файл або радкі «Выканаўца – Назва»)\n/profile або /lastfmstats [перыяд] — статыстыка скроблаў (уключыце статыстыку Last.fm у /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [перыяд] — PNG-графік вашых найбольш праслуханых трэкаў\n/genres [5..20] — графік жанраў з вашых спамповак (уключыце аналізатар у /settings)\n/lyrics [Выканаўца — Назва] — тэкст песні з публічных крыніц (без аргументу: апошні трэк; або адказ на аўдыё)\n/changelog — паказаць спіс змен\n/me — вашая статыстыка і любімыя жанры\n/queue — апошнія задачы спампоўкі\n/cancel &lt;id&gt; — скасаваць актыўную спампоўку\n/stop — скасаваць усе вашы актыўныя дзеянні адразу\n/convert auto|m4a|mp3|flac|opus — фармат вываду; адказ на трэк канвертуе менавіта яго\n/normalize on|off — выраўноўванне гучнасці ffmpeg\n/playlist &lt;назва&gt; — пошук публічных плейлістоў\n/fav — вашы захаваныя ⭐ трэкі (націсніце ☆ пад трэкам, каб захаваць)\n/similar — адказ на мой трэк: да 10 падобных трэкаў\n/artist &lt;ім'я&gt; — топ трэкаў выканаўцы\n/cover — адказ на мой трэк: вокладка альбома ў поўным памеры\n/albums — твае захаваныя альбомы\n/recognize — адказ на аюдьёфайл: распазнаць па гучанні\n/groupmode on|off — адмін: дазволіць бота ў гэтай групе\n/history [перыяд] — вашая апошняя гісторыя праслухоўванняў\n/dynamics [перыяд] — графік дынамікі жанраў у часе\n/digest daily|weekly|off — персанальны дайджэст у прыватны чат\n/digest quiet HH-HH|off — ціхія гадзіны па UTC\n/digest status — бягучы расклад дайджэсту",
            )
        }
        "{base}\n\n<b>Admin</b>\n/stats — counters\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;key or url&gt;\n/broadcast &lt;text&gt; — one-off notice to all known users" => {
            Some(
                "{base}\n\n<b>Адмін</b>\n/stats — лічильнікі\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;ключ або URL&gt;\n/broadcast &lt;текст&gt; — аднаразовае паведамленне ўсім вядомым карыстальнікам",
            )
        }
        "📝 Usage: <code>/lyrics Artist — Title</code>\nOr send <code>/lyrics</code> after a track, or reply to an audio message with <code>/lyrics</code>." => {
            Some(
                "📝 Выкарыстанне: <code>/lyrics Выканаўца — Назва</code>\nАбо адашліце <code>/lyrics</code> пасля трэка, або адкажыце на аўдыёпаведамленне з <code>/lyrics</code>.",
            )
        }
        "🔎 Looking up lyrics…" => Some("🔎 Шукаю тэкст песні…"),
        "<b>Lyrics</b>" => Some("<b>Тэкст песні</b>"),
        "timestamped" => Some("з таймкодамі"),
        "plain text" => Some("звычайны тэкст"),
        "😕 Lyrics not found in the public sources (LRCLIB, lyrics.ovh)." => {
            Some("😕 Тэкст песні не знойдзены ў публічных крыніцах (LRCLIB, lyrics.ovh).")
        }
        "Send a track first, reply to an audio message, or pass a query: <code>/lyrics Artist — Title</code>." => {
            Some(
                "Спачатку адашліце трэк, адкажыце на аўдыёпаведамленне або ўкажыце запыт: <code>/lyrics Выканаўца — Назва</code>.",
            )
        }
        "📄 Download .lrc" => Some("📄 Спампаваць .lrc"),
        "🔄 Other source" => Some("🔄 Іншая крыніца"),
        "📄 LRC file saved. Drop it next to the audio in your player." => {
            Some("📄 Файл LRC захаваны. Пакладзіце яго побач з аўдыё ў вашым плееры.")
        }
        "😕 The other public source has no lyrics for this track." => {
            Some("😕 Іншая публічная крыніца не мае тэксту для гэтага трэка.")
        }
        "<b>What's new</b>\n\n" => Some("<b>Што новага</b>\n\n"),
        "<b>Your stats</b>" => Some("<b>Вашая статыстыка</b>"),
        "Tracks counted" => Some("Трэкаў улічана"),
        "Listens" => Some("Праслухоўванняў"),
        "Genres" => Some("Жанры"),
        "Audio time" => Some("Час аўдыё"),
        "top: /genres" => Some("топ: /genres"),
        "⏳ This download is already being processed by another instance." => {
            Some("⏳ Гэтая спампоўка ўжо апрацоўваецца іншым экзэмплярам бота.")
        }
        "🛑 Download cancelled." => Some("🛑 Спампоўка скасавана."),
        "<b>Recent downloads</b>\n\n" => Some("<b>Апошнія спампоўкі</b>\n\n"),
        "No downloads yet." => Some("Спамповак яшчэ няма."),
        " · 🛑 cancelling" => Some(" · 🛑 скасоўваецца"),
        "Usage: <code>/cancel &lt;job id from /queue&gt;</code>" => {
            Some("Выкарыстанне: <code>/cancel &lt;id задачы з /queue&gt;</code>")
        }
        "🛑 Cancellation requested." => Some("🛑 Скасаванне запрошана."),
        "No active job with that id." => Some("Актыўнай задачы з такім id няма."),
        "Usage: <code>/convert auto|m4a|mp3|flac|opus</code> — applies to your next downloads. Reply to a track with <code>/convert &lt;format&gt;</code> to convert that track." => {
            Some(
                "Выкарыстанне: <code>/convert auto|m4a|mp3|flac|opus</code> — прымяняецца да вашых наступных спамповак. Адкажыце на трэк з <code>/convert &lt;фармат&gt;</code>, каб канвертаваць менавіта яго.",
            )
        }
        "This message is not a downloaded track I can convert." => {
            Some("Гэта паведамленне не з'яўляецца спампаваным трэкам, які я магу канвертаваць.")
        }
        "Converted to" => Some("Канвертавана ў"),
        "Conversion failed" => Some("Не ўдалося канвертаваць"),
        "Stopped: {jobs} download(s), {prompts} pending action(s)." => {
            Some("Спынена: спамповак — {jobs}, чакаючых дзеянняў — {prompts}.")
        }
        "Output format" => Some("Фармат вываду"),
        "ℹ️ Album & artist info" => Some("ℹ️ Пра альбом і выканаўца"),
        "⭐ Added to favorites." => Some("⭐ Дададзена ў выбранае."),
        "Removed from favorites." => Some("Выдалена са выбранага."),
        "⭐ Your favorites" => Some("⭐ Выбраныя трэкі"),
        "Nothing saved yet — tap ☆ under any track." => {
            Some("Пакуль нічога — націсні ☆ пад трэкам.")
        }
        "Reply to one of my tracks with /similar." => {
            Some("Адпраўце /similar у адказ на мой трэк.")
        }
        "No similar tracks found." => Some("Падобных трэкаў не знайшоў."),
        "🎧 Similar to «{track}»" => Some("🎧 Падобна да «{track}»"),
        "Usage: /artist &lt;name&gt;" => Some("Выкарыстанне: /artist &lt;ім'я&gt;"),
        "Artist not found in the catalog." => Some("Выканаўцы не знайшоў у каталогу."),
        "🎤 Top tracks of {name}" => Some("🎤 Топ трэкаў {name}"),
        "Reply to one of my tracks with /cover." => Some("Адпраўце /cover у адказ на мой трэк."),
        "This track has no cover." => Some("У гэтага трэка няма абкладінкі."),
        "⚙ Advanced" => Some("⚙ Дадатковае"),
        "<b>Advanced settings</b>" => Some("<b>Дадатковыя налады</b>"),
        "on" => Some("укл."),
        "off" => Some("выкл."),
        "Usage: <code>/normalize on|off</code> (ffmpeg loudnorm, opt-in)." => Some(
            "Выкарыстанне: <code>/normalize on|off</code> (выраўноўванне гучнасці ffmpeg, па жаданню).",
        ),
        "Admin only. Usage: <code>/groupmode on|off</code>" => {
            Some("Толькі для адміністратараў. Выкарыстанне: <code>/groupmode on|off</code>")
        }
        "✅ Group mode enabled." => Some("✅ Групавы рэжым уключаны."),
        "🛑 Group mode disabled." => Some("🛑 Групавы рэжым выключаны."),
        "🔎 Searching public playlists…" => Some("🔎 Шукаю публічныя плейлісты…"),
        "Playlists" => Some("Плейлісты"),
        "😕 No public playlists found." => Some("😕 Публічных плейлістоў не знойдзена."),
        "Usage: <code>/playlist &lt;name&gt;</code> — search public playlists (Deezer). Apple Music links already work as album URLs." => {
            Some(
                "Выкарыстанне: <code>/playlist &lt;назва&gt;</code> — пошук публічных плейлістоў (Deezer). Спасылкі Apple Music ужо працуюць як спасылкі на альбом.",
            )
        }
        "Usage: <code>/broadcast &lt;text&gt;</code>" => {
            Some("Выкарыстанне: <code>/broadcast &lt;text&gt;</code>")
        }
        "Broadcast complete" => Some("Рассылка завершана"),
        "Usage: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>" => {
            Some("Выкарыстанне: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>")
        }
        "<b>Settings</b>" => Some("<b>Налады</b>"),
        "Quality" => Some("Якасць"),
        "Language" => Some("Мова"),
        "Search source (inline)" => Some("Крыніца пошуку (inline)"),
        "Tags (album + genres)" => Some("Тэгі (альбом + жанры)"),
        "Last.fm" => Some("Last.fm"),
        "not connected (setup: /lastfm)" => Some("не падключана (налады: /lastfm)"),
        "HQ cover (1000px)" => Some("Вокладка HQ (1000px)"),
        "Last.fm stats" => Some("Статыстыка Last.fm"),
        "Genre analyzer" => Some("Аналізатар жанраў"),
        "🏷 Tags: album + genre" => Some("🏷 Тэгі: альбом і жанр"),
        "🎛 Genre analyzer" => Some("🎛 Аналізатар жанраў"),
        "📊 Genre chart" => Some("📊 Графік жанраў"),
        "🎛 Enable the genre analyzer in /settings first." => {
            Some("🎛 Спачатку ўключыце аналізатар жанраў у /settings.")
        }
        "No genre data yet — download a few tracks with the analyzer enabled." => {
            Some("Даных аб жанрах яшчэ няма — спампуйце некалькі трэкаў з уключаным аналізатарам.")
        }
        "Could not build the genre chart." => Some("Не ўдалося пабудаваць графік жанраў."),
        "Your genres" => Some("Твае жанры"),
        "{listens} listens · {genres} genres" => Some("{listens} праслухванняў · {genres} жанраў"),
        "Usage: /genres [5..20] — number of bars." => {
            Some("Выкарыстанне: /genres [5..20] — колькасць слупкоў.")
        }
        "📊 Last.fm stats + collages" => Some("📊 Статыстыка Last.fm + колажы"),
        "🎵 Last.fm is not configured on this bot yet." => {
            Some("🎵 Last.fm на гэтым боте яшчэ не наладжаны.")
        }
        "🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off" => {
            Some(
                "🎵 Last.fm падключаны як <b>{nick}</b>. Цяпер у інлайн-рэжыме пусты запыт будзе прапаноўваць тое, што вы слухаеце.\nАдключыць: /lastfm off",
            )
        }
        "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now." => {
            Some(
                "🎵 Last.fm: не падключаны.\nПадключыць: /lastfm &lt;ваш нікнэйм&gt; — пусты інлайн-запыт будзе прапаноўваць тое, што грае зараз.",
            )
        }
        "🔌 Last.fm disconnected." => Some("🔌 Last.fm адключаны."),
        "⚠️ Last.fm says there is no such user. Check the nickname spelling at last.fm." => {
            Some(
                "⚠️ Last.fm паведамляе, што такога карыстальніка няма. Праверце напісанне нікнэйму на last.fm.",
            )
        }
        "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public." => {
            Some(
                "✅ Падключаны як <b>{nick}</b>, але апошніх трэкаў яшчэ няма. Пераканайцеся, што ваш плеер скробліць на Last.fm і профіль з'яўляецца публічным.",
            )
        }
        "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it." => {
            Some(
                "✅ Падключаны як <b>{nick}</b>. Зараз грае: <b>{track}</b>. У інлайн-рэжыме пусты запыт прапануе яго.",
            )
        }
        "⚠️ Invalid Last.fm username (2–15 latin letters, digits, _ or -)." => {
            Some("⚠️ Няправільны нікнэйм Last.fm (2–15 лацінскіх літар, лічбаў, _ або -).")
        }
        "▶ Now playing: {track}" => Some("▶ Зараз грае: {track}"),
        "FLAC (lossless)" => Some("FLAC (без страт)"),
        "🖼 HQ cover (1000px)" => Some("🖼 Вокладка HQ (1000px)"),
        "⚠️ Your chosen search source is blocked by the server config — the bot falls back to YouTube. Ask the operator to enable it (SEARCH_PROVIDERS)." => {
            Some(
                "⚠️ Выбраная крыніца пошуку заблакаваная канфігурацыяй сервера — бот будзе шукаць у YouTube. Папрасіце аператара ўключыць яе (SEARCH_PROVIDERS).",
            )
        }
        "✅ Settings saved." => Some("✅ Налады захаваны."),
        "🇬🇧 English" => Some("🇬🇧 Беларуская"),
        "🌐 Language" => Some("🌐 Мова"),
        "<b>Language</b>\nPick the interface language:" => {
            Some("<b>Мова</b>\nАбярыце мову інтэрфейсу:")
        }
        "« Back" => Some("« Назад"),
        "◀ Back" => Some("◀ Назад"),
        "⬇️ Downloading… this usually takes a few seconds." => {
            Some("⬇️ Спампоўка… звычайна гэта займае некалькі секунд.")
        }
        "🔎 Searching…" => Some("🔎 Шукаю…"),
        "Nothing found. Try a different spelling or paste a direct link." => {
            Some("Нічога не знойдзена. Паспрабуйце іншае напісанне або ўстаўце прамую спасылку.")
        }
        "🚫 You are not allowed to use this bot." => {
            Some("🚫 Вам забаронена карыстацца гэтым ботом.")
        }
        "⛔ Admins only." => Some("⛔ Толькі для адміністратараў."),
        "⏳ Too many requests. Limit: {per_minute} per minute." => {
            Some("⏳ Занадта шмат запытаў. Ліміт: {per_minute} у хвіліну.")
        }
        "ℹ️ Send links and search queries in a private chat with me. Commands work here too." => {
            Some(
                "ℹ️ Адашліце спасылкі і пошукавыя запыты ў прыватным чаце з ботом. Каманды працуюць і тут.",
            )
        }
        "🔒 This track is private or requires signing in, so I cannot download it." => {
            Some("🔒 Гэты трэк прыватны або патрабуе ўваходу, таму я не магу яго спампаваць.")
        }
        "🌍 This track is region locked and is not available from the bot's location." => {
            Some("🌍 Гэты трэк заблакаваны ў рэгіёне і недаступны з месца працы бота.")
        }
        "⏱ The track is too long. Maximum duration is {} minutes." => {
            Some("⏱ Трэк занадта доўгі. Максімальная працягласць — {} хвілін.")
        }
        "📦 The file is too large. Maximum size is {} MB." => {
            Some("📦 Файл занадта вялікі. Максімальны памер — {} МБ.")
        }
        "🔍 I could not find that track on the available sources." => {
            Some("🔍 Не ўдалося знайсці гэты трэк у даступных крыніцах.")
        }
        "🤷 This link is not supported, or it needs a paid/DRM-protected source. Try a direct track link or search by name." => {
            Some(
                "🤷 Гэтая спасылка не падтрымліваецца або патрабуе платнай/DRM-абароненай крыніцы. Паспрабуйце прамую спасылку на трэк або пошук па назве.",
            )
        }
        "🚫 {} is disabled on this bot." => Some("🚫 {} адключаны на гэтым боте."),
        "📡 The source is temporarily unavailable. Please try again in a moment." => {
            Some("📡 Крыніца часова недаступная. Паўтарыце спробу праз імгненне.")
        }
        "🎧 I downloaded the track but could not prepare the audio file." => {
            Some("🎧 Трэк спампаваны, але не ўдалося падрыхтаваць аўдыёфайл.")
        }
        "⚠️ Something went wrong while preparing the track. Please try again." => {
            Some("⚠️ Падчас падрыхтоўкі трэка адбылася памылка. Паўтарыце спробу.")
        }
        "💿 <b>{title}</b>\n{count} track(s). Download the whole collection?" => {
            Some("💿 <b>{title}</b>\n{count} трэкаў. Спампаваць усю калекцыю?")
        }
        "⬇ Download all" => Some("⬇ Спампаваць усе"),
        "Cancel" => Some("Скасаваць"),
        "Cancelled." => Some("Скасавана."),
        "💿 Loading the album…" => Some("💿 Спампоўваю альбом…"),
        "💿 <b>{title}</b>\n{count} track(s) queued." => {
            Some("💿 <b>{title}</b>\n{count} трэкаў у чарзе.")
        }
        "💿 {current}/{total} — {title}" => Some("💿 {current}/{total} — {title}"),
        "✅ Album finished: {ok} sent, {failed} failed." => {
            Some("✅ Альбом завершаны: адаслана {ok}, з памылкамі {failed}.")
        }
        "This album or playlist has no downloadable tracks." => {
            Some("У гэтым альбоме або плейлісце няма трэкаў для спампоўкі.")
        }
        " and {extra} more" => Some(" і яшчэ {extra}"),
        "⚠ Failed: {list}{more}.\nSend /alb again — the finished tracks will come from cache instantly." => {
            Some(
                "⚠ Не ўдалося: {list}{more}.\nПаўтарыце /alb — паспяховыя трэкі будуць узяты з кэшу імгненна.",
            )
        }
        "Usage: /alb &lt;album name&gt;\nExample: /alb Master of Puppets" => {
            Some("Выкарыстанне: /alb &lt;назва альбому&gt;\nПрыклад: /alb Master of Puppets")
        }
        "🔎 Searching albums…" => Some("🔎 Шукаю альбомы…"),
        "<b>Albums for</b> «{query}»\nPick one:" => {
            Some("<b>Альбомы па запыце</b> «{query}»\nАбярыце адзін:")
        }
        "No albums found. Try a different artist or album title." => {
            Some("Альбомы не знойдзены. Паспрабуйце ўдакладніць выканаўцу або назву альбому.")
        }
        " · {count} tracks" => Some(" · {count} трэкаў"),
        "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>" => {
            Some(
                "<b>Статыстыка</b>\nТрэкаў у кэшы: <b>{}</b>\nВядомых карыстальнікаў: <b>{}</b>\nПаспяховых адправак: <b>{}</b>\nЧас працы: <b>{uptime}</b>",
            )
        }
        "🆕 <b>Bot update</b>\n\n{notes}\n\n💬 Developer channel: @maslougazIA" => {
            Some("🆕 <b>Абнаўленне бота</b>\n\n{notes}\n\n💬 Канал распрацоўніка: @maslougazIA")
        }
        "🗑 Purged <code>{key}</code> from cache." => {
            Some("🗑 Выдалена <code>{key}</code> з кэшу.")
        }
        "That key is not in the cache." => Some("Такога ключа няма ў кэшы."),
        "🚫 User {user_id} banned." => Some("🚫 Карыстальнік {user_id} забанены."),
        "✅ User {user_id} unbanned." => Some("✅ Карыстальнік {user_id} разбанены."),
        "Usage: /purge &lt;cache key or track URL&gt;" => {
            Some("Выкарыстанне: /purge &lt;ключ кэшу або спасылка на трэк&gt;")
        }
        "⌛ This result has expired. Please search again." => {
            Some("⌛ Гэты вынік састарэў. Паўтарыце пошук.")
        }
        "✅ Sent: {title}" => Some("✅ Адаслана: {title}"),
        "🔎 Results for “{query}” ({count}):" => Some("🔎 Вынікі для “{query}” ({count}):"),
        "Usage: /ban &lt;numeric user id&gt;" => {
            Some("Выкарыстанне: /ban &lt;лічбавы id карыстальніка&gt;")
        }
        "Open bot" => Some("Адкрыць бота"),
        "📩 Sent to your private messages with the bot." => {
            Some("📩 Адаслана вам у прыватныя паведамленні з ботом.")
        }
        "⏳ Preparing the track…" => Some("⏳ Рыхтую трэк…"),
        "Send /import followed by a playlist (Artist – Title lines), or just /import and then send the file or the list in the next message." => {
            Some(
                "Адашліце /import і спіс трэкаў (радкі «Выканаўца – Назва»), або проста /import — і ў наступным паведамленні адашліце файл або спіс.",
            )
        }
        "📥 Waiting for the playlist. Send a CSV/TXT/JSON file (Spotify, TuneMyMusic, Soundiiz export) or paste lines like <code>Artist – Title</code>. Send <code>cancel</code> to abort." => {
            Some(
                "📥 Чакаю плейліст. Адашліце файл CSV/TXT/JSON (экспарт Spotify, TuneMyMusic, Soundiiz) або ўстаўце радкі выгляду <code>Выканаўца – Назва</code>. Адашліце <code>cancel</code>, каб скасаваць.",
            )
        }
        "🛑 Playlist import cancelled." => Some("🛑 Імпарт плейліста скасаваны."),
        "🔎 Matching {count} imported tracks…" => Some("🔎 Шукаю {count} імпартаваных трэкаў…"),
        "No tracks found in that playlist." => Some("У гэтым плейлісце не знойдзена трэкаў."),
        "Too many tracks; import at most {max} at a time." => {
            Some("Занадта шмат трэкаў; імпартуйце не больш {max} за раз.")
        }
        "Could not read that playlist file." => Some("Не ўдалося прачытаць файл плейліста."),
        "📊 Enable Last.fm stats in /settings first." => {
            Some("📊 Спачатку ўключыце статыстыку Last.fm у /settings.")
        }
        "Connect your public Last.fm nickname with /lastfm <nick> first." => {
            Some("Падключыце публічны нікнэйм Last.fm праз /lastfm <nick>.")
        }
        "Usage: /profile [7d|1m|3m|6m|1y|all]" => {
            Some("Выкарыстанне: /profile [7d|1m|3m|6m|1y|all]")
        }
        "Usage: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]" => {
            Some("Выкарыстанне: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]")
        }
        "📊 <b>Last.fm: {name}</b>\nScrobbles (all time): <b>{playcount}</b>\nLibrary (all time): {tracks} tracks · {albums} albums · {artists} artists" => {
            Some(
                "📊 <b>Last.fm: {name}</b>\nСкроблаў (весь час): <b>{playcount}</b>\nБібліятэка (весь час): {tracks} трэкаў · {albums} альбомаў · {artists} выканаўцаў",
            )
        }
        "\nCountry: {country}" => Some("\nКраіна: {country}"),
        "\n\n<b>Top tracks ({period}):</b>" => Some("\n\n<b>Топ трэкаў ({period}):</b>"),
        "🎨 Generating collage…" => Some("🎨 Ствараю колаж…"),
        "Could not build the collage (Last.fm returned no data)." => {
            Some("Не ўдалося стварыць колаж (Last.fm вярнуў пустыя даныя).")
        }
        "⏳ Downloading: {track}" => Some("⏳ Спампоўка: {track}"),
        "<b>Listening history</b> · {period}\n\n" => {
            Some("<b>Гісторыя праслухоўванняў</b> · {period}\n\n")
        }
        "Nothing in this period yet — send a track first." => {
            Some("За гэты перыяд яшчэ нічога няма — спачатку адашліце трэк.")
        }
        "\n… and {hidden} earlier\n" => Some("\n… і яшчэ {hidden} раней\n"),
        "Usage: <code>/history [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Выкарыстанне: <code>/history [7d|1m|3m|6m|1y|all]</code>")
        }
        "Genre dynamics" => Some("Дынаміка жанраў"),
        "{cells} genre-points · {buckets} buckets · {period}" => {
            Some("{cells} ачкоў жанраў · {buckets} інтэрвалаў · {period}")
        }
        "No timed genre data yet. Enable the analyzer in /settings and download a few tracks." => {
            Some(
                "Даных аб жанрах з часам яшчэ няма. Уключыце аналізатар у /settings і спампуйце некалькі трэкаў.",
            )
        }
        "Could not build the dynamics chart." => Some("Не ўдалося пабудаваць графік дынамікі."),
        "Usage: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Выкарыстанне: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>")
        }
        "Usage:\n<code>/digest daily|weekly</code> — enable (default 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — quiet hours in UTC\n<code>/digest status</code> — current schedule\n<code>/digest off</code> — disable" => {
            Some(
                "Выкарыстанне:\n<code>/digest daily|weekly</code> — уключыць (па змаўчанні 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — ціхія гадзіны па UTC\n<code>/digest status</code> — бягучы расклад\n<code>/digest off</code> — выключыць",
            )
        }
        "✅ {period} digest enabled. Next: <b>{next}</b> UTC." => {
            Some("✅ Дайджэст {period} уключаны. Наступны: <b>{next}</b> UTC.")
        }
        "🛑 Digest disabled." => Some("🛑 Дайджэст выключаны."),
        "🌙 Quiet hours: <b>{window}</b> UTC." => Some("🌙 Ціхія гадзіны: <b>{window}</b> UTC."),
        "📅 Digest: <b>{state}</b>\nPeriod: {period}\nNext: {next} UTC\nQuiet hours: {quiet} UTC" => {
            Some(
                "📅 Дайджэст: <b>{state}</b>\nПерыяд: {period}\nНаступны: {next} UTC\nЦіхія гадзіны: {quiet} UTC",
            )
        }
        "No digest is enabled. Start with <code>/digest daily</code>." => {
            Some("Дайджэст не ўключаны. Пачніце з <code>/digest daily</code>.")
        }
        "none" => Some("няма"),
        "daily" => Some("штодзённы"),
        "weekly" => Some("штотыднёвы"),
        "📊 Your digest" => Some("📊 Твой дайджэст"),
        "Tracks" => Some("Трэкаў"),
        "Top tracks" => Some("Топ трэкаў"),
        "🎵 Your albums" => Some("🎵 Твае альбомы"),
        "Library is empty — save an album with the 💾 button under a delivered album" => {
            Some("Бібліятэка пустая — захавай альбом кнопкай 💾 пад дастаўленым альбомам")
        }
        "💾 Save album" => Some("💾 Захаваць альбом"),
        "🗑 Remove" => Some("🗑 Выдаліць"),
        "Saved" => Some("Захавана"),
        "Already saved" => Some("Ужо захавана"),
        "Removed from library" => Some("Выдалена з бібліятэкі"),
        "🗜 Packing album…" => Some("🗜 Пакую альбом…"),
        "Packing failed: no track could be downloaded" => {
            Some("Не ўдалося запакаваць: ніводзін трэк не загрузіўся")
        }
        "🧹 Auto-delete" => Some("🧹 Аютавыдаленне"),
        "Auto-delete off" => Some("Аютавыдаленне выключана"),
        "1 hour" => Some("1 гадзіна"),
        "6 hours" => Some("6 гадзін"),
        "12 hours" => Some("12 гадзін"),
        "24 hours" => Some("24 гадзіны"),
        "The bot deletes its own audio messages after this time (max 48 h). Applies only to messages sent after enabling." => {
            Some(
                "Бот выдаляе свае аўдыёпаведленні праз гэты час (макс. 48 гадз). Дзейнічае толькі на паведленні, дасланыя пасля ўключэння.",
            )
        }
        "Reply to an audio file with /recognize" => {
            Some("Адкажыце на аўдыёфайл камандай /recognize")
        }
        "Recognition is not configured on this server" => {
            Some("Распазнаванне не наладжана на гэтым серверы")
        }
        "🔍 Listening…" => Some("🔍 Слухаю…"),
        "Could not identify this track" => Some("Не ўдалося распазнаць гэты трэк"),
        "⬇ Download as archive" => Some("⬇ Спампаваць архівам"),
        "Auto-delete: {n} hours" => Some("Аютавыдаленне: {n} гадз"),
        "Set: {n} hours" => Some("Устаноўлена: {n} гадз"),
        "≈ {n} min" => Some("≈ {n} хв"),
        "≈ {n} sec" => Some("≈ {n} с"),
        "Album is too large to pack (limit {n} MB)" => {
            Some("Альбом занадта вялікі для архіва (ліміт {n} МБ)")
        }
        "📦 {album} — {n} tracks" => Some("📦 {album} — {n} трэкаў"),
        "Looks like: {artist} — {title}" => Some("Падобна на: {artist} — {title}"),
        "💿 {album} · {year}" => Some("💿 {album} · {year}"),
        "⚠ {n} lines could not be resolved and were skipped" => {
            Some("⚠ {n} радкоў не ўдалося распазнаць — прапушчана")
        }
        // ---- 3.0.0: /recs and /pod ----
        "/recs — personalized picks from your listening history" => {
            Some("/recs — персаналізаваныя рэкамендацыі з тваёй гісторыі праслухоўвання")
        }
        "🎯 <b>Recommended for you</b>" => Some("🎯 <b>Рэкамендавана для цябе</b>"),
        "Popular with listeners" => Some("Папулярна слухачамі"),
        "Based on {n} listens" => Some("На аснове {n} праслухоўванняў"),
        "🌱 Nothing to recommend yet — play a few tracks first." => {
            Some("🌱 Пакуль няма рэкамендацый — спачатку паслухай некалькі трэкаў.")
        }
        "Usage:\n<code>/pod &lt;RSS/Atom URL&gt;</code> — subscribe in this chat\n<code>/pod list</code> — feeds in this chat\n<code>/pod del &lt;n&gt;</code> — unsubscribe by number" => {
            Some(
                "Выкарыстанне:\n<code>/pod &lt;RSS/Atom URL&gt;</code> — падпісацца ў гэтым чаце\n<code>/pod list</code> — фіды гэтага чату\n<code>/pod del &lt;n&gt;</code> — адпісацца па нумары",
            )
        }
        "✅ Subscribed to <b>{title}</b>. New episodes will arrive here." => {
            Some("✅ Падпісана на <b>{title}</b>. Новыя выпускі з'явяцца тут.")
        }
        "You are already subscribed to this feed in this chat." => {
            Some("Ты ўжо падпісаны на гэты фід у гэтым чаце.")
        }
        "😕 Could not read that feed. Check the URL and try again." => {
            Some("😕 Не ўдалося прачытаць гэты фід. Правер URL і паспрабуй зноў.")
        }
        "You can subscribe to at most 10 feeds." => {
            Some("Можна падпісацца не больш чым на 10 фідаў.")
        }
        "The bot is at its feed limit right now. Try again later." => {
            Some("Бот зараз дасягнуў ліміту фідаў. Паспрабуй пазней.")
        }
        "No podcast subscriptions in this chat. Add one with <code>/pod &lt;URL&gt;</code>." => {
            Some(
                "У гэтым чаце няма падпісак на падкасты. Дадай праз <code>/pod &lt;URL&gt;</code>.",
            )
        }
        "🎙 Podcasts in this chat:" => Some("🎙 Падкасты ў гэтым чаце:"),
        "🗑 Unsubscribed from <b>{title}</b>." => Some("🗑 Адпісана ад <b>{title}</b>."),
        "Use a number from <code>/pod list</code>." => {
            Some("Выкарыстоўвай нумар са спісу <code>/pod list</code>.")
        }
        "🎙 <b>{feed_title}</b> · {n} new episode(s)" => {
            Some("🎙 <b>{feed_title}</b> · {n} новых выпускаў")
        }
        "🛑 Feed <b>{title}</b> stopped responding and was disabled. Remove it with /pod del." => {
            Some(
                "🛑 Фід <b>{title}</b> перастаў адказваць і быў адключаны. Прыбяры яго праз /pod del.",
            )
        }
        _ => None,
    }
}
