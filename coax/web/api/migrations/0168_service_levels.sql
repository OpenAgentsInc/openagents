-- Service levels and alerts (#13297, docs/deployment/service-levels.md). service-policy.coax
-- (#authority) decides what a good minute, a good join and an alert are; this stores the rows,
-- counts them for the thresholds service.c passes in, and keeps the alerts' open and resolved
-- times. Additive, and safe to run twice.

-- Each shard's minute, for the month's service levels: kept 400 days (verse.room_stats keeps a day).
-- Rolled up from every room-stats report (POST /api/playtest/room-stats, every 10 s a shard) by the
-- trigger below: the most players, the worst tick p99 and the worst unsaved checkpoint age it saw.
CREATE TABLE IF NOT EXISTS verse.room_minutes (
  shard int NOT NULL CHECK (shard BETWEEN 1 AND 99),
  minute bigint NOT NULL,          -- the minute's start, Unix ms
  reports int NOT NULL DEFAULT 0,
  players int NOT NULL DEFAULT 0,
  tick_p99_us bigint,              -- "tick p99 ms=" in microseconds
  unsaved_ms bigint,               -- "checkpoint unsaved age ms most=" (rooms from #13297 on)
  PRIMARY KEY (shard, minute)
);
CREATE INDEX IF NOT EXISTS room_minutes_minute ON verse.room_minutes (minute);
CREATE INDEX IF NOT EXISTS room_stats_at ON verse.room_stats (at);

-- One number from a report's "NAME=VALUE" line, or NULL (NAME: words and spaces only).
CREATE OR REPLACE FUNCTION verse.room_stat(b text, name text) RETURNS numeric LANGUAGE sql IMMUTABLE AS $$
  SELECT nullif(substring(b FROM '\n' || name || '=(-?[0-9]{1,15}(\.[0-9]{1,6})?)\n'), '')::numeric $$;

CREATE OR REPLACE FUNCTION verse.room_minutes_add() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  m bigint := (extract(epoch FROM NEW.at) * 1000)::bigint / 60000 * 60000;
  p int := coalesce(verse.room_stat(NEW.body, 'players'), 0)::int;
  t bigint := round(verse.room_stat(NEW.body, 'tick p99 ms') * 1000)::bigint;
  u bigint := verse.room_stat(NEW.body, 'checkpoint unsaved age ms most')::bigint;
BEGIN
  INSERT INTO verse.room_minutes AS r (shard, minute, reports, players, tick_p99_us, unsaved_ms) VALUES (NEW.shard, m, 1, p, t, u)
  ON CONFLICT (shard, minute) DO UPDATE SET reports = r.reports + 1, players = greatest(r.players, EXCLUDED.players),
    tick_p99_us = greatest(r.tick_p99_us, EXCLUDED.tick_p99_us), unsaved_ms = greatest(r.unsaved_ms, EXCLUDED.unsaved_ms);
  RETURN NULL;
END $$;
DROP TRIGGER IF EXISTS room_minutes_after_stats ON verse.room_stats;
CREATE TRIGGER room_minutes_after_stats AFTER INSERT ON verse.room_stats FOR EACH ROW EXECUTE FUNCTION verse.room_minutes_add();

-- The day of reports already kept, rolled up once.
INSERT INTO verse.room_minutes AS r (shard, minute, reports, players, tick_p99_us, unsaved_ms)
SELECT shard, (extract(epoch FROM at) * 1000)::bigint / 60000 * 60000, count(*),
       max(coalesce(verse.room_stat(body, 'players'), 0))::int, max(round(verse.room_stat(body, 'tick p99 ms') * 1000))::bigint,
       max(verse.room_stat(body, 'checkpoint unsaved age ms most'))::bigint
FROM verse.room_stats GROUP BY 1, 2
ON CONFLICT (shard, minute) DO NOTHING;

-- Each shard's facts for the alerts, at now_ms: whether its last report named its address and said
-- it drains, how long since it last reported, and from its stats reports the ones with a tick p95
-- over slow_us and the worst unsaved checkpoint within w_slow ms, the checkpoint failures within
-- w_fail ms and the checkpoints fenced within w_fenced ms. A counter's rise is its growth from the
-- report before, within one process (its generation); a new process counts from its start.
CREATE OR REPLACE FUNCTION verse.service_shard_facts(now_ms bigint, slow_us bigint, w_slow bigint, w_fail bigint, w_fenced bigint)
RETURNS TABLE (shard int, addressed boolean, draining boolean, silent_ms bigint, slow int, failures int, fenced int, unsaved_ms bigint)
LANGUAGE sql STABLE AS $$
  WITH r AS (
    SELECT s.shard, (extract(epoch FROM s.at) * 1000)::bigint AS at_ms,
           verse.room_stat(s.body, 'process generation') AS gen,
           round(verse.room_stat(s.body, 'tick p95 ms') * 1000) AS p95_us,
           verse.room_stat(s.body, 'checkpoint failures') AS fails,
           verse.room_stat(s.body, 'checkpoint stale owner refusals') AS fences,
           verse.room_stat(s.body, 'checkpoint unsaved age ms most') AS unsaved
    FROM verse.room_stats s
    WHERE s.at >= to_timestamp((now_ms - greatest(w_slow, w_fail, w_fenced) - 60000) / 1000.0)
  ), d AS (
    SELECT r.*, lag(gen) OVER w AS gen0, lag(fails) OVER w AS fails0, lag(fences) OVER w AS fences0
    FROM r WINDOW w AS (PARTITION BY r.shard ORDER BY r.at_ms)
  ), x AS (
    SELECT d.shard, d.at_ms, d.p95_us, d.unsaved,
           CASE WHEN d.gen0 IS NULL THEN 0 WHEN d.gen0 = d.gen THEN greatest(coalesce(d.fails - d.fails0, 0), 0) ELSE coalesce(d.fails, 0) END AS dfail,
           CASE WHEN d.gen0 IS NULL THEN 0 WHEN d.gen0 = d.gen THEN greatest(coalesce(d.fences - d.fences0, 0), 0) ELSE coalesce(d.fences, 0) END AS dfence
    FROM d
  )
  SELECT k.shard, k.endpoint IS NOT NULL, coalesce(k.draining IS NOT NULL AND k.draining = k.gen, false),
         now_ms - greatest(coalesce(k.at, 0), coalesce(k.seen_at, 0)),
         (SELECT count(*) FROM x WHERE x.shard = k.shard AND x.at_ms > now_ms - w_slow AND x.p95_us > slow_us)::int,
         coalesce((SELECT sum(x.dfail) FROM x WHERE x.shard = k.shard AND x.at_ms > now_ms - w_fail), 0)::int,
         coalesce((SELECT sum(x.dfence) FROM x WHERE x.shard = k.shard AND x.at_ms > now_ms - w_fenced), 0)::int,
         coalesce((SELECT max(x.unsaved) FROM x WHERE x.shard = k.shard AND x.at_ms > now_ms - w_slow), 0)::bigint
  FROM verse.room_shards k ORDER BY k.shard $$;

-- The alerts: one open row a key at most; a resolved one keeps its times (400 days).
CREATE TABLE IF NOT EXISTS verse.service_alerts (
  id bigserial PRIMARY KEY,
  key text NOT NULL,
  title text NOT NULL,
  todo text NOT NULL DEFAULT '',   -- the runbook line
  opened_ms bigint NOT NULL,
  resolved_ms bigint
);
CREATE UNIQUE INDEX IF NOT EXISTS service_alerts_open ON verse.service_alerts (key) WHERE resolved_ms IS NULL;
CREATE INDEX IF NOT EXISTS service_alerts_opened ON verse.service_alerts (opened_ms);

-- One run of the watch (service.c, every minute): alerts is the JSON array of what holds now,
-- [{"key", "title", "do"}]. A key not open opens; an open key not in it resolves. Answers
-- {"opened": [...], "resolved": [...], "open": N}, each alert {id, key, title, todo, opened_ms}.
-- Old rows go: minutes and resolved alerts after 400 days.
CREATE OR REPLACE FUNCTION verse.service_sync(now_ms bigint, alerts jsonb) RETURNS jsonb LANGUAGE sql AS $$
  WITH want AS (
    SELECT DISTINCT ON (a->>'key') a->>'key' AS key, coalesce(a->>'title', a->>'key') AS title, coalesce(a->>'do', '') AS todo
    FROM jsonb_array_elements(CASE WHEN jsonb_typeof(alerts) = 'array' THEN alerts ELSE '[]'::jsonb END) a
    WHERE jsonb_typeof(a) = 'object' AND a->>'key' IS NOT NULL
  ), opened AS (
    INSERT INTO verse.service_alerts (key, title, todo, opened_ms) SELECT w.key, w.title, w.todo, now_ms FROM want w
    ON CONFLICT (key) WHERE resolved_ms IS NULL DO NOTHING
    RETURNING id, key, title, todo, opened_ms
  ), resolved AS (
    UPDATE verse.service_alerts a SET resolved_ms = now_ms
    WHERE a.resolved_ms IS NULL AND NOT EXISTS (SELECT 1 FROM want w WHERE w.key = a.key)
    RETURNING a.id, a.key, a.title, a.todo, a.opened_ms
  ), aged AS (
    DELETE FROM verse.room_minutes WHERE minute < now_ms - 400::bigint * 86400000 RETURNING 1
  ), forgotten AS (
    DELETE FROM verse.service_alerts WHERE resolved_ms < now_ms - 400::bigint * 86400000 RETURNING 1
  )
  SELECT jsonb_build_object(
    'opened', coalesce((SELECT jsonb_agg(to_jsonb(o) ORDER BY o.key) FROM opened o), '[]'::jsonb),
    'resolved', coalesce((SELECT jsonb_agg(to_jsonb(r) ORDER BY r.key) FROM resolved r), '[]'::jsonb),
    'open', (SELECT count(*) FROM verse.service_alerts WHERE resolved_ms IS NULL) + (SELECT count(*) FROM opened) - (SELECT count(*) FROM resolved)) $$;

-- The mail for one run's opened and resolved alerts (email.begin's messages), to the address
-- given, words only: env is "production" or "staging", origin the site's, now_ms the run's time.
CREATE OR REPLACE FUNCTION verse.service_mail(sync jsonb, to_addr text, env text, origin text, now_ms bigint) RETURNS SETOF jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('to', to_addr,
    'subject', '[' || env || '] ' || CASE WHEN t.what = 'opened' THEN '' ELSE 'resolved: ' END || (a->>'title'),
    'text', CASE WHEN t.what = 'opened' THEN 'Opened ' ELSE 'Resolved ' END
            || to_char(to_timestamp(CASE WHEN t.what = 'opened' THEN (a->>'opened_ms')::bigint ELSE now_ms END / 1000.0) AT TIME ZONE 'America/Chicago', 'YYYY-MM-DD HH24:MI')
            || ' Chicago time, on ' || env || E'.\n\n' || (a->>'title')
            || CASE WHEN t.what = 'opened' AND a->>'todo' <> '' THEN E'\n\nDo: ' || (a->>'todo') ELSE '' END
            || E'\n\nNow: ' || origin || E'/api/playtest/service-health\nRunbook: https://github.com/OpenAgentsInc/openagents/blob/main/docs/deployment/service-levels.md\n',
    'idempotency_key', 'service-alert-' || env || '-' || (a->>'id') || '-' || t.what)
  FROM (SELECT 'opened' AS what, jsonb_array_elements(coalesce(sync->'opened', '[]'::jsonb)) AS a
        UNION ALL SELECT 'resolved', jsonb_array_elements(coalesce(sync->'resolved', '[]'::jsonb))) t $$;

-- The service levels over the last days, for the thresholds service.c passes in: one row each
-- (0 tick time, 1 join time, 2 checkpoint age) with the good count, the count, the worst value and
-- each Chicago day's [day, good, count]. Tick time and checkpoint age count shard-minutes with
-- players; join time counts the pages' room-join events (analytics.events, not our team's), each
-- from Play: the latest of the page's navigation, its Character Create's USE IN EVERGLADE and its
-- door's PLAY NOW in the hour before.
CREATE OR REPLACE FUNCTION verse.service_levels(now_ms bigint, days int, tick_us bigint, join_ms bigint, ckpt_ms bigint)
RETURNS TABLE (which int, good bigint, total bigint, worst bigint, daily jsonb)
LANGUAGE sql STABLE AS $$
  WITH m AS (
    SELECT to_char(to_timestamp(minute / 1000.0) AT TIME ZONE 'America/Chicago', 'YYYY-MM-DD') AS day, tick_p99_us, unsaved_ms
    FROM verse.room_minutes WHERE minute >= now_ms - days::bigint * 86400000 AND players > 0
  ), j AS (
    SELECT to_char(to_timestamp(e.ms / 1000.0) AT TIME ZONE 'America/Chicago', 'YYYY-MM-DD') AS day,
           (e.ms - greatest(e.ms - (e.p->>'ms')::float8,
              coalesce((SELECT max(s.ms) FROM analytics.events s WHERE s.sid = e.sid AND s.ev IN ('create-done', 'portal-shown')
                        AND s.ms <= e.ms AND s.ms >= e.ms - 3600000), 0)))::bigint AS wait
    FROM analytics.events e
    WHERE e.ev = 'room-join' AND e.ms >= now_ms - days::bigint * 86400000 AND e.auto IS DISTINCT FROM 'team'
      AND jsonb_typeof(e.p->'ms') = 'number' AND (e.p->>'ms')::float8 >= 0
  ), v AS (
    SELECT 0 AS which, day, tick_p99_us AS value, tick_p99_us <= tick_us AS ok FROM m WHERE tick_p99_us IS NOT NULL
    UNION ALL SELECT 1, day, wait, wait <= join_ms FROM j
    UNION ALL SELECT 2, day, unsaved_ms, unsaved_ms <= ckpt_ms FROM m WHERE unsaved_ms IS NOT NULL
  ), d AS (
    SELECT which, day, count(*) FILTER (WHERE ok) AS good, count(*) AS total FROM v GROUP BY 1, 2
  )
  SELECT w.which, coalesce((SELECT sum(d.good) FROM d WHERE d.which = w.which), 0)::bigint,
         coalesce((SELECT sum(d.total) FROM d WHERE d.which = w.which), 0)::bigint,
         (SELECT max(v.value) FROM v WHERE v.which = w.which)::bigint,
         coalesce((SELECT jsonb_agg(jsonb_build_array(d.day, d.good, d.total) ORDER BY d.day) FROM d WHERE d.which = w.which), '[]'::jsonb)
  FROM (VALUES (0), (1), (2)) w(which) ORDER BY w.which $$;
