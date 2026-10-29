-- Voice edits in a realm (eg-voice-edit; npc.c answers
-- POST /api/playtest/realm/<realm>/edit {sentence}). An admin holds the
-- realm's TALK TO EDIT button and says what to change ("put two barrels
-- here", "take that away", "undo"); the voice host hears it and the page
-- sends the words here. Gemini on Vertex AI picks one edit from the realm's
-- fixed catalogue below, under a response schema whose enums are its words;
-- this file holds the answer to the catalogue, and the page holds it to its
-- own (verse/everglade/edits.coax) and does it. No words of the sentence are
-- matched anywhere: the model reads it, and only its typed answer is checked.
-- Only an admin (admin_who in admin.c: admins.conf, by site login or NIP-98)
-- asks; the edit itself is published as the realm's world, an admin's only
-- (player.c site_session, verse.api).
--
--   -> 200 {edit, item?, count?, model}   edit: place | remove | undo
--      400 no sentence or no such realm, 401/403 not an admin, 422 nothing in
--      the catalogue fits, 429 one at a time, 503 no answer, 502 not an edit.
-- Every sentence and what was picked is kept.

-- The realms that take voice edits, and what each can do.
CREATE FUNCTION verse.voice_edit_catalogue(realm text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$ SELECT CASE realm
  WHEN 'everglade' THEN '{
    "place": {"says": "put one or more things on the ground in front of the speaker", "item": ["crate", "barrel", "bench", "fence", "stall"], "count": [1, 6, 1]},
    "remove": {"says": "take away the placed thing nearest the speaker"},
    "undo": {"says": "take back the last change"}
  }'::jsonb END $$;
CREATE FUNCTION verse.voice_edit_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['place', 'remove', 'undo'] $$;

CREATE TABLE verse.voice_edits (
  id bigserial PRIMARY KEY,
  ms bigint NOT NULL,
  realm text NOT NULL,
  pubkey text NOT NULL,         -- the admin who spoke
  sentence text NOT NULL,
  edit jsonb,                   -- what was picked, as the page got it; null for none
  error text,
  model text,
  ms_taken int
);
CREATE INDEX voice_edits_pubkey ON verse.voice_edits (pubkey, ms);

CREATE FUNCTION verse.voice_edit_prompt(realm text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'You turn what an admin said out loud into one edit of ' || initcap(realm) || ', a realm in the Verse (a small 3D world). '
    || 'They stand in the realm and speak one sentence to change it. Choose the one edit from this list that does what they asked, '
    || 'and its fields from what they said (which thing, how many). Leave out a field the edit does not have, or that they did not say. '
    || 'Speech recognition may mishear a word; pick what they most likely meant. If nothing here does what they asked '
    || E'(a question, something the list cannot make, talk that is not an edit), answer edit "none".\nThe edits:\n'
    || string_agg('- ' || n || ': ' || (c->>'says')
         || CASE WHEN c ? 'item' THEN '. item: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c->'item') k) ELSE '' END
         || CASE WHEN c ? 'count' THEN '. count ' || (c->'count'->>0) || ' to ' || (c->'count'->>1) || ' (usually ' || (c->'count'->>2) || ')' ELSE '' END, E'\n' ORDER BY o)
  FROM unnest(verse.voice_edit_order()) WITH ORDINALITY AS t(n, o), LATERAL (SELECT verse.voice_edit_catalogue(realm)->n AS c) x
  WHERE c IS NOT NULL $$;

CREATE FUNCTION verse.voice_edit_schema(realm text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('type', 'object', 'required', jsonb_build_array('edit'), 'properties', jsonb_build_object(
    'edit', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.voice_edit_order() || ARRAY['none'])),
    'item', jsonb_build_object('type', 'string', 'enum', verse.voice_edit_catalogue(realm)->'place'->'item'),
    'count', jsonb_build_object('type', 'integer'))) $$;

-- A sentence asked by an admin (pubkey checked by the caller): {"status", "error"} when it can't be,
-- else the row it is kept in and the Gemini request.
CREATE FUNCTION verse.voice_edit_ask(realm text, body jsonb, pubkey text, cap int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  sentence text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'sentence', 300) ELSE '' END;
  row_id bigint;
