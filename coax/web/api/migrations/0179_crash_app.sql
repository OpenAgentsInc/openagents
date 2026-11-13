-- Native app crash reports (#13379, docs/deployment/crash-reports.md). The iPhone and Mac apps post a
-- dump of each crash MetricKit hands them (POST /api/playtest/app-crash), and room-crash.coax makes
-- crash report v3 from it (source `app`). This lets a bucket be an app's, and gives the alerts one
-- row for the apps beside the pages' (service-policy.coax: `crash:app`, open while an app's report
-- came in the last hour). Safe to run twice.

ALTER TABLE verse.crash_buckets DROP CONSTRAINT IF EXISTS crash_buckets_source_check;
ALTER TABLE verse.crash_buckets ADD CONSTRAINT crash_buckets_source_check CHECK (source IN ('room', 'page', 'app'));

-- What the alerts are decided from: each room bucket heard within the window (its age, and its
-- newest report's shard and release), and one row each for the pages and the apps (bucket ''): the
-- age of the newest report no automated browser sent, that bucket's signature, and how many
-- buckets were heard. An age is never below 0.
CREATE OR REPLACE FUNCTION verse.crash_facts(now_ms bigint, window_ms bigint)
RETURNS TABLE (bucket text, source text, sig text, age_ms bigint, shard int, release text, buckets int)
LANGUAGE sql STABLE AS $$
  SELECT k.bucket, k.source, k.sig, greatest(now_ms - k.last_ms, 0), n.shard, coalesce(n.release, ''), 1
  FROM verse.crash_buckets k
  LEFT JOIN LATERAL (SELECT r.shard, r.release FROM verse.crash_reports r WHERE r.bucket = k.bucket ORDER BY r.id DESC LIMIT 1) n ON true
  WHERE k.source = 'room' AND k.last_ms > now_ms - window_ms
  UNION ALL
  SELECT '', s.source, coalesce((SELECT k.sig FROM verse.crash_buckets k WHERE k.source = s.source AND k.live_ms IS NOT NULL ORDER BY k.live_ms DESC LIMIT 1), ''),
         (SELECT CASE WHEN max(k.live_ms) IS NULL THEN -1 ELSE greatest(now_ms - max(k.live_ms), 0) END FROM verse.crash_buckets k WHERE k.source = s.source), NULL, '',
         (SELECT count(*) FROM verse.crash_buckets k WHERE k.source = s.source AND k.live_ms > now_ms - window_ms)::int
  FROM (VALUES ('page'), ('app')) s (source)
  ORDER BY 2 DESC, 1 $$;
