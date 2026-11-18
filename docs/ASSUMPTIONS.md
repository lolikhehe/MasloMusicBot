# Assumptions & open questions (per spec §13)

These defaults were taken where the spec leaves room, per the agent rule in §13 ("не питай — бери дефолт,
пиши в ASSUMPTIONS.md, продовжуй"). Each item names the escape hatch.

| # | Question (§13) | Default taken | How to change |
|---|---|---|---|
| 1 | Public or private bot | **Public**, with per-user limits (`RATE_LIMIT_PER_MIN`, `MAX_ACTIVE_DOWNLOADS_PER_USER`, ban list) | Add an allowlist gate in `handle_message`/`handle_inline` or set `ENABLED_PROVIDERS=` empty to disable downloads |
| 2 | Platforms in first release | **YouTube, YouTube Music, SoundCloud, Bandcamp** (direct). Spotify/Apple Music/Deezer single tracks via metadata match on YouTube | `ENABLED_PROVIDERS` / `SEARCH_PROVIDERS` env — comma-separated or `all` |
| 3 | Matchers in first release | **No** (deferred to M5); single external tracks are already matched by title via `ytsearch1:` | Implement duration ±3 s + similarity scoring; record in this file |
| 4 | Max track duration / file size | **20 min / 49 MB effective** (`MAX_DURATION_SECS=1200`, `MAX_FILE_MB` clamped to Telegram's 50 MB upload limit) | env; `MAX_FILE_MB` is clamped to 49 MB in `max_file_bytes()` — a bigger file simply cannot be uploaded by the hosted Bot API |
| 5 | Deployment & resources | 2–4 vCPU / 2–4 GB assumed; compose caps `mem_limit: 2g`, `pids_limit: 512`, `read_only` rootfs, `tmpfs /tmp` | adjust compose; tmpfs can be replaced by an SSD volume |
| 6 | Languages | **Ukrainian + English** (`/settings`), default from Telegram `language_code` | `DEFAULT_LANG=en|uk`; `ru`/`be` clients receive Ukrainian as the closest supported language |
| 7 | Admin panel beyond commands | **No**; `/stats`, `/ban`, `/unban`, `/purge` only | add commands in `crates/telegram/src/bot/commands.rs` |

## Further assumptions taken during implementation

- **`STORAGE_CHAT_ID`** — optional. When set (e.g. `-1001234567890`), the bot sends cold-path files there
  before editing the inline message, then caches the resulting `file_id`. Messages in the storage chat
  **must not be deleted**; if they are, inline `file_id`s die with them (the cached copy is invalidated
  on the next "wrong file identifier" error and the track re-downloads once).
- **Quality-aware cache** — the `tracks` PK is `(track_key, quality)` rather than §8's `(source, source_id,
  quality)`. `track_key` already encodes the source; adding quality keeps served copies honest per kbps.
- **`search_cache` (Postgres)** — §8 schema retained; queries TTL 10 minutes; L1 moka in front of it. The current branch uses Postgres for every shared table; SQLite was the earlier single-process design and is no longer used at runtime.
- **`hits` counter** — incremented synchronously per rate-limit check (one UPSERT in Postgres) so all replicas share the same per-user budget. This is a deliberate hot spot: at very large scale, batch it locally and flush periodically, accepting a small loss of precision.
- **Rate limiter** — the in-process `telegram::rate_limit` module was removed; `SharedLimiter` in `state.rs` is the only limiter and it delegates to Postgres.
- **Album confirmation** — collections (>1 track) first ask with an inline keyboard
  ("Download all? ✅ / ✖"), enforced cap `MAX_COLLECTION_TRACKS` (25), 2–3 parallel downloads,
  one progress message edited at most once per 2 s.
- **`.env` is operator-managed** — the agent never writes or commits it (safety-net enforced);
  `.env.example` documents every variable.
- **Local Bot API requires credentials the operator does not have** — the bot therefore runs against the
  public Telegram API by default (≤50 MB uploads); the `local` compose profile and the migration steps
  are documented in README for when `TELEGRAM_API_ID`/`TELEGRAM_API_HASH` become available.
- **CJK/emoji in tags** — lofty writes UTF-8; cover art forced to ≤320 px JPEG ≤200 KB.
- **Perf numbers** — see `docs/perf.md`; they are *not fabricated* and marked accordingly.
