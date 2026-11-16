-- SQLite schema for MasloMusicBot. Only metadata and Telegram file ids are stored.
-- File ids are bot-scoped: after a token change or a cloud<->local Bot API migration
-- the `tracks` table should be wiped (the bot re-downloads and re-populates it).

CREATE TABLE IF NOT EXISTS tracks (
    track_key     TEXT NOT NULL,
    quality       INTEGER NOT NULL DEFAULT 192,
    file_id       TEXT NOT NULL,
    file_unique_id TEXT NOT NULL,
    source_url    TEXT,
    provider      TEXT NOT NULL,
    title         TEXT NOT NULL,
    performer     TEXT,
    album         TEXT,
    duration_secs INTEGER,
    size_bytes    INTEGER,
    thumbnail_url TEXT,
    genres        TEXT NOT NULL DEFAULT '[]',
    created_at    INTEGER NOT NULL,
    last_used_at  INTEGER NOT NULL,
    uses          INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (track_key, quality)
);

CREATE INDEX IF NOT EXISTS idx_tracks_last_used ON tracks (last_used_at);
CREATE INDEX IF NOT EXISTS idx_tracks_uses ON tracks (uses);

-- L2 (persistent) search cache. L1 lives in moka. Expired rows are pruned on startup.
CREATE TABLE IF NOT EXISTS search_cache (
    provider    TEXT NOT NULL,
    query       TEXT NOT NULL,
    result      TEXT NOT NULL,
    expires_at  INTEGER NOT NULL,
    PRIMARY KEY (provider, query)
);

CREATE INDEX IF NOT EXISTS idx_search_cache_expires ON search_cache (expires_at);

CREATE TABLE IF NOT EXISTS users (
    user_id      INTEGER PRIMARY KEY,
    language     TEXT NOT NULL DEFAULT 'uk',
    quality      INTEGER NOT NULL DEFAULT 192,
    banned       INTEGER NOT NULL DEFAULT 0,
    search_source TEXT NOT NULL DEFAULT 'youtube',
    tags_enabled INTEGER NOT NULL DEFAULT 0,
    lastfm_user TEXT,
    hq_cover INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_users_last_seen ON users (last_seen_at);
