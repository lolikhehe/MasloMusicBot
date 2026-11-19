# Horizontal scaling (v2.1, promoted to main)

v2.1 replaces the single-process SQLite setup with a shared Postgres backend
and an optional webhook listener. After the 2026-11-18 promotion, main runs on
Postgres with long polling by default; webhooks are enabled by setting
`WEBHOOK_URL`. The design is still young — keep the rollback notes below handy.

## What had to change

| Concern | v2.0 (single process) | v2.1 (N replicas) |
|---|---|---|
| Metadata / file ids | SQLite WAL file on one volume | Postgres 17, schema embedded in the binary via `sqlx::migrate!` |
| SQL correctness | runtime strings, mistakes found in tests/prod | `sqlx::query!` / `query_scalar!` checked at compile time against `.sqlx` metadata |
| Callback selections | in-process `moka` caches | `selections` table (JSONB + TTL + kind) |
| Rate limits | in-process mutex/bucket per replica | atomic fixed-window upsert in `rate_limits` |
| Search cache | moka L1 + SQLite L2 | moka L1 + shared Postgres L2 |
| Updates | long polling: only one process gets updates | webhook: Telegram POSTs to any replica behind a proxy |
| Inline generation watches | per-process `watch` channel | unchanged: only the replica handling the query needs them |

`last_seen_at` writing is throttled per process (5 min per user per replica);
with N replicas this is at most N writes/user/interval, still negligible.

There is **no automatic data migration from the old SQLite file**. On the
2026-11-18 promotion the 12 user rows, 92 cached `file_id`s and the
`announced_version` marker were copied manually from `bot-data:/data/bot.db`
into the new Postgres volume with a small SQL script (generating `INSERT`
statements with Python's `sqlite3` module, then `psql -f`). New deployments
start with an empty cache and repopulate it on the next download.

v2.2 added `user_genre_events` (migration 0002). The migration's guarded
backfill had no source rows at the moment it ran, so the 9 existing genre
events and 2 counted tracks were imported from the old SQLite file afterwards
with `migrate_genres.sql` + a LATERAL `jsonb_array_elements_text` backfill. New
installations backfill automatically during migration 0002.

Deployment note: Docker Desktop's WSL disk image (`docker_data.vhdx`,
`ext4.vhdx`) was moved off the system drive to `D:\DockerDesktopWSL`. Docker
Desktop 4.95 does not reliably relocate the WSL data folder via its
`DataFolder` setting, so `C:\Users\<user>\AppData\Local\Docker\wsl` is a
directory junction pointing at `D:\DockerDesktopWSL`. Images, container
layers and named volumes (`bot-data`, `botapi`, `pg-data`) therefore live on
D while Docker keeps using its original path.

## Running it

```sh
docker compose build bot
docker compose up -d
```

The compose file starts Postgres, waits for `pg_isready`, and points the bot and
worker at `postgresql://musicbot:...` . Since v3.0.0 downloads (and their queue)
run in a separate `worker` process that shares the same Postgres — scale bot and
worker replicas independently. For webhooks, set in `.env`:

```sh
WEBHOOK_URL=https://music.example.com/telegram/webhook
WEBHOOK_SECRET=my-secret-token-123
```

The public URL must terminate TLS (port 443/80/88/8443) and proxy to the
container's `WEBHOOK_ADDR` (default `0.0.0.0:8080`). The path in `WEBHOOK_URL`
is used as the axum route. teloxide calls `set_webhook` on startup and
`delete_webhook` on shutdown; every replica can do this idempotently. With no
`WEBHOOK_URL`, the bot keeps long polling and one replica should run.

## Compile-time SQL

- All static statements live in `crates/storage/src/db.rs` and use
  `sqlx::query!` / `query_scalar!`. Column names and bind types are checked
  against a live schema at development time; the resulting metadata is
  committed in `.sqlx/`.
- Builds use `SQLX_OFFLINE=true` (Dockerfile and CI), so no database is needed
  to compile. Regenerate metadata after changing SQL or migrations with:

  ```sh
  $env:DATABASE_URL='postgresql://musicbot:devpass@127.0.0.1:55432/musicbot'
  sqlx migrate run
  cargo sqlx prepare --workspace -- --all-targets
  ```

- The only runtime-built statement is the batched genre upsert
  (`QueryBuilder`): the number of rows depends on the source's tag list.
- Tests use `#[sqlx::test(migrations = "../../migrations")]`, which gives each
  test an isolated database. CI provides Postgres as a service container.

## Brute-force check list for the webhook mode

1. `WEBHOOK_URL` unset → `set_webhook` is never called; polling works and
   `/help` answers in a private chat.
2. `WEBHOOK_URL` set → Telegram reports one webhook with the expected URL and
   `has_custom_certificate=false`; a `/help` update arriving on any replica
   is answered; a search button pressed on a different replica resolves
   because the selection is in Postgres.
3. Restart with the same `WEBHOOK_SECRET` → updates keep arriving; the old
   listener is deleted on graceful shutdown.
4. Two replicas, 100 messages from distinct users →
   `SELECT user_id, COUNT(*) FROM rate_limits GROUP BY user_id` matches the
   configured per-minute budget, not `budget × replicas`.

## v2.5 additions

- Migration `0006` adds `listens` (one metadata row per delivered track) and
  `digest_schedule` (opt-in planner with `next_send_at`). The digest sweep runs
  on every replica and claims a due row with a compare-and-swap on
  `next_send_at`, so no webhook farm or load-test replica set sends twice.
  Quiet hours defer the row by an hour instead of resetting the plan.
- `/history` reads `listens`; `/dynamics` reads the already-timestamped
  `user_genre_events` and renders an in-memory PNG that is deleted after upload.
- The mock Bot API used by the e2e tests now lives in `crates/mock-api` and is
  also runnable as the standalone `mock-bot-api` binary for `load/`.

## Known limits

- No sticky routing is needed for selections, but in-flight inline generation
  cancellation is per replica; that only affects which keystroke's answer is
  dropped, not correctness.
- Postgres connection pools are per process (`max_connections=32`). The
  Postgres `max_connections` must be at least `32 × (bot + worker instances) + headroom`.
- Each connection sets `synchronous_commit=off`: the table is a cache of
  Telegram `file_id`s and counters, so a crash may lose the newest writes but
  never user-visible data that cannot be rebuilt by re-downloading. This was
  chosen after a local benchmark showed the default WAL flush dominating
  `touch_track`/`ensure_user` latency (see `docs/perf.md`).
- `last_tracks` for argument-less `/lyrics` is in-memory: after a webhook
  restart or a switch to a different replica, `/lyrics` with no argument asks
  for an explicit query instead of guessing.
- `music_bot_*` histograms still need real traffic to validate S1–S4; see
  `docs/perf.md`. Local mock numbers are labelled as such and are not a
  substitute for production measurement.
