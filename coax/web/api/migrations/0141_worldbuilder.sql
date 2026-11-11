-- WorldBuilder (openagents.com/create-world, coax/web/api/worldbuilder.c): chat a world onto an
-- empty grid. No sign-in, no per-person limits; one cap for everyone, $50 a day of model spend
-- (worldbuilder-policy.coax decides; this keeps the numbers).
--
-- worldbuilder.days   a Chicago day's spend in millionths of a dollar: a call reserves the most it
--                     could cost before it starts (worldbuilder-policy.coax wb_admit), and settles
--                     at what Vertex says it used; calls let through and refused
-- worldbuilder.calls  one row a model call: when, its HTTP status, milliseconds, tokens and cost.
--                     No words, no address: what was typed stays with the page.
-- The weekly GCP audit reads the spend from either table, or from the API's log lines
-- ("worldbuilder: ... micros, the day's total ...").
-- Additive, and safe to run twice.
CREATE SCHEMA IF NOT EXISTS worldbuilder;

CREATE TABLE IF NOT EXISTS worldbuilder.days (
  day date PRIMARY KEY,
  spent_micros bigint NOT NULL DEFAULT 0,
  calls int NOT NULL DEFAULT 0,
  refused int NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS worldbuilder.calls (
  id bigserial PRIMARY KEY,
  at timestamptz NOT NULL DEFAULT now(),
  day date NOT NULL,
  status int NOT NULL,
  ms int NOT NULL,
  input int NOT NULL DEFAULT 0,
  cached int NOT NULL DEFAULT 0,
  output int NOT NULL DEFAULT 0,
  reserved bigint NOT NULL DEFAULT 0,
  micros bigint NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS worldbuilder_calls_day ON worldbuilder.calls (day);

-- Today, a Chicago day (the owner's): the cap resets at midnight there, and the page says so.
CREATE OR REPLACE FUNCTION worldbuilder.today() RETURNS date LANGUAGE sql STABLE AS $$
  SELECT (now() AT TIME ZONE 'America/Chicago')::date $$;

-- A model's generateContent answer as lines for the page (apps/openagents-web/worldbuilder.coax
-- reads them; nothing here reads words): "S" and the one line to show, then "A" and one edit a line,
-- its fields separated by "|" in this order:
--   op|asset|count|targets|names|anchor|anchor_names|side|distance|spread|size|factor|degrees
-- names and anchor_names joined by ",". "|", "," and line breaks inside a value become spaces.
-- NULL when the answer has no JSON object the schema allows.
CREATE OR REPLACE FUNCTION worldbuilder.clean(t text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(regexp_replace(coalesce(t, ''), '[|,\r\n]', ' ', 'g'), 200) $$;
CREATE OR REPLACE FUNCTION worldbuilder.num(j jsonb, k text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(j->k) = 'number' THEN (j->>k) ELSE '' END $$;
CREATE OR REPLACE FUNCTION worldbuilder.list(j jsonb, k text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(j->k) = 'array'
    THEN coalesce((SELECT string_agg(worldbuilder.clean(e), ',') FROM (SELECT jsonb_array_elements_text(j->k) e LIMIT 64) x), '')
    ELSE '' END $$;
CREATE OR REPLACE FUNCTION worldbuilder.lines(answer text) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE j jsonb; a jsonb; out text;
BEGIN
  BEGIN j := answer::jsonb; EXCEPTION WHEN others THEN RETURN NULL; END;
  IF jsonb_typeof(j) <> 'object' OR jsonb_typeof(j->'actions') IS DISTINCT FROM 'array' THEN RETURN NULL; END IF;
  out := 'S ' || worldbuilder.clean(j->>'say') || E'\n';
  FOR a IN SELECT e FROM jsonb_array_elements(j->'actions') e LIMIT 24 LOOP
    IF jsonb_typeof(a) <> 'object' THEN CONTINUE; END IF;
    out := out || 'A ' || worldbuilder.clean(a->>'op') || '|' || worldbuilder.clean(a->>'asset') || '|' || worldbuilder.num(a, 'count')
      || '|' || worldbuilder.clean(a->>'targets') || '|' || worldbuilder.list(a, 'names') || '|' || worldbuilder.clean(a->>'anchor')
      || '|' || worldbuilder.list(a, 'anchor_names') || '|' || worldbuilder.clean(a->>'side') || '|' || worldbuilder.num(a, 'distance')
      || '|' || worldbuilder.num(a, 'spread') || '|' || worldbuilder.num(a, 'size') || '|' || worldbuilder.num(a, 'factor')
      || '|' || worldbuilder.num(a, 'degrees') || E'\n';
  END LOOP;
  RETURN out;
END $$;

-- The text a generateContent answer carries (its parts that aren't thoughts) and its usage:
-- prompt tokens, cached tokens, output tokens (candidates and thoughts).
CREATE OR REPLACE FUNCTION worldbuilder.answer_text(b jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT string_agg(p->>'text', '') FROM jsonb_array_elements(coalesce(b #> '{candidates,0,content,parts}', '[]'::jsonb)) p
  WHERE NOT coalesce((p->>'thought')::boolean, false) $$;
