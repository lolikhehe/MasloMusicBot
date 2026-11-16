# MasloMusicBot

A fast Telegram music bot in Rust. Send a link or `Artist — Title` in a
private chat and the bot replies with the track (title, performer, duration,
cover art). Use `@your_bot <query>` inline to drop audio into any chat.

> This is not a streaming service or library. It stores only metadata and Telegram
> `file_id` identifiers — audio bytes stay in Telegram's servers. Providers with
> DRM (Spotify, Apple Music, Tidal) are used solely as public metadata lookups to
> find the same track on open sources; nothing is unlocked or bypassed.

## Quick start

1. Copy `.env.example` to `.env` and fill in `TELOXIDE_TOKEN` (from [@BotFather](https://t.me/BotFather))
   and `ADMINS` (your numeric Telegram user ID(s)). Leave everything else at defaults.

2. Run:

   ```sh
   docker compose up -d
   ```

3. Message your bot privately. Done.

By default the bot talks to the public Telegram Bot API (uploads up to 50 MB). To lift that to
2000 MB and serve files locally, enable the `local` profile (see **Local Bot API migration** below).

### BotFather setup (inline mode)

Inline results only work after two one-time commands in [@BotFather](https://t.me/BotFather):

1. `/setinline` — select your bot, set a placeholder, e.g. `Search a track…`
2. `/setinlinefeedback` — select your bot, set **100%**

Without `/setinlinefeedback` the bot never receives `chosen_inline_result`, so inline picks would
not be editable into messages.

## Commands

| Command | Who | Description |
|---|---|---|
| `/start` | everyone | Disclaimer and quick intro |
| `/help` | everyone | How to use the bot |
| `/settings` | everyone | Language (EN/UK) and audio quality (128–320 kbps) |
| `/stats` | admins | Tracks cached, users seen, uptime |
| `/ban <id>` / `/unban <id>` | admins | Block/restore a user |
| `/purge <key or URL>` | admins | Drop a cached track (e.g. after a rights complaint) |

In a private chat:

- Paste a link — YouTube, YouTube Music, SoundCloud, Bandcamp, Spotify, Apple Music, Deezer
- Send `Artist — Title` — best match from YouTube
- Send an album/playlist link — the bot first asks with an inline keyboard
  ("Download all N tracks? ✅/✖", cap `MAX_COLLECTION_TRACKS`, default 25) and then downloads
  with 2–3 parallel tasks, editing one progress message at most once per 2 s
- Tap a search result to download

In any chat, `@your_bot Artist — Title` offers candidates inline; picking one posts the audio in that
chat. Cached tracks post instantly; uncached tracks download and are edited into the message.

## Configuration

All settings come from environment variables (see `.env.example`). Highlights:

- `ENABLED_PROVIDERS` — comma-separated, or `all` (`youtube,youtube_music,soundcloud,bandcamp,spotify,apple_music,deezer`)
- `SEARCH_PROVIDERS` — `youtube,soundcloud,deezer` (anonymous search allowed for the bot; restrict e.g. to `youtube` to pin old behavior). Order is irrelevant; per-user choice wins. Search is two-tier cached (moka L1 + SQLite `search_cache` L2) keyed per source, and inline results carry no new file uploads.
- `MAX_DURATION_SECS` (default 1200), `MAX_FILE_MB` (default 100, up to 2000)
- `MAX_COLLECTION_TRACKS` (default 25), `MAX_ACTIVE_DOWNLOADS_PER_USER` (default 2)
- `RATE_LIMIT_PER_MIN` (default 10), `MAX_CONCURRENT_DOWNLOADS` (default 4)
- `METRICS_ADDR` — serves `/metrics` and `/healthz` (Prometheus format; empty or `off` disables)
- `STORAGE_CHAT_ID` — optional chat used to cache inline `file_id`s (see below)
- `DEFAULT_LANG` — `en` (default) or `uk`; without it, new users inherit Telegram `language_code`
- `LOG_FORMAT=json` for structured logs; `LOG_LEVEL` for verbosity
- `DATABASE_URL` — `sqlite:///path/to/bot.db` (default `sqlite://data/bot.db`)

The Docker image caps `mem_limit: 2g`, `pids_limit: 512`, drops all capabilities, runs as uid 10001,
and starts with a read-only rootfs + `tmpfs /tmp`.

## Local Bot API migration (one-time, optional)

The bundled local server raises the upload ceiling to 2000 MB and shares a filesystem with the bot.
You need `TELEGRAM_API_ID` and `TELEGRAM_API_HASH` from <https://my.telegram.org>.

1. **Migrate the bot off the cloud API first** — the cloud and local endpoints are mutually exclusive
   for a given bot for a while. While the bot is stopped or scaled down, call
   [`logOut`](https://core.telegram.org/bots/api#log-out):

   ```sh
   curl -s "https://api.telegram.org/bot${TELOXIDE_TOKEN}/logOut"
   ```

   Expect `{"ok":true,"result":true}`. The cloud API will not work for this bot for some minutes afterwards.

2. Put the credentials in `.env`:

   ```sh
   TELEGRAM_API_ID=123456
   TELEGRAM_API_HASH=0123456789abcdef0123456789abcdef
   TELEGRAM_API_URL=http://bot-api:8081/
   ```

3. Start both services:

   ```sh
   docker compose --profile local up -d
   ```

4. Health is visible via `/metrics` on port 9090 — the `/healthz` endpoint confirms the process and SQLite.

If you skip all this, `docker compose up -d` runs the bot against the public API and everything works
except uploads larger than 50 MB.

## yt-dlp upgrades

The yt-dlp binary is pinned and sha256-verified in `Dockerfile`:

```dockerfile
ARG YTDLP_VERSION=2026.10.26
ARG YTDLP_SHA256=767ba9dce7052aa196f8b12652f433d76de128ba168966fa84b6394d5e6c6795
```

To upgrade: pick the new tag on <https://github.com/yt-dlp/yt-dlp/releases/latest>, fetch its sha256
(`curl -s https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest | jq -r '.assets[] | select(.name=="yt-dlp_linux").digest'`),
update both args, then `docker compose build --no-cache bot`. Newer yt-dlp fixes the frequent
"YouTube extraction failed" breakage; the bot needs no rebuild for anything else.

## How it works

- **Fast path**: parse the URL offline → `track_key` → SQLite lookup → send by Telegram `file_id`.
  Repeat requests take milliseconds, no network needed.
- **Cold path**: `yt-dlp` metadata → in-flight dedup → bounded download to a temp directory →
  tags + cover via `ffmpeg`/`lofty` → uploaded → `file_id` stored → temp files deleted by the janitor.
- Audio is kept as m4a/AAC (no re-encode when possible) for speed.
- Concurrent cold requests for the same track share one download (one semaphore global permit + broadcast).
- Cold **inline** plays are not cached without `STORAGE_CHAT_ID` (Telegram's `editMessageMedia` does
  not return a `file_id`); with it, the cold file is sent to the storage chat to obtain a cached
  `file_id`. Never delete messages in that chat — the cached ids die with them.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo deny check
```

Layout: `crates/{common,storage,providers,downloader,audio,telegram,app}` — see `docs/decisions.md`.
The crate split mirrors the spec's §7.1 dependency direction; `common` serves everything else.

### Troubleshooting

- **Bot doesn't start**: check `TELOXIDE_TOKEN`; `docker compose logs -f bot`.
- **"source unavailable"**: yt-dlp may be blocked or YouTube may rate-limit bots from your IP
  (PO tokens). Try again later, or raise `MAX_DURATION_SECS`/refresh yt-dlp.
- **Inline picker does nothing**: run `/setinline` and `/setinlinefeedback` in BotFather (see above).
- **Downloads fail for everything**: verify network egress and that `YTDLP_PATH`/`FFMPEG_PATH` are executable.
- **Slow first download, instant repeats**: that is the cache working as designed.
- **`docker compose up` shows env warnings**: `.env` is optional; create it to silence them, but never commit it.

See `docs/decisions.md`, `docs/ASSUMPTIONS.md`, `docs/perf.md` for the technical detail and the
honest list of what is measured and what is not.
