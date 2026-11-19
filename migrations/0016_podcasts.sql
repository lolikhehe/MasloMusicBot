CREATE TABLE IF NOT EXISTS feeds (
    id            BIGSERIAL PRIMARY KEY,
    url           TEXT NOT NULL UNIQUE,
    title         TEXT NOT NULL DEFAULT '',
    etag          TEXT,
    last_modified TEXT,
    interval_secs BIGINT NOT NULL DEFAULT 3600,
    next_poll_at  BIGINT NOT NULL,
    fail_count    INTEGER NOT NULL DEFAULT 0,
    last_error    TEXT,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    added_at      BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_feeds_due ON feeds (next_poll_at) WHERE enabled;
CREATE TABLE IF NOT EXISTS feed_episodes (
    feed_id     BIGINT NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    guid        TEXT NOT NULL,
    title       TEXT NOT NULL,
    url         TEXT NOT NULL,
    published_at BIGINT,
    PRIMARY KEY (feed_id, guid)
);
CREATE TABLE IF NOT EXISTS podcast_subs (
    user_id BIGINT NOT NULL,
    chat_id BIGINT NOT NULL,
    feed_id BIGINT NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, chat_id, feed_id)
);
CREATE INDEX IF NOT EXISTS idx_podcast_subs_feed ON podcast_subs (feed_id);
