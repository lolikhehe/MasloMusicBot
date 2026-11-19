//! Polish translations. Keys are the English source strings from `i18n.rs`.
pub fn lookup(english: &str) -> Option<&'static str> {
    match english {
        "English" => Some("Angielski"),
        "⚖️ <b>Use responsibly.</b> Download only music you own, that is licensed under a free licence, or that you are otherwise allowed to copy. You are responsible for how you use this bot. The bot does not bypass DRM or paid access, and it stores only metadata plus Telegram file identifiers." => {
            Some(
                "⚖️ <b>Korzystaj odpowiedzialnie.</b> Pobieraj tylko muzykę, do której masz prawa, która jest na wolnej licencji lub którą możesz w inny sposób kopiować. Odpowiadasz za to, jak używasz tego bota. Bot nie omija DRM ani płatnego dostępu i przechowuje tylko metadane oraz identyfikatory plików Telegramu.",
            )
        }
        "🎵 <b>Music Bot</b>\n\nSend a track link, an album link, or just <code>Artist — Title</code>. I will find the track and send it as audio with tags and cover.\n\nYou can also use me inline: type <code>@this_bot song</code> in any chat." => {
            Some(
                "🎵 <b>Music Bot</b>\n\nWyślij link do utworu, albumu albo po prostu <code>Wykonawca — Tytuł</code>. Znajdę utwór i wyślę go jako audio z tagami i okładką.\n\nMożesz też używać mnie inline: wpisz <code>@this_bot utwór</code> na dowolnym czacie.",
            )
        }
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality (kbps or FLAC lossless), source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)\n/import [list] — import a playlist; with no argument the next message (file or Artist – Title lines) is used\n/profile or /lastfmstats [period] — scrobble statistics (enable Last.fm stats in /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — PNG chart of your most played items\n/genres [5..20] — genre chart from your downloads (enable the analyzer in /settings)\n/lyrics [Artist — Title] — song lyrics from public sources (no argument: last track; or reply to an audio)\n/changelog — show the release notes\n/me — your own counters and favourite genres\n/queue — recent download jobs\n/cancel &lt;id&gt; — cancel an active download\n/stop — cancel all your active actions at once\n/convert auto|m4a|mp3|flac|opus — output format; reply to a track to convert that one\n/normalize on|off — ffmpeg loudnorm pass\n/playlist &lt;name&gt; — search public playlists\n/fav — your saved ⭐ tracks (tap ☆ under a track to save it)\n/similar — reply to one of my tracks: up to 10 similar tracks\n/artist &lt;name&gt; — top tracks of an artist\n/cover — reply to one of my tracks: full-size album cover\n/groupmode on|off — admin: allow the bot in this group\n/history [period] — your recent listening history\n/dynamics [period] — genre dynamics over time chart\n/digest daily|weekly|off — personal digest to your private chat\n/digest quiet HH-HH|off — quiet hours in UTC\n/digest status — current digest schedule" => {
            Some(
                "<b>Jak używać</b>\n\n• Prywatny czat: wyślij link lub <code>Wykonawca — Tytuł</code>.\n• Inline: <code>@this_bot zapytanie</code> na dowolnym czacie. Puste zapytanie z podłączonym Last.fm zaproponuje to, co właśnie grasz.\n• Albumy i playlisty pobierane są po jednym utworze z komunikatem o postępie. Użyj <code>/alb &lt;nazwa&gt;</code>, aby znaleźć album po tytule.\n• Powtórne zapytania obsługiwane są z pamięci podręcznej.\n\n<b>Polecenia</b>\n/start — o bocie\n/help — ta pomoc\n/settings — język, jakość (kbps lub FLAC lossless), źródło i tagi\n/alb — znajdź album po nazwie i pobierz go\n/lastfm — podłącz Last.fm (puste zapytanie inline sugeruje utwór, który grasz)\n/import [lista] — importuj playlistę; bez argumentu następna wiadomość (plik lub linie „Wykonawca – Tytuł”) zostanie użyta\n/profile lub /lastfmstats [okres] — statystyki scrobblów (włącz statystyki Last.fm w /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [okres] — wykres PNG najczęściej słuchanych pozycji\n/genres [5..20] — wykres gatunków z twoich pobrań (włącz analizator w /settings)\n/lyrics [Wykonawca — Tytuł] — tekst piosenki z publicznych źródeł (bez argumentu: ostatni utwór lub odpowiedz na wiadomość audio)\n/changelog — pokaż listę zmian\n/me — twoje liczniki i ulubione gatunki\n/queue — ostatnie zadania pobierania\n/cancel &lt;id&gt; — anuluj aktywne pobieranie\n/stop — anuluj wszystkie swoje aktywne akcje naraz\n/convert auto|m4a|mp3|flac|opus — format wyjściowy; odpowiedz na utwór, aby przekonwertować właśnie go\n/normalize on|off — korekta głośności ffmpeg\n/playlist &lt;nazwa&gt; — szukaj publicznych playlist\n/fav — twoje zapisane ⭐ utwory (stuknij ☆ pod utworem, aby zapisać)\n/similar — odpowiedz na mój utwór: do 10 podobnych utworów\n/artist &lt;nazwa&gt; — najpopularniejsze utwory wykonawcy\n/cover — odpowiedz na mój utwór: okładka albumu w pełnym rozmiarze\n/groupmode on|off — administrator: zezwól botowi na działanie w tej grupie\n/history [okres] — twoja ostatnia historia odsłuchów\n/dynamics [okres] — wykres dynamiki gatunków w czasie\n/digest daily|weekly|off — osobisty digest wysyłany na prywatny czat\n/digest quiet HH-HH|off — ciche godziny w UTC\n/digest status — aktualny harmonogram",
            )
        }
        "{base}\n\n<b>Admin</b>\n/stats — counters\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;key or url&gt;\n/broadcast &lt;text&gt; — one-off notice to all known users" => {
            Some(
                "{base}\n\n<b>Administrator</b>\n/stats — liczniki\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;klucz lub adres URL&gt;\n/broadcast &lt;tekst&gt; — jednorazowe powiadomienie dla wszystkich znanych użytkowników",
            )
        }
        "📝 Usage: <code>/lyrics Artist — Title</code>\nOr send <code>/lyrics</code> after a track, or reply to an audio message with <code>/lyrics</code>." => {
            Some(
                "📝 Użycie: <code>/lyrics Wykonawca — Tytuł</code>\nAlbo wyślij <code>/lyrics</code> po utworze lub odpowiedz na wiadomość audio komendą <code>/lyrics</code>.",
            )
        }
        "🔎 Looking up lyrics…" => Some("🔎 Szukam tekstu piosenki…"),
        "<b>Lyrics</b>" => Some("<b>Tekst piosenki</b>"),
        "timestamped" => Some("z znacznikami czasu"),
        "plain text" => Some("zwykły tekst"),
        "😕 Lyrics not found in the public sources (LRCLIB, lyrics.ovh)." => {
            Some("😕 Nie znaleziono tekstu w publicznych źródłach (LRCLIB, lyrics.ovh).")
        }
        "Send a track first, reply to an audio message, or pass a query: <code>/lyrics Artist — Title</code>." => {
            Some(
                "Najpierw wyślij utwór, odpowiedz na wiadomość audio albo podaj zapytanie: <code>/lyrics Wykonawca — Tytuł</code>.",
            )
        }
        "📄 Download .lrc" => Some("📄 Pobierz .lrc"),
        "🔄 Other source" => Some("🔄 Inne źródło"),
        "📄 LRC file saved. Drop it next to the audio in your player." => {
            Some("📄 Plik LRC zapisany. Umieść go obok pliku audio w odtwarzaczu.")
        }
        "😕 The other public source has no lyrics for this track." => {
            Some("😕 Drugie publiczne źródło nie ma tekstu tego utworu.")
        }
        "<b>What's new</b>\n\n" => Some("<b>Co nowego</b>\n\n"),
        "<b>Your stats</b>" => Some("<b>Twoje statystyki</b>"),
        "Tracks counted" => Some("Utworów uwzględnionych"),
        "Listens" => Some("Odsłuchania"),
        "Genres" => Some("Gatunki"),
        "Audio time" => Some("Czas audio"),
        "top: /genres" => Some("top: /genres"),
        "⏳ This download is already being processed by another instance." => {
            Some("⏳ To pobieranie jest już obsługiwane przez inną instancję.")
        }
        "🛑 Download cancelled." => Some("🛑 Pobieranie anulowane."),
        "<b>Recent downloads</b>\n\n" => Some("<b>Ostatnie pobierania</b>\n\n"),
        "No downloads yet." => Some("Jeszcze nic nie pobrano."),
        " · 🛑 cancelling" => Some(" · 🛑 anulowanie"),
        "Usage: <code>/cancel &lt;job id from /queue&gt;</code>" => {
            Some("Użycie: <code>/cancel &lt;id zadania z /queue&gt;</code>")
        }
        "🛑 Cancellation requested." => Some("🛑 Anulowanie zgłoszone."),
        "No active job with that id." => Some("Brak aktywnego zadania o takim id."),
        "Usage: <code>/convert auto|m4a|mp3|flac|opus</code> — applies to your next downloads. Reply to a track with <code>/convert &lt;format&gt;</code> to convert that track." => {
            Some(
                "Użycie: <code>/convert auto|m4a|mp3|flac|opus</code> — dotyczy twoich kolejnych pobrań. Odpowiedz na utwór komendą <code>/convert &lt;format&gt;</code>, aby przekonwertować właśnie go.",
            )
        }
        "This message is not a downloaded track I can convert." => {
            Some("Ta wiadomość nie jest pobranym utworem, który mógłbym przekonwertować.")
        }
        "Converted to" => Some("Przekonwertowano na"),
        "Conversion failed" => Some("Konwersja nie powiodła się"),
        "Stopped: {jobs} download(s), {prompts} pending action(s)." => {
            Some("Zatrzymano: pobrań — {jobs}, oczekujących działań — {prompts}.")
        }
        "Output format" => Some("Format wyjściowy"),
        "ℹ️ Album & artist info" => Some("ℹ️ O albumie i wykonawcy"),
        "⭐ Added to favorites." => Some("⭐ Dodano do ulubionych."),
        "Removed from favorites." => Some("Usunięto z ulubionych."),
        "⭐ Your favorites" => Some("⭐ Twoje ulubione"),
        "Nothing saved yet — tap ☆ under any track." => {
            Some("Jeszcze nic — stuknij ☆ pod utworem.")
        }
        "Reply to one of my tracks with /similar." => {
            Some("Odpowiedz na mój utwór komendą /similar.")
        }
        "No similar tracks found." => Some("Nie znalazłem podobnych utworów."),
        "🎧 Similar to «{track}»" => Some("🎧 Podobne do «{track}»"),
        "Usage: /artist &lt;name&gt;" => Some("Użycie: /artist &lt;imię&gt;"),
        "Artist not found in the catalog." => Some("Nie znalazłem wykonawcy w katalogu."),
        "🎤 Top tracks of {name}" => Some("🎤 Najpopularniejsze utwory {name}"),
        "Reply to one of my tracks with /cover." => Some("Odpowiedz na mój utwór komendą /cover."),
        "This track has no cover." => Some("Ten utwór nie ma okładki."),
        "⚙ Advanced" => Some("⚙ Zaawansowane"),
        "<b>Advanced settings</b>" => Some("<b>Ustawienia zaawansowane</b>"),
        "on" => Some("wł."),
        "off" => Some("wył."),
        "Usage: <code>/normalize on|off</code> (ffmpeg loudnorm, opt-in)." => {
            Some("Użycie: <code>/normalize on|off</code> (ffmpeg loudnorm, opcjonalnie).")
        }
        "Admin only. Usage: <code>/groupmode on|off</code>" => {
            Some("Tylko dla administratorów. Użycie: <code>/groupmode on|off</code>")
        }
        "✅ Group mode enabled." => Some("✅ Tryb grupowy włączony."),
        "🛑 Group mode disabled." => Some("🛑 Tryb grupowy wyłączony."),
        "🔎 Searching public playlists…" => Some("🔎 Szukam publicznych playlist…"),
        "Playlists" => Some("Playlisty"),
        "😕 No public playlists found." => Some("😕 Nie znaleziono publicznych playlist."),
        "Usage: <code>/playlist &lt;name&gt;</code> — search public playlists (Deezer). Apple Music links already work as album URLs." => {
            Some(
                "Użycie: <code>/playlist &lt;nazwa&gt;</code> — szukaj publicznych playlist (Deezer). Linki Apple Music działają już jako adresy albumów.",
            )
        }
        "Usage: <code>/broadcast &lt;text&gt;</code>" => {
            Some("Użycie: <code>/broadcast &lt;tekst&gt;</code>")
        }
        "Broadcast complete" => Some("Rozsyłanie zakończone"),
        "Usage: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>" => {
            Some("Użycie: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>")
        }
        "<b>Settings</b>" => Some("<b>Ustawienia</b>"),
        "Quality" => Some("Jakość"),
        "Language" => Some("Język"),
        "Search source (inline)" => Some("Źródło wyszukiwania (inline)"),
        "Tags (album + genres)" => Some("Tagi (album + gatunki)"),
        "Last.fm" => Some("Last.fm"),
        "not connected (setup: /lastfm)" => Some("niepodłączone (konfiguracja: /lastfm)"),
        "HQ cover (1000px)" => Some("Okładka HQ (1000px)"),
        "Last.fm stats" => Some("Statystyki Last.fm"),
        "Genre analyzer" => Some("Analizator gatunków"),
        "🏷 Tags: album + genre" => Some("🏷 Tagi: album + gatunek"),
        "🎛 Genre analyzer" => Some("🎛 Analizator gatunków"),
        "📊 Genre chart" => Some("📊 Wykres gatunków"),
        "🎛 Enable the genre analyzer in /settings first." => {
            Some("🎛 Najpierw włącz analizator gatunków w /settings.")
        }
        "No genre data yet — download a few tracks with the analyzer enabled." => Some(
            "Jeszcze brak danych o gatunkach — pobierz kilka utworów z włączonym analizatorem.",
        ),
        "Could not build the genre chart." => Some("Nie udało się utworzyć wykresu gatunków."),
        "Your genres" => Some("Twoje gatunki"),
        "{listens} listens · {genres} genres" => Some("{listens} odsłuchań · {genres} gatunków"),
        "Usage: /genres [5..20] — number of bars." => {
            Some("Użycie: /genres [5..20] — liczba słupków.")
        }
        "📊 Last.fm stats + collages" => Some("📊 Statystyki Last.fm + kolaże"),
        "🎵 Last.fm is not configured on this bot yet." => {
            Some("🎵 Last.fm nie jest jeszcze skonfigurowany na tym bocie.")
        }
        "🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off" => {
            Some(
                "🎵 Last.fm podłączony jako <b>{nick}</b>. W trybie inline puste zapytanie będzie teraz sugerować to, co grasz.\nOdłącz: /lastfm off",
            )
        }
        "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now." => {
            Some(
                "🎵 Last.fm: niepodłączony.\nPodłącz: /lastfm &lt;twój nick&gt; — puste zapytanie inline będzie sugerować obecnie grany utwór.",
            )
        }
        "🔌 Last.fm disconnected." => Some("🔌 Last.fm odłączony."),
        "⚠️ Last.fm says there is no such user. Check the nickname spelling at last.fm." => {
            Some(
                "⚠️ Last.fm nie zna takiego użytkownika. Sprawdź pisownię nazwy użytkownika na last.fm.",
            )
        }
        "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public." => {
            Some(
                "✅ Podłączono jako <b>{nick}</b>, ale nie ma jeszcze ostatnich utworów. Upewnij się, że odtwarzacz scrobbluje do Last.fm, a profil jest publiczny.",
            )
        }
        "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it." => {
            Some(
                "✅ Podłączono jako <b>{nick}</b>. Teraz gra: <b>{track}</b>. W trybie inline puste zapytanie zaproponuje ten utwór.",
            )
        }
        "⚠️ Invalid Last.fm username (2–15 latin letters, digits, _ or -)." => Some(
            "⚠️ Nieprawidłowa nazwa użytkownika Last.fm (2–15 łacińskich liter, cyfr, _ lub -).",
        ),
        "▶ Now playing: {track}" => Some("▶ Teraz gra: {track}"),
        "FLAC (lossless)" => Some("FLAC (bez strat)"),
        "🖼 HQ cover (1000px)" => Some("🖼 Okładka HQ (1000px)"),
        "⚠️ Your chosen search source is blocked by the server config — the bot falls back to YouTube. Ask the operator to enable it (SEARCH_PROVIDERS)." => {
            Some(
                "⚠️ Wybrane źródło wyszukiwania jest zablokowane przez konfigurację serwera — bot wraca do YouTube. Poproś operatora o jego włączenie (SEARCH_PROVIDERS).",
            )
        }
        "✅ Settings saved." => Some("✅ Ustawienia zapisane."),
        "🇬🇧 English" => Some("🇬🇧 Angielski"),
        "🌐 Language" => Some("🌐 Język"),
        "<b>Language</b>\nPick the interface language:" => {
            Some("<b>Język</b>\nWybierz język interfejsu:")
        }
        "« Back" => Some("« Wstecz"),
        "⬇️ Downloading… this usually takes a few seconds." => {
            Some("⬇️ Pobieranie… zwykle trwa to kilka sekund.")
        }
        "🔎 Searching…" => Some("🔎 Szukam…"),
        "Nothing found. Try a different spelling or paste a direct link." => {
            Some("Nic nie znaleziono. Spróbuj innej pisowni lub wklej bezpośredni link.")
        }
        "🚫 You are not allowed to use this bot." => Some("🚫 Nie możesz korzystać z tego bota."),
        "⛔ Admins only." => Some("⛔ Tylko dla administratorów."),
        "⏳ Too many requests. Limit: {per_minute} per minute." => {
            Some("⏳ Zbyt wiele żądań. Limit: {per_minute} na minutę.")
        }
        "ℹ️ Send links and search queries in a private chat with me. Commands work here too." => {
            Some(
                "ℹ️ Wyślij linki i zapytania w prywatnym czacie ze mną. Komendy działają też tutaj.",
            )
        }
        "🔒 This track is private or requires signing in, so I cannot download it." => {
            Some("🔒 Ten utwór jest prywatny lub wymaga zalogowania, więc nie mogę go pobrać.")
        }
        "🌍 This track is region locked and is not available from the bot's location." => Some(
            "🌍 Ten utwór jest zablokowany regionalnie i nie jest dostępny z lokalizacji bota.",
        ),
        "⏱ The track is too long. Maximum duration is {} minutes." => {
            Some("⏱ Utwór jest za długi. Maksymalny czas trwania to {} minut.")
        }
        "📦 The file is too large. Maximum size is {} MB." => {
            Some("📦 Plik jest za duży. Maksymalny rozmiar to {} MB.")
        }
        "🔍 I could not find that track on the available sources." => {
            Some("🔍 Nie udało mi się znaleźć tego utworu w dostępnych źródłach.")
        }
        "🤷 This link is not supported, or it needs a paid/DRM-protected source. Try a direct track link or search by name." => {
            Some(
                "🤷 Ten link nie jest obsługiwany albo wymaga płatnego źródła lub źródła chronionego DRM. Spróbuj bezpośredniego linku do utworu lub wyszukaj po nazwie.",
            )
        }
        "🚫 {} is disabled on this bot." => Some("🚫 {} jest wyłączone na tym bocie."),
        "📡 The source is temporarily unavailable. Please try again in a moment." => {
            Some("📡 Źródło jest tymczasowo niedostępne. Spróbuj ponownie za chwilę.")
        }
        "🎧 I downloaded the track but could not prepare the audio file." => {
            Some("🎧 Pobrałem utwór, ale nie udało mi się przygotować pliku audio.")
        }
        "⚠️ Something went wrong while preparing the track. Please try again." => {
            Some("⚠️ Coś poszło nie tak podczas przygotowywania utworu. Spróbuj ponownie.")
        }
        "💿 <b>{title}</b>\n{count} track(s). Download the whole collection?" => {
            Some("💿 <b>{title}</b>\n{count} utworów. Pobrać całą kolekcję?")
        }
        "⬇ Download all" => Some("⬇ Pobierz wszystkie"),
        "Cancel" => Some("Anuluj"),
        "Cancelled." => Some("Anulowano."),
        "💿 Loading the album…" => Some("💿 Wczytuję album…"),
        "💿 <b>{title}</b>\n{count} track(s) queued." => {
            Some("💿 <b>{title}</b>\n{count} utworów w kolejce.")
        }
        "💿 {current}/{total} — {title}" => Some("💿 {current}/{total} — {title}"),
        "✅ Album finished: {ok} sent, {failed} failed." => {
            Some("✅ Album zakończony: wysłano {ok}, z błędem {failed}.")
        }
        "This album or playlist has no downloadable tracks." => {
            Some("Ten album ani playlista nie zawierają utworów do pobrania.")
        }
        " and {extra} more" => Some(" i jeszcze {extra}"),
        "⚠ Failed: {list}{more}.\nSend /alb again — the finished tracks will come from cache instantly." => {
            Some(
                "⚠ Nie udało się: {list}{more}.\nWyślij /alb ponownie — ukończone utwory zostaną natychmiast pobrane z pamięci podręcznej.",
            )
        }
        "Usage: /alb &lt;album name&gt;\nExample: /alb Master of Puppets" => {
            Some("Użycie: /alb &lt;nazwa albumu&gt;\nPrzykład: /alb Master of Puppets")
        }
        "🔎 Searching albums…" => Some("🔎 Szukam albumów…"),
        "<b>Albums for</b> «{query}»\nPick one:" => {
            Some("<b>Albumy dla</b> «{query}»\nWybierz jeden:")
        }
        "No albums found. Try a different artist or album title." => {
            Some("Nie znaleziono albumów. Spróbuj innego wykonawcy lub tytułu albumu.")
        }
        " · {count} tracks" => Some(" · {count} utworów"),
        "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>" => {
            Some(
                "<b>Statystyki</b>\nUtworów w pamięci podręcznej: <b>{}</b>\nZnani użytkownicy: <b>{}</b>\nUdane wysyłki: <b>{}</b>\nCzas pracy: <b>{uptime}</b>",
            )
        }
        "🆕 <b>Bot update</b>\n\n{notes}\n\n💬 Developer channel: @maslougazIA" => {
            Some("🆕 <b>Aktualizacja bota</b>\n\n{notes}\n\n💬 Kanał developera: @maslougazIA")
        }
        "🗑 Purged <code>{key}</code> from cache." => {
            Some("🗑 Usunięto <code>{key}</code> z pamięci podręcznej.")
        }
        "That key is not in the cache." => Some("Tego klucza nie ma w pamięci podręcznej."),
        "🚫 User {user_id} banned." => Some("🚫 Użytkownik {user_id} zablokowany."),
        "✅ User {user_id} unbanned." => Some("✅ Użytkownik {user_id} odblokowany."),
        "Usage: /purge &lt;cache key or track URL&gt;" => {
            Some("Użycie: /purge &lt;klucz pamięci podręcznej lub adres utworu&gt;")
        }
        "⌛ This result has expired. Please search again." => {
            Some("⌛ Ten wynik wygasł. Wyszukaj ponownie.")
        }
        "✅ Sent: {title}" => Some("✅ Wysłano: {title}"),
        "🔎 Results for “{query}” ({count}):" => Some("🔎 Wyniki dla „{query}” ({count}):"),
        "Usage: /ban &lt;numeric user id&gt;" => {
            Some("Użycie: /ban &lt;numeryczne id użytkownika&gt;")
        }
        "Open bot" => Some("Otwórz bota"),
        "📩 Sent to your private messages with the bot." => {
            Some("📩 Wysłano do twoich prywatnych wiadomości z botem.")
        }
        "⏳ Preparing the track…" => Some("⏳ Przygotowuję utwór…"),
        "Send /import followed by a playlist (Artist – Title lines), or just /import and then send the file or the list in the next message." => {
            Some(
                "Wyślij /import i listę utworów (linie „Wykonawca – Tytuł”) albo samą komendę /import, a w następnej wiadomości wyślij plik lub listę.",
            )
        }
        "📥 Waiting for the playlist. Send a CSV/TXT/JSON file (Spotify, TuneMyMusic, Soundiiz export) or paste lines like <code>Artist – Title</code>. Send <code>cancel</code> to abort." => {
            Some(
                "📥 Czekam na playlistę. Wyślij plik CSV/TXT/JSON (eksport ze Spotify, TuneMyMusic, Soundiiz) lub wklej linie w formacie <code>Wykonawca – Tytuł</code>. Wyślij <code>cancel</code>, aby przerwać.",
            )
        }
        "🛑 Playlist import cancelled." => Some("🛑 Import playlisty anulowany."),
        "🔎 Matching {count} imported tracks…" => {
            Some("🔎 Dopasowuję {count} importowanych utworów…")
        }
        "No tracks found in that playlist." => Some("Nie znaleziono utworów w tej playliście."),
        "Too many tracks; import at most {max} at a time." => {
            Some("Za dużo utworów; importuj najwyżej {max} naraz.")
        }
        "Could not read that playlist file." => Some("Nie udało się odczytać pliku playlisty."),
        "📊 Enable Last.fm stats in /settings first." => {
            Some("📊 Najpierw włącz statystyki Last.fm w /settings.")
        }
        "Connect your public Last.fm nickname with /lastfm <nick> first." => {
            Some("Najpierw podłącz swój publiczny nick Last.fm komendą /lastfm <nick>.")
        }
        "Usage: /profile [7d|1m|3m|6m|1y|all]" => Some("Użycie: /profile [7d|1m|3m|6m|1y|all]"),
        "Usage: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]" => {
            Some("Użycie: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]")
        }
        "📊 <b>Last.fm: {name}</b>\nScrobbles (all time): <b>{playcount}</b>\nLibrary (all time): {tracks} tracks · {albums} albums · {artists} artists" => {
            Some(
                "📊 <b>Last.fm: {name}</b>\nScrobble'y (cały czas): <b>{playcount}</b>\nBiblioteka (cały czas): {tracks} utworów · {albums} albumów · {artists} wykonawców",
            )
        }
        "\nCountry: {country}" => Some("\nKraj: {country}"),
        "\n\n<b>Top tracks ({period}):</b>" => Some("\n\n<b>Najlepsze utwory ({period}):</b>"),
        "🎨 Generating collage…" => Some("🎨 Tworzę kolaż…"),
        "Could not build the collage (Last.fm returned no data)." => {
            Some("Nie udało się utworzyć kolażu (Last.fm nie zwrócił danych).")
        }
        "⏳ Downloading: {track}" => Some("⏳ Pobieranie: {track}"),
        "<b>Listening history</b> · {period}\n\n" => {
            Some("<b>Historia odsłuchów</b> · {period}\n\n")
        }
        "Nothing in this period yet — send a track first." => {
            Some("W tym okresie jeszcze nic nie ma — najpierw wyślij utwór.")
        }
        "\n… and {hidden} earlier\n" => Some("\n… i jeszcze {hidden} wcześniej\n"),
        "Usage: <code>/history [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Użycie: <code>/history [7d|1m|3m|6m|1y|all]</code>")
        }
        "Genre dynamics" => Some("Dynamika gatunków"),
        "{cells} genre-points · {buckets} buckets · {period}" => {
            Some("{cells} punktów gatunków · {buckets} przedziałów · {period}")
        }
        "No timed genre data yet. Enable the analyzer in /settings and download a few tracks." => {
            Some(
                "Jeszcze brak danych o gatunkach z czasem. Włącz analizator w /settings i pobierz kilka utworów.",
            )
        }
        "Could not build the dynamics chart." => Some("Nie udało się utworzyć wykresu dynamiki."),
        "Usage: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Użycie: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>")
        }
        "Usage:\n<code>/digest daily|weekly</code> — enable (default 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — quiet hours in UTC\n<code>/digest status</code> — current schedule\n<code>/digest off</code> — disable" => {
            Some(
                "Użycie:\n<code>/digest daily|weekly</code> — włącz (domyślnie 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — ciche godziny w UTC\n<code>/digest status</code> — aktualny harmonogram\n<code>/digest off</code> — wyłącz",
            )
        }
        "✅ {period} digest enabled. Next: <b>{next}</b> UTC." => {
            Some("✅ Digest {period} włączony. Następny: <b>{next}</b> UTC.")
        }
        "🛑 Digest disabled." => Some("🛑 Digest wyłączony."),
        "🌙 Quiet hours: <b>{window}</b> UTC." => Some("🌙 Ciche godziny: <b>{window}</b> UTC."),
        "📅 Digest: <b>{state}</b>\nPeriod: {period}\nNext: {next} UTC\nQuiet hours: {quiet} UTC" => {
            Some(
                "📅 Digest: <b>{state}</b>\nOkres: {period}\nNastępny: {next} UTC\nCiche godziny: {quiet} UTC",
            )
        }
        "No digest is enabled. Start with <code>/digest daily</code>." => {
            Some("Digest nie jest włączony. Zacznij od <code>/digest daily</code>.")
        }
        "none" => Some("brak"),
        "daily" => Some("dzienny"),
        "weekly" => Some("tygodniowy"),
        "📊 Your digest" => Some("📊 Twój digest"),
        "Tracks" => Some("Utwory"),
        "Top tracks" => Some("Najlepsze utwory"),
        _ => None,
    }
}
