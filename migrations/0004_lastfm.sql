-- Connect a public Last.fm username; used only for the inline "now playing"
-- suggestion (public recent-tracks feed, no password ever leaves the client).
ALTER TABLE users ADD COLUMN lastfm_user TEXT;
