# Postgres backup & restore

Backups live on **D:**, not on the system drive.

## Backup

```powershell
powershell -File scripts\backup-postgres.ps1
```

The script runs `pg_dump` in the running `postgres` container, compresses the
result to `D:\music-bot-backups\musicbot-<timestamp>.sql.zip` and deletes dumps
older than 7 days. Schedule it with Task Scheduler if the bot carries data you
care about; the cache of `file_id`s is rebuildable, user settings are not.

## Restore

1. Stop the bot so nothing writes during the restore:

   ```sh
   docker compose stop bot
   ```

2. Pick a dump and extract it:

   ```powershell
   Expand-Archive D:\music-bot-backups\musicbot-YYYYMMDD-HHMMSS.sql.zip D:\music-bot-backups\restore
   ```

3. Replace the database contents (migrations are included in the dump's schema;
   if restoring into an empty cluster, let the bot apply them instead):

   ```sh
   docker compose exec -T postgres psql -U musicbot -d musicbot -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
   docker compose exec -T postgres psql -U musicbot -d musicbot < D:/music-bot-backups/restore/musicbot-....sql
   ```

4. Start the bot again:

   ```sh
   docker compose --profile local start bot
   ```

5. Check `curl http://127.0.0.1:9090/healthz` and that
   `SELECT COUNT(*) FROM users` matches the dump.

## Caveats

- A dump taken while a download job is `downloading` is fine: on startup
  `Db::migrate` requeues stale jobs to `queued`.
- Do not restore a v2.0 dump into Postgres — the schema changed; migrate from
  the SQLite era manually as documented in `docs/scaling.md`.
- The `botapi` volume (local Bot API session) is backed up by Docker's volume
  layer on D, not by `pg_dump`.
