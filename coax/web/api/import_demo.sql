-- One import of the demo's data (import_demo.py sends the rows on standard
-- input, one a record, and runs this in one transaction). Every insert is keyed on the
-- demo's own ids, so a second run with the same files changes nothing; rows
-- the import made carry their source (0009_demo_import.sql). Nothing here
-- updates or deletes a row openagents.com wrote itself.
\set ON_ERROR_STOP 1
BEGIN;
CREATE TEMP TABLE demo (kind text NOT NULL, key text NOT NULL, space text NOT NULL, doc jsonb NOT NULL) ON COMMIT DROP;
\copy demo FROM pstdin

-- Events: one a line of <space>/events/*.jsonl, its key the file and line.
WITH n AS (
  INSERT INTO analytics.events (ms, sid, acct, ev, page, ct, p, auto, space, host, import_key)
  SELECT (doc->>'t')::bigint, doc->>'sid', nullif(doc->>'acct', ''), doc->>'ev', left(coalesce(doc->>'page', ''), 32),
         CASE WHEN jsonb_typeof(doc->'ct') = 'number' THEN floor((doc->>'ct')::numeric)::bigint END,
         CASE WHEN jsonb_typeof(doc->'p') = 'object' THEN doc->'p' ELSE '{}' END,
         nullif(doc->>'auto', ''), space, 'playtest.openagents.com', key
  FROM demo WHERE kind = 'event' ORDER BY key
  ON CONFLICT (import_key) DO NOTHING RETURNING 1)
SELECT 'added', 'events', count(*) FROM n;

-- Sessions, from the imported events: first and last seen, events, the
-- robot agent the demo noted on session-start.
WITH s AS (
  SELECT sid, min(ms) AS f, max(ms) AS l, min(space) AS sp, count(*)::int AS n,
         (array_agg(p->>'agent' ORDER BY ms) FILTER (WHERE ev = 'session-start' AND p ? 'agent'))[1] AS ag
  FROM analytics.events WHERE import_key LIKE 'aethyr-demo-data/%' GROUP BY sid
), n AS (
  INSERT INTO analytics.sessions AS x (sid, first_ms, last_ms, space, host, agent, events)
  SELECT sid, f, l, sp, 'playtest.openagents.com', ag, n FROM s
  ON CONFLICT (sid) DO UPDATE SET first_ms = EXCLUDED.first_ms, last_ms = EXCLUDED.last_ms, agent = EXCLUDED.agent, events = EXCLUDED.events
  WHERE x.space IN ('kiki', 'dev')
    AND (x.first_ms, x.last_ms, x.agent, x.events) IS DISTINCT FROM (EXCLUDED.first_ms, EXCLUDED.last_ms, EXCLUDED.agent, EXCLUDED.events)
  RETURNING 1)
SELECT 'added', 'sessions', count(*) FROM n;

-- Players (players.json is shared by both namespaces): the demo kept the
-- SHA-256 of each key, as verse.players does, so an old key still works.
-- The demo's accounts are not ours: the account is left empty.
WITH n AS (
  INSERT INTO verse.players (id, key_sha256, account, created_ms, source)
  SELECT doc->>'id', doc->>'hash', NULL, floor((doc->>'at')::numeric * 1000)::bigint, key
  FROM demo WHERE kind = 'player' ORDER BY key
  ON CONFLICT DO NOTHING RETURNING 1)
SELECT 'added', 'players', count(*) FROM n;

-- Worlds with an owner (world-owners.json).
WITH n AS (
  INSERT INTO verse.worlds (id, owner_player, created_ms, source)
  SELECT doc->>'id', (SELECT p.id FROM verse.players p WHERE p.id = doc->>'owner'),
         floor((doc->>'at')::numeric * 1000)::bigint, key
  FROM demo WHERE kind = 'world' ORDER BY key
  ON CONFLICT DO NOTHING RETURNING 1)
SELECT 'added', 'worlds', count(*) FROM n;

-- Published worlds (world-<id>.json). A world openagents.com published
-- itself is never overwritten; one the import made follows the demo.
WITH src AS (
  SELECT DISTINCT ON (doc->>'world') doc->>'world' AS world, doc->'doc' AS d, (doc->>'version')::int AS v,
         left(coalesce(doc->>'publishedBy', ''), 40) AS who, (doc->>'publishedAt')::double precision AS at, key
  FROM demo WHERE kind = 'published' ORDER BY doc->>'world', space = 'kiki' DESC
), n AS (
  INSERT INTO verse.published AS x (world, doc, version, published_by, published_at, source)
  SELECT world, d, v, who, at, key FROM src
  ON CONFLICT (world) DO UPDATE SET doc = EXCLUDED.doc, version = EXCLUDED.version, published_by = EXCLUDED.published_by,
    published_at = EXCLUDED.published_at, source = EXCLUDED.source
  WHERE x.source LIKE 'aethyr-demo-data/%'
    AND (x.doc, x.version, x.published_by, x.published_at, x.source)
        IS DISTINCT FROM (EXCLUDED.doc, EXCLUDED.version, EXCLUDED.published_by, EXCLUDED.published_at, EXCLUDED.source)
  RETURNING 1)
SELECT 'added', 'published', count(*) FROM n;

-- The edit logs' publishes and resets (log-<id>.json); their offered changes
-- follow from verse.demo_files below.
WITH n AS (
  INSERT INTO verse.log (world, version, who, summary, at, state, by_player, source, import_key)
  SELECT doc->>'world', (doc->>'version')::int, left(coalesce(doc->>'who', ''), 40), left(coalesce(doc->>'summary', ''), 300),
         (doc->>'at')::double precision, doc->>'state', doc->'by'->>'player', split_part(key, '#', 1), key
  FROM demo WHERE kind = 'log' ORDER BY doc->>'world', (doc->>'at')::double precision
  ON CONFLICT (import_key) DO NOTHING RETURNING 1)
SELECT 'added', 'log', count(*) FROM n;

-- Feedback: joins, one-at-a-time answers, whole forms.
WITH n AS (
  INSERT INTO analytics.playtesters (id, at, email, player, sid, how, auto, space, source)
  SELECT doc->>'id', (doc->>'at')::double precision, doc->>'email', doc->>'player', doc->>'sid', doc->>'how',
         nullif(doc->>'auto', ''), space, key
  FROM demo WHERE kind = 'playtester' ORDER BY key
  ON CONFLICT (id) DO NOTHING RETURNING 1)
SELECT 'added', 'playtesters', count(*) FROM n;
WITH n AS (
  INSERT INTO analytics.survey_answers (id, at, rid, q, a, skip, n, "of", player, sid, "user", joined_as, auto, space, source)
  SELECT doc->>'id', (doc->>'at')::double precision, doc->>'rid', doc->>'q',
         CASE WHEN jsonb_typeof(doc->'a') IN ('string', 'array') THEN doc->'a' END,
         coalesce(doc->'skip' = 'true'::jsonb, false),
         CASE WHEN jsonb_typeof(doc->'n') = 'number' THEN (doc->>'n')::int END,
         CASE WHEN jsonb_typeof(doc->'of') = 'number' THEN (doc->>'of')::int END,
         doc->>'player', doc->>'sid', doc->>'user', doc->>'joinedAs', nullif(doc->>'auto', ''), space, key
  FROM demo WHERE kind = 'answer' ORDER BY key
  ON CONFLICT (id) DO NOTHING RETURNING 1)
SELECT 'added', 'survey_answers', count(*) FROM n;
WITH n AS (
  INSERT INTO analytics.surveys (id, at, answers, auto, space, source)
  SELECT doc->>'id', (doc->>'at')::double precision, coalesce(doc->'answers', '{}'), nullif(doc->>'auto', ''), space, key
  FROM demo WHERE kind = 'survey' ORDER BY key
  ON CONFLICT (id) DO NOTHING RETURNING 1)
SELECT 'added', 'surveys', count(*) FROM n;

-- Every file but the event logs, as it was.
WITH n AS (
  INSERT INTO verse.demo_files AS x (path, space, doc, md5, imported_ms)
  SELECT key, space, doc->'doc', doc->>'md5', verse.now_ms() FROM demo WHERE kind = 'file'
  ON CONFLICT (path) DO UPDATE SET doc = EXCLUDED.doc, md5 = EXCLUDED.md5, imported_ms = EXCLUDED.imported_ms
  WHERE x.md5 <> EXCLUDED.md5
  RETURNING 1)
SELECT 'added', 'files', count(*) FROM n;

-- The offered changes in the edit logs just kept (0010_verse_offers.sql).
SELECT 'added', 'offers', verse.offers_from_demo();
-- Awards, planets and planets' saved copies (0011_verse_planets.sql).
CREATE TEMP TABLE had ON COMMIT DROP AS SELECT (SELECT count(*) FROM verse.awards) AS a, (SELECT count(*) FROM verse.planets) AS p,
  (SELECT count(*) FROM verse.planet_saves) AS s;
SELECT 'added', 'realms', verse.planets_from_demo();
SELECT 'added', 'awards', (SELECT count(*) FROM verse.awards) - a FROM had
UNION ALL SELECT 'added', 'planets', (SELECT count(*) FROM verse.planets) - p FROM had
UNION ALL SELECT 'added', 'planet_saves', (SELECT count(*) FROM verse.planet_saves) - s FROM had;

-- What the database now holds from the demo, and the worlds openagents.com
-- had already published itself (kept, not overwritten).
SELECT 'held', 'events', count(*) FROM analytics.events WHERE import_key LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'sessions', count(*) FROM analytics.sessions s
  WHERE EXISTS (SELECT 1 FROM analytics.events e WHERE e.sid = s.sid AND e.import_key LIKE 'aethyr-demo-data/%')
UNION ALL SELECT 'held', 'players', count(*) FROM verse.players WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'worlds', count(*) FROM verse.worlds WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'published', count(*) FROM verse.published WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'log', count(*) FROM verse.log WHERE import_key LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'playtesters', count(*) FROM analytics.playtesters WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'survey_answers', count(*) FROM analytics.survey_answers WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'surveys', count(*) FROM analytics.surveys WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'files', count(*) FROM verse.demo_files
UNION ALL SELECT 'held', 'offers', count(*) FROM verse.offers WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'awards', count(*) FROM verse.awards WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'planets', count(*) FROM verse.planets WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'held', 'planet_saves', count(*) FROM verse.planet_saves WHERE source LIKE 'aethyr-demo-data/%'
UNION ALL SELECT 'kept', 'published', count(*) FROM verse.published x
  WHERE x.source IS NULL AND x.world IN (SELECT doc->>'world' FROM demo WHERE kind = 'published');
COMMIT;
