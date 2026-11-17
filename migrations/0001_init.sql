-- Postgres schema for MasloMusicBot (experimental v2.1 branch).
-- Only metadata, Telegram file identifiers and short-lived coordination state
-- are stored here. Audio bytes and covers are never written to disk by the bot.
--
-- Numeric columns are BIGINT on purpose: the Rust layer binds i64 everywhere,
-- and compile-time-checked sqlx macros then infer int8 for every parameter.

CREATE TABLE IF NOT EXISTS tracks (
    track_key       TEXT NOT NULL,
    quality         BIGINT NOT NULL DEFAULT 192,
    file_id         TEXT NOT NULL,
    file_unique_id  TEXT NOT NULL,
    source_url      TEXT,
    provider        TEXT NOT NULL,
    title           TEXT NOT NULL,
    performer       TEXT,
    album           TEXT,
    duration_secs   BIGINT,
    size_bytes      BIGINT,
    thumbnail_url   TEXT,
    genres          TEXT NOT NULL DEFAULT '[]',
    created_at      BIGINT NOT NULL,
    last_used_at    BIGINT NOT NULL,
    uses            BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (track_key, quality)
);

CREATE INDEX IF NOT EXISTS idx_tracks_last_used ON tracks (last_used_at);

CREATE TABLE IF NOT EXISTS users (
    user_id         BIGINT PRIMARY KEY,
    language        TEXT NOT NULL DEFAULT 'en',
    quality         BIGINT NOT NULL DEFAULT 192,
    banned          BIGINT NOT NULL DEFAULT 0,
    created_at      BIGINT NOT NULL,
    last_seen_at    BIGINT NOT NULL,
    search_source   TEXT NOT NULL DEFAULT 'youtube',
    tags_enabled    BIGINT NOT NULL DEFAULT 0,
    lastfm_user     TEXT,
    hq_cover        BIGINT NOT NULL DEFAULT 0,
    lastfm_stats    BIGINT NOT NULL DEFAULT 0,
    genre_analysis  BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS search_cache (
    provider    TEXT NOT NULL,
    query       TEXT NOT NULL,
    result      TEXT NOT NULL,
    expires_at  BIGINT NOT NULL,
    PRIMARY KEY (provider, query)
);

CREATE TABLE IF NOT EXISTS meta (
    meta_key    TEXT PRIMARY KEY,
    meta_value  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_genres (
    user_id     BIGINT NOT NULL,
    genre       TEXT NOT NULL,
    listens     BIGINT NOT NULL DEFAULT 0,
    updated_at  BIGINT NOT NULL,
    PRIMARY KEY (user_id, genre)
);

CREATE TABLE IF NOT EXISTS user_genre_tracks (
    user_id     BIGINT NOT NULL,
    track_key   TEXT NOT NULL,
    created_at  BIGINT NOT NULL,
    PRIMARY KEY (user_id, track_key)
);

-- Shared selection state. In the single-process SQLite build these lived in a
-- moka cache; with webhooks and several replicas a callback can land on any
-- instance, so the payload must be in the database.
CREATE TABLE IF NOT EXISTS selections (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,
    user_id     BIGINT NOT NULL,
    payload     JSONB NOT NULL,
    expires_at  BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_selections_expiry ON selections (expires_at);

-- Fixed-window per-user rate limits, shared by all replicas.
CREATE TABLE IF NOT EXISTS rate_limits (
    user_id      BIGINT PRIMARY KEY,
    window_start BIGINT NOT NULL,
    hits         BIGINT NOT NULL
);
