-- Recommendation inputs. Both views cover a rolling 90-day listens window and
-- collapse title/performer aliases (case + surrounding whitespace) into one
-- identity. Built WITH NO DATA, then populated by the plain REFRESHes at the
-- end of this migration; runtime refreshes use CONCURRENTLY (needs the UNIQUE
-- indexes below), never inside a migration.

CREATE MATERIALIZED VIEW mv_rec_item_stats AS
WITH recent AS (
    SELECT user_id, track_key, title, performer, created_at,
           lower(btrim(title)) AS title_key,
           lower(btrim(coalesce(performer, ''))) AS performer_key
    FROM listens
    WHERE created_at >= EXTRACT(EPOCH FROM now())::bigint - 90 * 86400
),
collapsed AS (
    SELECT title_key, performer_key,
           (array_agg(title ORDER BY created_at DESC))[1] AS title,
           (array_agg(coalesce(performer, '') ORDER BY created_at DESC))[1] AS performer,
           count(*) AS plays,
           count(DISTINCT user_id) AS listeners
    FROM recent
    GROUP BY title_key, performer_key
),
keys AS (
    SELECT title_key, performer_key, track_key,
           row_number() OVER (
               PARTITION BY title_key, performer_key
               ORDER BY count(*) DESC, max(created_at) DESC, track_key
           ) AS rn
    FROM recent
    GROUP BY title_key, performer_key, track_key
)
SELECT c.title_key, c.performer_key, c.title, c.performer,
       c.plays, c.listeners, k.track_key AS canonical_key
FROM collapsed c
JOIN keys k
  ON k.title_key = c.title_key
 AND k.performer_key = c.performer_key
 AND k.rn = 1
WITH NO DATA;

CREATE UNIQUE INDEX mv_rec_item_stats_pk
    ON mv_rec_item_stats (title_key, performer_key);

CREATE MATERIALIZED VIEW mv_rec_cooccurrence AS
WITH recent AS (
    SELECT user_id,
           lower(btrim(title)) AS title_key,
           lower(btrim(coalesce(performer, ''))) AS performer_key,
           created_at
    FROM listens
    WHERE created_at >= EXTRACT(EPOCH FROM now())::bigint - 90 * 86400
),
user_items AS (
    SELECT user_id, title_key, performer_key,
           count(*) AS plays,
           max(created_at) AS last_at,
           row_number() OVER (
               PARTITION BY user_id
               ORDER BY count(*) DESC, max(created_at) DESC
           ) AS rn
    FROM recent
    GROUP BY user_id, title_key, performer_key
),
top AS (
    SELECT * FROM user_items WHERE rn <= 50
),
pairs AS (
    SELECT a.user_id,
           a.title_key AS a_t, a.performer_key AS a_p, a.last_at AS a_last,
           b.title_key AS b_t, b.performer_key AS b_p, b.last_at AS b_last
    FROM top a
    JOIN top b
      ON a.user_id = b.user_id
     AND (a.title_key, a.performer_key) < (b.title_key, b.performer_key)
)
SELECT a_t, a_p, b_t, b_p,
       count(DISTINCT user_id) AS co_count,
       sum(exp(-((EXTRACT(EPOCH FROM now())::bigint - greatest(a_last, b_last))::float8
                 / 86400.0 / 30.0)))::float8 AS score
FROM pairs
GROUP BY a_t, a_p, b_t, b_p
HAVING count(DISTINCT user_id) >= 2
WITH NO DATA;

CREATE UNIQUE INDEX mv_rec_cooccurrence_pk
    ON mv_rec_cooccurrence (a_t, a_p, b_t, b_p);

INSERT INTO meta (meta_key, meta_value) VALUES ('rec_refreshed_at','0')
    ON CONFLICT (meta_key) DO NOTHING;

REFRESH MATERIALIZED VIEW mv_rec_item_stats;
REFRESH MATERIALIZED VIEW mv_rec_cooccurrence;
