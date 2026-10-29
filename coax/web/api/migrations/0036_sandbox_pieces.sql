-- The Sandbox's set pieces (rl-sandbox-pieces; npc.c answers
-- POST /api/playtest/sandbox/piece {sentence}). A visitor types one sentence
-- ("a flock of birds follows me") and the world gets a set piece: Gemini on
-- Vertex AI, Google's first, picks one piece from the fixed catalogue below
-- with its bounded fields, under a response schema whose enums are the
-- catalogue's words; then this file holds the answer to the catalogue
-- before the page sees it, and the page holds it to its own copy again
-- (verse/setpiece.coax, which lays the piece out). No words of the sentence
-- are matched here: the model reads it, and only its typed answer is checked.
--
--   -> 200 {piece, count?, kind?, colour?, size?, model}   only the fields the piece has
--      400 no sentence, 422 nothing in the catalogue fits, 429 one at a time
--      (a player's next piece three seconds after the last), 503 no answer
--      (the model's error or the day's pieces used up), 502 an answer that
--      isn't a piece.
-- A count out of range is held to it and a missing one is the piece's
-- default; a kind, colour or size not the piece's is its default (the first,
-- the first, medium). Every sentence and what was picked is kept.

-- The catalogue: each piece, what it is (the model reads this), and its
-- fields: count [min, max, default], kind, colour and size (their words, the
-- default first, except size: medium).
CREATE FUNCTION verse.piece_catalogue() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$ SELECT '{
  "flock": {"says": "a flock of birds that flies round whoever is walking and follows them", "count": [3, 12, 7], "kind": ["crow", "bat"]},
  "lanterns": {"says": "a ring of glowing lanterns", "count": [4, 16, 8],
               "colour": ["orange", "gold", "red", "green", "blue", "violet", "white"], "size": ["small", "medium", "large"]},
  "grove": {"says": "a grove: a stand of trees (round, pine, dead, or mixed with bushes)", "count": [3, 20, 9],
            "kind": ["round", "pine", "dead", "mixed"], "size": ["small", "medium", "large"]},
  "pond": {"says": "a pond ringed with stones and bushes", "size": ["small", "medium", "large"]}
}'::jsonb $$;
CREATE FUNCTION verse.piece_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['flock', 'lanterns', 'grove', 'pond'] $$;

CREATE TABLE verse.sandbox_pieces (
  id bigserial PRIMARY KEY,
  ms bigint NOT NULL,
  player text,                  -- verse.players id, when the page sent its key
  sentence text NOT NULL,
  piece jsonb,                  -- what was picked, as the page got it; null for none
  error text,
  model text,
  ms_taken int
);
CREATE INDEX sandbox_pieces_player ON verse.sandbox_pieces (player, ms);
CREATE INDEX sandbox_pieces_ms ON verse.sandbox_pieces (ms);

-- What the model is told: the Sandbox, the sentence's job, and every piece
-- with its fields.
CREATE FUNCTION verse.piece_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'You pick a set piece for the Sandbox, open ground in the Verse (a small 3D world) that visitors build on together. '
    || 'A visitor typed one sentence saying what they want in the world. Choose the one piece from this catalogue that best '
    || 'makes it, and its fields, from what they said (a number they gave, a colour, a kind of bird or tree, how big). '
    || 'Leave out a field the piece does not have, or that they did not ask for. If nothing in the catalogue comes close '
    || E'(a castle, a car, a question, something else entirely), answer piece "none".\nThe pieces:\n'
    || string_agg('- ' || n || ': ' || (c->>'says')
         || CASE WHEN c ? 'count' THEN '. count ' || (c->'count'->>0) || ' to ' || (c->'count'->>1) || ' (usually ' || (c->'count'->>2) || ')' ELSE '' END
         || CASE WHEN c ? 'kind' THEN '. kind: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c->'kind') k) ELSE '' END
         || CASE WHEN c ? 'colour' THEN '. colour: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c->'colour') k) ELSE '' END
         || CASE WHEN c ? 'size' THEN '. size: small, medium or large' ELSE '' END, E'\n' ORDER BY o)
  FROM unnest(verse.piece_order()) WITH ORDINALITY AS t(n, o), LATERAL (SELECT verse.piece_catalogue()->n AS c) x $$;

-- The response schema: the catalogue's words as enums.
CREATE FUNCTION verse.piece_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('type', 'object', 'required', jsonb_build_array('piece'), 'properties', jsonb_build_object(
    'piece', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.piece_order() || ARRAY['none'])),
    'count', jsonb_build_object('type', 'integer'),
    'kind', jsonb_build_object('type', 'string', 'enum', (SELECT jsonb_agg(DISTINCT k) FROM jsonb_each(verse.piece_catalogue()) e, jsonb_array_elements_text(coalesce(e.value->'kind', '[]')) k)),
    'colour', jsonb_build_object('type', 'string', 'enum', verse.piece_catalogue()->'lanterns'->'colour'),
    'size', jsonb_build_object('type', 'string', 'enum', jsonb_build_array('small', 'medium', 'large')))) $$;

