-- Unique active-job key: makes enqueue atomic dedup (double-tap safe cross-process).
DROP INDEX IF EXISTS idx_download_jobs_active_key;
CREATE UNIQUE INDEX idx_download_jobs_active_key
    ON download_jobs (user_id, (payload->>'key'))
    WHERE status IN ('queued','downloading','uploading');
