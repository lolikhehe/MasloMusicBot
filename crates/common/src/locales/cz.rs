//! Czech translations. Keys are the English source strings from `i18n.rs`.
pub fn lookup(english: &str) -> Option<&'static str> {
    match english {
        "English" => Some("Angličtina"),
        "⚖️ <b>Use responsibly.</b> Download only music you own, that is licensed under a free licence, or that you are otherwise allowed to copy. You are responsible for how you use this bot. The bot does not bypass DRM or paid access, and it stores only metadata plus Telegram file identifiers." => {
            Some(
                "⚖️ <b>Používejte zodpovědně.</b> Stahujte pouze hudbu, kterou vlastníte, která má svobodnou licenci nebo kterou smíte jinak kopírovat. Za použití tohoto bota jste zodpovědní vy. Bot neobchází DRM ani placený přístup a ukládá pouze metadata a identifikátory souborů Telegramu.",
            )
        }
        "🎵 <b>Music Bot</b>\n\nSend a track link, an album link, or just <code>Artist — Title</code>. I will find the track and send it as audio with tags and cover.\n\nYou can also use me inline: type <code>@this_bot song</code> in any chat." => {
            Some(
                "🎵 <b>Music Bot</b>\n\nPošlete odkaz na skladbu, album nebo jen <code>Interpret — Název</code>. Skladbu najdu a pošlu jako audio s tagy a obalem.\n\nMůžete mě používat i inline: napište <code>@this_bot píseň</code> v libovolném chatu.",
            )
        }
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality (kbps or FLAC lossless), source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)\n/import [list] — import a playlist; with no argument the next message (file or Artist – Title lines) is used\n/profile or /lastfmstats [period] — scrobble statistics (enable Last.fm stats in /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — PNG chart of your most played items\n/genres [5..20] — genre chart from your downloads (enable the analyzer in /settings)\n/lyrics [Artist — Title] — song lyrics from public sources (no argument: last track; or reply to an audio)\n/changelog — show the release notes\n/me — your own counters and favourite genres\n/queue — recent download jobs\n/cancel &lt;id&gt; — cancel an active download\n/stop — cancel all your active actions at once\n/convert auto|m4a|mp3|flac|opus — output format; reply to a track to convert that one\n/normalize on|off — ffmpeg loudnorm pass\n/playlist &lt;name&gt; — search public playlists\n/groupmode on|off — admin: allow the bot in this group\n/history [period] — your recent listening history\n/dynamics [period] — genre dynamics over time chart\n/digest daily|weekly|off — personal digest to your private chat\n/digest quiet HH-HH|off — quiet hours in UTC" => {
            Some(
                "<b>Jak používat</b>\n\n• Soukromý chat: pošlete odkaz nebo <code>Interpret — Název</code>.\n• Inline: <code>@this_bot dotaz</code> v libovolném chatu. Prázdný dotaz s připojeným Last.fm navrhne skladbu, která právě hraje.\n• Alba a playlisty se stahují po jedné skladbě se zprávou o průběhu. Pomocí <code>/alb &lt;název&gt;</code> najdete album podle názvu.\n• Opakované požadavky se vracejí z mezipaměti.\n\n<b>Příkazy</b>\n/start — o botovi\n/help — tato nápověda\n/settings — jazyk, kvalita (kbps nebo bezztrátový FLAC), zdroj a tagy\n/alb — najít album podle názvu a stáhnout ho\n/lastfm — připojit Last.fm (prázdný inline dotaz navrhne skladbu, která právě hraje)\n/import [seznam] — import playlisty; bez argumentu se použije další zpráva (soubor nebo řádky Interpret – Název)\n/profile nebo /lastfmstats [období] — statistiky scrobblů (zapněte statistiky Last.fm v /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [období] — PNG graf vašich nejčastěji hraných položek\n/genres [5..20] — graf žánrů z vašich stažených skladeb (zapněte analyzátor v /settings)\n/lyrics [Interpret — Název] — text písně z veřejných zdrojů (bez argumentu: poslední skladba nebo odpověď na audio)\n/changelog — zobrazit poznámky k vydání\n/me — vaše počítadla a oblíbené žánry\n/queue — nedávné úlohy stahování\n/cancel &lt;id&gt; — zrušit aktivní stahování\n/stop — okamžitě zrušit všechny vaše aktivní akce\n/convert auto|m4a|mp3|flac|opus — výstupní formát; odpověď na skladbu převede právě ji\n/normalize on|off — vyrovnání hlasitosti ffmpeg\n/playlist &lt;název&gt; — hledat veřejné playlisty\n/groupmode on|off — administrátor: povolit bota v této skupině\n/history [období] — vaše nedávná historie poslechu\n/dynamics [období] — graf vývoje žánrů v čase\n/digest daily|weekly|off — osobní přehled do vašeho soukromého chatu\n/digest quiet HH-HH|off — tiché hodiny v UTC",
            )
        }
        "📝 Usage: <code>/lyrics Artist — Title</code>\nOr send <code>/lyrics</code> after a track, or reply to an audio message with <code>/lyrics</code>." => {
            Some(
                "📝 Použití: <code>/lyrics Interpret — Název</code>\nNebo pošlete <code>/lyrics</code> po skladbě, nebo odpovězte na audio zprávu pomocí <code>/lyrics</code>.",
            )
        }
        "🔎 Looking up lyrics…" => Some("🔎 Hledám text písně…"),
        "<b>Lyrics</b>" => Some("<b>Text písně</b>"),
        "timestamped" => Some("s časovými značkami"),
        "plain text" => Some("prostý text"),
        "😕 Lyrics not found in the public sources (LRCLIB, lyrics.ovh)." => {
            Some("😕 Text písně nebyl ve veřejných zdrojích (LRCLIB, lyrics.ovh) nalezen.")
        }
        "Send a track first, reply to an audio message, or pass a query: <code>/lyrics Artist — Title</code>." => {
            Some(
                "Nejdřív pošlete skladbu, odpovězte na audio zprávu nebo zadejte dotaz: <code>/lyrics Interpret — Název</code>.",
            )
        }
        "📄 Download .lrc" => Some("📄 Stáhnout .lrc"),
        "🔄 Other source" => Some("🔄 Jiný zdroj"),
        "📄 LRC file saved. Drop it next to the audio in your player." => {
            Some("📄 Soubor LRC uložen. Umístěte ho vedle audia v přehrávači.")
        }
        "😕 The other public source has no lyrics for this track." => {
            Some("😕 Další veřejný zdroj nemá pro tuto skladbu text.")
        }
        "<b>What's new</b>\n\n" => Some("<b>Co je nového</b>\n\n"),
        "<b>Your stats</b>" => Some("<b>Vaše statistiky</b>"),
        "Tracks counted" => Some("Započtených skladeb"),
        "Listens" => Some("Přehrání"),
        "Genres" => Some("Žánrů"),
        "Audio time" => Some("Čas audia"),
        "top: /genres" => Some("top: /genres"),
        "⏳ This download is already being processed by another instance." => {
            Some("⏳ Toto stahování už zpracovává jiná instance.")
        }
        "🛑 Download cancelled." => Some("🛑 Stahování zrušeno."),
        "<b>Recent downloads</b>\n\n" => Some("<b>Nedávná stahování</b>\n\n"),
        "No downloads yet." => Some("Zatím žádná stahování."),
        "Usage: <code>/cancel &lt;job id from /queue&gt;</code>" => {
            Some("Použití: <code>/cancel &lt;id úlohy z /queue&gt;</code>")
        }
        "🛑 Cancellation requested." => Some("🛑 Zrušení vyžádáno."),
        "No active job with that id." => Some("Žádná aktivní úloha s tímto id neexistuje."),
        "Usage: <code>/convert auto|m4a|mp3|flac|opus</code> — applies to your next downloads. Reply to a track with <code>/convert &lt;format&gt;</code> to convert that track." => {
            Some(
                "Použití: <code>/convert auto|m4a|mp3|flac|opus</code> — platí pro vaše další stahování. Odpovězte na skladbu pomocí <code>/convert &lt;formát&gt;</code> a převedete právě ji.",
            )
        }
        "This message is not a downloaded track I can convert." => {
            Some("Tato zpráva není stažená skladba, kterou bych mohl převést.")
        }
        "Converted to" => Some("Převedeno do"),
        "Conversion failed" => Some("Převod se nepodařil"),
        "Stopped: {jobs} download(s), {prompts} pending action(s)." => {
            Some("Zastaveno: stahování — {jobs}, čekajících akcí — {prompts}.")
        }
        "Output format" => Some("Výstupní formát"),
        "on" => Some("zap."),
        "off" => Some("vyp."),
        "Usage: <code>/normalize on|off</code> (ffmpeg loudnorm, opt-in)." => {
            Some("Použití: <code>/normalize on|off</code> (ffmpeg loudnorm, volitelné).")
        }
        "Admin only. Usage: <code>/groupmode on|off</code>" => {
            Some("Pouze pro administrátory. Použití: <code>/groupmode on|off</code>")
        }
        "✅ Group mode enabled." => Some("✅ Skupinový režim zapnut."),
        "🛑 Group mode disabled." => Some("🛑 Skupinový režim vypnut."),
        "🔎 Searching public playlists…" => Some("🔎 Hledám veřejné playlisty…"),
        "Playlists" => Some("Playlisty"),
        "😕 No public playlists found." => Some("😕 Žádné veřejné playlisty nenalezeny."),
        "Usage: <code>/playlist &lt;name&gt;</code> — search public playlists (Deezer). Apple Music links already work as album URLs." => {
            Some(
                "Použití: <code>/playlist &lt;název&gt;</code> — hledat veřejné playlisty (Deezer). Odkazy Apple Music už fungují jako adresy alb.",
            )
        }
        "Usage: <code>/broadcast &lt;text&gt;</code>" => {
            Some("Použití: <code>/broadcast &lt;text&gt;</code>")
        }
        "Broadcast complete" => Some("Rozesílání dokončeno"),
        "Usage: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>" => {
            Some("Použití: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>")
        }
        "<b>Settings</b>" => Some("<b>Nastavení</b>"),
        "Quality" => Some("Kvalita"),
        "Language" => Some("Jazyk"),
        "Search source (inline)" => Some("Zdroj vyhledávání (inline)"),
        "Tags (album + genres)" => Some("Tagy (album + žánry)"),
        "Last.fm" => Some("Last.fm"),
        "not connected (setup: /lastfm)" => Some("nepřipojeno (nastavení: /lastfm)"),
        "HQ cover (1000px)" => Some("Obal HQ (1000px)"),
        "Last.fm stats" => Some("Statistiky Last.fm"),
        "Genre analyzer" => Some("Analyzátor žánrů"),
        "🏷 Tags: album + genre" => Some("🏷 Tagy: album + žánr"),
        "🎛 Genre analyzer" => Some("🎛 Analyzátor žánrů"),
        "📊 Genre chart" => Some("📊 Graf žánrů"),
        "🎛 Enable the genre analyzer in /settings first." => {
            Some("🎛 Nejdřív zapněte analyzátor žánrů v /settings.")
        }
        "No genre data yet — download a few tracks with the analyzer enabled." => Some(
            "Zatím nejsou žádná data o žánrech — stáhněte několik skladeb se zapnutým analyzátorem.",
        ),
        "Could not build the genre chart." => Some("Graf žánrů se nepodařilo vytvořit."),
        "Your genres" => Some("Vaše žánry"),
        "{tracks} tracks · {genres} genres" => Some("{tracks} skladeb · {genres} žánrů"),
        "Usage: /genres [5..20] — number of bars." => {
            Some("Použití: /genres [5..20] — počet sloupců.")
        }
        "📊 Last.fm stats + collages" => Some("📊 Statistiky Last.fm + koláže"),
        "🎵 Last.fm is not configured on this bot yet." => {
            Some("🎵 Last.fm zatím není na tomto botovi nastaven.")
        }
        "🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off" => {
            Some(
                "🎵 Last.fm připojen jako <b>{nick}</b>. V inline režimu bude bot při prázdném dotazu navrhovat skladbu, která právě hraje.\nOdpojit: /lastfm off",
            )
        }
        "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now." => {
            Some(
                "🎵 Last.fm: nepřipojeno.\nPřipojení: /lastfm &lt;vaše přezdívka&gt; — prázdný inline dotaz bude navrhovat skladbu, která právě hraje.",
            )
        }
        "🔌 Last.fm disconnected." => Some("🔌 Last.fm odpojen."),
        "⚠️ Last.fm says there is no such user. Check the nickname spelling at last.fm." => {
            Some("⚠️ Last.fm nezná takového uživatele. Zkontrolujte pravopis přezdívky na last.fm.")
        }
        "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public." => {
            Some(
                "✅ Připojeno jako <b>{nick}</b>, ale zatím žádné nedávné skladby. Zkontrolujte, že váš přehrávač scrobbluje na Last.fm a že je váš profil veřejný.",
            )
        }
        "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it." => {
            Some(
                "✅ Připojeno jako <b>{nick}</b>. Právě hraje: <b>{track}</b>. V inline režimu jej prázdný dotaz navrhne.",
            )
        }
        "⚠️ Invalid Last.fm username (2–15 latin letters, digits, _ or -)." => Some(
            "⚠️ Neplatné uživatelské jméno Last.fm (2–15 latinských písmen, číslic, _ nebo -).",
        ),
        "▶ Now playing: {track}" => Some("▶ Právě hraje: {track}"),
        "FLAC (lossless)" => Some("FLAC (bezztrátový)"),
        "🖼 HQ cover (1000px)" => Some("🖼 Obal HQ (1000px)"),
        "⚠️ Your chosen search source is blocked by the server config — the bot falls back to YouTube. Ask the operator to enable it (SEARCH_PROVIDERS)." => {
            Some(
                "⚠️ Zvolený zdroj vyhledávání je blokován konfigurací serveru — bot přejde na YouTube. Požádejte provozovatele o jeho povolení (SEARCH_PROVIDERS).",
            )
        }
        "✅ Settings saved." => Some("✅ Nastavení uloženo."),
        "🇬🇧 English" => Some("🇬🇧 Angličtina"),
        "🌐 Language" => Some("🌐 Jazyk"),
        "<b>Language</b>\nPick the interface language:" => {
            Some("<b>Jazyk</b>\nVyberte jazyk rozhraní:")
        }
        "« Back" => Some("« Zpět"),
        "⬇️ Downloading… this usually takes a few seconds." => {
            Some("⬇️ Stahuji… obvykle to trvá několik sekund.")
        }
        "🔎 Searching…" => Some("🔎 Hledám…"),
        "Nothing found. Try a different spelling or paste a direct link." => {
            Some("Nic nenalezeno. Zkuste jiný pravopis nebo vložte přímý odkaz.")
        }
        "🚫 You are not allowed to use this bot." => {
            Some("🚫 Nemáte povolení tento bot používat.")
        }
        "⛔ Admins only." => Some("⛔ Pouze pro administrátory."),
        "⏳ Too many requests. Limit: {per_minute} per minute." => {
            Some("⏳ Příliš mnoho požadavků. Limit: {per_minute} za minutu.")
        }
        "ℹ️ Send links and search queries in a private chat with me. Commands work here too." => {
            Some(
                "ℹ️ Odkazy a vyhledávací dotazy posílejte v soukromém chatu se mnou. Příkazy fungují i zde.",
            )
        }
        "🔒 This track is private or requires signing in, so I cannot download it." => {
            Some("🔒 Tato skladba je soukromá nebo vyžaduje přihlášení, proto ji nemohu stáhnout.")
        }
        "🌍 This track is region locked and is not available from the bot's location." => {
            Some("🌍 Tato skladba je regionově omezená a z umístění botu není dostupná.")
        }
        "⏱ The track is too long. Maximum duration is {} minutes." => {
            Some("⏱ Skladba je příliš dlouhá. Maximální délka je {} minut.")
        }
        "📦 The file is too large. Maximum size is {} MB." => {
            Some("📦 Soubor je příliš velký. Maximální velikost je {} MB.")
        }
        "🔍 I could not find that track on the available sources." => {
            Some("🔍 Tuto skladbu jsem v dostupných zdrojích nenašel.")
        }
        "🤷 This link is not supported, or it needs a paid/DRM-protected source. Try a direct track link or search by name." => {
            Some(
                "🤷 Tento odkaz není podporován nebo vyžaduje placený/DRM chráněný zdroj. Zkuste přímý odkaz na skladbu nebo hledání podle názvu.",
            )
        }
        "🚫 {} is disabled on this bot." => Some("🚫 {} je na tomto botovi vypnuto."),
        "📡 The source is temporarily unavailable. Please try again in a moment." => {
            Some("📡 Zdroj je dočasně nedostupný. Zkuste to prosím za chvíli znovu.")
        }
        "🎧 I downloaded the track but could not prepare the audio file." => {
            Some("🎧 Skladbu jsem stáhl, ale nepodařilo se mi připravit audio soubor.")
        }
        "⚠️ Something went wrong while preparing the track. Please try again." => {
            Some("⚠️ Při přípravě skladby došlo k chybě. Zkuste to prosím znovu.")
        }
        "💿 <b>{title}</b>\n{count} track(s). Download the whole collection?" => {
            Some("💿 <b>{title}</b>\n{count} skladeb. Stáhnout celou kolekci?")
        }
        "⬇ Download all" => Some("⬇ Stáhnout vše"),
        "Cancel" => Some("Zrušit"),
        "Cancelled." => Some("Zrušeno."),
        "💿 Loading the album…" => Some("💿 Načítám album…"),
        "💿 <b>{title}</b>\n{count} track(s) queued." => {
            Some("💿 <b>{title}</b>\nVe frontě je {count} skladeb.")
        }
        "💿 {current}/{total} — {title}" => Some("💿 {current}/{total} — {title}"),
        "✅ Album finished: {ok} sent, {failed} failed." => {
            Some("✅ Album dokončeno: odesláno {ok}, selhalo {failed}.")
        }
        "This album or playlist has no downloadable tracks." => {
            Some("Toto album nebo playlista neobsahují žádné stahovatelné skladby.")
        }
        " and {extra} more" => Some(" a dalších {extra}"),
        "⚠ Failed: {list}{more}.\nSend /alb again — the finished tracks will come from cache instantly." => {
            Some(
                "⚠ Selhalo: {list}{more}.\nZnovu pošlete /alb — úspěšné skladby se okamžitě vrátí z mezipaměti.",
            )
        }
        "Usage: /alb &lt;album name&gt;\nExample: /alb Master of Puppets" => {
            Some("Použití: /alb &lt;název alba&gt;\nPříklad: /alb Master of Puppets")
        }
        "🔎 Searching albums…" => Some("🔎 Hledám alba…"),
        "<b>Albums for</b> «{query}»\nPick one:" => {
            Some("<b>Alba pro</b> «{query}»\nVyberte jedno:")
        }
        "No albums found. Try a different artist or album title." => {
            Some("Žádná alba nenalezena. Zkuste jiného interpreta nebo název alba.")
        }
        " · {count} tracks" => Some(" · {count} skladeb"),
        "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>" => {
            Some(
                "<b>Statistiky</b>\nSkladeb v mezipaměti: <b>{}</b>\nZnámých uživatelů: <b>{}</b>\nÚspěšných odeslání: <b>{}</b>\nDoba provozu: <b>{uptime}</b>",
            )
        }
        "🆕 <b>Bot update</b>\n\n{notes}\n\n💬 Developer channel: @maslougazIA" => {
            Some("🆕 <b>Aktualizace bota</b>\n\n{notes}\n\n💬 Kanál vývojáře: @maslougazIA")
        }
        "🗑 Purged <code>{key}</code> from cache." => {
            Some("🗑 Z mezipaměti odstraněno <code>{key}</code>.")
        }
        "That key is not in the cache." => Some("Tento klíč v mezipaměti není."),
        "🚫 User {user_id} banned." => Some("🚫 Uživatel {user_id} zabanován."),
        "✅ User {user_id} unbanned." => Some("✅ Uživatel {user_id} odbanován."),
        "Usage: /purge &lt;cache key or track URL&gt;" => {
            Some("Použití: /purge &lt;klíč mezipaměti nebo adresa skladby&gt;")
        }
        "⌛ This result has expired. Please search again." => {
            Some("⌛ Tento výsledek vypršel. Hledejte znovu.")
        }
        "✅ Sent: {title}" => Some("✅ Odesláno: {title}"),
        "🔎 Results for “{query}” ({count}):" => Some("🔎 Výsledky pro „{query}“ ({count}):"),
        "Usage: /ban &lt;numeric user id&gt;" => {
            Some("Použití: /ban &lt;numerické id uživatele&gt;")
        }
        "Open bot" => Some("Otevřít bota"),
        "📩 Sent to your private messages with the bot." => {
            Some("📩 Odesláno do vašich soukromých zpráv s botem.")
        }
        "⏳ Preparing the track…" => Some("⏳ Připravuji skladbu…"),
        "Send /import followed by a playlist (Artist – Title lines), or just /import and then send the file or the list in the next message." => {
            Some(
                "Pošlete /import následovaný playlistou (řádky Interpret – Název), nebo jen /import a v další zprávě pošlete soubor či seznam.",
            )
        }
        "📥 Waiting for the playlist. Send a CSV/TXT/JSON file (Spotify, TuneMyMusic, Soundiiz export) or paste lines like <code>Artist – Title</code>. Send <code>cancel</code> to abort." => {
            Some(
                "📥 Čekám na playlistu. Pošlete soubor CSV/TXT/JSON (export ze Spotify, TuneMyMusic, Soundiiz) nebo vložte řádky ve tvaru <code>Interpret – Název</code>. Pro zrušení pošlete <code>cancel</code>.",
            )
        }
        "🛑 Playlist import cancelled." => Some("🛑 Import playlisty zrušen."),
        "🔎 Matching {count} imported tracks…" => {
            Some("🔎 Přiřazuji {count} importovaných skladeb…")
        }
        "No tracks found in that playlist." => {
            Some("V této playlistě nebyly nalezeny žádné skladby.")
        }
        "Too many tracks; import at most {max} at a time." => {
            Some("Příliš mnoho skladeb; najednou lze importovat nejvýše {max}.")
        }
        "Could not read that playlist file." => Some("Soubor playlisty se nepodařilo přečíst."),
        "📊 Enable Last.fm stats in /settings first." => {
            Some("📊 Nejdřív zapněte statistiky Last.fm v /settings.")
        }
        "Connect your public Last.fm nickname with /lastfm <nick> first." => {
            Some("Nejdřív připojte svou veřejnou přezdívku Last.fm pomocí /lastfm <nick>.")
        }
        "Usage: /profile [7d|1m|3m|6m|1y|all]" => Some("Použití: /profile [7d|1m|3m|6m|1y|all]"),
        "Usage: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]" => {
            Some("Použití: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]")
        }
        "📊 <b>Last.fm: {name}</b>\nScrobbles (all time): <b>{playcount}</b>\nLibrary (all time): {tracks} tracks · {albums} albums · {artists} artists" => {
            Some(
                "📊 <b>Last.fm: {name}</b>\nScrobblů (celkem): <b>{playcount}</b>\nKnihovna (celkem): {tracks} skladeb · {albums} alb · {artists} interpretů",
            )
        }
        "\nCountry: {country}" => Some("\nZemě: {country}"),
        "\n\n<b>Top tracks ({period}):</b>" => Some("\n\n<b>Top skladby ({period}):</b>"),
        "🎨 Generating collage…" => Some("🎨 Vytvářím koláž…"),
        "Could not build the collage (Last.fm returned no data)." => {
            Some("Koláž se nepodařilo vytvořit (Last.fm nevrátil žádná data).")
        }
        "⏳ Downloading: {track}" => Some("⏳ Stahuji: {track}"),
        "<b>Listening history</b> · {period}\n\n" => {
            Some("<b>Historie poslechu</b> · {period}\n\n")
        }
        "Nothing in this period yet — send a track first." => {
            Some("Za toto období zatím nic není — nejdřív pošlete skladbu.")
        }
        "\n… and {hidden} earlier\n" => Some("\n… a {hidden} dříve\n"),
        "Usage: <code>/history [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Použití: <code>/history [7d|1m|3m|6m|1y|all]</code>")
        }
        "Genre dynamics" => Some("Vývoj žánrů"),
        "{cells} genre-points · {buckets} buckets · {period}" => {
            Some("{cells} žánrových bodů · {buckets} intervalů · {period}")
        }
        "No timed genre data yet. Enable the analyzer in /settings and download a few tracks." => {
            Some(
                "Zatím nejsou žádná časová data o žánrech. Zapněte analyzátor v /settings a stáhněte několik skladeb.",
            )
        }
        "Could not build the dynamics chart." => Some("Graf vývoje se nepodařilo vytvořit."),
        "Usage: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Použití: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>")
        }
        "Usage:\n<code>/digest daily|weekly</code> — enable (default 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — quiet hours in UTC\n<code>/digest status</code> — current schedule\n<code>/digest off</code> — disable" => {
            Some(
                "Použití:\n<code>/digest daily|weekly</code> — zapnout (výchozí 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — tiché hodiny v UTC\n<code>/digest status</code> — aktuální rozvrh\n<code>/digest off</code> — vypnout",
            )
        }
        "✅ {period} digest enabled. Next: <b>{next}</b> UTC." => {
            Some("✅ {period} přehled zapnut. Další: <b>{next}</b> UTC.")
        }
        "🛑 Digest disabled." => Some("🛑 Přehled vypnut."),
        "🌙 Quiet hours: <b>{window}</b> UTC." => Some("🌙 Tiché hodiny: <b>{window}</b> UTC."),
        "📅 Digest: <b>{state}</b>\nPeriod: {period}\nNext: {next} UTC\nQuiet hours: {quiet} UTC" => {
            Some(
                "📅 Přehled: <b>{state}</b>\nObdobí: {period}\nDalší: {next} UTC\nTiché hodiny: {quiet} UTC",
            )
        }
        "No digest is enabled. Start with <code>/digest daily</code>." => {
            Some("Žádný přehled není zapnutý. Začněte pomocí <code>/digest daily</code>.")
        }
        "none" => Some("žádné"),
        "daily" => Some("denní"),
        "weekly" => Some("týdenní"),
        "📊 Your digest" => Some("📊 Váš přehled"),
        "Tracks" => Some("Skladby"),
        "Top tracks" => Some("Top skladby"),
        _ => None,
    }
}
