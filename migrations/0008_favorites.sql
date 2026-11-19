-- ⭐ per-user favorites: the ☆/⭐ toggle button under delivered tracks and
-- the /fav list. Title/performer are copied so the list stays readable; the
-- audio itself is always re-served from the existing tracks cache (file_id),
-- never from disk.
CREATE TABLE IF NOT EXISTS favorites (
    user_id   BIGINT NOT NULL,
    track_key TEXT NOT NULL,
    title     TEXT NOT NULL,
    performer TEXT,
    added_at  BIGINT NOT NULL,
    PRIMARY KEY (user_id, track_key)
);
