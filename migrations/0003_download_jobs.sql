-- Persistent download job ledger. A job is claimed atomically by one replica,
-- so two webhook instances never download the same request twice. An explicit
-- cancel flag is checked cooperatively between download/upload steps.

CREATE TABLE IF NOT EXISTS download_jobs (
    id               TEXT PRIMARY KEY,
    user_id          BIGINT NOT NULL,
    kind             TEXT NOT NULL,
    status           TEXT NOT NULL,
    payload          JSONB NOT NULL,
    created_at       BIGINT NOT NULL,
    updated_at       BIGINT NOT NULL,
    claimed_by       TEXT,
    cancel_requested BOOLEAN NOT NULL DEFAULT false,
    error            TEXT
);

CREATE INDEX IF NOT EXISTS idx_download_jobs_user_time
    ON download_jobs (user_id, created_at DESC);
