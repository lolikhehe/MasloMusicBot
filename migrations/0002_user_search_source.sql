-- Adds the per-user search source for inline/text search (/settings).
-- Existing databases never saw the column in 0001 (which is IF NOT EXISTS and
-- therefore a no-op for them), so this ALTER adds it with the same default.
ALTER TABLE users ADD COLUMN search_source TEXT NOT NULL DEFAULT 'youtube';