-- A sentence asked: {"status", "error"} when it can't be, else the row it is
-- kept in and the Gemini request.
CREATE FUNCTION verse.piece_ask(body jsonb, cap int, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  sentence text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'sentence', 300) ELSE '' END;
  who text := (SELECT id FROM verse.players WHERE k ~ '^[A-Za-z0-9_-]{20,128}$' AND key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex'));
  row_id bigint;
BEGIN
  IF sentence = '' THEN RETURN '{"status":400,"error":"say what to make"}'; END IF;
  IF who IS NOT NULL AND EXISTS (SELECT 1 FROM verse.sandbox_pieces WHERE player = who AND ms > verse.now_ms() - 3000) THEN
    RETURN '{"status":429,"error":"one set piece at a time"}';
  END IF;
  IF (SELECT count(*) FROM verse.sandbox_pieces WHERE ms > (extract(epoch FROM date_trunc('day', now())) * 1000)::bigint) >= cap THEN
    RETURN '{"status":503,"error":"the day''s set pieces are used up"}';
  END IF;
  INSERT INTO verse.sandbox_pieces (ms, player, sentence) VALUES (verse.now_ms(), who, sentence) RETURNING id INTO row_id;
  RETURN jsonb_build_object('status', 200, 'id', row_id, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', verse.piece_prompt()))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text', sentence)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'responseSchema', verse.piece_schema(),
      'temperature', 0, 'maxOutputTokens', 2000, 'thinkingConfig', jsonb_build_object('thinkingLevel', 'LOW'))));
END $$;

-- An answer held to the catalogue: the piece with only its own fields, or NULL.
CREATE FUNCTION verse.piece_hold(d jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  name text := CASE WHEN jsonb_typeof(d) = 'object' AND jsonb_typeof(d->'piece') = 'string' THEN d->>'piece' END;
  c jsonb := verse.piece_catalogue()->name;
  out jsonb;
  n numeric;
BEGIN
  IF c IS NULL OR name IS NULL THEN RETURN NULL; END IF;
  out := jsonb_build_object('piece', name);
  IF c ? 'count' THEN
    n := CASE WHEN jsonb_typeof(d->'count') = 'number' THEN round((d->>'count')::numeric) END;
    out := out || jsonb_build_object('count', CASE WHEN n IS NULL OR n <= 0 THEN (c->'count'->>2)::int
                                                   ELSE greatest((c->'count'->>0)::int, least((c->'count'->>1)::int, n))::int END);
  END IF;
  IF c ? 'kind' THEN out := out || jsonb_build_object('kind', CASE WHEN c->'kind' ? coalesce(d->>'kind', '') THEN d->>'kind' ELSE c->'kind'->>0 END); END IF;
  IF c ? 'colour' THEN out := out || jsonb_build_object('colour', CASE WHEN c->'colour' ? coalesce(d->>'colour', '') THEN d->>'colour' ELSE c->'colour'->>0 END); END IF;
  IF c ? 'size' THEN out := out || jsonb_build_object('size', CASE WHEN c->'size' ? coalesce(d->>'size', '') THEN d->>'size' ELSE 'medium' END); END IF;
  RETURN out;
END $$;

-- The model's answer (Gemini's generateContent response), held to the
-- catalogue and kept with the sentence.
CREATE FUNCTION verse.piece_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  r jsonb;
  t text;
  d jsonb;
  held jsonb;
  err text;
  status int := 200;
BEGIN
  BEGIN r := response::jsonb; EXCEPTION WHEN others THEN r := NULL; END;
  IF http = 200 AND r IS NOT NULL THEN
    SELECT string_agg(p->>'text', '') INTO t
    FROM jsonb_array_elements(coalesce(r->'candidates'->0->'content'->'parts', '[]')) p
    WHERE coalesce((p->>'thought')::boolean, false) = false AND p ? 'text';
    BEGIN d := t::jsonb; EXCEPTION WHEN others THEN d := NULL; END;
    IF d IS NULL THEN status := 503; err := 'no answer';
    ELSIF jsonb_typeof(d) = 'object' AND d->>'piece' = 'none' THEN status := 422; err := 'none fit';
    ELSE held := verse.piece_hold(d); IF held IS NULL THEN status := 502; err := 'not a piece'; END IF;
    END IF;
  ELSE
    status := 503;
    err := CASE WHEN http = 429 THEN 'busy' WHEN http = 0 THEN 'model unreachable' ELSE 'model error ' || http END;
  END IF;
  UPDATE verse.sandbox_pieces SET piece = held, error = err, model = piece_answer.model, ms_taken = taken WHERE id = (asked->>'id')::bigint;
  IF held IS NULL THEN RETURN jsonb_build_object('status', status, 'error', err); END IF;
  RETURN jsonb_build_object('status', 200) || held || jsonb_build_object('model', model);
END $$;
