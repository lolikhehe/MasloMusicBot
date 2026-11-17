-- Per-user genre analyzer. `user_genre_tracks` de-duplicates tracks so repeated
-- downloads (and cache hits) do not inflate the chart; `user_genres` keeps the
-- aggregate per genre.
ALTER TABLE users ADD COLUMN genre_analysis INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS user_genre_tracks (
    user_id    INTEGER NOT NULL,
    track_key  TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, track_key)
);

CREATE TABLE IF NOT EXISTS user_genres (
    user_id    INTEGER NOT NULL,
    genre      TEXT NOT NULL,
    listens    INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, genre)
);

CREATE INDEX IF NOT EXISTS idx_user_genres_listens
    ON user_genres (user_id, listens DESC);
