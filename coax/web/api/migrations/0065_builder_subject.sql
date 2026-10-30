-- The board always holds the thing asked for (byo-plan-subject). "Build a small red house next to a
-- tall tree" came back as pine trees and a camp: the catalogue had no house, and nothing checked that
-- the main thing asked for was on the board. Now:
--   - a house is in the catalogue (myrealm.coax draws it: walls in its colour, a roof, a door);
--   - the planner's typed answer names the main things asked for first ("asked": each a few of the
--     visitor's words and the catalogue feature that makes it, or none), under the response schema;
--   - builder_answer checks the board against that list, never against the prompt's words: a board
--     missing an asked-for feature (or holding it off) is asked again once, told what it left out;
--     when the second answer still misses it, or fails, the feature is put on from its typed fields.
-- realms.c calls the 6-argument builder_answer (with the request, so a retry can be made from it);
-- the 5-argument one stays for an older server still running.

-- A house, first in the order: what someone asks for most often is somewhere to live.
ALTER FUNCTION verse.builder_catalogue() RENAME TO builder_catalogue_0064;
CREATE FUNCTION verse.builder_catalogue() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_catalogue_0064() || '{
    "house": {"says": "a house: a cottage, hut, cabin, shed or shop, with walls in a colour, a roof and a door",
              "colour": ["white", "red", "blue", "green", "yellow", "brown", "grey"], "size": ["small", "medium", "large"],
              "count": [1, 5, 1], "most": 1}}'::jsonb $$;
ALTER FUNCTION verse.builder_order() RENAME TO builder_order_0064;
CREATE FUNCTION verse.builder_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['house'] || verse.builder_order_0064() $$;

ALTER FUNCTION verse.builder_label(jsonb) RENAME TO builder_label_0064;
CREATE FUNCTION verse.builder_label(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN f->>'feature' = 'house' THEN
    CASE WHEN coalesce((f->>'count')::int, 1) = 1 THEN 'A ' || (f->>'size') || ' ' || (f->>'colour') || ' house'
         ELSE (f->>'count') || ' ' || (f->>'size') || ' ' || (f->>'colour') || ' houses' END
  ELSE verse.builder_label_0064(f) END $$;

-- The main things asked for, as the planner answers them: at most 6, each {thing, feature}.
CREATE FUNCTION verse.builder_wants(v jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(v) = 'array' THEN coalesce((
    SELECT jsonb_agg(jsonb_build_object('thing', verse.builder_line(e->'thing', 60),
                                        'feature', CASE WHEN e->>'feature' = ANY (verse.builder_order()) THEN e->>'feature' ELSE 'none' END) ORDER BY o)
    FROM jsonb_array_elements(v) WITH ORDINALITY AS t(e, o)
    WHERE o <= 6 AND jsonb_typeof(e) = 'object' AND verse.builder_line(e->'thing', 60) <> ''), '[]') END $$;

-- What is asked for and not on the board, on: each {thing, feature}, a feature once. A tool is left
-- to the board's own tools (which one is the setup's and the planner's).
CREATE FUNCTION verse.builder_missing(wants jsonb, board jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(jsonb_agg(w ORDER BY o), '[]') FROM (
    SELECT DISTINCT ON (w->>'feature') w, o FROM jsonb_array_elements(coalesce(wants, '[]')) WITH ORDINALITY AS t(w, o)
    WHERE w->>'feature' NOT IN ('none', 'tool')
      AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(board->'features', '[]')) f
                      WHERE f->>'feature' = w->>'feature' AND coalesce((f->>'on')::boolean, false))
    ORDER BY w->>'feature', o) x $$;

-- The board with what is missing put on: one of that feature already there (off) is switched on,
-- else the feature is added from its typed fields (a character named as asked), held again.
CREATE FUNCTION verse.builder_repair(board jsonb, missing jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  m jsonb;
  feats jsonb := coalesce(board->'features', '[]');
  at int;
  held jsonb;
BEGIN
  FOR m IN SELECT e FROM jsonb_array_elements(coalesce(missing, '[]')) e LOOP
    SELECT o - 1 INTO at FROM jsonb_array_elements(feats) WITH ORDINALITY AS t(f, o) WHERE f->>'feature' = m->>'feature' ORDER BY o LIMIT 1;
    IF at IS NOT NULL THEN
      feats := jsonb_set(feats, ARRAY[at::text, 'on'], 'true');
    ELSE
      feats := feats || jsonb_build_array(jsonb_build_object('feature', m->>'feature', 'on', true)
                 || CASE WHEN m->>'feature' = 'character' THEN jsonb_build_object('name', left(m->>'thing', 24)) ELSE '{}' END);
    END IF;
  END LOOP;
  held := verse.builder_hold(board || jsonb_build_object('features', feats));
  RETURN coalesce(held, board);
END $$;

-- The planner is told to name what was asked for first, and to put each on the board.
ALTER FUNCTION verse.builder_prompt() RENAME TO builder_prompt_0064;
CREATE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_prompt_0064()
    || E'\nFirst, in "asked", name the main things they asked for, the one they asked for most first: each a few of their own words '
    || '(like "small red house" or "tall tree") and the feature from the catalogue that makes it, or none when nothing in it does. '
    || 'Every asked thing with a feature must be on the board, on true, with the fields that fit what they said (a red house is colour red). '
    || 'Then add what else fits.' $$;

-- The response schema: "asked" first, and every feature's colour (a house's too).
ALTER FUNCTION verse.builder_schema() RENAME TO builder_schema_0064;
CREATE FUNCTION verse.builder_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(jsonb_set(jsonb_set(s,
      '{properties,asked}', jsonb_build_object('type', 'array', 'items', jsonb_build_object('type', 'object', 'required', jsonb_build_array('thing', 'feature'),
        'propertyOrdering', jsonb_build_array('thing', 'feature'),
        'properties', jsonb_build_object('thing', jsonb_build_object('type', 'string'),
                                         'feature', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.builder_order() || ARRAY['none'])))))),
      '{required}', '["asked"]'::jsonb || (s->'required')),
      '{properties,features,items,properties,colour,enum}',
      (SELECT jsonb_agg(DISTINCT k) FROM jsonb_each(verse.builder_catalogue()) e, jsonb_array_elements_text(coalesce(e.value->'colour', '[]')) k))
    || jsonb_build_object('propertyOrdering', '["asked"]'::jsonb || (s->'required'))
  FROM (SELECT verse.builder_schema_0064() AS s) x $$;

