-- Per-listen genre events. `user_genres` keeps the all-time totals used by
-- /genres; this table adds a timestamp so charts can be filtered by period.
-- Events are written in the same transaction as the deduplicated track, so
-- one track counts once per distinct genre.

CREATE TABLE IF NOT EXISTS user_genre_events (
    user_id    BIGINT NOT NULL,
    genre      TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_genre_events_user_time
    ON user_genre_events (user_id, created_at);

-- One-time backfill from the old track/genre tables. Guarded: a malformed
-- legacy JSON blob must never make a migration fail and block startup.
DO $$
BEGIN
    INSERT INTO user_genre_events (user_id, genre, created_at)
    SELECT e.user_id, e.genre, e.created_at
    FROM (
        SELECT g.user_id AS user_id,
               g.created_at AS created_at,
               CASE WHEN t.genres ~ '^\s*\[' THEN t.genres::jsonb ELSE '[]'::jsonb END AS genres
        FROM user_genre_tracks g
        JOIN tracks t ON t.track_key = g.track_key
    ) rows
    CROSS JOIN LATERAL jsonb_array_elements_text(rows.genres) AS e(genre);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'genre event backfill skipped: %', SQLERRM;
END $$;
