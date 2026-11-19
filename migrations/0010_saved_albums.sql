-- Saved albums (bookmarks) for later re-download. `album_key` is "{source}:{id}"
-- and is the dedup identity; `payload_json` carries {"source":..., "id":...} so a
-- saved album can be re-resolved without storing audio or covers.
CREATE TABLE IF NOT EXISTS saved_albums (
    user_id      BIGINT NOT NULL,
    album_key    TEXT NOT NULL,
    title        TEXT NOT NULL,
    artist       TEXT NOT NULL,
    cover_url    TEXT,
    payload_json TEXT NOT NULL,
    added_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, album_key)
);
