-- Short-lived Telegram message refs for auto-delete, plus the per-user setting.
-- Only metadata is kept: the time at which a sent message should be removed.
CREATE TABLE IF NOT EXISTS sent_messages (
    chat_id      BIGINT NOT NULL,
    message_id   INT NOT NULL,
    user_id      BIGINT NOT NULL,
    delete_after TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (chat_id, message_id)
);

-- Due-scan for the periodic janitor task.
CREATE INDEX IF NOT EXISTS idx_sent_messages_delete_after
    ON sent_messages (delete_after);

-- Per-user auto-delete lifetime in hours; NULL = feature off. Mirrors the
-- lastfm_stats toggle: same users table, updated with one UPDATE statement.
ALTER TABLE users ADD COLUMN auto_delete_hours INT NULL;
