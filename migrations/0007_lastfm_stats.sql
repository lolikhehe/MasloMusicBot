-- Per-user switch for Last.fm scrobble statistics and collages.
ALTER TABLE users ADD COLUMN lastfm_stats INTEGER NOT NULL DEFAULT 0;
