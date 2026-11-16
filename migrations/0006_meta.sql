-- Small key/value store for one-shot, once-per-version notices.
-- `announced_version` remembers the last release whose patch notes were sent,
-- so a container restart with the same build does not message users again.
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
