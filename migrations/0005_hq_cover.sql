-- Per-user switch for high-quality audio covers (1000px / JPEG q2 instead of
-- the default 320px q4). Album downloads were already supported via yt-dlp
-- flat playlists and public album APIs.
ALTER TABLE users ADD COLUMN hq_cover INTEGER NOT NULL DEFAULT 0;
