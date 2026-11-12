-- Crash reports (#13298, docs/deployment/crash-reports.md). A room shard posts its report after a
-- crash (POST /api/playtest/room-crash, signed), and the minute's service watch folds each page's
-- `crash` analytics event that carries a trap line or frames into one. room-crash.coax writes and
-- checks every report (crash report v1); this stores them by bucket, the md5 of the source and the
-- signature the codec wrote, and counts them for the alerts service-policy.coax decides.
-- Additive, and safe to run twice.

CREATE TABLE IF NOT EXISTS verse.crash_buckets (
  bucket text PRIMARY KEY,            -- md5(source || ' ' || sig)
  source text NOT NULL CHECK (source IN ('room', 'page')),
  sig text NOT NULL,                  -- the report's sig line: the kind and its innermost Coax functions
  first_ms bigint NOT NULL,
  last_ms bigint NOT NULL,            -- the newest report, by the API's clock
  live_ms bigint,                     -- the newest report no automated browser sent
  reports bigint NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS crash_buckets_last ON verse.crash_buckets (last_ms);

-- The newest 20 reports of each bucket.
CREATE TABLE IF NOT EXISTS verse.crash_reports (
  id bigserial PRIMARY KEY,
  bucket text NOT NULL REFERENCES verse.crash_buckets (bucket) ON DELETE CASCADE,
  kept_ms bigint NOT NULL,
  at_ms bigint NOT NULL,              -- the report's own t
  source text NOT NULL,
  shard int,
  release text NOT NULL DEFAULT '',
  build text NOT NULL DEFAULT '',     -- a room binary's build ID: the name its symbols are kept under
  auto boolean NOT NULL DEFAULT false,
  body text NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS crash_reports_once ON verse.crash_reports (md5(body));   -- a report posted twice is kept once
CREATE INDEX IF NOT EXISTS crash_reports_bucket ON verse.crash_reports (bucket, id);

-- The last analytics event the watch has looked at.
CREATE TABLE IF NOT EXISTS verse.crash_cursor (
  one boolean PRIMARY KEY DEFAULT true CHECK (one),
  event_id bigint NOT NULL
);

-- Keeps one checked report; its bucket. A report whose bytes are already kept counts nothing.
-- Buckets unheard for 90 days go, with their reports.
CREATE OR REPLACE FUNCTION verse.crash_add(now_ms bigint, at_ms_ bigint, source_ text, shard_ int, release_ text, build_ text, sig_ text, auto_ boolean, body_ text)
RETURNS text LANGUAGE sql AS $$
  INSERT INTO verse.crash_buckets (bucket, source, sig, first_ms, last_ms)
  VALUES (md5(source_ || ' ' || sig_), source_, sig_, now_ms, now_ms)
  ON CONFLICT (bucket) DO NOTHING;
  WITH kept AS (
    INSERT INTO verse.crash_reports (bucket, kept_ms, at_ms, source, shard, release, build, auto, body)
    VALUES (md5(source_ || ' ' || sig_), now_ms, at_ms_, source_, shard_, coalesce(release_, ''), coalesce(build_, ''), auto_, body_)
    ON CONFLICT (md5(body)) DO NOTHING RETURNING bucket
  )
  UPDATE verse.crash_buckets k SET last_ms = now_ms, reports = k.reports + 1,
    live_ms = CASE WHEN auto_ THEN k.live_ms ELSE now_ms END
  FROM kept WHERE k.bucket = kept.bucket;
  DELETE FROM verse.crash_reports r WHERE r.bucket = md5(source_ || ' ' || sig_)
    AND r.id NOT IN (SELECT x.id FROM verse.crash_reports x WHERE x.bucket = md5(source_ || ' ' || sig_) ORDER BY x.id DESC LIMIT 20);
  DELETE FROM verse.crash_buckets k WHERE k.last_ms < now_ms - 90::bigint * 86400000;
  SELECT md5(source_ || ' ' || sig_);
$$;

-- The pages' `crash` events since the last call that carry a trap line or frames (host.js trapped),
-- at most `most`, and the cursor moved past them. The first call starts from now: nothing older.
-- o_auto: an automation mark on the event (a driven browser, a robot, a tool), but not our team's.
CREATE OR REPLACE FUNCTION verse.crash_page_take(most int)
RETURNS TABLE (o_ms bigint, o_page text, o_auto boolean, o_rel text, o_msg text, o_trap text, o_frames text)
LANGUAGE sql AS $$
  INSERT INTO verse.crash_cursor (one, event_id) SELECT true, coalesce(max(e.id), 0) FROM analytics.events e
  ON CONFLICT (one) DO NOTHING;
  WITH c AS (
    SELECT k.event_id AS from_id, coalesce((SELECT max(e.id) FROM analytics.events e), k.event_id) AS upto FROM verse.crash_cursor k
  ), took AS (
    SELECT e.id, e.ms, e.page, e.auto, e.p FROM analytics.events e, c
    WHERE e.id > c.from_id AND e.id <= c.upto AND e.ev = 'crash' AND (e.p->>'f0' IS NOT NULL OR e.p->>'trap' IS NOT NULL)
    ORDER BY e.id LIMIT most
  ), moved AS (
    UPDATE verse.crash_cursor k SET event_id = CASE WHEN (SELECT count(*) FROM took) >= most THEN (SELECT max(t.id) FROM took t) ELSE (SELECT c.upto FROM c) END
    RETURNING 1
  )
  SELECT t.ms, t.page, t.auto IS NOT NULL AND t.auto <> 'team', coalesce(t.p->>'rel', ''), coalesce(t.p->>'msg', ''), coalesce(t.p->>'trap', ''),
         concat_ws(E'\n', t.p->>'f0', t.p->>'f1', t.p->>'f2', t.p->>'f3', t.p->>'f4', t.p->>'f5', t.p->>'f6', t.p->>'f7')
  FROM took t, (SELECT count(*) FROM moved) m ORDER BY t.id;
$$;

-- What the alerts are decided from: each room bucket heard within the window (its age, and its
-- newest report's shard and release), and one row for the pages (bucket ''): the age of the newest
-- report no automated browser sent, that bucket's signature, and how many buckets were heard.
-- An age is never below 0: a report kept after the watch read its clock is new, not in the future.
CREATE OR REPLACE FUNCTION verse.crash_facts(now_ms bigint, window_ms bigint)
RETURNS TABLE (bucket text, source text, sig text, age_ms bigint, shard int, release text, buckets int)
LANGUAGE sql STABLE AS $$
  SELECT k.bucket, k.source, k.sig, greatest(now_ms - k.last_ms, 0), n.shard, coalesce(n.release, ''), 1
  FROM verse.crash_buckets k
  LEFT JOIN LATERAL (SELECT r.shard, r.release FROM verse.crash_reports r WHERE r.bucket = k.bucket ORDER BY r.id DESC LIMIT 1) n ON true
  WHERE k.source = 'room' AND k.last_ms > now_ms - window_ms
  UNION ALL
  SELECT '', 'page', coalesce((SELECT k.sig FROM verse.crash_buckets k WHERE k.source = 'page' AND k.live_ms IS NOT NULL ORDER BY k.live_ms DESC LIMIT 1), ''),
         (SELECT CASE WHEN max(k.live_ms) IS NULL THEN -1 ELSE greatest(now_ms - max(k.live_ms), 0) END FROM verse.crash_buckets k WHERE k.source = 'page'), NULL, '',
         (SELECT count(*) FROM verse.crash_buckets k WHERE k.source = 'page' AND k.live_ms > now_ms - window_ms)::int
  ORDER BY 2 DESC, 1 $$;

-- GET /api/playtest/crashes: the 50 buckets heard last in 30 days, each with its newest report.
CREATE OR REPLACE FUNCTION verse.crash_list(now_ms bigint) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(to_jsonb(x) ORDER BY x.last_ms DESC, x.bucket), '[]'::jsonb) FROM (
    SELECT k.bucket, k.source, k.sig, k.first_ms, k.last_ms, k.reports, n.shard, n.release, n.build, n.body AS newest
    FROM verse.crash_buckets k
    LEFT JOIN LATERAL (SELECT r.shard, r.release, r.build, r.body FROM verse.crash_reports r WHERE r.bucket = k.bucket ORDER BY r.id DESC LIMIT 1) n ON true
    WHERE k.last_ms > now_ms - 30::bigint * 86400000
    ORDER BY k.last_ms DESC LIMIT 50) x $$;
