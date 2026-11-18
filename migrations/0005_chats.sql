-- Opt-in group mode. A group is silent until an admin enables it with
-- /groupmode on; private chats are always allowed.

CREATE TABLE IF NOT EXISTS chats (
    chat_id    BIGINT PRIMARY KEY,
    enabled    BOOLEAN NOT NULL DEFAULT false,
    added_by   BIGINT,
    created_at BIGINT NOT NULL
);
