# Decisions & known limitations

## Spec truncation (original) — now resolved

The Markdown spec at `C:\Users\pylyp\Downloads\Telegram-бот для завантаження музики на Rust технічне завдання для агента.md`
is **truncated** after section 6. The complete 25-page spec was later recovered from
`C:\Users\pylyp\Downloads\Telegram-бот для завантаження музики на Rust технічне завдання для агента.pdf` (extracted to
`C:\Users\pylyp\AppData\Local\Temp\opencode\spec.txt`). The recovered sections are:

- §7 АРХІТЕКТУРА КОДУ — workspace `crates/{common,storage,providers,downloader,audio,telegram,app}`,
  dependency direction `common ← everything`.
- §8 МОДЕЛЬ ДАНИХ — SQL schema (tracks with quality-aware PK, `search_cache`, users).
- §9 ПЛАН РОБІТ — етапи 1–11, milestones M1–M5, including the exact yt-dlp command shape,
  inline-mode `STORAGE_CHAT_ID` flow (этап 7), local Bot API migration (этап 8), Docker/cargo-chef
  (этап 9), `/metrics` + `docs/perf.md` (этап 10), CI (этап 11).
- §10 БЕЗПЕКА, §11 ПРАВИЛА ДЛЯ АГЕНТА, §12 РИЗИКИ, §13 ВІДКРИТІ ПИТАННЯ (defaults), §14 КРИТЕРІЇ ГОТОВНОСТІ.

Where this document contradicts the earlier truncation-era text, the full spec wins.

## Spec vs. implementation (current)

- **Layout**: §7.1 workspace `crates/*` (was a single crate). `common` has no project-internal
  dependencies; everything depends on it.
- **Cache key**: `track_key` keeps the source-prefixed offline key (e.g. `youtube:<id>`), but the
  `tracks` table PK is `(track_key, quality)` and records `size_bytes` — a deliberate deviation from
  §8 (`PRIMARY KEY(source, source_id, quality)`) to keep the existing offline key; quality is part of
  the key because different qualities produce different `file_id`s.
- **Language**: implemented in English + Ukrainian, switched via `/settings`. New users default from
  their Telegram `language_code` (`uk`/`ua`/`ru`/`be` → `uk`, `en` → `en`, otherwise `DEFAULT_LANG`),
  per §9 етап 6 ("мова з налаштувань, за відсутності — з language_code Telegram").
- **Albums/playlists**: YouTube/YouTube Music, SoundCloud sets and Bandcamp albums via yt-dlp flat
  playlists, capped at `MAX_COLLECTION_TRACKS` (default **25**, §9). Collections require a confirmation
  button ("Завантажити всі?") before download; delivery runs with 2–3 parallel tasks and one progress
  message edited at most once per 2 s (§9 етап 6). Deezer albums/playlists and Apple Music albums are
  expanded via public JSON APIs and each track is matched via `ytsearch1:`. **Spotify albums and
  playlists are refused** — no public API listing a user's collection without OAuth. Single Spotify
  tracks resolve via oEmbed metadata.
- **Matchers** (Spotify/Apple/Deezer → same track on an open source, duration ±3 s): **deferred to M5**
  per §13.3 default; never an audio bypass — metadata only.
- **Inline mode**: implemented per §9 етап 7 — cached tracks answer instantly (`InlineQueryResultCachedAudio`);
  uncached tracks answer articles with a URL keyboard to force `inline_message_id`; stale inline queries
  are cancelled by generation counter (`inline_generations`); `answerInlineQuery` uses `cache_time=30`,
  `is_personal=false`. When `STORAGE_CHAT_ID` is set, cold inline results are first **sent to the storage
  chat**, the resulting `file_id` is cached and used for `editMessageMedia` — inline audio messages then
  persist and the send is cached. **Storage chat messages must never be deleted** (cached `file_id` would
  die with them). Without `STORAGE_CHAT_ID`, cold inline plays are not cached (Telegram returns no
  `file_id` for inline-edited media).
- **yt-dlp command shape**: aligned to §9 етап 3 — `-f bestaudio[ext=m4a]/bestaudio`, `-N 4`,
  `--socket-timeout 15`, `--retries 3`, `--print after_move:filepath` (used to locate the output),
  `--` before the target, `--ignore-config`, per-invocation timeout, `kill_on_drop`. One extra
  exponential retry (2 s) on transient failures inside the manager.
- **Concurrency**: global semaphore (`MAX_CONCURRENT_DOWNLOADS`) + in-flight dedup keyed by
  `(track_key, quality)` (broadcast channel) + per-user cap on concurrently active downloads
  (`MAX_ACTIVE_DOWNLOADS_PER_USER`, default 2) + per-user sliding-window rate limit on handlers
  and callbacks. Cache-hit paths never enter the download semaphore (§7.3).
