# Performance notes (spec §9 етап 10 / §14)

> Honesty first: this document does **not** contain invented numbers. Real latency depends on the
> operator's network, IP reputation with YouTube/SoundCloud, and Telegram datacenter location, none
> of which the dev environment has. Numbers marked **NOT MEASURED** must be re-measured after deploy.

## Scenarios and status

| ID | Scenario (from spec) | Target | Status |
|---|---|---|---|
| S1 | Cache hit, start of audio playback | < 300 ms | **NOT MEASURED** — architecture supports it (offline key parse + one `SELECT` + Telegram send; no external network). Measure with `music_bot_request_seconds` histogram p95 on cache hits. |
| S2 | Cold path, new 3–5 min track | ≈ 3–6 s | **NOT MEASURED** — dominated by yt-dlp fetch + Telegram upload. Measure `music_bot_download_seconds` and `music_bot_upload_seconds` on first send. |
| S3 | 20 concurrent requests for 5 tracks | 1 download each | **Architecture addressed, not measured** — in-flight dedup `(track_key, quality)` + global semaphore 4 + per-user cap 2. Verify by burst-sending the same link from several accounts and watching `music_bot_downloads_inflight`. |
| S4 | Inline search burst | < 1 s cached / < 2 s cold | Served via moka L1 (10 min TTL) + SQLite L2; `music_bot_inline_seconds` histogram records the full handler latency. |

## What *is* measured on the dev machine (Windows, x64)

- `cargo test --workspace`: all tests complete in ~0.1 s.
- `cargo build --release`: ~1 min 45 s, binary links lean (`lto=fat`, `codegen-units=1`, `strip`).
- Mock-API smoke test: image boots, connects, `/healthz` returns `{"status":"ok"}`; no real yt-dlp
  download exercised (no network egress to providers in the harness).

## How to measure after deploy

1. Deploy with `METRICS_ADDR=0.0.0.0:9090` and scrape `http://bot:9090/metrics`.
2. S1: send a cached link 50 times; read `histogram_quantile(0.95, rate(music_bot_request_seconds_bucket[5m]))`.
3. S2: send 10 different fresh links; read `music_bot_download_seconds` and `music_bot_upload_seconds`.
4. S3: 20 users send the same link within 2 s; `music_bot_downloads_inflight` should peak at the
   semaphore limit (4) and exactly one download per track is performed.
5. S4: `curl "https://api.telegram.org/bot<TOKEN>/answerInlineQuery"` behaviour is already covered by the
   handler; measure `music_bot_inline_seconds` during a typing burst in any chat.

## Profiling hooks available today

- tracing structured logs (`LOG_FORMAT=json`), per-request timing spans in the handler.
- `metrics-exporter-prometheus` recorder on `METRICS_ADDR`.
- No tokio-console/flamegraph integration yet — add behind a `debug` feature if needed.

## Deployment verification (2026-11-16)

Live deploy succeeded: `docker compose up -d`, container `music-bot-bot-1` healthy, `/healthz` returns
`{"status":"ok"}`, bot connected to Telegram as `MaslougazeMusicBot`.

First two user requests landed before the yt-dlp `TMPDIR` fix (see decisions.md): downloads failed fast
(exit 127), so end-to-end numbers do **not** reflect a working cold path yet. Measured counters:

- `music_bot_requests_total = 2`
- `music_bot_request_seconds_sum ≈ 0.92 s` → ~0.46 s avg over 2 requests (all fast-fail paths)

Local microbenchmark (SQLite cache layer only — the hot path for an S1-style hit, measured on this
Windows x64 dev machine with `cargo run --release -p storage --example cache_bench`):

- `get_track` (cache hot path): 1000 iterations, avg ≈ 34 µs, p50 ≈ 31 µs, p95 ≈ 47 µs, max ≈ 92 µs.
- `touch_track` (hit bookkeeping UPSERT): avg ≈ 98 µs, p50 ≈ 45 µs, p95 ≈ 68 µs, one outlier ≈ 19 ms.

S1/S2/S3/S4 remain **NOT MEASURED** end-to-end until the operator (or user) exercises the bot against
real sources post-fix; re-check `/metrics` exports then and update the table above. Honest framing:
the database is demonstrably sub-millisecond; network-to-Telegram upload dominates wall-clock and is
the number to watch.

## Addendum (2026-11-16, post-deno)

Cold-path yt-dlp download verified inside the container after the deno fix (same flags as the
downloader: `-f bestaudio[ext=m4a]/bestaudio -N 4 --no-playlist --ignore-config`): fetched a
19 s YouTube test clip as a valid M4A with no EJS warning. End-to-end via Telegram still
awaits operator traffic; S2 remains NOT MEASURED until then.

## Optimization pass (2026-11-16, Rust micro-opts, no behavior change)

Changes: single-roundtrip `ensure_user` (`INSERT … ON CONFLICT … RETURNING`), SQLite page
cache 64 MiB + `temp_store=MEMORY` + statement cache 256, per-user rate limiter now O(1)
(no full-map sweep per keystroke), inline-generation counters and per-user download
semaphores moved to bounded `moka` caches (were unbounded `Mutex<HashMap>` = memory leak),
single call sites for command parsing, `truncate`/`escape_html` single pass / fast path,
dropped redundant host-lowercase allocation in `parse_input`.

