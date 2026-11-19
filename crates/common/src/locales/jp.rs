//! Japanese translations. Keys are the English source strings from `i18n.rs`.
pub fn lookup(english: &str) -> Option<&'static str> {
    match english {
        "English" => Some("英語"),
        "⚖️ <b>Use responsibly.</b> Download only music you own, that is licensed under a free licence, or that you are otherwise allowed to copy. You are responsible for how you use this bot. The bot does not bypass DRM or paid access, and it stores only metadata plus Telegram file identifiers." => {
            Some(
                "⚖️ <b>責任ある利用をお願いします。</b> 自分が所有している音源、自由ライセンスのもの、またはコピーが許可されている音源のみをダウンロードしてください。このボットの利用方法についての責任は利用者にあります。ボットはDRMや有料アクセスを回避せず、保存するのはメタデータとTelegramのファイル識別子のみです。",
            )
        }
        "🎵 <b>Music Bot</b>\n\nSend a track link, an album link, or just <code>Artist — Title</code>. I will find the track and send it as audio with tags and cover.\n\nYou can also use me inline: type <code>@this_bot song</code> in any chat." => {
            Some(
                "🎵 <b>Music Bot</b>\n\n曲のリンク、アルバムのリンク、あるいは <code>アーティスト — タイトル</code> だけでも送信してください。曲を見つけて、タグとジャケット付きの音声として送ります。\n\nインラインでも使えます。任意のチャットで <code>@this_bot song</code> と入力してください。",
            )
        }
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality (kbps or FLAC lossless), source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)\n/import [list] — import a playlist; with no argument the next message (file or Artist – Title lines) is used\n/profile or /lastfmstats [period] — scrobble statistics (enable Last.fm stats in /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — PNG chart of your most played items\n/genres [5..20] — genre chart from your downloads (enable the analyzer in /settings)\n/lyrics [Artist — Title] — song lyrics from public sources (no argument: last track; or reply to an audio)\n/changelog — show the release notes\n/me — your own counters and favourite genres\n/queue — recent download jobs\n/cancel &lt;id&gt; — cancel an active download\n/stop — cancel all your active actions at once\n/convert auto|m4a|mp3|flac|opus — output format; reply to a track to convert that one\n/normalize on|off — ffmpeg loudnorm pass\n/playlist &lt;name&gt; — search public playlists\n/groupmode on|off — admin: allow the bot in this group\n/history [period] — your recent listening history\n/dynamics [period] — genre dynamics over time chart\n/digest daily|weekly|off — personal digest to your private chat\n/digest quiet HH-HH|off — quiet hours in UTC" => {
            Some(
                "<b>使い方</b>\n\n• プライベートチャット: リンクまたは <code>アーティスト — タイトル</code> を送信してください。\n• インライン: 任意のチャットで <code>@this_bot query</code>。Last.fm を接続済みで検索語を空にすると、再生中の曲を提案します。\n• アルバムやプレイリストは進捗表示付きで1曲ずつダウンロードします。<code>/alb &lt;name&gt;</code> でタイトルからアルバムを探せます。\n• 同じリクエストはキャッシュから返されます。\n\n<b>コマンド</b>\n/start — ボットについて\n/help — このヘルプ\n/settings — 言語、音質（kbps または FLAC ロスレス）、検索元、タグ\n/alb — 名前でアルバムを探してダウンロード\n/lastfm — Last.fm を接続（空のインライン検索で再生中の曲を提案）\n/import [list] — プレイリストをインポート。引数なしの場合、次のメッセージ（ファイルまたは「アーティスト – タイトル」形式の行）を使用します\n/profile または /lastfmstats [period] — スクロブル統計（/settings で Last.fm 統計を有効にしてください）\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — 最も再生した項目のPNGチャート\n/genres [5..20] — ダウンロード履歴からジャンルチャート（/settings でアナライザーを有効にしてください）\n/lyrics [アーティスト — タイトル] — 公開ソースから歌詞を取得（引数なし: 最後の曲、または音声に返信）\n/changelog — リリースノートを表示\n/me — 自分のカウンターとお気に入りジャンル\n/queue — 最近のダウンロード状況\n/cancel &lt;id&gt; — 進行中のダウンロードをキャンセル\n/stop — 自分の進行中の操作をすべてキャンセル\n/convert auto|m4a|mp3|flac|opus — 出力形式。曲に返信するとその曲だけを変換\n/normalize on|off — ffmpeg loudnorm による音量正規化\n/playlist &lt;name&gt; — 公開プレイリストを検索\n/groupmode on|off — 管理者: このグループでのボット利用を許可\n/history [period] — 最近の再生履歴\n/dynamics [period] — ジャンルの経年変化チャート\n/digest daily|weekly|off — プライベートチャットへの個人ダイジェスト\n/digest quiet HH-HH|off — UTC の静寂時間",
            )
        }
        "📝 Usage: <code>/lyrics Artist — Title</code>\nOr send <code>/lyrics</code> after a track, or reply to an audio message with <code>/lyrics</code>." => {
            Some(
                "📝 使い方: <code>/lyrics Artist — Title</code>\nまたは曲の送信後に <code>/lyrics</code> を送るか、音声メッセージに <code>/lyrics</code> と返信してください。",
            )
        }
        "🔎 Looking up lyrics…" => Some("🔎 歌詞を検索中…"),
        "<b>Lyrics</b>" => Some("<b>歌詞</b>"),
        "timestamped" => Some("タイムスタンプ付き"),
        "plain text" => Some("プレーンテキスト"),
        "😕 Lyrics not found in the public sources (LRCLIB, lyrics.ovh)." => {
            Some("😕 公開ソース（LRCLIB、lyrics.ovh）で歌詞が見つかりませんでした。")
        }
        "Send a track first, reply to an audio message, or pass a query: <code>/lyrics Artist — Title</code>." => {
            Some(
                "まず曲を送信するか、音声メッセージに返信するか、検索語を指定してください: <code>/lyrics Artist — Title</code>。",
            )
        }
        "📄 Download .lrc" => Some("📄 .lrc をダウンロード"),
        "🔄 Other source" => Some("🔄 別のソース"),
        "📄 LRC file saved. Drop it next to the audio in your player." => Some(
            "📄 LRCファイルを保存しました。プレイヤーの音声ファイルと同じ場所に置いてください。",
        ),
        "😕 The other public source has no lyrics for this track." => {
            Some("😕 別の公開ソースにもこの曲の歌詞はありません。")
        }
        "<b>What's new</b>\n\n" => Some("<b>新機能</b>\n\n"),
        "<b>Your stats</b>" => Some("<b>あなたの統計</b>"),
        "Tracks counted" => Some("集計された曲数"),
        "Listens" => Some("再生回数"),
        "Genres" => Some("ジャンル数"),
        "Audio time" => Some("音声の再生時間"),
        "top: /genres" => Some("トップ: /genres"),
        "⏳ This download is already being processed by another instance." => {
            Some("⏳ このダウンロードは別のインスタンスですでに処理中です。")
        }
        "🛑 Download cancelled." => Some("🛑 ダウンロードをキャンセルしました。"),
        "<b>Recent downloads</b>\n\n" => Some("<b>最近のダウンロード</b>\n\n"),
        "No downloads yet." => Some("まだダウンロードはありません。"),
        "Usage: <code>/cancel &lt;job id from /queue&gt;</code>" => {
            Some("使い方: <code>/cancel &lt;/queue のジョブID&gt;</code>")
        }
        "🛑 Cancellation requested." => Some("🛑 キャンセルを要求しました。"),
        "No active job with that id." => Some("そのIDの進行中のジョブはありません。"),
        "Usage: <code>/convert auto|m4a|mp3|flac|opus</code> — applies to your next downloads. Reply to a track with <code>/convert &lt;format&gt;</code> to convert that track." => {
            Some(
                "使い方: <code>/convert auto|m4a|mp3|flac|opus</code> — 次のダウンロードから適用されます。曲に <code>/convert &lt;形式&gt;</code> と返信するとその曲を変換します。",
            )
        }
        "This message is not a downloaded track I can convert." => {
            Some("このメッセージは変換できるダウンロード済みの曲ではありません。")
        }
        "Converted to" => Some("変換先"),
        "Conversion failed" => Some("変換に失敗しました"),
        "Stopped: {jobs} download(s), {prompts} pending action(s)." => {
            Some("停止しました: ダウンロード {jobs} 件、保留中の操作 {prompts} 件。")
        }
        "Output format" => Some("出力形式"),
        "ℹ️ Album & artist info" => Some("ℹ️ アルバムとアーティストの情報"),
        "⭐ Added to favorites." => Some("⭐ お気に入りに追加しました。"),
        "Removed from favorites." => Some("お気に入りから削除しました。"),
        "⭐ Your favorites" => Some("⭐ お気に入り"),
        "Nothing saved yet — tap ☆ under any track." => {
            Some("まだ何もありません — 曲の下の ☆ をタップ。")
        }
        "Reply to one of my tracks with /similar." => {
            Some("私の曲に /similar で返信してください。")
        }
        "No similar tracks found." => Some("似た曲は見つかりませんでした。"),
        "🎧 Similar to «{track}»" => Some("🎧 «{track}» に似た曲"),
        "Usage: /artist <name>" => Some("使い方: /artist <名前>"),
        "Artist not found in the catalog." => Some("カタログにアーティストが見つかりません。"),
        "🎤 Top tracks of {name}" => Some("🎤 {name} の人気曲"),
        "Reply to one of my tracks with /cover." => Some("私の曲に /cover で返信してください。"),
        "This track has no cover." => Some("この曲にはカバーがありません。"),
        "⚙ Advanced" => Some("⚙ 詳細設定"),
        "<b>Advanced settings</b>" => Some("<b>詳細設定</b>"),
        "on" => Some("オン"),
        "off" => Some("オフ"),
        "Usage: <code>/normalize on|off</code> (ffmpeg loudnorm, opt-in)." => {
            Some("使い方: <code>/normalize on|off</code> (ffmpeg loudnorm、任意設定)。")
        }
        "Admin only. Usage: <code>/groupmode on|off</code>" => {
            Some("管理者のみ。使い方: <code>/groupmode on|off</code>")
        }
        "✅ Group mode enabled." => Some("✅ グループモードを有効にしました。"),
        "🛑 Group mode disabled." => Some("🛑 グループモードを無効にしました。"),
        "🔎 Searching public playlists…" => Some("🔎 公開プレイリストを検索中…"),
        "Playlists" => Some("プレイリスト"),
        "😕 No public playlists found." => Some("😕 公開プレイリストが見つかりませんでした。"),
        "Usage: <code>/playlist &lt;name&gt;</code> — search public playlists (Deezer). Apple Music links already work as album URLs." => {
            Some(
                "使い方: <code>/playlist &lt;name&gt;</code> — 公開プレイリストを検索します (Deezer)。Apple Music のリンクはアルバムURLとしてそのまま使えます。",
            )
        }
        "Usage: <code>/broadcast &lt;text&gt;</code>" => {
            Some("使い方: <code>/broadcast &lt;text&gt;</code>")
        }
        "Broadcast complete" => Some("配信完了"),
        "Usage: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>" => {
            Some("使い方: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>")
        }
        "<b>Settings</b>" => Some("<b>設定</b>"),
        "Quality" => Some("音質"),
        "Language" => Some("言語"),
        "Search source (inline)" => Some("検索元 (インライン)"),
        "Tags (album + genres)" => Some("タグ (アルバム + ジャンル)"),
        "Last.fm" => Some("Last.fm"),
        "not connected (setup: /lastfm)" => Some("未接続 (設定: /lastfm)"),
        "HQ cover (1000px)" => Some("HQジャケット (1000px)"),
        "Last.fm stats" => Some("Last.fm 統計"),
        "Genre analyzer" => Some("ジャンルアナライザー"),
        "🏷 Tags: album + genre" => Some("🏷 タグ: アルバム + ジャンル"),
        "🎛 Genre analyzer" => Some("🎛 ジャンルアナライザー"),
        "📊 Genre chart" => Some("📊 ジャンルチャート"),
        "🎛 Enable the genre analyzer in /settings first." => {
            Some("🎛 まず /settings でジャンルアナライザーを有効にしてください。")
        }
        "No genre data yet — download a few tracks with the analyzer enabled." => Some(
            "ジャンルデータがまだありません — アナライザーを有効にして数曲ダウンロードしてください。",
        ),
        "Could not build the genre chart." => Some("ジャンルチャートを作成できませんでした。"),
        "Your genres" => Some("あなたのジャンル"),
        "{listens} listens · {genres} genres" => Some("{listens} 回再生 · {genres} ジャンル"),
        "Usage: /genres [5..20] — number of bars." => {
            Some("使い方: /genres [5..20] — 棒グラフの本数。")
        }
        "📊 Last.fm stats + collages" => Some("📊 Last.fm 統計 + コラージュ"),
        "🎵 Last.fm is not configured on this bot yet." => {
            Some("🎵 このボットでは Last.fm がまだ設定されていません。")
        }
        "🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off" => {
            Some(
                "🎵 <b>{nick}</b> として Last.fm に接続しました。これでインラインモードで検索語を空にすると再生中の曲を提案します。\n切断: /lastfm off",
            )
        }
        "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now." => {
            Some(
                "🎵 Last.fm: 未接続です。\n接続: /lastfm &lt;ニックネーム&gt; — 空のインライン検索で再生中の曲を提案します。",
            )
        }
        "🔌 Last.fm disconnected." => Some("🔌 Last.fm を切断しました。"),
        "⚠️ Last.fm says there is no such user. Check the nickname spelling at last.fm." => {
            Some(
                "⚠️ Last.fm にそのユーザーは存在しません。last.fm でニックネームの綴りを確認してください。",
            )
        }
        "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public." => {
            Some(
                "✅ <b>{nick}</b> として接続しましたが、最近の曲がまだありません。プレイヤーが Last.fm にスクロブルしていることと、プロフィールが公開されていることを確認してください。",
            )
        }
        "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it." => {
            Some(
                "✅ <b>{nick}</b> として接続しました。再生中: <b>{track}</b>。インラインモードで検索語を空にするとこの曲を提案します。",
            )
        }
        "⚠️ Invalid Last.fm username (2–15 latin letters, digits, _ or -)." => {
            Some("⚠️ Last.fm のユーザー名が無効です (2–15文字の英字、数字、_ または -)。")
        }
        "▶ Now playing: {track}" => Some("▶ 再生中: {track}"),
        "FLAC (lossless)" => Some("FLAC (ロスレス)"),
        "🖼 HQ cover (1000px)" => Some("🖼 HQジャケット (1000px)"),
        "⚠️ Your chosen search source is blocked by the server config — the bot falls back to YouTube. Ask the operator to enable it (SEARCH_PROVIDERS)." => {
            Some(
                "⚠️ 選択した検索元はサーバー設定でブロックされているため、ボットは YouTube にフォールバックします。管理者に有効化を依頼してください (SEARCH_PROVIDERS)。",
            )
        }
        "✅ Settings saved." => Some("✅ 設定を保存しました。"),
        "🇬🇧 English" => Some("🇬🇧 英語"),
        "🌐 Language" => Some("🌐 言語"),
        "<b>Language</b>\nPick the interface language:" => {
            Some("<b>言語</b>\nインターフェースの言語を選択してください:")
        }
        "« Back" => Some("« 戻る"),
        "⬇️ Downloading… this usually takes a few seconds." => {
            Some("⬇️ ダウンロード中… 通常は数秒かかります。")
        }
        "🔎 Searching…" => Some("🔎 検索中…"),
        "Nothing found. Try a different spelling or paste a direct link." => {
            Some("何も見つかりませんでした。綴りを変えるか、直接リンクを貼り付けてください。")
        }
        "🚫 You are not allowed to use this bot." => {
            Some("🚫 このボットを使用する権限がありません。")
        }
        "⛔ Admins only." => Some("⛔ 管理者のみです。"),
        "⏳ Too many requests. Limit: {per_minute} per minute." => {
            Some("⏳ リクエストが多すぎます。上限: 1分あたり {per_minute} 件。")
        }
        "ℹ️ Send links and search queries in a private chat with me. Commands work here too." => {
            Some(
                "ℹ️ リンクや検索語は私とのプライベートチャットで送信してください。コマンドはここでも使えます。",
            )
        }
        "🔒 This track is private or requires signing in, so I cannot download it." => {
            Some("🔒 この曲は非公開かログインが必要なため、ダウンロードできません。")
        }
        "🌍 This track is region locked and is not available from the bot's location." => {
            Some("🌍 この曲は地域制限があり、ボットの所在地からは利用できません。")
        }
        "⏱ The track is too long. Maximum duration is {} minutes." => {
            Some("⏱ 曲が長すぎます。最大時間は {} 分です。")
        }
        "📦 The file is too large. Maximum size is {} MB." => {
            Some("📦 ファイルが大きすぎます。最大サイズは {} MB です。")
        }
        "🔍 I could not find that track on the available sources." => {
            Some("🔍 利用可能なソースでその曲が見つかりませんでした。")
        }
        "🤷 This link is not supported, or it needs a paid/DRM-protected source. Try a direct track link or search by name." => {
            Some(
                "🤷 このリンクは対応していないか、有料・DRM保護されたソースが必要です。曲の直接リンクか名前検索をお試しください。",
            )
        }
        "🚫 {} is disabled on this bot." => Some("🚫 {} はこのボットでは無効です。"),
        "📡 The source is temporarily unavailable. Please try again in a moment." => {
            Some("📡 ソースが一時的に利用できません。少し待ってからもう一度お試しください。")
        }
        "🎧 I downloaded the track but could not prepare the audio file." => {
            Some("🎧 曲をダウンロードしましたが、音声ファイルを準備できませんでした。")
        }
        "⚠️ Something went wrong while preparing the track. Please try again." => {
            Some("⚠️ 曲の準備中に問題が発生しました。もう一度お試しください。")
        }
        "💿 <b>{title}</b>\n{count} track(s). Download the whole collection?" => {
            Some("💿 <b>{title}</b>\n{count}曲あります。アルバム全体をダウンロードしますか？")
        }
        "⬇ Download all" => Some("⬇ すべてダウンロード"),
        "Cancel" => Some("キャンセル"),
        "Cancelled." => Some("キャンセルしました。"),
        "💿 Loading the album…" => Some("💿 アルバムを読み込み中…"),
        "💿 <b>{title}</b>\n{count} track(s) queued." => {
            Some("💿 <b>{title}</b>\n{count}曲をキューに追加しました。")
        }
        "💿 {current}/{total} — {title}" => Some("💿 {current}/{total} — {title}"),
        "✅ Album finished: {ok} sent, {failed} failed." => {
            Some("✅ アルバム完了: {ok}件送信、{failed}件失敗。")
        }
        "This album or playlist has no downloadable tracks." => {
            Some("このアルバムまたはプレイリストにはダウンロードできる曲がありません。")
        }
        " and {extra} more" => Some(" と 他{extra}件"),
        "⚠ Failed: {list}{more}.\nSend /alb again — the finished tracks will come from cache instantly." => {
            Some(
                "⚠ 失敗: {list}{more}.\n/alb を再実行してください — 成功した曲はキャッシュから即座に送られます。",
            )
        }
        "Usage: /alb &lt;album name&gt;\nExample: /alb Master of Puppets" => {
            Some("使い方: /alb &lt;アルバム名&gt;\n例: /alb Master of Puppets")
        }
        "🔎 Searching albums…" => Some("🔎 アルバムを検索中…"),
        "<b>Albums for</b> «{query}»\nPick one:" => {
            Some("<b>検索結果:</b> «{query}»\n1つ選んでください:")
        }
        "No albums found. Try a different artist or album title." => Some(
            "アルバムが見つかりませんでした。アーティスト名やアルバム名を変えてお試しください。",
        ),
        " · {count} tracks" => Some(" · {count}曲"),
        "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>" => {
            Some(
                "<b>統計</b>\nキャッシュ済み曲数: <b>{}</b>\n既知ユーザー数: <b>{}</b>\n送信成功数: <b>{}</b>\n稼働時間: <b>{uptime}</b>",
            )
        }
        "🆕 <b>Bot update</b>\n\n{notes}\n\n💬 Developer channel: @maslougazIA" => {
            Some("🆕 <b>ボットの更新</b>\n\n{notes}\n\n💬 開発者チャンネル: @maslougazIA")
        }
        "🗑 Purged <code>{key}</code> from cache." => {
            Some("🗑 キャッシュから <code>{key}</code> を削除しました。")
        }
        "That key is not in the cache." => Some("そのキーはキャッシュにありません。"),
        "🚫 User {user_id} banned." => Some("🚫 ユーザー {user_id} をBANしました。"),
        "✅ User {user_id} unbanned." => Some("✅ ユーザー {user_id} のBANを解除しました。"),
        "Usage: /purge &lt;cache key or track URL&gt;" => {
            Some("使い方: /purge &lt;キャッシュキーまたは曲のURL&gt;")
        }
        "⌛ This result has expired. Please search again." => {
            Some("⌛ この検索結果は期限切れです。もう一度検索してください。")
        }
        "✅ Sent: {title}" => Some("✅ 送信しました: {title}"),
        "🔎 Results for “{query}” ({count}):" => Some("🔎 “{query}” の検索結果 ({count}件):"),
        "Usage: /ban &lt;numeric user id&gt;" => Some("使い方: /ban &lt;数値のユーザーID&gt;"),
        "Open bot" => Some("ボットを開く"),
        "📩 Sent to your private messages with the bot." => {
            Some("📩 ボットとのプライベートメッセージに送信しました。")
        }
        "⏳ Preparing the track…" => Some("⏳ 曲を準備中…"),
        "Send /import followed by a playlist (Artist – Title lines), or just /import and then send the file or the list in the next message." => {
            Some(
                "/import の後にプレイリスト（「アーティスト – タイトル」形式の行）を送るか、/import だけを送って次のメッセージでファイルまたはリストを送信してください。",
            )
        }
        "📥 Waiting for the playlist. Send a CSV/TXT/JSON file (Spotify, TuneMyMusic, Soundiiz export) or paste lines like <code>Artist – Title</code>. Send <code>cancel</code> to abort." => {
            Some(
                "📥 プレイリストを待っています。CSV/TXT/JSON ファイル（Spotify、TuneMyMusic、Soundiiz のエクスポート）を送るか、<code>Artist – Title</code> 形式の行を貼り付けてください。中止するには <code>cancel</code> を送ってください。",
            )
        }
        "🛑 Playlist import cancelled." => {
            Some("🛑 プレイリストのインポートをキャンセルしました。")
        }
        "🔎 Matching {count} imported tracks…" => {
            Some("🔎 {count}曲のインポート済み曲を照合中…")
        }
        "No tracks found in that playlist." => {
            Some("そのプレイリストには曲が見つかりませんでした。")
        }
        "Too many tracks; import at most {max} at a time." => {
            Some("曲数が多すぎます。一度にインポートできるのは最大 {max} 曲です。")
        }
        "Could not read that playlist file." => {
            Some("そのプレイリストファイルを読み込めませんでした。")
        }
        "📊 Enable Last.fm stats in /settings first." => {
            Some("📊 まず /settings で Last.fm 統計を有効にしてください。")
        }
        "Connect your public Last.fm nickname with /lastfm <nick> first." => {
            Some("まず /lastfm <nick> で公開されている Last.fm のニックネームを接続してください。")
        }
        "Usage: /profile [7d|1m|3m|6m|1y|all]" => Some("使い方: /profile [7d|1m|3m|6m|1y|all]"),
        "Usage: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]" => {
            Some("使い方: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]")
        }
        "📊 <b>Last.fm: {name}</b>\nScrobbles (all time): <b>{playcount}</b>\nLibrary (all time): {tracks} tracks · {albums} albums · {artists} artists" => {
            Some(
                "📊 <b>Last.fm: {name}</b>\nスクロブル数 (全期間): <b>{playcount}</b>\nライブラリ (全期間): {tracks}曲 · {albums}アルバム · {artists}アーティスト",
            )
        }
        "\nCountry: {country}" => Some("\n国: {country}"),
        "\n\n<b>Top tracks ({period}):</b>" => Some("\n\n<b>トップ曲 ({period}):</b>"),
        "🎨 Generating collage…" => Some("🎨 コラージュを生成中…"),
        "Could not build the collage (Last.fm returned no data)." => {
            Some("コラージュを作成できませんでした (Last.fm からデータが返されませんでした)。")
        }
        "⏳ Downloading: {track}" => Some("⏳ ダウンロード中: {track}"),
        "<b>Listening history</b> · {period}\n\n" => Some("<b>再生履歴</b> · {period}\n\n"),
        "Nothing in this period yet — send a track first." => {
            Some("この期間のデータはまだありません — まず曲を送信してください。")
        }
        "\n… and {hidden} earlier\n" => Some("\n… 他に{hidden}件\n"),
        "Usage: <code>/history [7d|1m|3m|6m|1y|all]</code>" => {
            Some("使い方: <code>/history [7d|1m|3m|6m|1y|all]</code>")
        }
        "Genre dynamics" => Some("ジャンルの推移"),
        "{cells} genre-points · {buckets} buckets · {period}" => {
            Some("{cells}ジャンルポイント · {buckets}区間 · {period}")
        }
        "No timed genre data yet. Enable the analyzer in /settings and download a few tracks." => {
            Some(
                "時刻付きのジャンルデータがまだありません。/settings でアナライザーを有効にして数曲ダウンロードしてください。",
            )
        }
        "Could not build the dynamics chart." => Some("推移チャートを作成できませんでした。"),
        "Usage: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>" => {
            Some("使い方: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>")
        }
        "Usage:\n<code>/digest daily|weekly</code> — enable (default 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — quiet hours in UTC\n<code>/digest status</code> — current schedule\n<code>/digest off</code> — disable" => {
            Some(
                "使い方:\n<code>/digest daily|weekly</code> — 有効化 (既定は 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — UTC の静寂時間\n<code>/digest status</code> — 現在の設定\n<code>/digest off</code> — 無効化",
            )
        }
        "✅ {period} digest enabled. Next: <b>{next}</b> UTC." => {
            Some("✅ {period} ダイジェストを有効にしました。次回: <b>{next}</b> UTC。")
        }
        "🛑 Digest disabled." => Some("🛑 ダイジェストを無効にしました。"),
        "🌙 Quiet hours: <b>{window}</b> UTC." => Some("🌙 静寂時間: <b>{window}</b> UTC。"),
        "📅 Digest: <b>{state}</b>\nPeriod: {period}\nNext: {next} UTC\nQuiet hours: {quiet} UTC" => {
            Some(
                "📅 ダイジェスト: <b>{state}</b>\n期間: {period}\n次回: {next} UTC\n静寂時間: {quiet} UTC",
            )
        }
        "No digest is enabled. Start with <code>/digest daily</code>." => Some(
            "ダイジェストは有効ではありません。まず <code>/digest daily</code> を実行してください。",
        ),
        "none" => Some("なし"),
        "daily" => Some("毎日"),
        "weekly" => Some("毎週"),
        "📊 Your digest" => Some("📊 あなたのダイジェスト"),
        "Tracks" => Some("曲数"),
        "Top tracks" => Some("トップ曲"),
        _ => None,
    }
}
