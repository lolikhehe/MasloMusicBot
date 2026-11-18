-- Per-user switch for the album-info photo (cover + artist/album facts)
-- sent before an album download. On by default.
ALTER TABLE users ADD COLUMN album_info BIGINT NOT NULL DEFAULT 1;
