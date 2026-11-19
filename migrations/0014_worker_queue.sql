-- Worker queue: bot enqueues (status='queued'), worker claims via SKIP LOCKED.
CREATE INDEX IF NOT EXISTS idx_download_jobs_active_key
    ON download_jobs (user_id, (payload->>'key'))
    WHERE status IN ('queued','downloading','uploading');
CREATE INDEX IF NOT EXISTS idx_download_jobs_queued
    ON download_jobs (created_at) WHERE status = 'queued';
