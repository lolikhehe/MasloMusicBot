# MasloMusicBot

A fast Telegram music bot in Rust. Send a link or `Artist — Title` in a
private chat and the bot replies with the track (title, performer, duration,
cover art). Use `@your_bot <query>` inline to drop audio into any chat.

> This is not a streaming service or library. It stores only metadata and Telegram
> `file_id` identifiers — audio bytes stay in Telegram's servers. Providers
> with DRM (Spotify, Apple Music, Tidal) are used solely as public metadata
> lookups to find the same track on open sources; nothing is unlocked or bypassed.

## Quick start

1. Copy `.env.example` to `.env` and fill in:

   - `TELOXIDE_TOKEN` — from [@BotFather](https://t.me/BotFather)
   - `TELEGRAM_API_ID` / `TELEGRAM_API_HASH` — from <https://my.telegram.org> (required by the bundled local Bot API server)
   - `ADMINS` — your numeric Telegram user ID(s)

2. Run:

   ```sh
   docker compose up -d
   ```

3. Message your bot privately. Done.

`docker compose` starts two services: a local `tdlib/telegram-bot-api` server
(which raises the upload limit to 2000 MB and keeps file uploads local) and the
`music-bot` itself. SQLite data and temp files live in the `bot-data` volume.

To run against the public Telegram API instead of the local server, leave
`TELEGRAM_API_URL` unset and run only the `bot` service (25 MB upload limit).

## Commands

| Command | Who | Description |
|---|---|---|
| `/start` | everyone | Disclaimer and quick intro |
| `/help` | everyone | How to use the bot |
| `/settings` | everyone | Language (EN/UK) and audio quality (128–320 kbps) |
| `/stats` | admins | Tracks cached, users seen, sends, uptime |
| `/ban <id>` / `/unban <id>` | admins | Block/restore a user |
| `/purge <key or URL>` | admins | Drop a cached track (e.g. after a rights complaint) |

In a private chat:

- Paste a link — YouTube, YouTube Music, SoundCloud, Bandcamp, Spotify, Apple Music, Deezer
- Send `Artist — Title` — best match from YouTube
- Send a playlist/album link — downloads up to `MAX_ALBUM_TRACKS` tracks
- Tap a search result to download

In any chat, `@your_bot Artist — Title` offers candidates inline; picking one
posts the audio in that chat.

## Configuration

All settings come from environment variables (see `.env.example`). Highlights:

- `ENABLED_PROVIDERS` — comma-separated, or `all` (`youtube,youtube_music,soundcloud,bandcamp,spotify,apple_music,deezer`)
- `SEARCH_PROVIDERS` — `youtube` (only supported search provider)
- `MAX_DURATION_SECS` (default 1200), `MAX_FILE_MB` (default 100, up to 2000), `MAX_ALBUM_TRACKS` (default 10)
- `RATE_LIMIT_PER_MIN` (default 10), `MAX_CONCURRENT_DOWNLOADS` (default 4)
- `METRICS_ADDR` — Prometheus `/metrics` listener; empty or `off` disables
- `DATABASE_URL` — `sqlite:///path/to/bot.db` (default `sqlite://data/bot.db`)
- `LOG_FORMAT=json` for structured logs; `LOG_LEVEL` for verbosity

## Legal & responsibility

You are responsible for what you download and share. Only use the bot for
content you own, or content that is Creative Commons / explicitly free to
distribute. The bot stores no copyrighted audio itself; everything flows
through the provider APIs and Telegram. DRM protections are never bypassed.
Set `ENABLED_PROVIDERS` to the subset you have the right to use, and use
`/purge <key>` to forget cached items on request.

## How it works

- **Fast path**: parse the URL offline → `track_key` → SQLite lookup → send by
  Telegram `file_id`. Repeat requests take milliseconds, no network needed.
- **Cold path**: `yt-dlp` metadata → in-flight dedup → bounded download to a
  temp directory → tags + cover via `ffmpeg`/`lofty` → uploaded through the
  local Bot API → `file_id` stored → temp files deleted.
- Audio is kept as m4a/AAC (no re-encode when possible) for speed.
- Concurrent cold requests for the same track share one download.
- Cold **inline** plays are not cached (Telegram's `editMessageMedia` does not
  return a `file_id`); if the track was already cached, inline sends it from
  the cache.

## Development

```sh
cargo test          # 25 unit tests (URL parsing, i18n, DB, sanitizing, commands)
cargo clippy --all-targets
cargo fmt --check
```

The code is organized as: `config` / `domain` / `error` / `i18n` / `state` /
`metrics` / `shutdown` — `cache::db` (SQLite) — `providers::{url, ytdlp, external}`
— `audio::tags` — `download::manager` (in-flight dedup + semaphore) —
`bot::{commands, handlers, callbacks, inline, send}` — `main`.

See `docs/decisions.md` for the technical decisions made while building this.

### Troubleshooting

- **Bot doesn't start**: check `TELOXIDE_TOKEN`; check logs (`docker compose logs -f bot`).
- **"source unavailable"**: yt-dlp may be blocked/rate-limited from your region or IP. Try again later; YouTube may require a PO token — see yt-dlp's docs.
- **Downloads fail for everything**: check network egress and that `YTDLP_PATH`/`FFMPEG_PATH` are executable (they are in the Docker image).
- **Slow first download, instant repeats**: that is the cache working as designed.
