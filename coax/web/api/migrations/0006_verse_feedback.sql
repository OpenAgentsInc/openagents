-- Feedback from players (pt-feedback; player.c answers POST
-- /api/playtest/survey/answer and /api/playtest/survey). The demo's rules
-- (aethyr-cpp tools/serve.py, tools/analytics.py clean_survey_answer at
-- demo/kiki-2026-10-28d): one answer a record, written once, into the tables
-- the import also fills (0002_events.sql), with the session it came from.
CREATE INDEX survey_answers_sid ON analytics.survey_answers (sid);

-- {"status", "body"} for one request: parts is the path after /api/playtest/.
CREATE FUNCTION verse.feedback(parts text[], body jsonb, auto text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  a jsonb := body->'a';
  skip boolean := coalesce(body->'skip' = 'true'::jsonb, false);
  n int; total int;
  now_s double precision := extract(epoch FROM clock_timestamp());
BEGIN
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN RETURN verse.r(400, '{"error":"expected an object"}'); END IF;
  IF parts = ARRAY['survey'] THEN   -- the whole form at once (before the demo's revision 7)
    INSERT INTO analytics.surveys (id, at, answers, auto, space) VALUES (verse.hex(16), now_s, body, nullif(auto, ''), 'web');
    RETURN verse.r(200, '{}');
  END IF;
  IF parts <> ARRAY['survey', 'answer'] THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
  IF coalesce(body->>'rid', '') !~ '^[0-9a-f]{16}$' OR jsonb_typeof(body->'rid') <> 'string' THEN
    RETURN verse.r(400, '{"error":"expected a respondent id"}');
  END IF;
  IF coalesce(body->>'q', '') !~ '^[A-Za-z][A-Za-z0-9_]{0,23}$' OR jsonb_typeof(body->'q') <> 'string' THEN
    RETURN verse.r(400, '{"error":"expected a question id"}');
  END IF;
  IF skip THEN a := NULL;
  ELSIF jsonb_typeof(a) = 'string' THEN a := to_jsonb(left(btrim(a #>> '{}'), 1000));
  ELSIF jsonb_typeof(a) = 'array' THEN
    a := coalesce((SELECT jsonb_agg(to_jsonb(left(v #>> '{}', 80)) ORDER BY o) FROM jsonb_array_elements(a) WITH ORDINALITY x(v, o)
                   WHERE o <= 16 AND jsonb_typeof(v) = 'string'), '[]');
  ELSIF a IS NOT NULL AND jsonb_typeof(a) <> 'null' THEN RETURN verse.r(400, '{"error":"an answer is text or a list of choices"}');
  ELSE a := NULL;
  END IF;
  IF a IN ('""'::jsonb, '[]'::jsonb) OR a IS NULL THEN a := NULL; skip := true; END IF;
  n := CASE WHEN jsonb_typeof(body->'n') = 'number' AND (body->>'n') ~ '^[0-9]+$' AND (body->>'n')::int BETWEEN 1 AND 99 THEN (body->>'n')::int END;
  total := CASE WHEN jsonb_typeof(body->'of') = 'number' AND (body->>'of') ~ '^[0-9]+$' AND (body->>'of')::int BETWEEN 1 AND 99 THEN (body->>'of')::int END;
  INSERT INTO analytics.survey_answers (id, at, rid, q, a, skip, n, "of", player, sid, "user", joined_as, auto, space)
  VALUES (verse.hex(16), now_s, body->>'rid', body->>'q', a, skip, n, total, left(coalesce(body->>'player', ''), 32),
          left(coalesce(body->>'sid', ''), 80), left(coalesce(body->>'user', ''), 64), left(coalesce(body->>'joinedAs', ''), 120),
          nullif(auto, ''), 'web');
  RETURN verse.r(200, '{"saved":true}');
END $$;
