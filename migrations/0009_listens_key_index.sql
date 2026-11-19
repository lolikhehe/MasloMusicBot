-- heard_before() looks up the keys of one search page inside a user's listen
-- history. Listens grow without bound, so index the (user, key) pair instead of
-- scanning the whole history on every page.
CREATE INDEX IF NOT EXISTS idx_listens_user_key ON listens (user_id, track_key);
