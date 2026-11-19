-- Remember the message that carried each cached audio so a cache hit can be
-- delivered with copy_message: the local Bot API rebuilds file_id sends from
-- its own record and drops title/performer, while a copy keeps the rendered
-- attributes. One-time cache reset: existing rows have no message reference
-- and can never take the copy path. Favorites keep their own metadata rows and
-- refill as tracks are re-downloaded.
ALTER TABLE tracks ADD COLUMN chat_id BIGINT NULL;
ALTER TABLE tracks ADD COLUMN message_id BIGINT NULL;

DELETE FROM tracks;
