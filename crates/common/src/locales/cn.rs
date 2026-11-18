//! Simplified Chinese translations. Keys are the English source strings from `i18n.rs`.
pub fn lookup(english: &str) -> Option<&'static str> {
    match english {
        "English" => Some("英语"),
        "⚖️ <b>Use responsibly.</b> Download only music you own, that is licensed under a free licence, or that you are otherwise allowed to copy. You are responsible for how you use this bot. The bot does not bypass DRM or paid access, and it stores only metadata plus Telegram file identifiers." => {
            Some(
                "⚖️ <b>请负责任使用。</b>只下载你拥有的、采用自由许可的、或以其他方式获准复制的音乐。你对如何使用本机器人负责。本机器人不绕过 DRM 或付费访问，仅存储元数据及 Telegram 文件标识符。",
            )
        }
        "🎵 <b>Music Bot</b>\n\nSend a track link, an album link, or just <code>Artist — Title</code>. I will find the track and send it as audio with tags and cover.\n\nYou can also use me inline: type <code>@this_bot song</code> in any chat." => {
            Some(
                "🎵 <b>音乐机器人</b>\n\n发送歌曲链接、专辑链接，或直接输入 <code>艺人 — 标题</code>。我会找到这首歌，并以带标签和封面的音频发送给你。\n\n你也可以在任意聊天中内联使用我：输入 <code>@this_bot song</code>。",
            )
        }
        "<b>How to use</b>\n\n• Private chat: send a link or <code>Artist — Title</code>.\n• Inline: <code>@this_bot query</code> in any chat. Empty query with Last.fm connected proposes what you're playing now.\n• Albums and playlists are downloaded one track at a time with a progress message. Use <code>/alb &lt;name&gt;</code> to find an album by title.\n• Repeat requests are served from cache.\n\n<b>Commands</b>\n/start — about the bot\n/help — this help\n/settings — language, quality (kbps or FLAC lossless), source and tags\n/alb — find an album by name and download it\n/lastfm — connect Last.fm (empty inline query suggests the track you're playing)\n/import [list] — import a playlist; with no argument the next message (file or Artist – Title lines) is used\n/profile or /lastfmstats [period] — scrobble statistics (enable Last.fm stats in /settings)\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — PNG chart of your most played items\n/genres [5..20] — genre chart from your downloads (enable the analyzer in /settings)\n/lyrics [Artist — Title] — song lyrics from public sources (no argument: last track; or reply to an audio)\n/changelog — show the release notes\n/me — your own counters and favourite genres\n/queue — recent download jobs\n/cancel &lt;id&gt; — cancel an active download\n/stop — cancel all your active actions at once\n/convert auto|m4a|mp3|flac|opus — output format; reply to a track to convert that one\n/normalize on|off — ffmpeg loudnorm pass\n/playlist &lt;name&gt; — search public playlists\n/groupmode on|off — admin: allow the bot in this group\n/history [period] — your recent listening history\n/dynamics [period] — genre dynamics over time chart\n/digest daily|weekly|off — personal digest to your private chat\n/digest quiet HH-HH|off — quiet hours in UTC" => {
            Some(
                "<b>使用说明</b>\n\n• 私聊：发送链接或 <code>艺人 — 标题</code>。\n• 内联：在任意聊天中输入 <code>@this_bot query</code>。连接 Last.fm 后，空查询会推荐你正在播放的歌曲。\n• 专辑和歌单会逐首下载并显示进度消息。使用 <code>/alb &lt;name&gt;</code> 可按标题查找专辑。\n• 重复的请求将从缓存中提供。\n\n<b>命令</b>\n/start — 关于本机器人\n/help — 本帮助\n/settings — 语言、音质（kbps 或 FLAC 无损）、来源和标签\n/alb — 按名称查找专辑并下载\n/lastfm — 连接 Last.fm（空的内联查询会推荐你正在播放的歌曲）\n/import [list] — 导入歌单；不带参数时，将下一条消息（文件或“艺人 – 标题”行）作为列表\n/profile 或 /lastfmstats [period] — 听歌记录统计（在 /settings 中启用 Last.fm 统计）\n/collage &lt;tracks|albums|artists&gt; [3x3..10x10] [period] — 你最常播放内容的 PNG 图表\n/genres [5..20] — 根据下载记录生成流派图表（在 /settings 中启用分析器）\n/lyrics [Artist — Title] — 从公开来源获取歌词（无参数：最近一首歌曲；或回复音频消息）\n/changelog — 显示更新日志\n/me — 你自己的计数和喜爱的流派\n/queue — 最近的下载任务\n/cancel &lt;id&gt; — 取消正在进行的下载\n/stop — 立即取消你所有正在进行的操作\n/convert auto|m4a|mp3|flac|opus — 输出格式；回复某首歌曲可仅转换该首\n/normalize on|off — ffmpeg 响度均衡处理\n/playlist &lt;name&gt; — 搜索公开歌单\n/groupmode on|off — 管理员：允许本机器人在本群使用\n/history [period] — 你最近的收听历史\n/dynamics [period] — 流派随时间变化的图表\n/digest daily|weekly|off — 向你私聊发送个人摘要\n/digest quiet HH-HH|off — 以 UTC 表示的静默时段",
            )
        }
        "📝 Usage: <code>/lyrics Artist — Title</code>\nOr send <code>/lyrics</code> after a track, or reply to an audio message with <code>/lyrics</code>." => {
            Some(
                "📝 用法：<code>/lyrics Artist — Title</code>\n或在某首歌曲后发送 <code>/lyrics</code>，或回复音频消息并输入 <code>/lyrics</code>。",
            )
        }
        "🔎 Looking up lyrics…" => Some("🔎 正在查找歌词…"),
        "<b>Lyrics</b>" => Some("<b>歌词</b>"),
        "timestamped" => Some("带时间戳"),
        "plain text" => Some("纯文本"),
        "😕 Lyrics not found in the public sources (LRCLIB, lyrics.ovh)." => {
            Some("😕 在公开来源（LRCLIB、lyrics.ovh）中未找到歌词。")
        }
        "Send a track first, reply to an audio message, or pass a query: <code>/lyrics Artist — Title</code>." => {
            Some(
                "请先发送一首歌曲、回复音频消息，或提供查询内容：<code>/lyrics Artist — Title</code>。",
            )
        }
        "📄 Download .lrc" => Some("📄 下载 .lrc"),
        "🔄 Other source" => Some("🔄 其他来源"),
        "📄 LRC file saved. Drop it next to the audio in your player." => {
            Some("📄 LRC 文件已保存。请将其放在播放器中音频文件的旁边。")
        }
        "😕 The other public source has no lyrics for this track." => {
            Some("😕 另一个公开来源没有这首歌的歌词。")
        }
        "<b>What's new</b>\n\n" => Some("<b>更新内容</b>\n\n"),
        "<b>Your stats</b>" => Some("<b>你的统计</b>"),
        "Tracks counted" => Some("已统计曲目"),
        "Listens" => Some("收听次数"),
        "Genres" => Some("流派数"),
        "Audio time" => Some("音频时长"),
        "top: /genres" => Some("热门：/genres"),
        "⏳ This download is already being processed by another instance." => {
            Some("⏳ 该下载正在由另一个实例处理。")
        }
        "🛑 Download cancelled." => Some("🛑 下载已取消。"),
        "<b>Recent downloads</b>\n\n" => Some("<b>最近的下载</b>\n\n"),
        "No downloads yet." => Some("暂无下载记录。"),
        "Usage: <code>/cancel &lt;job id from /queue&gt;</code>" => {
            Some("用法：<code>/cancel &lt;来自 /queue 的任务 id&gt;</code>")
        }
        "🛑 Cancellation requested." => Some("🛑 已请求取消。"),
        "No active job with that id." => Some("没有该 id 的活跃任务。"),
        "Usage: <code>/convert auto|m4a|mp3|flac|opus</code> — applies to your next downloads. Reply to a track with <code>/convert &lt;format&gt;</code> to convert that track." => {
            Some(
                "用法：<code>/convert auto|m4a|mp3|flac|opus</code> — 适用于你之后的下载。回复某首歌曲并输入 <code>/convert &lt;format&gt;</code> 可转换该首歌曲。",
            )
        }
        "This message is not a downloaded track I can convert." => {
            Some("这条消息不是我可以转换的已下载歌曲。")
        }
        "Converted to" => Some("已转换为"),
        "Conversion failed" => Some("转换失败"),
        "Stopped: {jobs} download(s), {prompts} pending action(s)." => {
            Some("已停止：{jobs} 个下载任务，{prompts} 个待处理操作。")
        }
        "Output format" => Some("输出格式"),
        "ℹ️ Album & artist info" => Some("ℹ️ 专辑与艺人信息"),
        "⚙ Advanced" => Some("⚙ 更多设置"),
        "<b>Advanced settings</b>" => Some("<b>更多设置</b>"),
        "on" => Some("开启"),
        "off" => Some("关闭"),
        "Usage: <code>/normalize on|off</code> (ffmpeg loudnorm, opt-in)." => {
            Some("用法：<code>/normalize on|off</code>（ffmpeg loudnorm，需手动开启）。")
        }
        "Admin only. Usage: <code>/groupmode on|off</code>" => {
            Some("仅限管理员。用法：<code>/groupmode on|off</code>")
        }
        "✅ Group mode enabled." => Some("✅ 群组模式已开启。"),
        "🛑 Group mode disabled." => Some("🛑 群组模式已关闭。"),
        "🔎 Searching public playlists…" => Some("🔎 正在搜索公开歌单…"),
        "Playlists" => Some("歌单"),
        "😕 No public playlists found." => Some("😕 未找到公开歌单。"),
        "Usage: <code>/playlist &lt;name&gt;</code> — search public playlists (Deezer). Apple Music links already work as album URLs." => {
            Some(
                "用法：<code>/playlist &lt;name&gt;</code> — 搜索公开歌单（Deezer）。Apple Music 链接已可作为专辑链接使用。",
            )
        }
        "Usage: <code>/broadcast &lt;text&gt;</code>" => {
            Some("用法：<code>/broadcast &lt;text&gt;</code>")
        }
        "Broadcast complete" => Some("广播完成"),
        "Usage: <code>/genres [5..20] [week|month|3m|6m|year|all]</code>" => {
            Some("用法：<code>/genres [5..20] [week|month|3m|6m|year|all]</code>")
        }
        "<b>Settings</b>" => Some("<b>设置</b>"),
        "Quality" => Some("音质"),
        "Language" => Some("语言"),
        "Search source (inline)" => Some("搜索来源（内联）"),
        "Tags (album + genres)" => Some("标签（专辑 + 流派）"),
        "Last.fm" => Some("Last.fm"),
        "not connected (setup: /lastfm)" => Some("未连接（设置：/lastfm）"),
        "HQ cover (1000px)" => Some("HQ 封面（1000px）"),
        "Last.fm stats" => Some("Last.fm 统计"),
        "Genre analyzer" => Some("流派分析器"),
        "🏷 Tags: album + genre" => Some("🏷 标签：专辑 + 流派"),
        "🎛 Genre analyzer" => Some("🎛 流派分析器"),
        "📊 Genre chart" => Some("📊 流派图表"),
        "🎛 Enable the genre analyzer in /settings first." => {
            Some("🎛 请先在 /settings 中开启流派分析器。")
        }
        "No genre data yet — download a few tracks with the analyzer enabled." => {
            Some("暂无流派数据 —— 请先开启分析器下载几首歌曲。")
        }
        "Could not build the genre chart." => Some("无法生成流派图表。"),
        "Your genres" => Some("你的流派"),
        "{listens} listens · {genres} genres" => Some("{listens} 次播放 · {genres} 个流派"),
        "Usage: /genres [5..20] — number of bars." => Some("用法：/genres [5..20] — 柱子数量。"),
        "📊 Last.fm stats + collages" => Some("📊 Last.fm 统计 + 拼贴图"),
        "🎵 Last.fm is not configured on this bot yet." => Some("🎵 本机器人尚未配置 Last.fm。"),
        "🎵 Last.fm connected as <b>{nick}</b>. In inline mode with an empty query the bot will now suggest what you're playing.\nDisconnect: /lastfm off" => {
            Some(
                "🎵 Last.fm 已连接为 <b>{nick}</b>。在内联模式下使用空查询，机器人现在会推荐你正在播放的歌曲。\n断开连接：/lastfm off",
            )
        }
        "🎵 Last.fm: not connected.\nConnect: /lastfm &lt;your nickname&gt; — inline empty query will suggest what's playing now." => {
            Some(
                "🎵 Last.fm：未连接。\n连接：/lastfm &lt;你的昵称&gt; — 空的内联查询将推荐你正在播放的歌曲。",
            )
        }
        "🔌 Last.fm disconnected." => Some("🔌 Last.fm 已断开连接。"),
        "⚠️ Last.fm says there is no such user. Check the nickname spelling at last.fm." => {
            Some("⚠️ Last.fm 表示不存在该用户。请检查昵称在 last.fm 上的拼写。")
        }
        "✅ Connected as <b>{nick}</b>, but no recent tracks yet. Make sure your player scrobbles to Last.fm and your profile is public." => {
            Some(
                "✅ 已连接为 <b>{nick}</b>，但暂无最近的歌曲。请确保你的播放器向 Last.fm 记录播放，且个人资料为公开。",
            )
        }
        "✅ Connected as <b>{nick}</b>. Now playing: <b>{track}</b>. In inline mode, an empty query will propose it." => {
            Some(
                "✅ 已连接为 <b>{nick}</b>。正在播放：<b>{track}</b>。在内联模式下，空查询将推荐这首歌。",
            )
        }
        "⚠️ Invalid Last.fm username (2–15 latin letters, digits, _ or -)." => {
            Some("⚠️ Last.fm 用户名无效（2–15 个拉丁字母、数字、_ 或 -）。")
        }
        "▶ Now playing: {track}" => Some("▶ 正在播放：{track}"),
        "FLAC (lossless)" => Some("FLAC（无损）"),
        "🖼 HQ cover (1000px)" => Some("🖼 HQ 封面（1000px）"),
        "⚠️ Your chosen search source is blocked by the server config — the bot falls back to YouTube. Ask the operator to enable it (SEARCH_PROVIDERS)." => {
            Some(
                "⚠️ 你选择的搜索来源被服务器配置屏蔽 —— 机器人将回退到 YouTube。请联系运营者启用它（SEARCH_PROVIDERS）。",
            )
        }
        "✅ Settings saved." => Some("✅ 设置已保存。"),
        "🇬🇧 English" => Some("🇬🇧 英语"),
        "🌐 Language" => Some("🌐 语言"),
        "<b>Language</b>\nPick the interface language:" => Some("<b>语言</b>\n选择界面语言："),
        "« Back" => Some("« 返回"),
        "⬇️ Downloading… this usually takes a few seconds." => {
            Some("⬇️ 正在下载…通常需要几秒钟。")
        }
        "🔎 Searching…" => Some("🔎 正在搜索…"),
        "Nothing found. Try a different spelling or paste a direct link." => {
            Some("未找到结果。请尝试其他拼写或粘贴直接链接。")
        }
        "🚫 You are not allowed to use this bot." => Some("🚫 你不可以使用本机器人。"),
        "⛔ Admins only." => Some("⛔ 仅限管理员。"),
        "⏳ Too many requests. Limit: {per_minute} per minute." => {
            Some("⏳ 请求过于频繁。限制：每分钟 {per_minute} 次。")
        }
        "ℹ️ Send links and search queries in a private chat with me. Commands work here too." => {
            Some("ℹ️ 请在与我的私聊中发送链接和搜索查询。命令在这里同样可用。")
        }
        "🔒 This track is private or requires signing in, so I cannot download it." => {
            Some("🔒 这首歌是私密的或需要登录，因此我无法下载。")
        }
        "🌍 This track is region locked and is not available from the bot's location." => {
            Some("🌍 这首歌有地区限制，在机器人所在位置不可用。")
        }
        "⏱ The track is too long. Maximum duration is {} minutes." => {
            Some("⏱ 歌曲过长。最长时长为 {} 分钟。")
        }
        "📦 The file is too large. Maximum size is {} MB." => {
            Some("📦 文件过大。最大大小为 {} MB。")
        }
        "🔍 I could not find that track on the available sources." => {
            Some("🔍 在可用来源中找不到这首歌。")
        }
        "🤷 This link is not supported, or it needs a paid/DRM-protected source. Try a direct track link or search by name." => {
            Some(
                "🤷 该链接不受支持，或需要付费/受 DRM 保护的来源。请尝试直接歌曲链接或按名称搜索。",
            )
        }
        "🚫 {} is disabled on this bot." => Some("🚫 {} 在本机器人上已禁用。"),
        "📡 The source is temporarily unavailable. Please try again in a moment." => {
            Some("📡 来源暂时不可用。请稍后再试。")
        }
        "🎧 I downloaded the track but could not prepare the audio file." => {
            Some("🎧 已下载歌曲，但无法准备音频文件。")
        }
        "⚠️ Something went wrong while preparing the track. Please try again." => {
            Some("⚠️ 准备歌曲时出现问题。请重试。")
        }
        "💿 <b>{title}</b>\n{count} track(s). Download the whole collection?" => {
            Some("💿 <b>{title}</b>\n共 {count} 首歌曲。下载整个合集吗？")
        }
        "⬇ Download all" => Some("⬇ 下载全部"),
        "Cancel" => Some("取消"),
        "Cancelled." => Some("已取消。"),
        "💿 Loading the album…" => Some("💿 正在加载专辑…"),
        "💿 <b>{title}</b>\n{count} track(s) queued." => {
            Some("💿 <b>{title}</b>\n已将 {count} 首歌曲加入队列。")
        }
        "💿 {current}/{total} — {title}" => Some("💿 {current}/{total} — {title}"),
        "✅ Album finished: {ok} sent, {failed} failed." => {
            Some("✅ 专辑下载完成：成功发送 {ok} 首，失败 {failed} 首。")
        }
        "This album or playlist has no downloadable tracks." => {
            Some("该专辑或歌单中没有可下载的歌曲。")
        }
        " and {extra} more" => Some(" 等另外 {extra} 首"),
        "⚠ Failed: {list}{more}.\nSend /alb again — the finished tracks will come from cache instantly." => {
            Some("⚠ 失败：{list}{more}。\n再次发送 /alb — 已完成的歌曲将立即从缓存中获取。")
        }
        "Usage: /alb &lt;album name&gt;\nExample: /alb Master of Puppets" => {
            Some("用法：/alb &lt;专辑名称&gt;\n示例：/alb Master of Puppets")
        }
        "🔎 Searching albums…" => Some("🔎 正在搜索专辑…"),
        "<b>Albums for</b> «{query}»\nPick one:" => {
            Some("<b>专辑搜索结果</b> «{query}»\n请选择一个：")
        }
        "No albums found. Try a different artist or album title." => {
            Some("未找到专辑。请尝试其他艺人或专辑名称。")
        }
        " · {count} tracks" => Some(" · {count} 首歌曲"),
        "<b>Stats</b>\nCached tracks: <b>{}</b>\nKnown users: <b>{}</b>\nSuccessful sends: <b>{}</b>\nUptime: <b>{uptime}</b>" => {
            Some(
                "<b>统计</b>\n缓存歌曲数：<b>{}</b>\n已知用户数：<b>{}</b>\n成功发送数：<b>{}</b>\n运行时间：<b>{uptime}</b>",
            )
        }
        "🆕 <b>Bot update</b>\n\n{notes}\n\n💬 Developer channel: @maslougazIA" => {
            Some("🆕 <b>机器人更新</b>\n\n{notes}\n\n💬 开发者频道：@maslougazIA")
        }
        "🗑 Purged <code>{key}</code> from cache." => {
            Some("🗑 已从缓存中清除 <code>{key}</code>。")
        }
        "That key is not in the cache." => Some("该键不在缓存中。"),
        "🚫 User {user_id} banned." => Some("🚫 用户 {user_id} 已被封禁。"),
        "✅ User {user_id} unbanned." => Some("✅ 用户 {user_id} 已被解封。"),
        "Usage: /purge &lt;cache key or track URL&gt;" => {
            Some("用法：/purge &lt;缓存键或歌曲 URL&gt;")
        }
        "⌛ This result has expired. Please search again." => {
            Some("⌛ 该结果已过期。请重新搜索。")
        }
        "✅ Sent: {title}" => Some("✅ 已发送：{title}"),
        "🔎 Results for “{query}” ({count}):" => Some("🔎 “{query}” 的搜索结果（{count}）："),
        "Usage: /ban &lt;numeric user id&gt;" => Some("用法：/ban &lt;用户数字 id&gt;"),
        "Open bot" => Some("打开机器人"),
        "📩 Sent to your private messages with the bot." => {
            Some("📩 已发送到你与该机器人的私聊中。")
        }
        "⏳ Preparing the track…" => Some("⏳ 正在准备歌曲…"),
        "Send /import followed by a playlist (Artist – Title lines), or just /import and then send the file or the list in the next message." => {
            Some(
                "发送 /import 并附上歌单（“艺人 – 标题”行），或仅发送 /import，然后在下一条消息中发送文件或列表。",
            )
        }
        "📥 Waiting for the playlist. Send a CSV/TXT/JSON file (Spotify, TuneMyMusic, Soundiiz export) or paste lines like <code>Artist – Title</code>. Send <code>cancel</code> to abort." => {
            Some(
                "📥 正在等待歌单。请发送 CSV/TXT/JSON 文件（Spotify、TuneMyMusic、Soundiiz 导出）或粘贴形如 <code>Artist – Title</code> 的行。发送 <code>cancel</code> 以中止。",
            )
        }
        "🛑 Playlist import cancelled." => Some("🛑 歌单导入已取消。"),
        "🔎 Matching {count} imported tracks…" => Some("🔎 正在匹配 {count} 首导入的歌曲…"),
        "No tracks found in that playlist." => Some("在该歌单中未找到歌曲。"),
        "Too many tracks; import at most {max} at a time." => {
            Some("歌曲过多；每次最多导入 {max} 首。")
        }
        "Could not read that playlist file." => Some("无法读取该歌单文件。"),
        "📊 Enable Last.fm stats in /settings first." => {
            Some("📊 请先在 /settings 中启用 Last.fm 统计。")
        }
        "Connect your public Last.fm nickname with /lastfm <nick> first." => {
            Some("请先使用 /lastfm <nick> 连接你的公开 Last.fm 昵称。")
        }
        "Usage: /profile [7d|1m|3m|6m|1y|all]" => Some("用法：/profile [7d|1m|3m|6m|1y|all]"),
        "Usage: /collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]" => {
            Some("用法：/collage <tracks|albums|artists> [3x3..10x10] [7d|1m|3m|6m|1y|all]")
        }
        "📊 <b>Last.fm: {name}</b>\nScrobbles (all time): <b>{playcount}</b>\nLibrary (all time): {tracks} tracks · {albums} albums · {artists} artists" => {
            Some(
                "📊 <b>Last.fm：{name}</b>\n记录播放数（全部时间）：<b>{playcount}</b>\n音乐库（全部时间）：{tracks} 首歌曲 · {albums} 张专辑 · {artists} 位艺人",
            )
        }
        "\nCountry: {country}" => Some("\n国家：{country}"),
        "\n\n<b>Top tracks ({period}):</b>" => Some("\n\n<b>热门歌曲（{period}）：</b>"),
        "🎨 Generating collage…" => Some("🎨 正在生成拼贴图…"),
        "Could not build the collage (Last.fm returned no data)." => {
            Some("无法生成拼贴图（Last.fm 未返回数据）。")
        }
        "⏳ Downloading: {track}" => Some("⏳ 正在下载：{track}"),
        "<b>Listening history</b> · {period}\n\n" => Some("<b>收听历史</b> · {period}\n\n"),
        "Nothing in this period yet — send a track first." => {
            Some("该时间段暂无记录 —— 请先发送一首歌曲。")
        }
        "\n… and {hidden} earlier\n" => Some("\n… 以及更早的 {hidden} 条\n"),
        "Usage: <code>/history [7d|1m|3m|6m|1y|all]</code>" => {
            Some("用法：<code>/history [7d|1m|3m|6m|1y|all]</code>")
        }
        "Genre dynamics" => Some("流派动态"),
        "{cells} genre-points · {buckets} buckets · {period}" => {
            Some("{cells} 个流派点 · {buckets} 个时间区间 · {period}")
        }
        "No timed genre data yet. Enable the analyzer in /settings and download a few tracks." => {
            Some("暂无带时间的流派数据。请在 /settings 中启用分析器并下载几首歌曲。")
        }
        "Could not build the dynamics chart." => Some("无法生成动态图表。"),
        "Usage: <code>/dynamics [7d|1m|3m|6m|1y|all]</code>" => {
            Some("用法：<code>/dynamics [7d|1m|3m|6m|1y|all]</code>")
        }
        "Usage:\n<code>/digest daily|weekly</code> — enable (default 09:00 UTC)\n<code>/digest quiet HH-HH|off</code> — quiet hours in UTC\n<code>/digest status</code> — current schedule\n<code>/digest off</code> — disable" => {
            Some(
                "用法：\n<code>/digest daily|weekly</code> — 启用（默认 09:00 UTC）\n<code>/digest quiet HH-HH|off</code> — 以 UTC 表示的静默时段\n<code>/digest status</code> — 当前计划\n<code>/digest off</code> — 禁用",
            )
        }
        "✅ {period} digest enabled. Next: <b>{next}</b> UTC." => {
            Some("✅ 已启用 {period} 摘要。下次发送：<b>{next}</b> UTC。")
        }
        "🛑 Digest disabled." => Some("🛑 摘要已禁用。"),
        "🌙 Quiet hours: <b>{window}</b> UTC." => Some("🌙 静默时段：<b>{window}</b> UTC。"),
        "📅 Digest: <b>{state}</b>\nPeriod: {period}\nNext: {next} UTC\nQuiet hours: {quiet} UTC" => {
            Some(
                "📅 摘要：<b>{state}</b>\n周期：{period}\n下次发送：{next} UTC\n静默时段：{quiet} UTC",
            )
        }
        "No digest is enabled. Start with <code>/digest daily</code>." => {
            Some("未启用任何摘要。请先使用 <code>/digest daily</code>。")
        }
        "none" => Some("无"),
        "daily" => Some("每日"),
        "weekly" => Some("每周"),
        "📊 Your digest" => Some("📊 你的摘要"),
        "Tracks" => Some("歌曲数"),
        "Top tracks" => Some("热门歌曲"),
        _ => None,
    }
}