ALTER TABLE verse.realm_plans ADD COLUMN asked_for jsonb, ADD COLUMN retried boolean NOT NULL DEFAULT false, ADD COLUMN repaired jsonb;

-- The request asked again: the first answer, and what it left out.
CREATE FUNCTION verse.builder_retry(request jsonb, said text, missing jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(request, '{contents}', (request->'contents') || jsonb_build_array(
    jsonb_build_object('role', 'model', 'parts', jsonb_build_array(jsonb_build_object('text', coalesce(said, '')))),
    jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text',
      'That board leaves out what they asked for: '
      || (SELECT string_agg('"' || (m->>'thing') || '" (the ' || (m->>'feature') || ' feature)', ', ') FROM jsonb_array_elements(missing) m)
      || '. Answer the whole board again with each of those on it, on true, and keep the rest.'))))) $$;

-- The model's text in a generateContent answer, or NULL.
CREATE FUNCTION verse.builder_said(response text) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  r jsonb;
BEGIN
  BEGIN r := response::jsonb; EXCEPTION WHEN others THEN RETURN NULL; END;
  RETURN (SELECT string_agg(p->>'text', '') FROM jsonb_array_elements(coalesce(r->'candidates'->0->'content'->'parts', '[]')) p
          WHERE coalesce((p->>'thought')::boolean, false) = false AND p ? 'text');
END $$;

-- The model's answer, held, checked against what was asked for and kept. asked: {id, setup} from
-- builder_ask, and {retry: true} on the second try; request: the first try's request (NULL on the
-- second). Answers as the 5-argument one does, with "asked", or {status 200, retry: REQUEST} when
-- the board leaves out something asked for and may be asked again (the board kept meanwhile is
-- already repaired, so a failed second try still answers).
CREATE FUNCTION verse.builder_answer(asked jsonb, http int, response text, model text, taken int, request jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  again boolean := coalesce((asked->>'retry')::boolean, false);
  plan_id bigint := (asked->>'id')::bigint;
  first_board jsonb;
  first_wants jsonb;
  out jsonb;
  said text := verse.builder_said(response);
  d jsonb;
  wants jsonb;
  missing jsonb;
  nb jsonb;
  failed boolean := false;
BEGIN
  IF again THEN SELECT p.board, p.asked_for INTO first_board, first_wants FROM verse.realm_plans p WHERE p.id = plan_id; END IF;
  out := verse.builder_answer(asked - 'retry', http, response, model, taken);
  BEGIN d := said::jsonb; EXCEPTION WHEN others THEN d := NULL; END;
  wants := verse.builder_wants(CASE WHEN jsonb_typeof(d) = 'object' THEN d->'asked' END);
  IF wants IS NULL OR (again AND wants = '[]') THEN wants := coalesce(first_wants, '[]'); END IF;
  IF (out->>'status')::int <> 200 THEN
    IF NOT again OR first_board IS NULL THEN RETURN out; END IF;
    nb := first_board;   -- the second try failed: the first board, already repaired
    failed := true;
  ELSE
    nb := out->'board';
    missing := verse.builder_missing(wants, nb);
    IF jsonb_array_length(missing) > 0 THEN
      nb := verse.builder_repair(nb, missing);
      IF NOT again AND request IS NOT NULL THEN
        UPDATE verse.realm_plans SET board = nb, asked_for = wants, retried = true, repaired = missing WHERE id = plan_id;
        RETURN jsonb_build_object('status', 200, 'retry', verse.builder_retry(request, said, missing));
      END IF;
    END IF;
  END IF;
  UPDATE verse.realm_plans SET board = nb, error = NULL, asked_for = wants, ms_taken = taken,
    repaired = CASE WHEN jsonb_array_length(coalesce(missing, '[]')) > 0 THEN missing WHEN again AND NOT failed THEN NULL ELSE repaired END
  WHERE id = plan_id;
  RETURN jsonb_build_object('status', 200, 'plan', plan_id, 'board', nb, 'model', model, 'asked', wants,
                            'lines', verse.builder_lines(plan_id, nb));
END $$;