Measured (`cargo run --release -p storage --example cache_bench`, Windows x64 dev machine,
a 100-row temp-file DB, 1000 iterations, noisy shared desktop — treat as indicative):

| Run | get_track avg | get_track p50 | get_track p95 | touch_track avg |
|---|---|---|---|---|
| before (docs baseline) | ~34 µs | ~31 µs | ~47 µs | ~98 µs |
| with `mmap_size=256MB` | ~51–52 µs | ~35–49 µs | ~59–76 µs | ~114–129 µs |
| without mmap (final) | 33.0 / 34.7 / 34.4 µs | 30.1 / 31.7 / 31.4 µs | 47.2 / 54.1 / 55.1 µs | ~95 / ~124 µs |

Conclusion: `mmap_size` was a measurable regression on this workload and was removed; the
rest of the pass is semantics-preserving and should mainly help under real concurrency
(DB roundtrips halved on the message path; limiter no longer scans all users per call).
End-to-end S1/S2 still require live traffic to measure.

## Optimization pass (2026-11-17, v2.0.3: network + full-code pass)

Scope: shared HTTP plumbing, SQLite hot path, collage/stat rendering, callbacks and the
small allocations on the message path.

Network:
- Every metadata/image client in the workspace is now built by `providers::http::client`
  (10 s connect timeout, 90 s idle-pool reuse, 60 s TCP keep-alive, shared user agent).
  Previously `ExternalResolver`, `LastFmClient` and the cover downloader each built their
  own client, so every call paid a fresh TLS handshake.
- `reqwest/gzip` is enabled workspace-wide; JSON catalog responses shrink on the wire.
- Response bodies are streamed with hard byte ceilings (`providers::http::fetch_bytes`,
  cover download in `audio::tags`) instead of `Response::bytes()`, which trusted
  `content-length` or buffered an unbounded body.
- Collage artwork: unique queries are resolved once, unique URLs are downloaded once,
  duplicate cells share one `Arc<[u8]>`. Both phases are bounded by the same 6-permit
  semaphore, and the shared `state.http` client replaces a per-collage `reqwest::Client`.

Database:
- `ensure_user` is now read-first on the message hot path; `last_seen_at` is only written
  at most once per user per 5 minutes (map swept above 8 192 entries). First contact still
  inserts and returns the row.
- `record_genres` folds the toggle check and track dedupe into one `INSERT … SELECT …
  WHERE EXISTS`, then upserts all genres in one multi-row `QueryBuilder` statement
  (was: toggle SELECT + per-genre statement inside a transaction).
- `stats()` is one statement with three scalar subqueries instead of three round trips.

Rendering / Telegram layer:
- `collage::render` runs on `spawn_blocking`; the custom conditional in `render_bars` does
  too. Covers use `Arc<[u8]>`, duplicate covers are decoded/resized once per canvas,
  resize uses `Triangle` (the 202 px cell hides the difference), and `fill_rect` /
  `blend_rect` write the raw buffer with a 256-entry alpha LUT.
- `/profile` runs the profile and top-tracks calls concurrently.
- Settings callbacks patch the already-read `UserRecord` instead of re-reading the row
  after every toggle.
- Rate limiter is a `std::sync::Mutex` with a sync `allow`; the critical section never
  awaits. Health server uses a stack buffer and writes header/body separately.
- `trim_log` no longer panics when yt-dlp stderr has non-ASCII text at the cut point.
- Embedded cover budget bug fixed: the check used Telegram's 200 KB thumbnail cap instead
  of the 1.5 MB audio budget, silently dropping HQ covers above ~200 KB.

Measured locally (Windows x64, `cargo run --release`, noisy shared desktop; indicative only):

| Benchmark | Before | After |
|---|---|---|
| `render_bench`: collage 10×10, 100 distinct 400 px JPEG covers, avg | 388.09 ms | 214.77 ms |
| same, p50 / max | 348.85 / 554.82 ms | 219.60 / 221.56 ms |
| `render_bench`: 10-bar genre chart, avg | 11.16 ms | 2.48 ms |
| same, p50 / max | 3.92 / 40.53 ms | 2.37 / 2.83 ms |
| `cache_bench`: `ensure_user` avg | 56.60 µs | 32.99 µs |
| same, p50 / p95 / max | 37.8 / 75.3 µs / 3.71 ms | 25.7 / 91 µs / 263.6 µs |

Notes:
- The collage benchmark uses 100 *distinct* covers, so the decoded-cover reuse is not
  exercised; real album/artist charts repeat artwork and gain more than the table shows.
- `ensure_user` p95 moved within run-to-run noise (shared desktop); the average halves
  because the 5-minute throttle removes nearly all write transactions from the loop.
- End-to-end S1–S4 remain **NOT MEASURED** until post-deploy traffic; re-check
  `music_bot_*` histograms and update the scenario table above.
