# Decisions & known limitations

## Spec truncation

The product spec (`C:\Users\pylyp\Downloads\Telegram-бот для завантаження музики на Rust технічне завдання для агента.md`)
is **truncated**: the file literally ends at section 6 (Non-functional requirements / stack
notes). Sections 7–14 referenced by the text (implementation stages, Docker packaging, CI,
metrics format details, tests & acceptance criteria, etc.) are missing, so the repository was
built from sections 1–6 plus the stack it names (teloxide, SQLite via sqlx, moka, lofty,
Prometheus metrics, Docker compose). Any requirement living in the missing sections is
therefore not implemented by definition.

## Spec vs. implementation

- **Language**: implemented in English + Ukrainian, switched via `/settings`.
- **Albums/playlists**: YouTube/YouTube Music (incl. `music.youtube.com`), SoundCloud sets and
  Bandcamp albums are expanded via yt-dlp's flat playlist output, capped at `MAX_ALBUM_TRACKS`.
  Deezer albums/playlists and Apple Music album collections are expanded via their public
  JSON APIs and matched individually on open sources via `ytsearch1:`. **Spotify albums and
  playlists are refused** — Spotify exposes no public, stable API for listing another
  user's saved/owned collections without OAuth, and the tracklist itself is metadata we
  should not scrape at that scale. Single Spotify tracks work: the public oEmbed endpoint
  supplies title/artist/thumbnail, then the same track is matched on YouTube.
- **Search provider**: only YouTube (`ytsearch`), gated by `SEARCH_PROVIDERS`.
- **Inline mode**: cached tracks are sent instantly from SQLite/`file_id`. Cold inline plays
  download and are edited into the message via `editMessageMedia`, but Telegram does not
  return a `file_id` for inline-edited media, so **cold inline plays are not cached**. To
  preserve the fast path, inline answers also show a URL button ("Open bot") that lets the
  user open the bot privately, where the send caches normally.
- **Covers**: fetched over HTTPS with a size cap; converted to ≤320 px JPEG with ffmpeg
  (`ffmpeg` is already required by yt-dlp for m4a/AAC extraction).
- **No re-encode**: when yt-dlp can produce m4a/AAC directly we keep it; ffmpeg is only used
  for cover scaling and for falling back to AAC conversion when needed.
- **file_id cache**: stored in SQLite with a `file_id` + `file_unique_id` pair, per
  `track_key` (offline-parseable key such as `youtube:<video_id>`). Cache hits are sent
  without any network round-trip except to Telegram itself.
- **In-flight dedup**: concurrent requests for the same `track_key` share one download via a
  broadcast channel; waiters receive either the prepared audio or a clone of the error.
- **Invalid cached file_ids**: on a Telegram "wrong file identifier" error during a cache
  send, the entry is purged and the track is downloaded once, then re-cached.
- **Rate limiting**: sliding-window per user (`RATE_LIMIT_PER_MIN`), checked on commands,
  messages, inline queries and callback queries.
- **Legal**: no DRM/paid-content bypass of any kind; `/start` shows a disclaimer; providers
  are toggleable; admin `/purge` removes a cached `file_id` by key or URL.

## Crate / toolchain notes

- Rust 1.99 / edition 2024. Pin exact minimum versions in `Cargo.toml`; `Cargo.lock` is committed.
- **teloxide 0.17 / teloxide-core 0.13**: `Message::from()` was deprecated in favor of the
  public `from: Option<User>` field. `Dispatcher::dispatch(&mut self)`;
  `Bot::set_api_url(reqwest::Url)` consumes and returns `Self`.
- **metrics-exporter-prometheus 0.18**: `PrometheusBuilder::with_http_listener(addr).install()`
  returns `Result<()>` (no handle). `install_recorder()` is the handle-returning variant; the
  bot uses the plain `install()` and the default exporter registry.
- **lofty 0.25**: tags are written with `TagExt::save_to_path` — `Mp4Ilst` for m4a/mp4,
  `Id3v2` for mp3, `VorbisComments` for flac/ogg/opus. Tagging failures are logged, not fatal.
- **sqlx 0.9**: SQLite via `sqlite` feature (bundled SQLite), `SqliteConnectOptions` with
  `journal_mode=WAL`, `busy_timeout=10s`, `foreign_keys=on`, `create_if_missing`. The schema
  is embedded with `include_str!("../../migrations/0001_init.sql")` and executed with
  `sqlx::raw_sql` (no migration bookkeeping needed for a single initial schema).
- **Process isolation**: yt-dlp is invoked as a process (`tokio::process::Command`) with
  an argv array — never a shell — with `--ignore-config`, `--no-playlist`, timeouts and
  `kill_on_drop`. No user input is ever interpolated into a command line.
- **mimalloc** is the global allocator as requested by the spec.

## Known non-goals / risks

- No Redis yet: SQLite WAL is enough for the spec's load ("tens initially, design for hundreds").
  If needed, the cache is behind the `cache::db` module and can be swapped.
- Quality setting applies at download time (kbps → `--audio-quality`); cached tracks are served
  at whatever quality they were first cached in.
- yt-dlp breakage is an operational fact of life: on extraction failures the bot reports
  "source unavailable" and admins should upgrade the binary in the Docker image.
- The bot-api container needs network access to Telegram's data centers; the bot itself needs
  egress to the chosen providers and to `t.me` if using the public API.
