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

## Ops incident (2026-11-16): yt-dlp exits 127 inside the container

Symptom: every download failed with `yt-dlp failed status=exit status: 127` and stderr
`error while loading shared libraries: libz.so.1: failed to map segment from shared object`.

Root cause: the pinned `yt-dlp` binary is a PyInstaller onefile bundle. On start it extracts
itself to a temp dir and `mmap`s its bundled `libz.so.1` (and friends) from there. The compose
stack mounts `/tmp` as tmpfs with `noexec` (hardening default), so those mappings fail — the
error names `libz.so.1` but the faulting object is the extracted bundle's library, not the
system one. The image itself was verified good: running yt-dlp with an exec-allowed temp dir
(e.g. `docker run -v bot-data:/data -e TMPDIR=/data/tmp`) prints `2026.10.26` successfully.

Fix (defense in depth, both layers):
- `Dockerfile`: `ENV TMPDIR=/data/tmp` — image works out of the box on any runtime.
- `docker-compose.yml`: `TMPDIR: /data/tmp` on the bot service + comment explaining why — overrides
  the runtime env and protects against accidental future `tmpfs /tmp` re-adds.
Confirmed: container recreated, `yt-dlp --version` → `2026.10.26`, `/healthz` → `{"status":"ok"}`,
bot reconnected to Telegram as `MaslougazeMusicBot`.

Related ops hygiene: added `.dockerignore` (`target/`, `.git/`, `.env`, `data/`). Previously the
build context was ~820 MB (mostly `target/`), which made `docker compose build` look hung —
it was still transferring the context. Image build now runs in seconds.

Also: `music_bot_requests_total` was 2 and `music_bot_request_seconds` recorded ~0.46 s avg
on the first two user messages (pre-fix, downloads failing fast). Real counter data only appears
after traffic; `/metrics` renders counters lazily — a fresh container shows none of them.

## Ops incident (2026-11-16): inline uncached track lands in private chat, not the group

Symptom: user picks an uncached inline result ("Завантажую: …" placeholder); the audio is
delivered to the user's private chat with the bot and the placeholder stays "Завантажую:"
forever. Logs: `Bad Request: invalid message content specified` on editMessageMedia.

Root causes:
1. `STORAGE_CHAT_ID` was effectively empty at runtime (the value in the env file is not a
   clean numeric chat id after trimming — consistent with `STORAGE_CHAT_ID=` / CRLF in a
   Windows env file; compose passes the trailing CR, `env_optional` trims to empty, storage
   is silently disabled). With no storage chat there is no way to mint a `file_id` for an
   inline message.
2. Code unconditionally tried `editMessageMedia` with a freshly uploaded file for the inline
   message. Bot API forbids that: "When an inline message is edited, a new file can't be
   uploaded; use a previously uploaded file via its file_id or specify a URL." (text→media
   replacement itself IS allowed). Telegram rejected it, `fallback_private` sent the audio to
   the user in DM, and the placeholder was never updated.

Fix (code):
- `inline.rs`: no more blind upload-edit for inline messages; without a storage chat the track
  goes straight to private delivery. The inline placeholder is edited to
  `i18n::sent_to_private` ("Трек надіслано вам в особисті повідомлення з ботом") so the group
  doesn't keep a stale "Завантажую:". Storage file_id-edit failures are now logged (warn)
  instead of swallowed by `let _ =`.

Action for the operator: set `STORAGE_CHAT_ID` in the env file to a real numeric chat id of a
channel where the bot is an admin (e.g. `STORAGE_CHAT_ID=-100xxxxxxxxxx`, no trailing
spaces/CR) and recreate the container (`docker compose up -d --force-recreate`). Then uncached
inline picks are uploaded to the storage channel, the placeholder is swapped to the audio in
the group, and the `file_id` is cached for instant inline replay. Verify the storage channel
receives the audio and `/metrics` shows the upload.

## Follow-up (2026-11-16): inline audio works without a storage channel

Inline music bots can usually drop audio into any chat because they mint a `file_id` somewhere
first (upload to a storage channel) and then `editMessageMedia` the inline message — membership
in the target chat is never required for inline edits. This bot now does the same thing even
without `STORAGE_CHAT_ID`: the cold upload target falls back to the user's own private chat
with the bot (guaranteed reachable once they started it), the resulting `file_id` is cached,
and the group's placeholder is edited to the audio. The user's PM then holds a copy of the
track (it is the only durable holder of that `file_id`; do not delete it). Side effect: the
PM copy stays until the user deletes it — set `STORAGE_CHAT_ID` to a storage channel to avoid
the duplicate in DM. No public HTTP URL is needed (an `audio_url`-based inline result was
rejected: the bot host has no public address Telegram clients can fetch).

## Ops (2026-11-16): deno pinned as yt-dlp JS runtime (EJS)

Symptom: yt-dlp 2026.10.26 warns `No supported JavaScript runtime could be found`; YouTube
extraction without a JS runtime is deprecated and formats may go missing.

Fix:
- `Dockerfile`: pinned `deno` v2.9.8 (`deno-x86_64-unknown-linux-gnu.zip`, sha256-verified,
  same pattern as yt-dlp) into `/usr/local/bin`. `unzip` added to the apt install line.
- `DENO_DIR=/data/tmp/deno` in `Dockerfile` and `docker-compose.yml`: deno caches compiled
  scripts; the container root fs is `read_only`, so the cache must live on the `bot-data`
  volume. It is NOT swept by the temp janitor (only `track-*` directories are), so the
  cache persists across downloads.
- yt-dlp discovers deno in PATH by default (no `--js-runtimes` flag needed).

Confirmed: image rebuilt, container recreated healthy, bot reconnected to Telegram,
`yt-dlp --skip-download` on a YouTube URL now prints the title with no EJS warning, and a
full `bestaudio[ext=m4a]/bestaudio` download inside the container succeeds (valid M4A,
`DENO_DIR` cache created on the volume).
