-- Per-user toggle for album/genre tags caption on sent tracks, and a persistent
-- place for genre metadata so cache-hit sends can show it too.
ALTER TABLE users ADD COLUMN tags_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tracks ADD COLUMN genres TEXT NOT NULL DEFAULT '[]';
