//! French translations. Keys are the English source strings from `i18n.rs`.
pub fn lookup(english: &str) -> Option<&'static str> {
    match english {
        "English" => Some("Anglais"),
        "⚖️ <b>Use responsibly.</b> Download only music you own, that is licensed under a free licence, or that you are otherwise allowed to copy. You are responsible for how you use this bot. The bot does not bypass DRM or paid access, and it stores only metadata plus Telegram file identifiers." => {
            Some(
                "⚖️ <b>Utilisez-le de façon responsable.</b> Téléchargez uniquement de la musique que vous possédez, qui est sous licence libre ou que vous êtes autorisé à copier. Vous êtes responsable de l'utilisation que vous faites de ce bot. Le bot ne contourne ni les DRM ni l'accès payant, et il ne stocke que les métadonnées ainsi que les identifiants de fichiers Telegram.",
            )
        }
        "🎵 <b>Music Bot</b>\n\nSend a track link, an album link, or just <code>Artist — Title</code>. I will find the track and send it as audio with tags and cover.\n\nYou can also use me inline: type <code>@this_bot song</code> in any chat." => {
            Some(
                "🎵 <b>Music Bot</b>\n\nEnvoyez un lien de morceau, un lien d'album ou simplement <code>Artiste — Titre</code>. Je trouverai le morceau et vous l'enverrai en audio avec les tags et la pochette.\n\nVous pouvez aussi m'utiliser en mode inline : tapez <code>@this_bot chanson</code> dans n'importe quel chat.",
            )
        }
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality (kbps or FLAC lossless), source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)\n/import [list] — import a playlist; with no argument the next message (file or Artist – Title lines) is used\n/profile or /lastfmstats [period] — scrobble statistics (enable Last.fm stats in /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — PNG chart of your most played items\n/genres [5..20] — genre chart from your downloads (enable the analyzer in /settings)\n/lyrics [Artist — Title] — song lyrics from public sources (no argument: last track; or reply to an audio)\n/changelog — show the release notes\n/me — your own counters and favourite genres\n/queue — recent download jobs\n/cancel &lt;id&gt; — cancel an active download\n/stop — cancel all your active actions at once\n/convert auto|m4a|mp3|flac|opus — output format; reply to a track to convert that one\n/normalize on|off — ffmpeg loudnorm pass\n/playlist &lt;name&gt; — search public playlists\n/fav — your saved ⭐ tracks (tap ☆ under a track to save it)\n/similar — reply to one of my tracks: up to 10 similar tracks\n/artist &lt;name&gt; — top tracks of an artist\n/cover — reply to one of my tracks: full-size album cover\n/albums — your saved albums\n/recognize — reply to an audio file: identify it by sound\n/groupmode on|off — admin: allow the bot in this group\n/history [period] — your recent listening history\n/dynamics [period] — genre dynamics over time chart\n/digest daily|weekly|off — personal digest to your private chat\n/digest quiet HH-HH|off — quiet hours in UTC\n/digest status — current digest schedule" => {
            Some(
                "<b>Comment utiliser</b>\n\n• Chat privé : envoyez un lien ou <code>Artiste — Titre</code>.\n• Inline : <code>@this_bot requête</code> dans n'importe quel chat. Une requête vide avec Last.fm connecté propose ce que vous écoutez actuellement.\n• Les albums et les playlists sont téléchargés morceau par morceau avec un message de progression. Utilisez <code>/alb &lt;nom&gt;</code> pour trouver un album par son titre.\n• Les demandes répétées sont servies depuis le cache.\n\n<b>Commandes</b>\n/start — à propos du bot\n/help — cette aide\n/settings — langue, qualité (kbps ou FLAC sans perte), source et tags\n/alb — trouver un album par nom et le télécharger\n/lastfm — connecter Last.fm (une requête inline vide suggère le morceau que vous écoutez)\n/import [liste] — importer une playlist ; sans argument, le prochain message (fichier ou lignes « Artiste – Titre ») est utilisé\n/profile ou /lastfmstats [période] — statistiques de scrobbles (activez les stats Last.fm dans /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [période] — graphique PNG de vos éléments les plus écoutés\n/genres [5..20] — graphique des genres de vos téléchargements (activez l'analyseur dans /settings)\n/lyrics [Artiste — Titre] — paroles de chansons provenant de sources publiques (sans argument : dernier morceau ; ou répondez à un audio)\n/changelog — afficher les notes de version\n/me — vos compteurs et genres favoris\n/queue — téléchargements récents\n/cancel &lt;id&gt; — annuler un téléchargement actif\n/stop — annuler toutes vos actions actives d'un coup\n/convert auto|m4a|mp3|flac|opus — format de sortie ; répondez à un morceau pour le convertir\n/normalize on|off — passe de normalisation ffmpeg loudnorm\n/playlist &lt;nom&gt; — rechercher des playlists publiques\n/fav — vos ⭐ morceaux enregistrés (appuyez sur ☆ sous un morceau pour l'enregistrer)\n/similar — répondez à l'un de mes titres : jusqu'à 10 titres similaires\n/artist &lt;nom&gt; — meilleurs titres d'un artiste\n/cover — répondez à l'un de mes titres : pochette d'album en pleine taille\n/albums — vos albums enregistrés\n/recognize — répondez à un fichier audio : identification par le son\n/groupmode on|off — admin : autoriser le bot dans ce groupe\n/history [période] — votre historique d'écoute récent\n/dynamics [période] — graphique de dynamique des genres au fil du temps\n/digest daily|weekly|off — digest personnel dans votre chat privé\n/digest quiet HH-HH|off — heures de silence en UTC\n/digest status — planning actuel",
            )
        }
        "{base}\n\n<b>Admin</b>\n/stats — counters\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;key or url&gt;\n/broadcast &lt;text&gt; — one-off notice to all known users" => {
            Some(
                "{base}\n\n<b>Admin</b>\n/stats — compteurs\n/ban &lt;id&gt;\n/unban &lt;id&gt;\n/purge &lt;clé ou URL&gt;\n/broadcast &lt;texte&gt; — notification unique à tous les utilisateurs connus",
            )
        }
        "📝 Usage: <code>/lyrics Artist — Title</code>\nOr send <code>/lyrics</code> after a track, or reply to an audio message with <code>/lyrics</code>." => {
            Some(
                "📝 Utilisation : <code>/lyrics Artiste — Titre</code>\nOu envoyez <code>/lyrics</code> après un morceau, ou répondez à un message audio avec <code>/lyrics</code>.",
            )
        }
        "🔎 Looking up lyrics…" => Some("🔎 Recherche des paroles…"),
        "<b>Lyrics</b>" => Some("<b>Paroles</b>"),
        "timestamped" => Some("avec horodatage"),
        "plain text" => Some("texte brut"),
        "😕 Lyrics not found in the public sources (LRCLIB, lyrics.ovh)." => {
            Some("😕 Paroles introuvables dans les sources publiques (LRCLIB, lyrics.ovh).")
        }
        "Send a track first, reply to an audio message, or pass a query: <code>/lyrics Artist — Title</code>." => {
            Some(
                "Envoyez d'abord un morceau, répondez à un message audio ou indiquez une requête : <code>/lyrics Artiste — Titre</code>.",
            )
        }
        "📄 Download .lrc" => Some("📄 Télécharger .lrc"),
        "🔄 Other source" => Some("🔄 Autre source"),
        "📄 LRC file saved. Drop it next to the audio in your player." => {
            Some("📄 Fichier LRC enregistré. Placez-le à côté de l'audio dans votre lecteur.")
        }
        "😕 The other public source has no lyrics for this track." => {
            Some("😕 L'autre source publique n'a pas de paroles pour ce morceau.")
        }
        "<b>What's new</b>\n\n" => Some("<b>Nouveautés</b>\n\n"),
        "<b>Your stats</b>" => Some("<b>Vos statistiques</b>"),
        "Tracks counted" => Some("Morceaux comptés"),
        "Listens" => Some("Écoutes"),
        "Genres" => Some("Genres"),
        "Audio time" => Some("Temps d'audio"),
        "top: /genres" => Some("top : /genres"),
        "⏳ This download is already being processed by another instance." => {
            Some("⏳ Ce téléchargement est déjà traité par une autre instance.")
        }
        "🛑 Download cancelled." => Some("🛑 Téléchargement annulé."),
        "<b>Recent downloads</b>\n\n" => Some("<b>Téléchargements récents</b>\n\n"),
        "No downloads yet." => Some("Aucun téléchargement pour l'instant."),
        " · 🛑 cancelling" => Some(" · 🛑 annulation en cours"),
        "Usage: <code>/cancel &lt;job id from /queue&gt;</code>" => {
            Some("Utilisation : <code>/cancel &lt;id de tâche de /queue&gt;</code>")
        }
        "🛑 Cancellation requested." => Some("🛑 Annulation demandée."),
        "No active job with that id." => Some("Aucune tâche active avec cet identifiant."),
        "Usage: <code>/convert auto|m4a|mp3|flac|opus</code> — applies to your next downloads. Reply to a track with <code>/convert &lt;format&gt;</code> to convert that track." => {
            Some(
                "Utilisation : <code>/convert auto|m4a|mp3|flac|opus</code> — s'applique à vos prochains téléchargements. Répondez à un morceau avec <code>/convert &lt;format&gt;</code> pour convertir ce morceau précis.",
            )
        }
        "This message is not a downloaded track I can convert." => {
            Some("Ce message n'est pas un morceau téléchargé que je peux convertir.")
        }
        "Converted to" => Some("Converti en"),
        "Conversion failed" => Some("Échec de la conversion"),
        "Stopped: {jobs} download(s), {prompts} pending action(s)." => {
            Some("Arrêté : {jobs} téléchargement(s), {prompts} action(s) en attente.")
        }
        "Output format" => Some("Format de sortie"),
        "ℹ️ Album & artist info" => Some("ℹ️ Infos album et artiste"),
        "⭐ Added to favorites." => Some("⭐ Ajouté aux favoris."),
        "Removed from favorites." => Some("Retiré des favoris."),
        "⭐ Your favorites" => Some("⭐ Vos favoris"),
        "Nothing saved yet — tap ☆ under any track." => {
            Some("Rien d'enregistré — appuyez sur ☆ sous un titre.")
        }
        "Reply to one of my tracks with /similar." => {
            Some("Répondez à l'un de mes titres avec /similar.")
        }
        "No similar tracks found." => Some("Aucun titre similaire trouvé."),
        "🎧 Similar to «{track}»" => Some("🎧 Similaire à «{track}»"),
        "Usage: /artist &lt;name&gt;" => Some("Utilisation : /artist &lt;nom&gt;"),
        "Artist not found in the catalog." => Some("Artiste introuvable dans le catalogue."),
        "🎤 Top tracks of {name}" => Some("🎤 Meilleurs titres de {name}"),
        "Reply to one of my tracks with /cover." => {
            Some("Répondez à l'un de mes titres avec /cover.")
        }
        "This track has no cover." => Some("Ce titre n'a pas de pochette."),
        "⚙ Advanced" => Some("⚙ Avancé"),
        "<b>Advanced settings</b>" => Some("<b>Réglages avancés</b>"),
        "on" => Some("activé"),
        "off" => Some("désactivé"),
        "Usage: <code>/normalize on|off</code> (ffmpeg loudnorm, opt-in)." => {
            Some("Utilisation : <code>/normalize on|off</code> (ffmpeg loudnorm, sur demande).")
        }
        "Admin only. Usage: <code>/groupmode on|off</code>" => {
            Some("Réservé aux administrateurs. Utilisation : <code>/groupmode on|off</code>")
        }
        "✅ Group mode enabled." => Some("✅ Mode groupe activé."),
        "🛑 Group mode disabled." => Some("🛑 Mode groupe désactivé."),
        "🔎 Searching public playlists…" => Some("🔎 Recherche de playlists publiques…"),
        "Playlists" => Some("Playlists"),
        "😕 No public playlists found." => Some("😕 Aucune playlist publique trouvée."),
        "Usage: <code>/playlist &lt;name&gt;</code> — search public playlists (Deezer). Apple Music links already work as album URLs." => {
            Some(
                "Utilisation : <code>/playlist &lt;nom&gt;</code> — rechercher des playlists publiques (Deezer). Les liens Apple Music fonctionnent déjà comme URL d'album.",
            )
        }
        "Usage: <code>/broadcast &lt;text&gt;</code>" => {
            Some("Utilisation : <code>/broadcast &lt;texte&gt;</code>")
        }
        "Broadcast complete" => Some("Diffusion terminée"),
        "Usage: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>" => {
            Some("Utilisation : <code>/genres [5..20] [week|month|3m|6m|year|all]</code>")
        }
        "<b>Settings</b>" => Some("<b>Paramètres</b>"),
        "Quality" => Some("Qualité"),
        "Language" => Some("Langue"),
        "Search source (inline)" => Some("Source de recherche (inline)"),
        "Tags (album + genres)" => Some("Tags (album + genres)"),
        "Last.fm" => Some("Last.fm"),
        "not connected (setup: /lastfm)" => Some("non connecté (configuration : /lastfm)"),
        "HQ cover (1000px)" => Some("Pochette HQ (1000px)"),
        "Last.fm stats" => Some("Statistiques Last.fm"),
        "Genre analyzer" => Some("Analyseur de genres"),
        "🏷 Tags: album + genre" => Some("🏷 Tags : album + genre"),
        "🎛 Genre analyzer" => Some("🎛 Analyseur de genres"),
        "📊 Genre chart" => Some("📊 Graphique des genres"),
        "🎛 Enable the genre analyzer in /settings first." => {
            Some("🎛 Activez d'abord l'analyseur de genres dans /settings.")
        }
        "No genre data yet — download a few tracks with the analyzer enabled." => Some(
            "Pas encore de données de genres — téléchargez quelques morceaux avec l'analyseur activé.",
        ),
        "Could not build the genre chart." => Some("Impossible de créer le graphique des genres."),
        "Your genres" => Some("Vos genres"),
        "{listens} listens · {genres} genres" => Some("{listens} écoutes · {genres} genres"),
        "Usage: /genres [5..20] — number of bars." => {
            Some("Utilisation : /genres [5..20] — nombre de barres.")
        }
        "📊 Last.fm stats + collages" => Some("📊 Statistiques Last.fm + collages"),
        "🎵 Last.fm is not configured on this bot yet." => {
            Some("🎵 Last.fm n'est pas encore configuré sur ce bot.")
        }
        "🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off" => {
            Some(
                "🎵 Last.fm connecté en tant que <b>{nick}</b>. En mode inline avec une requête vide, le bot proposera désormais ce que vous écoutez.\nDéconnexion : /lastfm off",
            )
        }
        "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now." => {
            Some(
                "🎵 Last.fm : non connecté.\nConnexion : /lastfm &lt;votre pseudonyme&gt; — une requête inline vide suggérera ce qui est en cours de lecture.",
            )
        }
        "🔌 Last.fm disconnected." => Some("🔌 Last.fm déconnecté."),
        "⚠️ Last.fm says there is no such user. Check the nickname spelling at last.fm." => {
            Some(
                "⚠️ Last.fm indique qu'il n'existe pas d'utilisateur de ce nom. Vérifiez l'orthographe du pseudonyme sur last.fm.",
            )
        }
        "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public." => {
            Some(
                "✅ Connecté en tant que <b>{nick}</b>, mais aucun morceau récent pour l'instant. Vérifiez que votre lecteur scrobble vers Last.fm et que votre profil est public.",
            )
        }
        "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it." => {
            Some(
                "✅ Connecté en tant que <b>{nick}</b>. En cours de lecture : <b>{track}</b>. En mode inline, une requête vide le proposera.",
            )
        }
        "⚠️ Invalid Last.fm username (2–15 latin letters, digits, _ or -)." => {
            Some("⚠️ Pseudonyme Last.fm invalide (2–15 lettres latines, chiffres, _ ou -).")
        }
        "▶ Now playing: {track}" => Some("▶ En cours de lecture : {track}"),
        "FLAC (lossless)" => Some("FLAC (sans perte)"),
        "🖼 HQ cover (1000px)" => Some("🖼 Pochette HQ (1000px)"),
        "⚠️ Your chosen search source is blocked by the server config — the bot falls back to YouTube. Ask the operator to enable it (SEARCH_PROVIDERS)." => {
            Some(
                "⚠️ La source de recherche choisie est bloquée par la configuration du serveur — le bot revient à YouTube. Demandez à l'exploitant de l'activer (SEARCH_PROVIDERS).",
            )
        }
        "✅ Settings saved." => Some("✅ Paramètres enregistrés."),
        "🇬🇧 English" => Some("🇬🇧 Anglais"),
        "🌐 Language" => Some("🌐 Langue"),
        "<b>Language</b>\nPick the interface language:" => {
            Some("<b>Langue</b>\nChoisissez la langue de l'interface :")
        }
        "« Back" => Some("« Retour"),
        "⬇️ Downloading… this usually takes a few seconds." => {
            Some("⬇️ Téléchargement… cela prend généralement quelques secondes.")
        }
        "🔎 Searching…" => Some("🔎 Recherche…"),
        "Nothing found. Try a different spelling or paste a direct link." => {
            Some("Rien trouvé. Essayez une autre orthographe ou collez un lien direct.")
        }
        "🚫 You are not allowed to use this bot." => {
            Some("🚫 Vous n'êtes pas autorisé à utiliser ce bot.")
        }
        "⛔ Admins only." => Some("⛔ Réservé aux administrateurs."),
        "⏳ Too many requests. Limit: {per_minute} per minute." => {
            Some("⏳ Trop de requêtes. Limite : {per_minute} par minute.")
        }
        "ℹ️ Send links and search queries in a private chat with me. Commands work here too." => {
            Some(
                "ℹ️ Envoyez les liens et les requêtes de recherche dans un chat privé avec moi. Les commandes fonctionnent aussi ici.",
            )
        }
        "🔒 This track is private or requires signing in, so I cannot download it." => Some(
            "🔒 Ce morceau est privé ou nécessite une connexion, je ne peux donc pas le télécharger.",
        ),
        "🌍 This track is region locked and is not available from the bot's location." => {
            Some("🌍 Ce morceau est géobloqué et n'est pas disponible depuis l'emplacement du bot.")
        }
        "⏱ The track is too long. Maximum duration is {} minutes." => {
            Some("⏱ Le morceau est trop long. La durée maximale est de {} minutes.")
        }
        "📦 The file is too large. Maximum size is {} MB." => {
            Some("📦 Le fichier est trop volumineux. La taille maximale est de {} Mo.")
        }
        "🔍 I could not find that track on the available sources." => {
            Some("🔍 Je n'ai pas trouvé ce morceau sur les sources disponibles.")
        }
        "🤷 This link is not supported, or it needs a paid/DRM-protected source. Try a direct track link or search by name." => {
            Some(
                "🤷 Ce lien n'est pas pris en charge, ou il nécessite une source payante/protégée par DRM. Essayez un lien direct vers le morceau ou une recherche par nom.",
            )
        }
        "🚫 {} is disabled on this bot." => Some("🚫 {} est désactivé sur ce bot."),
        "📡 The source is temporarily unavailable. Please try again in a moment." => Some(
            "📡 La source est temporairement indisponible. Veuillez réessayer dans un instant.",
        ),
        "🎧 I downloaded the track but could not prepare the audio file." => {
            Some("🎧 J'ai téléchargé le morceau mais je n'ai pas pu préparer le fichier audio.")
        }
        "⚠️ Something went wrong while preparing the track. Please try again." => Some(
            "⚠️ Une erreur est survenue lors de la préparation du morceau. Veuillez réessayer.",
        ),
        "💿 <b>{title}</b>\n{count} track(s). Download the whole collection?" => {
            Some("💿 <b>{title}</b>\n{count} morceau(x). Télécharger toute la collection ?")
        }
        "⬇ Download all" => Some("⬇ Tout télécharger"),
        "Cancel" => Some("Annuler"),
        "Cancelled." => Some("Annulé."),
        "💿 Loading the album…" => Some("💿 Chargement de l'album…"),
        "💿 <b>{title}</b>\n{count} track(s) queued." => {
            Some("💿 <b>{title}</b>\n{count} morceau(x) mis en file d'attente.")
        }
        "💿 {current}/{total} — {title}" => Some("💿 {current}/{total} — {title}"),
        "✅ Album finished: {ok} sent, {failed} failed." => {
            Some("✅ Album terminé : {ok} envoyé(s), {failed} en échec.")
        }
        "This album or playlist has no downloadable tracks." => {
            Some("Cet album ou cette playlist ne contient aucun morceau téléchargeable.")
        }
        " and {extra} more" => Some(" et {extra} de plus"),
        "⚠ Failed: {list}{more}.\nSend /alb again — the finished tracks will come from cache instantly." => {
            Some(
                "⚠ Échec : {list}{more}.\nRenvoyez /alb — les morceaux déjà terminés viendront instantanément du cache.",
            )
        }
        "Usage: /alb &lt;album name&gt;\nExample: /alb Master of Puppets" => {
            Some("Utilisation : /alb &lt;nom de l'album&gt;\nExemple : /alb Master of Puppets")
        }
        "🔎 Searching albums…" => Some("🔎 Recherche d'albums…"),
        "<b>Albums for</b> «{query}»\nPick one:" => {
            Some("<b>Albums pour</b> «{query}»\nChoisissez-en un :")
        }
        "No albums found. Try a different artist or album title." => {
            Some("Aucun album trouvé. Essayez un autre artiste ou un autre titre d'album.")
        }
        " · {count} tracks" => Some(" · {count} morceaux"),
        "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>" => {
            Some(
                "<b>Statistiques</b>\nMorceaux en cache : <b>{}</b>\nUtilisateurs connus : <b>{}</b>\nEnvois réussis : <b>{}</b>\nTemps de fonctionnement : <b>{uptime}</b>",
            )
        }
        "🆕 <b>Bot update</b>\n\n{notes}\n\n💬 Developer channel: @maslougazIA" => Some(
            "🆕 <b>Mise à jour du bot</b>\n\n{notes}\n\n💬 Canal du développeur : @maslougazIA",
        ),
        "🗑 Purged <code>{key}</code> from cache." => {
            Some("🗑 <code>{key}</code> supprimé du cache.")
        }
        "That key is not in the cache." => Some("Cette clé n'est pas dans le cache."),
        "🚫 User {user_id} banned." => Some("🚫 Utilisateur {user_id} banni."),
        "✅ User {user_id} unbanned." => Some("✅ Utilisateur {user_id} débanni."),
        "Usage: /purge &lt;cache key or track URL&gt;" => {
            Some("Utilisation : /purge &lt;clé de cache ou URL de morceau&gt;")
        }
        "⌛ This result has expired. Please search again." => {
            Some("⌛ Ce résultat a expiré. Veuillez relancer une recherche.")
        }
        "✅ Sent: {title}" => Some("✅ Envoyé : {title}"),
        "🔎 Results for “{query}” ({count}):" => {
            Some("🔎 Résultats pour “{query}” ({count}) :")
        }
        "Usage: /ban &lt;numeric user id&gt;" => {
            Some("Utilisation : /ban &lt;identifiant utilisateur numérique&gt;")
        }
        "Open bot" => Some("Ouvrir le bot"),
        "📩 Sent to your private messages with the bot." => {
            Some("📩 Envoyé dans vos messages privés avec le bot.")
        }
        "⏳ Preparing the track…" => Some("⏳ Préparation du morceau…"),
        "Send /import followed by a playlist (Artist – Title lines), or just /import and then send the file or the list in the next message." => {
            Some(
                "Envoyez /import suivi d'une playlist (lignes « Artiste – Titre »), ou simplement /import puis envoyez le fichier ou la liste dans le message suivant.",
            )
        }
        "📥 Waiting for the playlist. Send a CSV/TXT/JSON file (Spotify, TuneMyMusic, Soundiiz export) or paste lines like <code>Artist – Title</code>. Send <code>cancel</code> to abort." => {
            Some(
                "📥 En attente de la playlist. Envoyez un fichier CSV/TXT/JSON (export Spotify, TuneMyMusic, Soundiiz) ou collez des lignes comme <code>Artiste – Titre</code>. Envoyez <code>cancel</code> pour annuler.",
            )
        }
        "🛑 Playlist import cancelled." => Some("🛑 Import de playlist annulé."),
        "🔎 Matching {count} imported tracks…" => {
            Some("🔎 Recherche de {count} morceaux importés…")
        }
        "No tracks found in that playlist." => Some("Aucun morceau trouvé dans cette playlist."),
        "Too many tracks; import at most {max} at a time." => {
            Some("Trop de morceaux ; importez-en au plus {max} à la fois.")
        }
        "Could not read that playlist file." => Some("Impossible de lire ce fichier de playlist."),
        "📊 Enable Last.fm stats in /settings first." => {
            Some("📊 Activez d'abord les statistiques Last.fm dans /settings.")
        }
        "Connect your public Last.fm nickname with /lastfm <nick> first." => {
            Some("Connectez d'abord votre pseudonyme Last.fm public avec /lastfm <pseudo>.")
        }
        "Usage: /profile [7d|1m|3m|6m|1y|all]" => {
            Some("Utilisation : /profile [7d|1m|3m|6m|1y|all]")
        }
        "Usage: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]" => {
            Some("Utilisation : /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]")
        }
        "📊 <b>Last.fm: {name}</b>\nScrobbles (all time): <b>{playcount}</b>\nLibrary (all time): {tracks} tracks · {albums} albums · {artists} artists" => {
            Some(
                "📊 <b>Last.fm : {name}</b>\nScrobbles (depuis toujours) : <b>{playcount}</b>\nBibliothèque (depuis toujours) : {tracks} morceaux · {albums} albums · {artists} artistes",
            )
        }
        "\nCountry: {country}" => Some("\nPays : {country}"),
        "\n\n<b>Top tracks ({period}):</b>" => {
            Some("\n\n<b>Morceaux les plus écoutés ({period}) :</b>")
        }
        "🎨 Generating collage…" => Some("🎨 Génération du collage…"),
        "Could not build the collage (Last.fm returned no data)." => {
            Some("Impossible de créer le collage (Last.fm n'a renvoyé aucune donnée).")
        }
        "⏳ Downloading: {track}" => Some("⏳ Téléchargement : {track}"),
        "<b>Listening history</b> · {period}\n\n" => {
            Some("<b>Historique d'écoute</b> · {period}\n\n")
        }
        "Nothing in this period yet — send a track first." => {
            Some("Rien pour cette période — envoyez d'abord un morceau.")
        }
        "\n… and {hidden} earlier\n" => Some("\n… et {hidden} plus ancien(s)\n"),
        "Usage: <code>/history [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Utilisation : <code>/history [7d|1m|3m|6m|1y|all]</code>")
        }
        "Genre dynamics" => Some("Dynamique des genres"),
        "{cells} genre-points · {buckets} buckets · {period}" => {
            Some("{cells} points-genres · {buckets} intervalles · {period}")
        }
        "No timed genre data yet. Enable the analyzer in /settings and download a few tracks." => {
            Some(
                "Pas encore de données de genres horodatées. Activez l'analyseur dans /settings et téléchargez quelques morceaux.",
            )
        }
        "Could not build the dynamics chart." => {
            Some("Impossible de créer le graphique de dynamique.")
        }
        "Usage: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>" => {
            Some("Utilisation : <code>/dynamics [7d|1m|3m|6m|1y|all]</code>")
        }
        "Usage:\n<code>/digest daily|weekly</code> — enable (default 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — quiet hours in UTC\n<code>/digest status</code> — current schedule\n<code>/digest off</code> — disable" => {
            Some(
                "Utilisation :\n<code>/digest daily|weekly</code> — activer (09:00 UTC par défaut)\n<code>/digest quiet HH-HH|off</code> — heures de silence en UTC\n<code>/digest status</code> — planning actuel\n<code>/digest off</code> — désactiver",
            )
        }
        "✅ {period} digest enabled. Next: <b>{next}</b> UTC." => {
            Some("✅ Digest {period} activé. Prochain : <b>{next}</b> UTC.")
        }
        "🛑 Digest disabled." => Some("🛑 Digest désactivé."),
        "🌙 Quiet hours: <b>{window}</b> UTC." => {
            Some("🌙 Heures de silence : <b>{window}</b> UTC.")
        }
        "📅 Digest: <b>{state}</b>\nPeriod: {period}\nNext: {next} UTC\nQuiet hours: {quiet} UTC" => {
            Some(
                "📅 Digest : <b>{state}</b>\nPériode : {period}\nProchain : {next} UTC\nHeures de silence : {quiet} UTC",
            )
        }
        "No digest is enabled. Start with <code>/digest daily</code>." => {
            Some("Aucun digest n'est activé. Commencez avec <code>/digest daily</code>.")
        }
        "none" => Some("aucune"),
        "daily" => Some("daily"),
        "weekly" => Some("weekly"),
        "📊 Your digest" => Some("📊 Votre digest"),
        "Tracks" => Some("Morceaux"),
        "Top tracks" => Some("Morceaux les plus écoutés"),
        "🎵 Your albums" => Some("🎵 Vos albums"),
        "Library is empty — save an album with the 💾 button under a delivered album" => Some(
            "La bibliothèque est vide — enregistrez un album avec le bouton 💾 sous un album reçu",
        ),
        "💾 Save album" => Some("💾 Enregistrer l'album"),
        "🗑 Remove" => Some("🗑 Retirer"),
        "Saved" => Some("Enregistré"),
        "Already saved" => Some("Déjà enregistré"),
        "Removed from library" => Some("Retiré de la bibliothèque"),
        "🗜 Packing album…" => Some("🗜 Compression de l'album…"),
        "Packing failed: no track could be downloaded" => {
            Some("Échec de la compression : aucun morceau n'a pu être téléchargé")
        }
        "🧹 Auto-delete" => Some("🧹 Suppression automatique"),
        "Auto-delete off" => Some("Suppression automatique désactivée"),
        "1 hour" => Some("1 heure"),
        "6 hours" => Some("6 heures"),
        "12 hours" => Some("12 heures"),
        "24 hours" => Some("24 heures"),
        "The bot deletes its own audio messages after this time (max 48 h). Applies only to messages sent after enabling." => {
            Some(
                "Le bot supprime ses propres messages audio après ce délai (max. 48 h). Ne s'applique qu'aux messages envoyés après l'activation.",
            )
        }
        "Reply to an audio file with /recognize" => {
            Some("Répondez à un fichier audio avec /recognize")
        }
        "Recognition is not configured on this server" => {
            Some("La reconnaissance n'est pas configurée sur ce serveur")
        }
        "🔍 Listening…" => Some("🔍 Écoute en cours…"),
        "Could not identify this track" => Some("Impossible d'identifier ce morceau"),
        "Download album" => Some("Télécharger l'album"),
        "Auto-delete: {n} hours" => Some("Suppression auto : {n} h"),
        "Set: {n} hours" => Some("Défini : {n} h"),
        "≈ {n} min" => Some("≈ {n} min"),
        "Album is too large to pack (limit {n} MB)" => {
            Some("L'album est trop volumineux à compresser (limite {n} Mo)")
        }
        "📦 {album} — {n} tracks" => Some("📦 {album} — {n} morceaux"),
        "Looks like: {artist} — {title}" => Some("Ça ressemble à : {artist} — {title}"),
        "💿 {album} · {year}" => Some("💿 {album} · {year}"),
        _ => None,
    }
}
