//! Dutch translations. Keys are the English source strings from `i18n.rs`.
pub fn lookup(english: &str) -> Option<&'static str> {
    match english {
        "English" => Some("Engels"),
        "⚖️ <b>Use responsibly.</b> Download only music you own, that is licensed under a free licence, or that you are otherwise allowed to copy. You are responsible for how you use this bot. The bot does not bypass DRM or paid access, and it stores only metadata plus Telegram file identifiers." => {
            Some(
                "⚖️ <b>Gebruik verantwoord.</b> Download alleen muziek die je bezit, die onder een vrije licentie valt of die je anderszins mag kopiëren. Je bent zelf verantwoordelijk voor hoe je deze bot gebruikt. De bot omzeilt geen DRM of betaaltoegang en slaat alleen metadata plus Telegram-bestandsidentifiers op.",
            )
        }
        "🎵 <b>Music Bot</b>\n\nSend a track link, an album link, or just <code>Artist — Title</code>. I will find the track and send it as audio with tags and cover.\n\nYou can also use me inline: type <code>@this_bot song</code> in any chat." => {
            Some(
                "🎵 <b>Music Bot</b>\n\nStuur een link naar een nummer, een albumlink of gewoon <code>Artiest — Titel</code>. Ik zoek het nummer en stuur het als audio met tags en hoesafbeelding.\n\nJe kunt me ook inline gebruiken: typ <code>@this_bot liedje</code> in elke chat.",
            )
        }
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality (kbps or FLAC lossless), source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)\n/import [list] — import a playlist; with no argument the next message (file or Artist – Title lines) is used\n/profile or /lastfmstats [period] — scrobble statistics (enable Last.fm stats in /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — PNG chart of your most played items\n/genres [5..20] — genre chart from your downloads (enable the analyzer in /settings)\n/lyrics [Artist — Title] — song lyrics from public sources (no argument: last track; or reply to an audio)\n/changelog — show the release notes\n/me — your own counters and favourite genres\n/queue — recent download jobs\n/cancel &lt;id&gt; — cancel an active download\n/stop — cancel all your active actions at once\n/convert auto|m4a|mp3|flac|opus — output format; reply to a track to convert that one\n/normalize on|off — ffmpeg loudnorm pass\n/playlist &lt;name&gt; — search public playlists\n/fav — your saved ⭐ tracks (tap ☆ under a track to save it)\n/similar — reply to one of my tracks: up to 10 similar tracks\n/artist &lt;name&gt; — top tracks of an artist\n/cover — reply to one of my tracks: full-size album cover\n/albums — your saved albums\n/recognize — reply to an audio file: identify it by sound\n/groupmode on|off — admin: allow the bot in this group\n/history [period] — your recent listening history\n/dynamics [period] — genre dynamics over time chart\n/digest daily|weekly|off — personal digest to your private chat\n/digest quiet HH-HH|off — quiet hours in UTC\n/digest status — current digest schedule" => {
            Some(
                "<b>Hoe te gebruiken</b>\n\n• Privéchat: stuur een link of <code>Artiest — Titel</code>.\n• Inline: <code>@this_bot zoekterm</code> in elke chat. Een lege zoekterm met een verbonden Last.fm stelt voor wat je nu speelt.\n• Albums en afspeellijsten worden één nummer tegelijk gedownload met een voortgangsbericht. Gebruik <code>/alb &lt;naam&gt;</code> om een album op titel te vinden.\n• Herhaalde verzoeken komen uit de cache.\n\n<b>Commando's</b>\n/start — over de bot\n/help — deze help\n/settings — taal, kwaliteit (kbps of FLAC lossless), bron en tags\n/alb — zoek een album op naam en download het\n/lastfm — verbind Last.fm (lege inline-zoekterm stelt het nummer voor dat je speelt)\n/import [lijst] — importeer een afspeellijst; zonder argument wordt het volgende bericht (bestand of regels Artiest – Titel) gebruikt\n/profile of /lastfmstats [periode] — scrobble-statistieken (schakel Last.fm-stats in via /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [periode] — PNG-grafiek van je meest gespeelde items\n/genres [5..20] — genresgrafiek van je downloads (schakel de analyser in via /settings)\n/lyrics [Artiest — Titel] — songteksten uit publieke bronnen (geen argument: laatste nummer; of antwoord op een audio)\n/changelog — toon de releasenotes\n/me — je eigen tellers en favoriete genres\n/queue — recente downloadopdrachten\n/cancel &lt;id&gt; — annuleer een actieve download\n/stop — annuleer al je actieve acties in één keer\n/convert auto|m4a|mp3|flac|opus — uitvoerformaat; antwoord op een nummer om alleen dat te converteren\n/normalize on|off — ffmpeg loudnorm-bewerking\n/playlist &lt;naam&gt; — zoek publieke afspeellijsten\n/fav — je opgeslagen ⭐ nummers (tik op ☆ onder een nummer om het op te slaan)\n/similar — antwoord op een van mijn nummers: maximaal 10 vergelijkbare nummers\n/artist &lt;naam&gt; — topnummers van een artiest\n/cover — antwoord op een van mijn nummers: albumhoes in volledige grootte\n/albums — je opgeslagen albums\n/recognize — antwoord op een audiobestand: herken op geluid\n/groupmode on|off — admin: sta de bot toe in deze groep\n/history [periode] — je recente luistergeschiedenis\n/dynamics [periode] — grafiek van het genreverloop in de tijd\n/digest daily|weekly|off — persoonlijk digest naar je privéchat\n/digest quiet HH-HH|off — stille uren in UTC\n/digest status — huidig schema",
            )
        }
        "{base}\n\n<b>Admin</b>\n/stats — counters\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;key or url&gt;\n/broadcast &lt;text&gt; — one-off notice to all known users" => {
            Some(
                "{base}\n\n<b>Admin</b>\n/stats — tellers\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;sleutel of URL&gt;\n/broadcast &lt;tekst&gt; — eenmalige melding aan alle bekende gebruikers",
            )
        }
        "📝 Usage: <code>/lyrics Artist — Title</code>\nOr send <code>/lyrics</code> after a track, or reply to an audio message with <code>/lyrics</code>." => {
            Some(
                "📝 Gebruik: <code>/lyrics Artiest — Titel</code>\nOf stuur <code>/lyrics</code> na een nummer, of antwoord met <code>/lyrics</code> op een audioboodschap.",
            )
        }
        "🔎 Looking up lyrics…" => Some("🔎 Songtekst zoeken…"),
        "<b>Lyrics</b>" => Some("<b>Songtekst</b>"),
        "timestamped" => Some("met tijdstempels"),
        "plain text" => Some("platte tekst"),
        "😕 Lyrics not found in the public sources (LRCLIB, lyrics.ovh)." => {
            Some("😕 Songtekst niet gevonden in de publieke bronnen (LRCLIB, lyrics.ovh).")
        }
        "Send a track first, reply to an audio message, or pass a query: <code>/lyrics Artist — Title</code>." => {
            Some(
                "Stuur eerst een nummer, antwoord op een audioboodschap of geef een zoekterm: <code>/lyrics Artiest — Titel</code>.",
            )
        }
        "📄 Download .lrc" => Some("📄 Download .lrc"),
        "🔄 Other source" => Some("🔄 Andere bron"),
        "📄 LRC file saved. Drop it next to the audio in your player." => {
            Some("📄 LRC-bestand opgeslagen. Zet het naast het audiobestand in je speler.")
        }
        "😕 The other public source has no lyrics for this track." => {
            Some("😕 De andere publieke bron heeft geen songtekst voor dit nummer.")
        }
        "<b>What's new</b>\n\n" => Some("<b>Wat is er nieuw</b>\n\n"),
        "<b>Your stats</b>" => Some("<b>Jouw statistieken</b>"),
        "Tracks counted" => Some("Nummers geteld"),
        "Listens" => Some("Beluisteringen"),
        "Genres" => Some("Genres"),
        "Audio time" => Some("Audiotijd"),
        "top: /genres" => Some("top: /genres"),
        "⏳ This download is already being processed by another instance." => {
            Some("⏳ Deze download wordt al verwerkt door een ander exemplaar.")
        }
        "🛑 Download cancelled." => Some("🛑 Download geannuleerd."),
        "<b>Recent downloads</b>\n\n" => Some("<b>Recente downloads</b>\n\n"),
        "No downloads yet." => Some("Nog geen downloads."),
        " · 🛑 cancelling" => Some(" · 🛑 annuleren"),
        "Usage: <code>/cancel &lt;job id from /queue&gt;</code>" => {
            Some("Gebruik: <code>/cancel &lt;taak-id uit /queue&gt;</code>")
        }
        "🛑 Cancellation requested." => Some("🛑 Annulering aangevraagd."),
        "No active job with that id." => Some("Geen actieve taak met die id."),
        "Usage: <code>/convert auto|m4a|mp3|flac|opus</code> — applies to your next downloads. Reply to a track with <code>/convert &lt;format&gt;</code> to convert that track." => {
            Some(
                "Gebruik: <code>/convert auto|m4a|mp3|flac|opus</code> — geldt voor je volgende downloads. Antwoord op een nummer met <code>/convert &lt;formaat&gt;</code> om dat nummer te converteren.",
            )
        }
        "This message is not a downloaded track I can convert." => {
            Some("Dit bericht is geen gedownload nummer dat ik kan converteren.")
        }
        "Converted to" => Some("Geconverteerd naar"),
        "Conversion failed" => Some("Conversie mislukt"),
        "Stopped: {jobs} download(s), {prompts} pending action(s)." => {
            Some("Gestopt: {jobs} download(s), {prompts} wachtende actie(s).")
        }
        "Output format" => Some("Uitvoerformaat"),
        "ℹ️ Album & artist info" => Some("ℹ️ Album- en artiestinfo"),
        "⭐ Added to favorites." => Some("⭐ Toegevoegd aan favorieten."),
        "Removed from favorites." => Some("Verwijderd uit favorieten."),
        "⭐ Your favorites" => Some("⭐ Je favorieten"),
        "Nothing saved yet — tap ☆ under any track." => {
            Some("Nog niets — tik op ☆ onder een nummer.")
        }
        "Reply to one of my tracks with /similar." => {
            Some("Beantwoord een van mijn nummers met /similar.")
        }
        "No similar tracks found." => Some("Geen vergelijkbare nummers gevonden."),
        "🎧 Similar to «{track}»" => Some("🎧 Vergelijkbaar met «{track}»"),
        "Usage: /artist &lt;name&gt;" => Some("Gebruik: /artist &lt;naam&gt;"),
        "Artist not found in the catalog." => Some("Artiest niet gevonden in de catalogus."),
        "🎤 Top tracks of {name}" => Some("🎤 Topnummers van {name}"),
        "Reply to one of my tracks with /cover." => {
            Some("Beantwoord een van mijn nummers met /cover.")
        }
        "This track has no cover." => Some("Dit nummer heeft geen hoesje."),
        "⚙ Advanced" => Some("⚙ Extra"),
        "<b>Advanced settings</b>" => Some("<b>Extra instellingen</b>"),
        "on" => Some("aan"),
        "off" => Some("uit"),
        "Usage: <code>/normalize on|off</code> (ffmpeg loudnorm, opt-in)." => {
            Some("Gebruik: <code>/normalize on|off</code> (ffmpeg loudnorm, optioneel).")
        }
        "Admin only. Usage: <code>/groupmode on|off</code>" => {
            Some("Alleen voor admins. Gebruik: <code>/groupmode on|off</code>")
        }
        "✅ Group mode enabled." => Some("✅ Groepsmodus ingeschakeld."),
        "🛑 Group mode disabled." => Some("🛑 Groepsmodus uitgeschakeld."),
        "🔎 Searching public playlists…" => Some("🔎 Publieke afspeellijsten zoeken…"),
        "Playlists" => Some("Afspeellijsten"),
        "😕 No public playlists found." => Some("😕 Geen publieke afspeellijsten gevonden."),
        "Usage: <code>/playlist &lt;name&gt;</code> — search public playlists (Deezer). Apple Music links already work as album URLs." => {
            Some(
                "Gebruik: <code>/playlist &lt;naam&gt;</code> — zoek publieke afspeellijsten (Deezer). Apple Music-links werken al als album-URL's.",
            )
        }
        "Usage: <code>/broadcast &lt;text&gt;</code>" => {
            Some("Gebruik: <code>/broadcast &lt;tekst&gt;</code>")
        }
        "Broadcast complete" => Some("Rondzending voltooid"),
        "Usage: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>" => {
            Some("Gebruik: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>")
        }
        "<b>Settings</b>" => Some("<b>Instellingen</b>"),
        "Quality" => Some("Kwaliteit"),
        "Language" => Some("Taal"),
        "Search source (inline)" => Some("Zoekbron (inline)"),
        "Tags (album + genres)" => Some("Tags (album + genres)"),
        "Last.fm" => Some("Last.fm"),
        "not connected (setup: /lastfm)" => Some("niet verbonden (instellen: /lastfm)"),
        "HQ cover (1000px)" => Some("HQ-hoes (1000px)"),
        "Last.fm stats" => Some("Last.fm-statistieken"),
        "Genre analyzer" => Some("Genre-analyser"),
        "🏷 Tags: album + genre" => Some("🏷 Tags: album + genre"),
        "🎛 Genre analyzer" => Some("🎛 Genre-analyser"),
        "📊 Genre chart" => Some("📊 Genresgrafiek"),
        "🎛 Enable the genre analyzer in /settings first." => {
            Some("🎛 Schakel eerst de genre-analyser in via /settings.")
        }
        "No genre data yet — download a few tracks with the analyzer enabled." => {
            Some("Nog geen genregegevens — download een paar nummers met de analyser ingeschakeld.")
        }
        "Could not build the genre chart." => Some("Kon de genresgrafiek niet maken."),
        "Your genres" => Some("Jouw genres"),
        "{listens} listens · {genres} genres" => {
            Some("{listens} luisterbeurten · {genres} genres")
        }
        "Usage: /genres [5..20] — number of bars." => {
            Some("Gebruik: /genres [5..20] — aantal balken.")
        }
        "📊 Last.fm stats + collages" => Some("📊 Last.fm-statistieken + collages"),
        "🎵 Last.fm is not configured on this bot yet." => {
            Some("🎵 Last.fm is op deze bot nog niet ingesteld.")
        }
        "🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off" => {
            Some(
                "🎵 Last.fm verbonden als <b>{nick}</b>. In de inline-modus stelt de bot bij een lege zoekterm nu voor wat je speelt.\nOntkoppelen: /lastfm off",
            )
        }
        "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now." => {
            Some(
                "🎵 Last.fm: niet verbonden.\nVerbinden: /lastfm &lt;jouw nickname&gt; — een lege inline-zoekterm stelt voor wat er nu speelt.",
            )
        }
        "🔌 Last.fm disconnected." => Some("🔌 Last.fm ontkoppeld."),
        "⚠️ Last.fm says there is no such user. Check the nickname spelling at last.fm." => {
            Some(
                "⚠️ Last.fm kent deze gebruiker niet. Controleer de spelling van je nickname op last.fm.",
            )
        }
        "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public." => {
            Some(
                "✅ Verbonden als <b>{nick}</b>, maar er zijn nog geen recente nummers. Zorg dat je speler naar Last.fm scrobbelt en dat je profiel openbaar is.",
            )
        }
        "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it." => {
            Some(
                "✅ Verbonden als <b>{nick}</b>. Nu speelt: <b>{track}</b>. In de inline-modus wordt het bij een lege zoekterm voorgesteld.",
            )
        }
        "⚠️ Invalid Last.fm username (2–15 latin letters, digits, _ or -)." => {
            Some("⚠️ Ongeldige Last.fm-gebruikersnaam (2–15 Latijnse letters, cijfers, _ of -).")
        }
        "▶ Now playing: {track}" => Some("▶ Nu speelt: {track}"),
        "FLAC (lossless)" => Some("FLAC (lossless)"),
        "🖼 HQ cover (1000px)" => Some("🖼 HQ-hoes (1000px)"),
        "⚠️ Your chosen search source is blocked by the server config — the bot falls back to YouTube. Ask the operator to enable it (SEARCH_PROVIDERS)." => {
            Some(
                "⚠️ De door jou gekozen zoekbron is geblokkeerd door de serverconfiguratie — de bot valt terug op YouTube. Vraag de beheerder om hem in te schakelen (SEARCH_PROVIDERS).",
            )
        }
        "✅ Settings saved." => Some("✅ Instellingen opgeslagen."),
        "🇬🇧 English" => Some("🇬🇧 Engels"),
        "🌐 Language" => Some("🌐 Taal"),
        "<b>Language</b>\nPick the interface language:" => {
            Some("<b>Taal</b>\nKies de taal van de interface:")
        }
        "« Back" => Some("« Terug"),
        "⬇️ Downloading… this usually takes a few seconds." => {
            Some("⬇️ Downloaden… dit duurt meestal een paar seconden.")
        }
        "🔎 Searching…" => Some("🔎 Zoeken…"),
        "Nothing found. Try a different spelling or paste a direct link." => {
            Some("Niets gevonden. Probeer een andere spelling of plak een directe link.")
        }
        "🚫 You are not allowed to use this bot." => Some("🚫 Je mag deze bot niet gebruiken."),
        "⛔ Admins only." => Some("⛔ Alleen admins."),
        "⏳ Too many requests. Limit: {per_minute} per minute." => {
            Some("⏳ Te veel verzoeken. Limiet: {per_minute} per minuut.")
        }
        "ℹ️ Send links and search queries in a private chat with me. Commands work here too." => {
            Some(
                "ℹ️ Stuur links en zoekopdrachten in een privéchat met mij. Commando's werken hier ook.",
            )
        }
        "🔒 This track is private or requires signing in, so I cannot download it." => {
            Some("🔒 Dit nummer is privé of vereist aanmelding, dus ik kan het niet downloaden.")
        }
        "🌍 This track is region locked and is not available from the bot's location." => {
            Some("🌍 Dit nummer is regio-gebonden en niet beschikbaar vanaf de locatie van de bot.")
        }
        "⏱ The track is too long. Maximum duration is {} minutes." => {
            Some("⏱ Het nummer is te lang. De maximale duur is {} minuten.")
        }
        "📦 The file is too large. Maximum size is {} MB." => {
            Some("📦 Het bestand is te groot. De maximale grootte is {} MB.")
        }
        "🔍 I could not find that track on the available sources." => {
            Some("🔍 Ik kon dat nummer niet vinden op de beschikbare bronnen.")
        }
        "🤷 This link is not supported, or it needs a paid/DRM-protected source. Try a direct track link or search by name." => {
            Some(
                "🤷 Deze link wordt niet ondersteund of vereist een betaalde/DRM-beveiligde bron. Probeer een directe link naar het nummer of zoek op naam.",
            )
        }
        "🚫 {} is disabled on this bot." => Some("🚫 {} is uitgeschakeld op deze bot."),
        "📡 The source is temporarily unavailable. Please try again in a moment." => {
            Some("📡 De bron is tijdelijk niet beschikbaar. Probeer het over een moment opnieuw.")
        }
        "🎧 I downloaded the track but could not prepare the audio file." => {
            Some("🎧 Ik heb het nummer gedownload, maar kon het audiobestand niet voorbereiden.")
        }
        "⚠️ Something went wrong while preparing the track. Please try again." => {
            Some("⚠️ Er ging iets mis bij het voorbereiden van het nummer. Probeer het opnieuw.")
        }
        "💿 <b>{title}</b>\n{count} track(s). Download the whole collection?" => {
            Some("💿 <b>{title}</b>\n{count} nummer(s). De hele collectie downloaden?")
        }
        "⬇ Download all" => Some("⬇ Alles downloaden"),
        "Cancel" => Some("Annuleren"),
        "Cancelled." => Some("Geannuleerd."),
        "💿 Loading the album…" => Some("💿 Album laden…"),
        "💿 <b>{title}</b>\n{count} track(s) queued." => {
            Some("💿 <b>{title}</b>\n{count} nummer(s) in de wachtrij.")
        }
        "💿 {current}/{total} — {title}" => Some("💿 {current}/{total} — {title}"),
        "✅ Album finished: {ok} sent, {failed} failed." => {
            Some("✅ Album klaar: {ok} verzonden, {failed} mislukt.")
        }
        "This album or playlist has no downloadable tracks." => {
            Some("Dit album of deze afspeellijst bevat geen downloadbare nummers.")
        }
        " and {extra} more" => Some(" en nog {extra}"),
        "⚠ Failed: {list}{more}.\nSend /alb again — the finished tracks will come from cache instantly." => {
            Some(
                "⚠ Mislukt: {list}{more}.\nStuur /alb opnieuw — de al voltooide nummers komen direct uit de cache.",
            )
        }
        "Usage: /alb &lt;album name&gt;\nExample: /alb Master of Puppets" => {
            Some("Gebruik: /alb &lt;albumnaam&gt;\nVoorbeeld: /alb Master of Puppets")
        }
        "🔎 Searching albums…" => Some("🔎 Albums zoeken…"),
        "<b>Albums for</b> «{query}»\nPick one:" => {
            Some("<b>Albums voor</b> «{query}»\nKies er een:")
        }
        "No albums found. Try a different artist or album title." => {
            Some("Geen albums gevonden. Probeer een andere artiest of albumnaam.")
        }
        " · {count} tracks" => Some(" · {count} nummers"),
        "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>" => {
            Some(
                "<b>Statistieken</b>\nNummers in cache: <b>{}</b>\nBekende gebruikers: <b>{}</b>\nSuccesvolle verzendingen: <b>{}</b>\nUptime: <b>{uptime}</b>",
            )
        }
        "🆕 <b>Bot update</b>\n\n{notes}\n\n💬 Developer channel: @maslougazIA" => {
            Some("🆕 <b>Bot-update</b>\n\n{notes}\n\n💬 Ontwikkelaarskanaal: @maslougazIA")
        }
        "🗑 Purged <code>{key}</code> from cache." => {
            Some("🗑 <code>{key}</code> uit de cache verwijderd.")
        }
        "That key is not in the cache." => Some("Die sleutel zit niet in de cache."),
        "🚫 User {user_id} banned." => Some("🚫 Gebruiker {user_id} geband."),
        "✅ User {user_id} unbanned." => Some("✅ Ban van gebruiker {user_id} opgeheven."),
        "Usage: /purge &lt;cache key or track URL&gt;" => {
            Some("Gebruik: /purge &lt;cachesleutel of nummer-URL&gt;")
        }
        "⌛ This result has expired. Please search again." => {
            Some("⌛ Dit resultaat is verlopen. Zoek opnieuw.")
        }
        "✅ Sent: {title}" => Some("✅ Verzonden: {title}"),
        "🔎 Results for “{query}” ({count}):" => {
            Some("🔎 Resultaten voor “{query}” ({count}):")
        }
        "Usage: /ban &lt;numeric user id&gt;" => {
            Some("Gebruik: /ban &lt;numerieke gebruikers-id&gt;")
        }
        "Open bot" => Some("Bot openen"),
        "📩 Sent to your private messages with the bot." => {
            Some("📩 Verzonden naar je privéberichten met de bot.")
        }
        "⏳ Preparing the track…" => Some("⏳ Nummer voorbereiden…"),
        "Send /import followed by a playlist (Artist – Title lines), or just /import and then send the file or the list in the next message." => {
            Some(
                "Stuur /import gevolgd door een afspeellijst (regels Artiest – Titel), of alleen /import en stuur het bestand of de lijst in het volgende bericht.",
            )
        }
        "📥 Waiting for the playlist. Send a CSV/TXT/JSON file (Spotify, TuneMyMusic, Soundiiz export) or paste lines like <code>Artist – Title</code>. Send <code>cancel</code> to abort." => {
            Some(
                "📥 Wachten op de afspeellijst. Stuur een CSV/TXT/JSON-bestand (export van Spotify, TuneMyMusic of Soundiiz) of plak regels zoals <code>Artiest – Titel</code>. Stuur <code>cancel</code> om af te breken.",
            )
        }
        "🛑 Playlist import cancelled." => Some("🛑 Afspeellijst-import geannuleerd."),
        "🔎 Matching {count} imported tracks…" => {
            Some("🔎 {count} geïmporteerde nummers zoeken…")
        }
        "No tracks found in that playlist." => Some("Geen nummers gevonden in die afspeellijst."),
        "Too many tracks; import at most {max} at a time." => {
            Some("Te veel nummers; importeer er maximaal {max} tegelijk.")
        }
        "Could not read that playlist file." => Some("Kon dat afspeellijstbestand niet lezen."),
        "📊 Enable Last.fm stats in /settings first." => {
            Some("📊 Schakel eerst Last.fm-statistieken in via /settings.")
        }
        "Connect your public Last.fm nickname with /lastfm <nick> first." => {
            Some("Verbind je openbare Last.fm-nickname eerst met /lastfm <nick>.")
        }
        "Usage: /profile [7d|1m|3m|6m|1y|all]" => Some("Gebruik: /profile [7d|1m|3m|6m|1y|all]"),
        "Usage: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]" => {
            Some("Gebruik: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]")
        }
        "📊 <b>Last.fm: {name}</b>\nScrobbles (all time): <b>{playcount}</b>\nLibrary (all time): {tracks} tracks · {albums} albums · {artists} artists" => {
            Some(
                "📊 <b>Last.fm: {name}</b>\nScrobbles (aller tijden): <b>{playcount}</b>\nBibliotheek (aller tijden): {tracks} nummers · {albums} albums · {artists} artiesten",
            )
        }
        "\nCountry: {country}" => Some("\nLand: {country}"),
        "\n\n<b>Top tracks ({period}):</b>" => Some("\n\n<b>Topnummers ({period}):</b>"),
        "🎨 Generating collage…" => Some("🎨 Collage maken…"),
        "Could not build the collage (Last.fm returned no data)." => {
            Some("Kon de collage niet maken (Last.fm gaf geen gegevens terug).")
        }
        "⏳ Downloading: {track}" => Some("⏳ Downloaden: {track}"),
        "<b>Listening history</b> · {period}\n\n" => {
            Some("<b>Luistergeschiedenis</b> · {period}\n\n")
        }
        "Nothing in this period yet — send a track first." => {
            Some("Nog niets in deze periode — stuur eerst een nummer.")
        }
        "\n… and {hidden} earlier\n" => Some("\n… en {hidden} eerder\n"),
        "Usage: <code>/history [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Gebruik: <code>/history [7d|1m|3m|6m|1y|all]</code>")
        }
        "Genre dynamics" => Some("Genreverloop"),
        "{cells} genre-points · {buckets} buckets · {period}" => {
            Some("{cells} genrepunten · {buckets} intervallen · {period}")
        }
        "No timed genre data yet. Enable the analyzer in /settings and download a few tracks." => {
            Some(
                "Nog geen genregegevens met tijdsaanduiding. Schakel de analyser in via /settings en download een paar nummers.",
            )
        }
        "Could not build the dynamics chart." => Some("Kon de grafiek van het verloop niet maken."),
        "Usage: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Gebruik: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>")
        }
        "Usage:\n<code>/digest daily|weekly</code> — enable (default 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — quiet hours in UTC\n<code>/digest status</code> — current schedule\n<code>/digest off</code> — disable" => {
            Some(
                "Gebruik:\n<code>/digest daily|weekly</code> — inschakelen (standaard 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — stille uren in UTC\n<code>/digest status</code> — huidig schema\n<code>/digest off</code> — uitschakelen",
            )
        }
        "✅ {period} digest enabled. Next: <b>{next}</b> UTC." => {
            Some("✅ {period}-digest ingeschakeld. Volgende: <b>{next}</b> UTC.")
        }
        "🛑 Digest disabled." => Some("🛑 Digest uitgeschakeld."),
        "🌙 Quiet hours: <b>{window}</b> UTC." => Some("🌙 Stille uren: <b>{window}</b> UTC."),
        "📅 Digest: <b>{state}</b>\nPeriod: {period}\nNext: {next} UTC\nQuiet hours: {quiet} UTC" => {
            Some(
                "📅 Digest: <b>{state}</b>\nPeriode: {period}\nVolgende: {next} UTC\nStille uren: {quiet} UTC",
            )
        }
        "No digest is enabled. Start with <code>/digest daily</code>." => {
            Some("Er is geen digest ingeschakeld. Begin met <code>/digest daily</code>.")
        }
        "none" => Some("geen"),
        "daily" => Some("dagelijks"),
        "weekly" => Some("wekelijks"),
        "📊 Your digest" => Some("📊 Jouw digest"),
        "Tracks" => Some("Nummers"),
        "Top tracks" => Some("Topnummers"),
        "🎵 Your albums" => Some("🎵 Jouw albums"),
        "Library is empty — save an album with the 💾 button under a delivered album" => {
            Some("Bibliotheek is leeg — bewaar een album met de 💾-knop onder een bezorgd album")
        }
        "💾 Save album" => Some("💾 Album bewaren"),
        "🗑 Remove" => Some("🗑 Verwijderen"),
        "Saved" => Some("Bewaard"),
        "Already saved" => Some("Al bewaard"),
        "Removed from library" => Some("Uit de bibliotheek verwijderd"),
        "🗜 Packing album…" => Some("🗜 Album wordt verpakt…"),
        "Packing failed: no track could be downloaded" => {
            Some("Verpakken mislukt: er kon geen nummer worden gedownload")
        }
        "🧹 Auto-delete" => Some("🧹 Automatisch verwijderen"),
        "Auto-delete off" => Some("Automatisch verwijderen uit"),
        "1 hour" => Some("1 uur"),
        "6 hours" => Some("6 uur"),
        "12 hours" => Some("12 uur"),
        "24 hours" => Some("24 uur"),
        "The bot deletes its own audio messages after this time (max 48 h). Applies only to messages sent after enabling." => {
            Some(
                "De bot verwijdert zijn eigen audioberichten na deze tijd (max. 48 uur). Geldt alleen voor berichten die na het inschakelen zijn verzonden.",
            )
        }
        "Reply to an audio file with /recognize" => {
            Some("Antwoord op een audiobestand met /recognize")
        }
        "Recognition is not configured on this server" => {
            Some("Herkenning is niet geconfigureerd op deze server")
        }
        "🔍 Listening…" => Some("🔍 Luisteren…"),
        "Could not identify this track" => Some("Dit nummer kon niet worden herkend"),
        "Download album" => Some("Album downloaden"),
        "Auto-delete: {n} hours" => Some("Automatisch verwijderen: {n} uur"),
        "Set: {n} hours" => Some("Ingesteld: {n} uur"),
        "≈ {n} min" => Some("≈ {n} min"),
        "Album is too large to pack (limit {n} MB)" => {
            Some("Album is te groot om te verpakken (limiet {n} MB)")
        }
        "📦 {album} — {n} tracks" => Some("📦 {album} — {n} nummers"),
        "Looks like: {artist} — {title}" => Some("Lijkt op: {artist} — {title}"),
        "💿 {album} · {year}" => Some("💿 {album} · {year}"),
        _ => None,
    }
}
