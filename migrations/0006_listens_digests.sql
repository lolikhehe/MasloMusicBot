-- Listening history and the opt-in digest planner (v2.5).
--
-- `listens` is one row per successfully delivered track. Only metadata is kept
-- (title / artist / genre labels / unix timestamp); no audio bytes, covers or
-- lyrics ever reach the disk. It powers /history and the digest summaries.
--
-- `digest_schedule` is the durable planner: every replica reads the same
-- `next_send_at` and claims a due row with a compare-and-swap, so a webhook
-- farm (or the load-test replica set) never sends a digest twice.

CREATE TABLE IF NOT EXISTS listens (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL,
    track_key   TEXT NOT NULL,
    title       TEXT NOT NULL,
    performer   TEXT,
    genres      JSONB NOT NULL DEFAULT '[]',
    created_at  BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_listens_user_time
    ON listens (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS digest_schedule (
    user_id          BIGINT PRIMARY KEY,
    period           TEXT NOT NULL DEFAULT 'daily',
    enabled          BOOLEAN NOT NULL DEFAULT true,
    next_send_at     BIGINT NOT NULL,
    last_sent_at     BIGINT,
    -- UTC hours, half-open interval [start, end). start == end means no quiet window.
    quiet_start_utc  SMALLINT NOT NULL DEFAULT 0,
    quiet_end_utc    SMALLINT NOT NULL DEFAULT 0,
    created_at       BIGINT NOT NULL,
    updated_at       BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_digest_due
    ON digest_schedule (next_send_at) WHERE enabled;
