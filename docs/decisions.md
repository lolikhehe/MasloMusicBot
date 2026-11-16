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

## Last.fm "now playing" inline proposal (2026-11-16)

- `LASTFM_API_KEY` (optional env) enables the feature; without it `/lastfm`
  reports "not configured" and inline empty query stays empty. Key is free at
  https://www.last.fm/api/account.
- Users connect with `/lastfm <public nick>` (never a password; only the public
  recent-tracks feed is read) / `/lastfm off` disconnects / `/lastfm` shows
  status. Nickname stored in `users.lastfm_user` (migration 0004, auto-applied).
- With Last.fm connected and the inline query **empty**, the bot lists the
  track currently (or most recently) playing as the single result; picking it
  goes through the normal inline pipeline (cache keyed `lastfm:<slug>`, tags
  and captions included, placeholder shows "▶ Зараз грає: …"). A normal text
  query behaves exactly as before.
- Failure mode is graceful: no key / Last.fm down / nick wrong → empty answer
  with a warn log, never an error to the user.
- Data minimization: no OAuth, no password, no listening-history persistence;
  only the nickname is stored.

## Response-time pass (2026-11-16, round 2)

- Inline search/describe now races the in-flight work against a per-user
  `tokio::sync::watch` generation channel: the moment the user keeps typing,
  the next keystroke aborts the running yt-dlp/HTTP search instead of letting
  it finish and discarding its output. Saves ~1-3 s of CPU per stale keystroke.
- `touch_track` (cache-hit bookkeeping) is spawned fire-and-forget; the user
  no longer waits for an extra SQLite WRITE before the reply.
- Cover download+resize now overlaps the audio download when the thumbnail URL
  is already known (search picks always carry one) instead of running after
  yt-dlp; capped at 8 s so a stuck cover never delays the send.
- iTunes genre lookup (tags feature) bounded at 5 s; earlier it could stall a
  send on a slow API.
- End-to-end cold-path latency is dominated by yt-dlp's network fetch and
  Telegram's upload; the above removes the remaining avoidable waits on the
  bot side. Warm-path remains SQLite (~30 µs) + one Telegram API call.

## Inline rate budget (2026-11-16)

