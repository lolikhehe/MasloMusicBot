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
