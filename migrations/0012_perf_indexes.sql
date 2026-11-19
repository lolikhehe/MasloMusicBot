-- Janitor expiry scans and the finished-job cleanup/average lookups are
-- sequential scans today. Index the filter columns so these stay cheap as the
-- cache and job ledger grow.
CREATE INDEX IF NOT EXISTS idx_search_cache_expires ON search_cache (expires_at);

CREATE INDEX IF NOT EXISTS idx_download_jobs_status_updated
    ON download_jobs (status, updated_at);
