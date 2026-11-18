//! German translations. Keys are the English source strings from `i18n.rs`.
pub fn lookup(english: &str) -> Option<&'static str> {
    match english {
        "English" => Some("Englisch"),
        "⚖️ <b>Use responsibly.</b> Download only music you own, that is licensed under a free licence, or that you are otherwise allowed to copy. You are responsible for how you use this bot. The bot does not bypass DRM or paid access, and it stores only metadata plus Telegram file identifiers." => {
            Some(
                "⚖️ <b>Verwende es verantwortungsvoll.</b> Lade nur Musik herunter, die du besitzt, die unter einer freien Lizenz steht oder die du anderweitig kopieren darfst. Du bist für die Nutzung dieses Bots verantwortlich. Der Bot umgeht kein DRM und keine bezahlten Zugänge, und er speichert nur Metadaten sowie Telegram-Datei-IDs.",
            )
        }
        "🎵 <b>Music Bot</b>\n\nSend a track link, an album link, or just <code>Artist — Title</code>. I will find the track and send it as audio with tags and cover.\n\nYou can also use me inline: type <code>@this_bot song</code> in any chat." => {
            Some(
                "🎵 <b>Music Bot</b>\n\nSende einen Track-Link, einen Album-Link oder einfach <code>Interpret — Titel</code>. Ich finde den Track und sende ihn als Audio mit Tags und Cover.\n\nDu kannst mich auch inline nutzen: schreibe <code>@this_bot Lied</code> in einem beliebigen Chat.",
            )
        }
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality (kbps or FLAC lossless), source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)\n/import [list] — import a playlist; with no argument the next message (file or Artist – Title lines) is used\n/profile or /lastfmstats [period] — scrobble statistics (enable Last.fm stats in /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — PNG chart of your most played items\n/genres [5..20] — genre chart from your downloads (enable the analyzer in /settings)\n/lyrics [Artist — Title] — song lyrics from public sources (no argument: last track; or reply to an audio)\n/changelog — show the release notes\n/me — your own counters and favourite genres\n/queue — recent download jobs\n/cancel &lt;id&gt; — cancel an active download\n/stop — cancel all your active actions at once\n/convert auto|m4a|mp3|flac|opus — output format; reply to a track to convert that one\n/normalize on|off — ffmpeg loudnorm pass\n/playlist &lt;name&gt; — search public playlists\n/groupmode on|off — admin: allow the bot in this group\n/history [period] — your recent listening history\n/dynamics [period] — genre dynamics over time chart\n/digest daily|weekly|off — personal digest to your private chat\n/digest quiet HH-HH|off — quiet hours in UTC" => {
            Some(
                "<b>So benutzt du mich</b>\n\n• Privatchat: Sende einen Link oder <code>Interpret — Titel</code>.\n• Inline: <code>@this_bot Anfrage</code> in einem beliebigen Chat. Eine leere Anfrage schlägt bei verbundenem Last.fm vor, was du gerade hörst.\n• Alben und Playlists werden ein Track nach dem anderen mit einer Fortschrittsmeldung heruntergeladen. Nutze <code>/alb &lt;Name&gt;</code>, um ein Album nach Titel zu finden.\n• Wiederholte Anfragen kommen aus dem Cache.\n\n<b>Befehle</b>\n/start — über den Bot\n/help — diese Hilfe\n/settings — Sprache, Qualität (kbps oder FLAC verlustfrei), Quelle und Tags\n/alb — ein Album nach Name finden und herunterladen\n/lastfm — Last.fm verbinden (eine leere Inline-Anfrage schlägt den gerade laufenden Track vor)\n/import [Liste] — eine Playlist importieren; ohne Argument wird die nächste Nachricht (Datei oder Zeilen der Form Interpret – Titel) als Liste verwendet\n/profile oder /lastfmstats [Zeitraum] — Scrobble-Statistik (Last.fm-Statistik in /settings aktivieren)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [Zeitraum] — PNG-Diagramm deiner meistgehörten Einträge\n/genres [5..20] — Genre-Diagramm aus deinen Downloads (Analysator in /settings aktivieren)\n/lyrics [Interpret — Titel] — Liedtext aus öffentlichen Quellen (ohne Argument: letzter Track; oder antworte auf ein Audio)\n/changelog — die Versionshinweise anzeigen\n/me — deine eigenen Zähler und Lieblingsgenres\n/queue — letzte Download-Aufträge\n/cancel &lt;id&gt; — einen aktiven Download abbrechen\n/stop — alle deine aktiven Aktionen auf einmal abbrechen\n/convert auto|m4a|mp3|flac|opus — Ausgabeformat; antworte auf einen Track, um nur diesen zu konvertieren\n/normalize on|off — ffmpeg-Loudnorm-Durchlauf\n/playlist &lt;name&gt; — öffentliche Playlists durchsuchen\n/groupmode on|off — Admin: den Bot in dieser Gruppe erlauben\n/history [Zeitraum] — deine letzte Hörhistorie\n/dynamics [Zeitraum] — Diagramm der Genre-Entwicklung im Zeitverlauf\n/digest daily|weekly|off — persönlicher Digest in deinen Privatchat\n/digest quiet HH-HH|off — ruhige Stunden in UTC",
            )
        }
        "{base}\n\n<b>Admin</b>\n/stats — counters\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;key or url&gt;\n/broadcast &lt;text&gt; — one-off notice to all known users" => {
            Some(
                "{base}\n\n<b>Admin</b>\n/stats — Zähler\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;key or url&gt;\n/broadcast &lt;text&gt; — einmalige Mitteilung an alle bekannten Nutzer",
            )
        }
        "📝 Usage: <code>/lyrics Artist — Title</code>\nOr send <code>/lyrics</code> after a track, or reply to an audio message with <code>/lyrics</code>." => {
            Some(
                "📝 Verwendung: <code>/lyrics Interpret — Titel</code>\nOder sende <code>/lyrics</code> nach einem Track oder antworte mit <code>/lyrics</code> auf eine Audionachricht.",
            )
        }
        "🔎 Looking up lyrics…" => Some("🔎 Suche nach Liedtext…"),
        "<b>Lyrics</b>" => Some("<b>Liedtext</b>"),
        "timestamped" => Some("mit Zeitstempeln"),
        "plain text" => Some("Klartext"),
        "😕 Lyrics not found in the public sources (LRCLIB, lyrics.ovh)." => {
            Some("😕 Liedtext in den öffentlichen Quellen (LRCLIB, lyrics.ovh) nicht gefunden.")
        }
        "Send a track first, reply to an audio message, or pass a query: <code>/lyrics Artist — Title</code>." => {
            Some(
                "Sende zuerst einen Track, antworte auf eine Audionachricht oder gib eine Anfrage an: <code>/lyrics Interpret — Titel</code>.",
            )
        }
        "📄 Download .lrc" => Some("📄 .lrc herunterladen"),
        "🔄 Other source" => Some("🔄 Andere Quelle"),
        "📄 LRC file saved. Drop it next to the audio in your player." => {
            Some("📄 LRC-Datei gespeichert. Lege sie neben die Audiodatei in deinem Player.")
        }
        "😕 The other public source has no lyrics for this track." => {
            Some("😕 Die andere öffentliche Quelle hat keinen Liedtext für diesen Track.")
        }
        "<b>What's new</b>\n\n" => Some("<b>Neuigkeiten</b>\n\n"),
        "<b>Your stats</b>" => Some("<b>Deine Statistik</b>"),
        "Tracks counted" => Some("Gezählte Tracks"),
        "Listens" => Some("Wiedergaben"),
        "Genres" => Some("Genres"),
        "Audio time" => Some("Audiozeit"),
        "top: /genres" => Some("Top: /genres"),
        "⏳ This download is already being processed by another instance." => {
            Some("⏳ Dieser Download wird bereits von einer anderen Instanz verarbeitet.")
        }
        "🛑 Download cancelled." => Some("🛑 Download abgebrochen."),
        "<b>Recent downloads</b>\n\n" => Some("<b>Letzte Downloads</b>\n\n"),
        "No downloads yet." => Some("Noch keine Downloads."),
        " · 🛑 cancelling" => Some(" · 🛑 wird abgebrochen"),
        "Usage: <code>/cancel &lt;job id from /queue&gt;</code>" => {
            Some("Verwendung: <code>/cancel &lt;Auftrags-ID aus /queue&gt;</code>")
        }
        "🛑 Cancellation requested." => Some("🛑 Abbruch angefordert."),
        "No active job with that id." => Some("Kein aktiver Auftrag mit dieser ID."),
        "Usage: <code>/convert auto|m4a|mp3|flac|opus</code> — applies to your next downloads. Reply to a track with <code>/convert &lt;format&gt;</code> to convert that track." => {
            Some(
                "Verwendung: <code>/convert auto|m4a|mp3|flac|opus</code> — gilt für deine nächsten Downloads. Antworte mit <code>/convert &lt;Format&gt;</code> auf einen Track, um nur diesen zu konvertieren.",
            )
        }
        "This message is not a downloaded track I can convert." => {
            Some("Diese Nachricht ist kein heruntergeladener Track, den ich konvertieren kann.")
        }
        "Converted to" => Some("Konvertiert in"),
        "Conversion failed" => Some("Konvertierung fehlgeschlagen"),
        "Stopped: {jobs} download(s), {prompts} pending action(s)." => {
            Some("Gestoppt: {jobs} Download(s), {prompts} ausstehende Aktion(en).")
        }
        "Output format" => Some("Ausgabeformat"),
        "ℹ️ Album & artist info" => Some("ℹ️ Über Album & Künstler"),
        "⚙ Advanced" => Some("⚙ Erweitert"),
        "<b>Advanced settings</b>" => Some("<b>Erweiterte Einstellungen</b>"),
        "on" => Some("an"),
        "off" => Some("aus"),
        "Usage: <code>/normalize on|off</code> (ffmpeg loudnorm, opt-in)." => {
            Some("Verwendung: <code>/normalize on|off</code> (ffmpeg-Loudnorm, optional).")
        }
        "Admin only. Usage: <code>/groupmode on|off</code>" => {
            Some("Nur für Admins. Verwendung: <code>/groupmode on|off</code>")
        }
        "✅ Group mode enabled." => Some("✅ Gruppenmodus aktiviert."),
        "🛑 Group mode disabled." => Some("🛑 Gruppenmodus deaktiviert."),
        "🔎 Searching public playlists…" => Some("🔎 Suche in öffentlichen Playlists…"),
        "Playlists" => Some("Playlists"),
        "😕 No public playlists found." => Some("😕 Keine öffentlichen Playlists gefunden."),
        "Usage: <code>/playlist &lt;name&gt;</code> — search public playlists (Deezer). Apple Music links already work as album URLs." => {
            Some(
                "Verwendung: <code>/playlist &lt;Name&gt;</code> — öffentliche Playlists durchsuchen (Deezer). Apple-Music-Links funktionieren bereits als Album-URLs.",
            )
        }
        "Usage: <code>/broadcast &lt;text&gt;</code>" => {
            Some("Verwendung: <code>/broadcast &lt;Text&gt;</code>")
        }
        "Broadcast complete" => Some("Rundsendung abgeschlossen"),
        "Usage: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>" => {
            Some("Verwendung: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>")
        }
        "<b>Settings</b>" => Some("<b>Einstellungen</b>"),
        "Quality" => Some("Qualität"),
        "Language" => Some("Sprache"),
        "Search source (inline)" => Some("Suchquelle (Inline)"),
        "Tags (album + genres)" => Some("Tags (Album + Genres)"),
        "Last.fm" => Some("Last.fm"),
        "not connected (setup: /lastfm)" => Some("nicht verbunden (Einrichtung: /lastfm)"),
        "HQ cover (1000px)" => Some("HQ-Cover (1000px)"),
        "Last.fm stats" => Some("Last.fm-Statistik"),
        "Genre analyzer" => Some("Genre-Analysator"),
        "🏷 Tags: album + genre" => Some("🏷 Tags: Album + Genre"),
        "🎛 Genre analyzer" => Some("🎛 Genre-Analysator"),
        "📊 Genre chart" => Some("📊 Genre-Diagramm"),
        "🎛 Enable the genre analyzer in /settings first." => {
            Some("🎛 Aktiviere zuerst den Genre-Analysator in /settings.")
        }
        "No genre data yet — download a few tracks with the analyzer enabled." => Some(
            "Noch keine Genre-Daten — lade ein paar Tracks mit aktiviertem Analysator herunter.",
        ),
        "Could not build the genre chart." => {
            Some("Das Genre-Diagramm konnte nicht erstellt werden.")
        }
        "Your genres" => Some("Deine Genres"),
        "{listens} listens · {genres} genres" => Some("{listens} Wiedergaben · {genres} Genres"),
        "Usage: /genres [5..20] — number of bars." => {
            Some("Verwendung: /genres [5..20] — Anzahl der Balken.")
        }
        "📊 Last.fm stats + collages" => Some("📊 Last.fm-Statistik + Collages"),
        "🎵 Last.fm is not configured on this bot yet." => {
            Some("🎵 Last.fm ist auf diesem Bot noch nicht konfiguriert.")
        }
        "🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off" => {
            Some(
                "🎵 Last.fm verbunden als <b>{nick}</b>. Im Inline-Modus schlägt der Bot bei einer leeren Anfrage jetzt vor, was du gerade hörst.\nTrennen: /lastfm off",
            )
        }
        "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now." => {
            Some(
                "🎵 Last.fm: nicht verbunden.\nVerbinden: /lastfm &lt;dein Nickname&gt; — eine leere Inline-Anfrage schlägt vor, was gerade läuft.",
            )
        }
        "🔌 Last.fm disconnected." => Some("🔌 Last.fm getrennt."),
        "⚠️ Last.fm says there is no such user. Check the nickname spelling at last.fm." => {
            Some(
                "⚠️ Last.fm kennt diesen Nutzer nicht. Prüfe die Schreibweise des Nicknames auf last.fm.",
            )
        }
        "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public." => {
            Some(
                "✅ Verbunden als <b>{nick}</b>, aber noch keine aktuellen Tracks. Stelle sicher, dass dein Player an Last.fm scrobbelt und dein Profil öffentlich ist.",
            )
        }
        "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it." => {
            Some(
                "✅ Verbunden als <b>{nick}</b>. Läuft gerade: <b>{track}</b>. Im Inline-Modus schlägt eine leere Anfrage ihn vor.",
            )
        }
        "⚠️ Invalid Last.fm username (2–15 latin letters, digits, _ or -)." => Some(
            "⚠️ Ungültiger Last.fm-Benutzername (2–15 lateinische Buchstaben, Ziffern, _ oder -).",
        ),
        "▶ Now playing: {track}" => Some("▶ Läuft gerade: {track}"),
        "FLAC (lossless)" => Some("FLAC (verlustfrei)"),
        "🖼 HQ cover (1000px)" => Some("🖼 HQ-Cover (1000px)"),
        "⚠️ Your chosen search source is blocked by the server config — the bot falls back to YouTube. Ask the operator to enable it (SEARCH_PROVIDERS)." => {
            Some(
                "⚠️ Die gewählte Suchquelle ist durch die Serverkonfiguration blockiert — der Bot fällt auf YouTube zurück. Bitte den Betreiber, sie zu aktivieren (SEARCH_PROVIDERS).",
            )
        }
        "✅ Settings saved." => Some("✅ Einstellungen gespeichert."),
        "🇬🇧 English" => Some("🇬🇧 Englisch"),
        "🌐 Language" => Some("🌐 Sprache"),
        "<b>Language</b>\nPick the interface language:" => {
            Some("<b>Sprache</b>\nWähle die Sprache der Oberfläche:")
        }
        "« Back" => Some("« Zurück"),
        "⬇️ Downloading… this usually takes a few seconds." => {
            Some("⬇️ Wird heruntergeladen… dauert normalerweise ein paar Sekunden.")
        }
        "🔎 Searching…" => Some("🔎 Suche…"),
        "Nothing found. Try a different spelling or paste a direct link." => Some(
            "Nichts gefunden. Versuche eine andere Schreibweise oder füge einen direkten Link ein.",
        ),
        "🚫 You are not allowed to use this bot." => {
            Some("🚫 Du darfst diesen Bot nicht verwenden.")
        }
        "⛔ Admins only." => Some("⛔ Nur für Admins."),
        "⏳ Too many requests. Limit: {per_minute} per minute." => {
            Some("⏳ Zu viele Anfragen. Limit: {per_minute} pro Minute.")
        }
        "ℹ️ Send links and search queries in a private chat with me. Commands work here too." => {
            Some(
                "ℹ️ Sende Links und Suchanfragen in einem Privatchat mit mir. Befehle funktionieren auch hier.",
            )
        }
        "🔒 This track is private or requires signing in, so I cannot download it." => Some(
            "🔒 Dieser Track ist privat oder erfordert eine Anmeldung, daher kann ich ihn nicht herunterladen.",
        ),
        "🌍 This track is region locked and is not available from the bot's location." => Some(
            "🌍 Dieser Track ist regionsgesperrt und vom Standort des Bots aus nicht verfügbar.",
        ),
        "⏱ The track is too long. Maximum duration is {} minutes." => {
            Some("⏱ Der Track ist zu lang. Die maximale Dauer beträgt {} Minuten.")
        }
        "📦 The file is too large. Maximum size is {} MB." => {
            Some("📦 Die Datei ist zu groß. Die maximale Größe beträgt {} MB.")
        }
        "🔍 I could not find that track on the available sources." => {
            Some("🔍 Ich konnte diesen Track in den verfügbaren Quellen nicht finden.")
        }
        "🤷 This link is not supported, or it needs a paid/DRM-protected source. Try a direct track link or search by name." => {
            Some(
                "🤷 Dieser Link wird nicht unterstützt oder benötigt eine kostenpflichtige/DRM-geschützte Quelle. Versuche einen direkten Track-Link oder suche nach dem Namen.",
            )
        }
        "🚫 {} is disabled on this bot." => Some("🚫 {} ist auf diesem Bot deaktiviert."),
        "📡 The source is temporarily unavailable. Please try again in a moment." => Some(
            "📡 Die Quelle ist vorübergehend nicht verfügbar. Versuche es in einem Moment erneut.",
        ),
        "🎧 I downloaded the track but could not prepare the audio file." => Some(
            "🎧 Ich habe den Track heruntergeladen, konnte aber die Audiodatei nicht aufbereiten.",
        ),
        "⚠️ Something went wrong while preparing the track. Please try again." => {
            Some("⚠️ Beim Aufbereiten des Tracks ist etwas schiefgelaufen. Versuche es erneut.")
        }
        "💿 <b>{title}</b>\n{count} track(s). Download the whole collection?" => {
            Some("💿 <b>{title}</b>\n{count} Track(s). Die ganze Sammlung herunterladen?")
        }
        "⬇ Download all" => Some("⬇ Alle herunterladen"),
        "Cancel" => Some("Abbrechen"),
        "Cancelled." => Some("Abgebrochen."),
        "💿 Loading the album…" => Some("💿 Album wird geladen…"),
        "💿 <b>{title}</b>\n{count} track(s) queued." => {
            Some("💿 <b>{title}</b>\n{count} Track(s) in der Warteschlange.")
        }
        "💿 {current}/{total} — {title}" => Some("💿 {current}/{total} — {title}"),
        "✅ Album finished: {ok} sent, {failed} failed." => {
            Some("✅ Album fertig: {ok} gesendet, {failed} fehlgeschlagen.")
        }
        "This album or playlist has no downloadable tracks." => {
            Some("Dieses Album oder diese Playlist enthält keine herunterladbaren Tracks.")
        }
        " and {extra} more" => Some(" und {extra} weitere"),
        "⚠ Failed: {list}{more}.\nSend /alb again — the finished tracks will come from cache instantly." => {
            Some(
                "⚠ Fehlgeschlagen: {list}{more}.\nSende /alb erneut — die fertigen Tracks kommen sofort aus dem Cache.",
            )
        }
        "Usage: /alb &lt;album name&gt;\nExample: /alb Master of Puppets" => {
            Some("Verwendung: /alb &lt;Albumname&gt;\nBeispiel: /alb Master of Puppets")
        }
        "🔎 Searching albums…" => Some("🔎 Suche nach Alben…"),
        "<b>Albums for</b> «{query}»\nPick one:" => {
            Some("<b>Alben für</b> «{query}»\nWähle eines:")
        }
        "No albums found. Try a different artist or album title." => {
            Some("Keine Alben gefunden. Versuche einen anderen Interpreten oder Albumtitel.")
        }
        " · {count} tracks" => Some(" · {count} Tracks"),
        "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>" => {
            Some(
                "<b>Statistik</b>\nTracks im Cache: <b>{}</b>\nBekannte Nutzer: <b>{}</b>\nErfolgreiche Sendungen: <b>{}</b>\nLaufzeit: <b>{uptime}</b>",
            )
        }
        "🆕 <b>Bot update</b>\n\n{notes}\n\n💬 Developer channel: @maslougazIA" => {
            Some("🆕 <b>Bot-Update</b>\n\n{notes}\n\n💬 Entwicklerkanal: @maslougazIA")
        }
        "🗑 Purged <code>{key}</code> from cache." => {
            Some("🗑 <code>{key}</code> aus dem Cache entfernt.")
        }
        "That key is not in the cache." => Some("Dieser Schlüssel ist nicht im Cache."),
        "🚫 User {user_id} banned." => Some("🚫 Nutzer {user_id} gesperrt."),
        "✅ User {user_id} unbanned." => Some("✅ Nutzer {user_id} entsperrt."),
        "Usage: /purge &lt;cache key or track URL&gt;" => {
            Some("Verwendung: /purge &lt;Cache-Schlüssel oder Track-URL&gt;")
        }
        "⌛ This result has expired. Please search again." => {
            Some("⌛ Dieses Ergebnis ist abgelaufen. Bitte suche erneut.")
        }
        "✅ Sent: {title}" => Some("✅ Gesendet: {title}"),
        "🔎 Results for “{query}” ({count}):" => {
            Some("🔎 Ergebnisse für „{query}“ ({count}):")
        }
        "Usage: /ban &lt;numeric user id&gt;" => {
            Some("Verwendung: /ban &lt;numerische Nutzer-ID&gt;")
        }
        "Open bot" => Some("Bot öffnen"),
        "📩 Sent to your private messages with the bot." => {
            Some("📩 In deine Privatnachrichten mit dem Bot gesendet.")
        }
        "⏳ Preparing the track…" => Some("⏳ Track wird vorbereitet…"),
        "Send /import followed by a playlist (Artist – Title lines), or just /import and then send the file or the list in the next message." => {
            Some(
                "Sende /import gefolgt von einer Playlist (Zeilen der Form Interpret – Titel), oder einfach /import und dann die Datei bzw. die Liste in der nächsten Nachricht.",
            )
        }
        "📥 Waiting for the playlist. Send a CSV/TXT/JSON file (Spotify, TuneMyMusic, Soundiiz export) or paste lines like <code>Artist – Title</code>. Send <code>cancel</code> to abort." => {
            Some(
                "📥 Warte auf die Playlist. Sende eine CSV-/TXT-/JSON-Datei (Export von Spotify, TuneMyMusic, Soundiiz) oder füge Zeilen wie <code>Interpret – Titel</code> ein. Sende <code>cancel</code> zum Abbrechen.",
            )
        }
        "🛑 Playlist import cancelled." => Some("🛑 Playlist-Import abgebrochen."),
        "🔎 Matching {count} imported tracks…" => {
            Some("🔎 Ordne {count} importierte Tracks zu…")
        }
        "No tracks found in that playlist." => {
            Some("In dieser Playlist wurden keine Tracks gefunden.")
        }
        "Too many tracks; import at most {max} at a time." => {
            Some("Zu viele Tracks; importiere höchstens {max} auf einmal.")
        }
        "Could not read that playlist file." => {
            Some("Die Playlist-Datei konnte nicht gelesen werden.")
        }
        "📊 Enable Last.fm stats in /settings first." => {
            Some("📊 Aktiviere zuerst die Last.fm-Statistik in /settings.")
        }
        "Connect your public Last.fm nickname with /lastfm <nick> first." => {
            Some("Verbinde zuerst deinen öffentlichen Last.fm-Nickname mit /lastfm <nick>.")
        }
        "Usage: /profile [7d|1m|3m|6m|1y|all]" => Some("Verwendung: /profile [7d|1m|3m|6m|1y|all]"),
        "Usage: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]" => {
            Some("Verwendung: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]")
        }
        "📊 <b>Last.fm: {name}</b>\nScrobbles (all time): <b>{playcount}</b>\nLibrary (all time): {tracks} tracks · {albums} albums · {artists} artists" => {
            Some(
                "📊 <b>Last.fm: {name}</b>\nScrobbles (gesamte Zeit): <b>{playcount}</b>\nBibliothek (gesamte Zeit): {tracks} Tracks · {albums} Alben · {artists} Interpreten",
            )
        }
        "\nCountry: {country}" => Some("\nLand: {country}"),
        "\n\n<b>Top tracks ({period}):</b>" => Some("\n\n<b>Top-Tracks ({period}):</b>"),
        "🎨 Generating collage…" => Some("🎨 Collage wird erstellt…"),
        "Could not build the collage (Last.fm returned no data)." => {
            Some("Die Collage konnte nicht erstellt werden (Last.fm hat keine Daten geliefert).")
        }
        "⏳ Downloading: {track}" => Some("⏳ Wird heruntergeladen: {track}"),
        "<b>Listening history</b> · {period}\n\n" => Some("<b>Hörhistorie</b> · {period}\n\n"),
        "Nothing in this period yet — send a track first." => {
            Some("In diesem Zeitraum noch nichts — sende zuerst einen Track.")
        }
        "\n… and {hidden} earlier\n" => Some("\n… und {hidden} frühere\n"),
        "Usage: <code>/history [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Verwendung: <code>/history [7d|1m|3m|6m|1y|all]</code>")
        }
        "Genre dynamics" => Some("Genre-Entwicklung"),
        "{cells} genre-points · {buckets} buckets · {period}" => {
            Some("{cells} Genre-Punkte · {buckets} Intervalle · {period}")
        }
        "No timed genre data yet. Enable the analyzer in /settings and download a few tracks." => {
            Some(
                "Noch keine zeitlich erfassten Genre-Daten. Aktiviere den Analysator in /settings und lade ein paar Tracks herunter.",
            )
        }
        "Could not build the dynamics chart." => {
            Some("Das Entwicklungsdiagramm konnte nicht erstellt werden.")
        }
        "Usage: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Verwendung: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>")
        }
        "Usage:\n<code>/digest daily|weekly</code> — enable (default 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — quiet hours in UTC\n<code>/digest status</code> — current schedule\n<code>/digest off</code> — disable" => {
            Some(
                "Verwendung:\n<code>/digest daily|weekly</code> — aktivieren (Standard 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — ruhige Stunden in UTC\n<code>/digest status</code> — aktueller Zeitplan\n<code>/digest off</code> — deaktivieren",
            )
        }
        "✅ {period} digest enabled. Next: <b>{next}</b> UTC." => {
            Some("✅ {period}-Digest aktiviert. Nächster: <b>{next}</b> UTC.")
        }
        "🛑 Digest disabled." => Some("🛑 Digest deaktiviert."),
        "🌙 Quiet hours: <b>{window}</b> UTC." => Some("🌙 Ruhige Stunden: <b>{window}</b> UTC."),
        "📅 Digest: <b>{state}</b>\nPeriod: {period}\nNext: {next} UTC\nQuiet hours: {quiet} UTC" => {
            Some(
                "📅 Digest: <b>{state}</b>\nZeitraum: {period}\nNächster: {next} UTC\nRuhige Stunden: {quiet} UTC",
            )
        }
        "No digest is enabled. Start with <code>/digest daily</code>." => {
            Some("Kein Digest aktiviert. Starte mit <code>/digest daily</code>.")
        }
        "none" => Some("keine"),
        "daily" => Some("täglich"),
        "weekly" => Some("wöchentlich"),
        "📊 Your digest" => Some("📊 Dein Digest"),
        "Tracks" => Some("Tracks"),
        "Top tracks" => Some("Top-Tracks"),
        _ => None,
    }
}