BEGIN
  IF verse.voice_edit_catalogue(realm) IS NULL THEN RETURN '{"status":400,"error":"this realm takes no voice edits"}'; END IF;
  IF sentence = '' THEN RETURN '{"status":400,"error":"say what to change"}'; END IF;
  IF EXISTS (SELECT 1 FROM verse.voice_edits v WHERE v.pubkey = voice_edit_ask.pubkey AND v.ms > verse.now_ms() - 1500) THEN
    RETURN '{"status":429,"error":"one edit at a time"}';
  END IF;
  IF (SELECT count(*) FROM verse.voice_edits WHERE ms > (extract(epoch FROM date_trunc('day', now())) * 1000)::bigint) >= cap THEN
    RETURN '{"status":503,"error":"the day''s voice edits are used up"}';
  END IF;
  INSERT INTO verse.voice_edits (ms, realm, pubkey, sentence) VALUES (verse.now_ms(), realm, pubkey, sentence) RETURNING id INTO row_id;
  RETURN jsonb_build_object('status', 200, 'id', row_id, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', verse.voice_edit_prompt(realm)))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text', sentence)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'responseSchema', verse.voice_edit_schema(realm),
      'temperature', 0, 'maxOutputTokens', 2000, 'thinkingConfig', jsonb_build_object('thinkingLevel', 'LOW'))));
END $$;

-- An answer held to the realm's catalogue: the edit with only its own fields, or NULL.
CREATE FUNCTION verse.voice_edit_hold(realm text, d jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  name text := CASE WHEN jsonb_typeof(d) = 'object' AND jsonb_typeof(d->'edit') = 'string' THEN d->>'edit' END;
  c jsonb := verse.voice_edit_catalogue(realm)->name;
  out jsonb;
  n numeric;
BEGIN
  IF c IS NULL OR name IS NULL THEN RETURN NULL; END IF;
  out := jsonb_build_object('edit', name);
  IF c ? 'item' THEN
    IF NOT (c->'item' ? coalesce(d->>'item', '')) THEN RETURN NULL; END IF;   -- a place names what it places
    out := out || jsonb_build_object('item', d->>'item');
  END IF;
  IF c ? 'count' THEN
    n := CASE WHEN jsonb_typeof(d->'count') = 'number' THEN round((d->>'count')::numeric) END;
    out := out || jsonb_build_object('count', CASE WHEN n IS NULL OR n <= 0 THEN (c->'count'->>2)::int
                                                   ELSE greatest((c->'count'->>0)::int, least((c->'count'->>1)::int, n))::int END);
  END IF;
  RETURN out;
END $$;

CREATE FUNCTION verse.voice_edit_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  r jsonb;
  t text;
  d jsonb;
  held jsonb;
  err text;
  status int := 200;
  realm text := (SELECT v.realm FROM verse.voice_edits v WHERE v.id = (asked->>'id')::bigint);
BEGIN
  BEGIN r := response::jsonb; EXCEPTION WHEN others THEN r := NULL; END;
  IF http = 200 AND r IS NOT NULL THEN
    SELECT string_agg(p->>'text', '') INTO t
    FROM jsonb_array_elements(coalesce(r->'candidates'->0->'content'->'parts', '[]')) p
    WHERE coalesce((p->>'thought')::boolean, false) = false AND p ? 'text';
    BEGIN d := t::jsonb; EXCEPTION WHEN others THEN d := NULL; END;
    IF d IS NULL THEN status := 503; err := 'no answer';
    ELSIF jsonb_typeof(d) = 'object' AND d->>'edit' = 'none' THEN status := 422; err := 'none fit';
    ELSE held := verse.voice_edit_hold(realm, d); IF held IS NULL THEN status := 502; err := 'not an edit'; END IF;
    END IF;
  ELSE
    status := 503;
    err := CASE WHEN http = 429 THEN 'busy' WHEN http = 0 THEN 'model unreachable' ELSE 'model error ' || http END;
  END IF;
  UPDATE verse.voice_edits SET edit = held, error = err, model = voice_edit_answer.model, ms_taken = taken WHERE id = (asked->>'id')::bigint;
  IF held IS NULL THEN RETURN jsonb_build_object('status', status, 'error', err); END IF;
  RETURN jsonb_build_object('status', 200) || held || jsonb_build_object('model', model);
END $$;