- **Health**: `/healthz` (returns 200 / `{"status":"ok"}` when `SELECT 1` succeeds, else 503) and
  `/metrics` are served by one minimal tokio HTTP listener on `METRICS_ADDR` (default `0.0.0.0:9090`;
  empty/`off` disables). Docker healthcheck curls `/healthz`; Prometheus scrapes `/metrics`.
  Counters/histograms/gauges: requests/cache_hits/downloads/download_errors/uploads,
  `music_bot_request_seconds`, `download_seconds`, `upload_seconds`, `inline_seconds`,
  `music_bot_downloads_inflight`, `music_bot_downloads_queued`.
- **Metrics note**: counters without a listener are still installed (`install_recorder`) so
  `metrics::counter!` never panics; nothing is served until the listener is bound.
- **Cache layers**: moka L1 (10 min TTL) → SQLite L2 (`search_cache` table, 10 min TTL) → yt-dlp.
  A `file_id` cache miss is the only time the download path runs.
- **Invalid cached file_ids**: on a Telegram "wrong file identifier" error, the entry is purged and
  the track re-downloaded once, then re-cached.
- **In-flight dedup**: waiters on the same `(track_key, quality)` share one download via broadcast;
  Leader failure is cloned to waiters. Closed channel → waiter retries as leader.

## Crate / toolchain notes

- Rust 1.99 / edition 2024. `Cargo.lock` committed; `cargo-chef` layer caching in Docker.
- **teloxide 0.17 / teloxide-core 0.13**: `msg.from` is `Option<User>`; `Message::from()` deprecated.
  `Dispatcher::dispatch(&mut self)`; graceful shutdown via `ShutdownToken::shutdown()` — `IdleShutdownError`
  is accepted and logged (empty dispatcher). `Bot::set_api_url` consumes+returns `Self`.
- **metrics-exporter-prometheus 0.18**: `PrometheusBuilder::install_recorder()` returns a handle used by
  the custom `/metrics` renderer; no `with_http_listener` needed.
- **lofty 0.25**: `TagExt::save_to_path`; `Mp4Ilst`/`Id3v2`/`VorbisComments`; failures logged, not fatal.
- **sqlx 0.9**: SQLite WAL + `synchronous=NORMAL`, 10 s busy timeout; schema embedded via
  `include_str!("../../../migrations/0001_init.sql")` executed with `sqlx::raw_sql` on connect.
- **yt-dlp**: invoked as a process (`tokio::process::Command`), argv array — never a shell — with
  `--ignore-config`, `--no-playlist`, timeouts and `kill_on_drop`. No user input is interpolated into
  a command line; URLs must parse as `http(s)` URLs recognized by a provider.
- **cargo-deny**: advisories/bans/licenses/sources green; unavoidable `paste` / `proc-macro-error2`
  (unmaintained, via lofty and teloxide→aquamarine) are explicitly ignored in `deny.toml`;
  `RUSTSEC-2026-0459` (libsqlite3-sys <0.38 via sqlx-sqlite 0.9) is accepted: the CVE's attack vector
  is a maliciously crafted SQLite database file, and ours are bot-owned single-tenant files. sqlx
  currently pins `libsqlite3-sys >=0.30.1 <0.38.0`; re-check on every sqlx upgrade.
- **mimalloc** is the global allocator.

## Known non-goals / risks

- No Redis (§6 load assumption); `Db` is the single persistence behind one module.
- Quality is not part of a user's settings migration: switching quality does not re-encode cached files
  (different key → stored separately on next cold download).
- yt-dlp breakage is an operational fact: the bot reports "source unavailable"; admins upgrade the
  pinned yt-dlp by editing `YTDLP_VERSION` and `YTDLP_SHA256` in the Dockerfile.
- Temporary-directory janitor: every 10 minutes, directories in `TMP_DIR` older than 30 minutes are
  deleted (startup sweep included). This bounds disk usage after crashes.
- `hits` on cache hits are written synchronously per send (§11 "пачками" batching deferred — trivial to
  add at the cost of losing precision on unclean shutdown).
- Docker healthcheck is only as good as `/healthz`: it verifies process + SQLite, not Telegram connectivity.
- The local Bot API server (`tdlib/telegram-bot-api`) is an **opt-in profile** (`local`) because this
  deployment has no `TELEGRAM_API_ID`/`TELEGRAM_API_HASH`; the bot runs against the public API by
  default with a 50 MB upload ceiling. See README for the migration steps.
