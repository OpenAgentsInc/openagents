-- Fairy Forest's courses (ff-courses, verse/fairy-forest/README.md "Trails
-- and courses"): a player hangs vine hoops through the trees, flies them
-- against the clock and shares a link (#course=<id>); whoever opens it flies
-- the same hoops and their best time shows on it. player.c hands these over:
--
--   GET  /api/playtest/course/<id>        {id, realm, by, mine, hoops, times, best}: times are the 10
--                                         fastest (one a player: {name, ms, mine}); best is yours, or null
--   POST /api/playtest/course/<id>        {realm, hoops}: hoops are [x, y, z, yaw] each, 3 to 12; a new id
--                                         is yours; yours again replaces its hoops and clears its times;
--                                         someone else's is 403 -> the course
--   POST /api/playtest/course/<id>/time   {ms}: a run flown, 1 s to an hour; your best is kept -> the course
--
-- The page makes the id (8 to 16 of [a-z0-9]) so SHARE copies the link at
-- once. Abuse: a player key for every write; at most 30 new courses a player
-- a day; a time at most every 2 s a course; names are verse.player_name's.
CREATE TABLE verse.courses (
  id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]{8,16}$'),
  realm text NOT NULL,
  owner text NOT NULL REFERENCES verse.players (id),
  hoops jsonb NOT NULL,
  made_ms bigint NOT NULL,
  changed_ms bigint NOT NULL
);
CREATE INDEX courses_owner ON verse.courses (owner, made_ms);

CREATE TABLE verse.course_times (
  course text NOT NULL REFERENCES verse.courses (id) ON DELETE CASCADE,
  player text NOT NULL REFERENCES verse.players (id),
  ms int NOT NULL CHECK (ms BETWEEN 1000 AND 3600000),
  at_ms bigint NOT NULL,
  runs int NOT NULL DEFAULT 1,
  PRIMARY KEY (course, player)
);
CREATE INDEX course_times_fast ON verse.course_times (course, ms);

-- Hoops as sent, checked: 3 to 12 of four numbers each, inside the realm's reach; else null.
CREATE FUNCTION verse.course_hoops(v jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(v) = 'array' AND jsonb_array_length(v) BETWEEN 3 AND 12
               AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(v) h
                                WHERE jsonb_typeof(h) <> 'array' OR jsonb_array_length(h) <> 4
                                   OR EXISTS (SELECT 1 FROM jsonb_array_elements(h) n
                                               WHERE jsonb_typeof(n) <> 'number' OR abs((n #>> '{}')::numeric) > 1000))
         THEN (SELECT jsonb_agg((SELECT jsonb_agg(round((n #>> '{}')::numeric, 2) ORDER BY o) FROM jsonb_array_elements(h) WITH ORDINALITY AS t(n, o)) ORDER BY ho)
                 FROM jsonb_array_elements(v) WITH ORDINALITY AS u(h, ho))
         END
$$;

CREATE FUNCTION verse.course_view(cid text, me text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('id', c.id, 'realm', c.realm, 'by', verse.player_name(c.owner), 'mine', me IS NOT NULL AND c.owner = me,
           'hoops', c.hoops,
           'times', coalesce((SELECT jsonb_agg(jsonb_build_object('name', verse.player_name(t.player), 'ms', t.ms, 'mine', me IS NOT NULL AND t.player = me)
                                               ORDER BY t.ms, t.at_ms)
                              FROM (SELECT * FROM verse.course_times x WHERE x.course = c.id ORDER BY x.ms, x.at_ms LIMIT 10) t), '[]'),
           'best', (SELECT t.ms FROM verse.course_times t WHERE t.course = c.id AND t.player = me))
  FROM verse.courses c WHERE c.id = cid
$$;

ALTER FUNCTION verse.api(text, text[], jsonb, jsonb) RENAME TO api_0015;

CREATE FUNCTION verse.api(method text, parts text[], body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  b jsonb := CASE WHEN jsonb_typeof(body) = 'object' THEN body ELSE '{}' END;
  me text;
  cid text := parts[2];
  c verse.courses;
  hs jsonb;
  realm_ text;
  ms_ int;
  now_ bigint := verse.now_ms();
BEGIN
  IF parts[1] IS DISTINCT FROM 'course' THEN RETURN verse.api_0015(method, parts, body, who); END IF;
  IF cid IS NULL OR cid !~ '^[a-z0-9]{8,16}$' OR array_length(parts, 1) > 3 OR (array_length(parts, 1) = 3 AND parts[3] <> 'time') THEN
    RETURN verse.r(404, '{"error":"unknown"}');
  END IF;
  me := verse.player_of(who->>'key');
  SELECT * INTO c FROM verse.courses WHERE id = cid;
  IF method = 'GET' THEN
    IF array_length(parts, 1) <> 2 THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
    IF c.id IS NULL THEN RETURN verse.r(404, '{"error":"no such course"}'); END IF;
    RETURN verse.r(200, verse.course_view(cid, me));
  END IF;
  IF me IS NULL THEN RETURN verse.r(403, '{"error":"a player key is needed"}'); END IF;

  IF array_length(parts, 1) = 2 THEN
    hs := verse.course_hoops(b->'hoops');
    IF hs IS NULL THEN RETURN verse.r(400, '{"error":"hoops: 3 to 12 of [x, y, z, yaw]"}'); END IF;
    IF c.id IS NULL THEN
      realm_ := CASE WHEN b->>'realm' ~ '^[a-z0-9-]{1,40}$' THEN b->>'realm' ELSE 'fairy-forest' END;
      IF (SELECT count(*) FROM verse.courses WHERE owner = me AND made_ms > now_ - 86400000) >= 30 THEN
        RETURN verse.r(429, '{"error":"that is enough courses for today"}');
      END IF;
      INSERT INTO verse.courses (id, realm, owner, hoops, made_ms, changed_ms) VALUES (cid, realm_, me, hs, now_, now_);
    ELSIF c.owner <> me THEN
      RETURN verse.r(403, '{"error":"that course is someone else''s"}');
    ELSIF c.hoops IS DISTINCT FROM hs THEN
      UPDATE verse.courses SET hoops = hs, changed_ms = now_ WHERE id = cid;
      DELETE FROM verse.course_times WHERE course = cid;
    END IF;
    RETURN verse.r(200, verse.course_view(cid, me));
  END IF;

  -- a time
  IF c.id IS NULL THEN RETURN verse.r(404, '{"error":"no such course"}'); END IF;
  IF jsonb_typeof(b->'ms') <> 'number' THEN RETURN verse.r(400, '{"error":"ms: the run in milliseconds"}'); END IF;
  ms_ := round((b->>'ms')::numeric);
  IF ms_ < 1000 OR ms_ > 3600000 THEN RETURN verse.r(400, '{"error":"ms: 1 s to an hour"}'); END IF;
  IF EXISTS (SELECT 1 FROM verse.course_times WHERE course = cid AND player = me AND at_ms > now_ - 2000) THEN
    RETURN verse.r(429, '{"error":"one run at a time"}');
  END IF;
  INSERT INTO verse.course_times (course, player, ms, at_ms) VALUES (cid, me, ms_, now_)
    ON CONFLICT (course, player) DO UPDATE SET ms = least(verse.course_times.ms, EXCLUDED.ms), at_ms = EXCLUDED.at_ms,
      runs = verse.course_times.runs + 1;
  RETURN verse.r(200, verse.course_view(cid, me));
END
$$;