After `answerInlineQuery` stopped using a shared server cache (correct, but it
removes Telegram's shield), every keystroke reached the bot and the regular
per-user limiter (10/min) was exhausted in seconds — inline then answered with
an empty result list ("інлайн пошук ліг"). Fix: a dedicated, much looser
inline limiter (`RATE_LIMIT_INLINE_PER_MIN`, default 120, 0 = unlimited) used
only by inline handlers; the download/callback budget is unchanged.

## Alternative-source fallback chain (2026-11-16)

Cold downloads now iterate open catalogs on "this catalog can't serve it" failures:
`Url(link) → Search("Artist Title") (ytsearch1:) → SoundCloudSearch("Artist Title") (scsearch1:)`.
Triggers: `Private`, `RegionLocked`, `NotFound`, `SourceUnavailable` (rate limits/retries
handled per attempt as before). `TooLong`/`TooLarge` are final and never fall through.
Directives:
- Only metadata available to the bot is ever used to hunt elsewhere (search query /
  resolved artist+title). A bare restricted link with no metadata has nothing to search,
  so it gets only its own attempt, then the usual error.
- Fallback downloads keep the original cache key (the user's pick is fulfilled by
  whichever catalog served it). SoundCloud audio flows through the YouTube-keyed cache;
  genres from SoundCloud are persisted as usual.
- Hot path unchanged; cost appears only on a refusal (one extra yt-dlp subprocess per
  extra catalog plus `clear_dir` between attempts). Fallbacks counted via
  `music_bot_download_fallbacks_total`.
- This is NOT a bypass: restricted/member-only/DRM content stays undownloadable; the
  fallback only finds copies that catalogs themselves serve openly (SoundCloud
  artist uploads, official Topic channels, etc.).

## Album failures: visibility and a Telegram-side 429 fix (2026-11-16)

`/alb` test album "Gloire Éternelle" (Deezer 231087412, 10 tracks, HQ covers on):
7/10 sent. DB diff showed the 3 failures were "La Veuve et Le Martyr"
(yt-dlp 403 on YouTube; SoundCloud fallback also failed), "In'El" and
"Sonata en Mi Mineur" (no yt-dlp error logged — almost certainly Telegram
upload-side 429s: ~10 large files were uploaded at once because the per-user
semaphore was released before send). Causes were invisible because no error
was logged on the album path and the user only saw counts.

Fixes: (1) per-track prepare/upload errors are now logged with key and the
final album message lists the failed track names plus a "retry /alb — cache
makes finished tracks instant" hint; (2) album delivery holds a slot for the
whole download+upload (was: download only), capped at
MAX_ACTIVE_DOWNLOADS_PER_USER (default 3) — stops burst-uploading into one
chat; (3) yt-dlp HTTP 403 counts as transient (one retry) — it is usually
rate limiting, fallback catalogs still run after; (4) `clear_dir` no longer
deletes `cover.*` (it raced the concurrent early-cover task and caused
"audio tagging skipped: cover read: No such file"); (5) titles are normalized
with `TrackMeta::without_artist_prefix` so Telegram's "performer – title"
does not show the artist twice (old cache entries are normalized at send
time, fresh downloads at merge time).

## Local Bot API for >50 MB uploads (2026-11-16)

Cloud Bot API caps uploads at 50 MB, which makes FLAC (a 6.5-minute render is
~78 MB) undeliverable — the send failed with a raw network error on /sendAudio.
Telegram's own answer is the local Bot API server (tdlib/telegram-bot-api):
self-hosted, `--local` mode, uploads go over MTProto up to ~2 GB. The repo
already carried a compose profile for it, but pointed at a nonexistent
`tdlib/telegram-bot-api` Docker image (pull access denied, verified) — switched
to `aiogram/telegram-bot-api` (48M pulls, env-driven entrypoint:
TELEGRAM_API_ID/HASH, TELEGRAM_LOCAL=1 → --local, port 8081) and boot-tested it
(HTTP listener answers on /bot path; / returns 404 as expected).

Code changes to make the two modes coexist safely:
- `max_file_bytes()` is now mode-aware: cloud mode keeps the 49 MB clamp
  (uploads above that can never be sent); local mode allows up to 2 GB, still
  bounded by MAX_FILE_MB. FLAC therefore stays deliverable in both modes.
- The automatic FLAC→m4a 320 fallback stays as the safety net for anything
  beyond the active ceiling.

Migration is operator-side and one-time (README "Local Bot API migration"):
create api_id/api_hash at my.telegram.org, `logOut` on the cloud API while the
bot is stopped, set TELEGRAM_API_ID/HASH/URL in .env, then
`docker compose --profile local up -d --force-recreate`. Without those steps
the bot runs exactly as before (cloud API + 49 MB clamp).

## /settings: FLAC lossless option (2026-11-16)

Quality is `users.quality: INTEGER`, where 0 is now reserved as the lossless
sentinel (settable only via `/settings → FLAC (lossless)`). With 0 the
download path selects `bestaudio[acodec=flac]/bestaudio/best` (a real FLAC
source is preferred when one exists) and converts with
`--audio-format flac` with NO `--audio-quality` (bitrate is meaningless for
FLAC). With kbps values behavior is unchanged (m4a, `--audio-quality {n}K`).
TrackTagger/VorbisComments handle .flac natively; upload filename derives its
extension from the produced file, so .flac files keep .flac names. Cache key is
`(track_key, quality)` — FLAC entries live under quality 0. Honest note:
YouTube never serves true lossless, so for most YouTube downloads this is a
format choice (decode best source → FLAC, no generational re-encode quality
settings), not hi-res; Bandcamp/SoundCloud may genuinely benefit. FLAC files
run ~3–4× the m4a size. Telegram's hosted Bot API rejects audio uploads above
50 MB, so `MAX_FILE_MB` is clamped to an effective 49 MB regardless of the env
value; a FLAC render that still exceeds it auto-downgrades to m4a 320 kbps and
the caption says so.

## yt-dlp auto-refresh at container start (experimental branch, 2026-11-16)

Manual yt-dlp bumps (pinned version + SHA256 in Dockerfile) did not scale:
YouTube forces a new extractor every ~17–20 days. `docker/entrypoint.sh` now
refreshes the standalone yt-dlp binary into the writable /data volume on
container start when YTDLP_AUTO_UPDATE=true (default on the experimental
branch), verifies it answers --version, and points the bot at it via
YTDLP_PATH; any failure falls back to the pinned binary. Admins revert by
setting YTDLP_AUTO_UPDATE=false or checking out main. Known limits and the
full comparison of alternatives (cobalt, NewPipeExtractor, rusty_ytdl) are in
docs/yt-dlp-alternatives.md — the short version: there is no maintenance-free
YouTube downloader; updates moved from "us, manually" to "the build,
automatically".

## /alb: album search by name (2026-11-16)

`/alb <name>` searches public album catalogs in parallel — Deezer
(`api.deezer.com/search/album`) and Apple Music (iTunes Search API,
`entity=album`) — both anonymous, no keys, metadata only. Candidates are merged
(Deezer first), deduplicated by normalized `artist|title`, capped at 6, and
cached in memory for 10 min. The user picks one from an inline keyboard
(`alq:<id>`), which resolves through the same path as an album link
(`resolve_album`) and asks for confirmation before downloading any tracks —
collections never download silently. Audio still comes from the usual open
sources; Spotify remains unsupported (no anonymous API). Private chats only,
per-message rate limiting applies like for any command. Failures on one catalog
do not sink the other; both failing surfaces an error instead of "not found".

## /settings: HQ cover toggle (2026-11-16)

Albums/playlists were already supported (YouTube Music / YouTube playlist / SoundCloud
set / Bandcamp / Deezer / Apple Music — flat-list or public album API, confirmation
keyboard, 2-3 parallel tracks, cap MAX_COLLECTION_TRACKS=25; Spotify refused, no auth
API). What was missing is a way to get better covers.

`/settings` now has an "🖼 HQ cover (1000px)" toggle (per user, default off). When on,
the cover pipeline keeps up to 1000px at JPEG q2 (stepping down to 640px q4 only if
that would exceed Telegram's 200 KB thumbnail budget); default stays 320px q4.
JPEG sources that are already small are passed through as today. Implemented as
`users.hq_cover` (migration 0005, auto-applied), threaded through deliver paths
(PM, search selection, albums, inline).

## Album/genre tags: cold vs cached sends (2026-11-16)

- Cached inline answers are served by Telegram from the `file_id` with no bot
  involvement and carry **no caption** — so with the tags toggle on, inline
  results are now answered as Articles (not CachedAudio) and the placeholder is
  edited to the audio with the tags caption on pick. Cost: one fast file_id edit
  instead of native streaming.
- Cache-hit sends (PM, album flow, inline edit path) now enrich legacy cache
  entries on demand when tags are on and the cached meta has no genres: one
  public iTunes call on the caption path only (genres persisted on cold sends
  in `tracks.genres`; rows written before the feature have `[]` and are filled
  lazily).
- Note: live cache rows live in `/data/bot.db-wal` (WAL, 2+ MB) — backup/copy of
  `bot.db` alone shows a stale empty snapshot; always copy `bot.db*` together.

## Inline search personalization fix + tags feature (2026-11-16)

- `answerInlineQuery` was sent with `cache_time=30` and `is_personal=false`, so Telegram
  served a shared 30-second answer cache for identical query text regardless of the user's
  `/settings` search source. Now `cache_time=0`, `is_personal=true` — results change
  immediately after switching sources.
- The env var `SEARCH_PROVIDERS=youtube` in the operator `.env` silently pinned inline
  search to YouTube (user's Deezer/SoundCloud choice fell back with no signal). The
  fallback now logs a warning, and `/settings` shows a "blocked by server config" notice
  when the chosen source is outside the allow-list. Operators who want the full choice set
  should set `SEARCH_PROVIDERS=youtube,soundcloud,deezer` or remove the line (new default).
- New per-user **tags** toggle (`/settings`, default off): sent tracks show a short
  caption `💿 <album>` + genre hashtags (`#Rock #Metal`). Genres come from the source when
  available (yt-dlp `genres`, e.g. SoundCloud), otherwise from a single public iTunes
  Search API call (±15 s duration match, metadata only). Genres are persisted
  (`tracks.genres`) so cache-hit sends keep the caption.
- Album title: exact from Deezer metadata; SoundCloud tracks carry no album (chapters only).
- `TrackMeta.genres` is `#[serde(default)]` for backward compatibility with cached search
  payloads.

## Inline/text search source per user (2026-11-16)

`/settings` now offers a search-source row: **YouTube** (default), **SoundCloud**,
**Deezer**. The value is stored per user (`users.search_source`, migration
`0002_user_search_source.sql`, applied automatically at startup when the column is
missing — 0001 is a no-op for existing databases). The choice applies to both inline
search and private-chat text search.

Implementation notes:
- SoundCloud search goes through `yt-dlp "scsearchN:…"`; results are normal SoundCloud
  URLs, so caching keys match direct links and the audio downloads directly from SoundCloud.
- Deezer search uses the public `https://api.deezer.com/search` (no auth); results are
  displayed with Deezer's own metadata/cover and the audio is matched on YouTube via the
  same `ytsearch1:` path as single Deezer track links (cache key `deezer:track:<id>`).
- **Spotify was intentionally not added**: there is no anonymous Spotify search API
  (`open.spotify.com` has no public search endpoint; oEmbed only resolves known URLs; the
  Web API requires OAuth client credentials). A "Spotify search" button would either be
  broken or require storing user credentials — both are worse than not shipping it.
- SEARCH_PROVIDERS default changed to `youtube,soundcloud,deezer` (operator allow-list,
  per user setting falls back to the first allowed source). Restrict with
  `SEARCH_PROVIDERS=youtube` to pin old behavior.
- All search is still metadata-only; Spotify/DRM is never bypassed.

## Optimization pass decisions (2026-11-16)

- `mmap_size` for SQLite was tried and **reverted**: on the tiny hot cache DB it consistently
  regressed `get_track` (avg ~34→~52 µs, p50 ~31→~35–49 µs over three runs). Kept
  `cache_size=-65536`, `temp_store=MEMORY`, `statement_cache_capacity(256)`.
- `ensure_user` uses `INSERT … ON CONFLICT DO UPDATE … RETURNING` — one round trip instead of
  two; SQLite ≥ 3.35 required (bookworm ships 3.40).
- Rate limiter no longer `retain`s the whole users map on every call (was O(total users)
  per keystroke); empty-entry sweep is triggered only above 4096 tracked users.
- Unbounded `Mutex<HashMap<…>>` leaks fixed: inline-generation counters and per-user download
  semaphores are now bounded `moka` caches (10k entries, generations idle-expire after 10 min).
- Release profile was already `lto=fat`, `codegen-units=1`, `strip=true`, and mimalloc is the
  global allocator — left alone.

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
