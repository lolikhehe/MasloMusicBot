-- /convert looks up an already-downloaded track by the Telegram file id of
-- the audio the bot sent. Without this index that query walks idx_tracks_last_used
-- and filters by file_id (O(N) on a large cache), so index the filter column.
CREATE INDEX IF NOT EXISTS idx_tracks_file_id ON tracks (file_id);
