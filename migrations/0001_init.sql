CREATE TABLE IF NOT EXISTS tracks (
    track_key       TEXT PRIMARY KEY,
    file_id         TEXT NOT NULL,
    file_unique_id  TEXT NOT NULL,
    source_url      TEXT,
    provider        TEXT NOT NULL,
    title           TEXT NOT NULL,
    performer       TEXT,
    album           TEXT,
    duration_secs   INTEGER,
    thumbnail_url   TEXT,
    created_at      INTEGER NOT NULL,
    last_used_at    INTEGER NOT NULL,
    uses            INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_tracks_last_used ON tracks(last_used_at);
CREATE INDEX IF NOT EXISTS idx_tracks_provider ON tracks(provider);

CREATE TABLE IF NOT EXISTS users (
    user_id      INTEGER PRIMARY KEY,
    language     TEXT NOT NULL DEFAULT 'en',
    quality      INTEGER NOT NULL DEFAULT 192,
    banned       INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_users_banned ON users(banned);
