-- Voice edits answer what isn't an edit (eg-voice-chat, after 0054): a question or small talk said to
-- TALK TO EDIT used to come back 422 and the page showed "DIDN'T CATCH AN EDIT IN: '...'". Now the
-- model answers it. The schema gains "reply": for edit "none", a short spoken answer to what was said;
-- for an edit, "". voice_edit_answer turns none with a reply into
--   200 {edit: "chat", reply, model}
-- (the page shows and speaks it: everglade-voice.coax) and keeps 422 only for none with no reply.
-- The request now carries the time in Chicago, so "what time is it" has an answer. The reply is plain
-- text for the page's line and voice: no quotes, backslashes or line breaks, at most 300 characters.

CREATE OR REPLACE FUNCTION verse.voice_edit_prompt(realm text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  WITH c AS (SELECT verse.voice_edit_catalogue(realm) AS c)
  SELECT 'You turn what an admin said out loud into one edit of ' || initcap(realm) || ', a realm in the Verse (a small 3D world). '
    || 'They stand in the realm and speak one sentence to change it. Choose the one edit below that does what they asked, '
    || 'and fill its fields from what they said; every field the edit does not use gets its "not said" value. '
    || 'Speech recognition may mishear a word; pick what they most likely meant. When the sentence is unclear, still pick the edit it most likely means. '
    || 'Answer edit "none" only when it is not an edit at all (a question, small talk, thanks). '
    || 'Never answer a thing they did not ask for: when what they named is not in these lists, the item is something and name is what they said. '
    || 'reply is "" for every edit. For none, reply answers them the way a friendly builder standing beside them would, out loud: '
    || 'one to three short plain sentences, no lists or markdown. Answer the question itself when you can (what is in this realm and what you '
    || 'can do come from the lists below; the current time is given); when you cannot know, say so briefly. '
    || E'\nThe things that can be put down (width x depth x height in metres at size 1):\n'
    || (SELECT string_agg(i || ' ' || (c.c->'sizes'->>(o - 1)::int), '; ' ORDER BY o) FROM jsonb_array_elements_text(c.c->'items') WITH ORDINALITY AS t(i, o))
    || E'\nThings with a top that others can go onto: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'onto') k)
    || E'\nColours: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'colours') k)
    || E'\nThe city''s buildings: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'buildings') k)
    || E'\nThe city''s people and cats: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'who') k)
    || E'\nThe edits:\n'
    || (SELECT string_agg('- ' || n || ': ' || (e->>'says')
         || E'\n  for example: ' || (SELECT string_agg(k, '; ') FROM jsonb_array_elements_text(e->'examples') k), E'\n' ORDER BY o)
        FROM unnest(verse.voice_edit_order()) WITH ORDINALITY AS t(n, o), LATERAL (SELECT c.c->'edits'->n AS e) x WHERE e IS NOT NULL)
    || E'\n- none: not an edit; reply answers them. for example: what time is it => none, reply with the time; '
    || 'is this working? => none, reply yes, I can hear you; hello => none, reply with a greeting and what they can ask for; '
    || 'who lives here? => none, reply naming a few of the city''s people'
  FROM c WHERE c.c IS NOT NULL $$;

-- 0051's schema with a reply, last: what to do is decided before what to say.
CREATE OR REPLACE FUNCTION verse.voice_edit_schema(realm text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  WITH c AS (SELECT verse.voice_edit_catalogue(realm) AS c)
  SELECT jsonb_build_object('type', 'object',
    'required', '["edit", "item", "name", "count", "arrange", "onto", "colour", "size", "direction", "metres", "turn", "all", "who", "building", "reply"]'::jsonb,
    'propertyOrdering', '["edit", "item", "name", "count", "arrange", "onto", "colour", "size", "direction", "metres", "turn", "all", "who", "building", "reply"]'::jsonb,
    'properties', jsonb_build_object(
    'edit', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.voice_edit_order() || ARRAY['none'])),
    'item', jsonb_build_object('type', 'string', 'description', 'the kind of thing they named (a person, cat or building of the city by its kind); something when they named no kind ("that", "it", "this") or a thing not in the list',
                               'enum', (c.c->'items') || (c.c->'kinds') || '["something"]'::jsonb),
    'name', jsonb_build_object('type', 'string', 'description', 'only when they named a thing, a person or a building that is NOT in the lists: what they said, as said ("dragon", "Bob"); else ""'),
    'count', jsonb_build_object('type', 'integer', 'description', 'for place: how many (1 to 40; "two" is 2, "a" is 1, a plural with no number is 3); else 1'),
    'arrange', jsonb_build_object('type', 'string', 'description', 'for place: front (the usual), row, ring (all around, around me, in a circle), scatter (spread about), fill (fill the field with)',
                                  'enum', c.c->'arrange'),
    'onto', jsonb_build_object('type', 'string', 'description', 'for place: the thing they said to put it ON; ground when they did not say on',
                               'enum', '["ground"]'::jsonb || (c.c->'onto')),
    'colour', jsonb_build_object('type', 'string', 'description', 'for place: the colour they said; unsaid when none', 'enum', (c.c->'colours') || '["unsaid"]'::jsonb),
    'size', jsonb_build_object('type', 'number', 'description', 'for place: big 1.5, huge 2.5, small 0.6, tiny 0.4, else 1; for resize: the factor (bigger 1.5, smaller 0.67, twice 2, half 0.5)'),
    'direction', jsonb_build_object('type', 'string', 'description', 'for move: which way; here (to in front of the speaker) when not said', 'enum', c.c->'directions'),
    'metres', jsonb_build_object('type', 'number', 'description', 'for move: how far, when they said; else 0'),
    'turn', jsonb_build_object('type', 'number', 'description', 'for rotate: degrees, positive to the right, around is 180; else 0'),
    'all', jsonb_build_object('type', 'boolean', 'description', 'for remove, move, resize, rotate: true when they mean more than one (these, those, a plural, all, clear), else false'),
    'who', jsonb_build_object('type', 'string', 'description', 'for remove or restore: the person or cat of the city they said by name; nobody when they said no such name',
                              'enum', '["nobody"]'::jsonb || (c.c->'who')),
    'building', jsonb_build_object('type', 'string', 'description', 'for remove or restore: the city''s building they named; unnamed when none',
                                   'enum', '["unnamed"]'::jsonb || (c.c->'buildings')),
    'reply', jsonb_build_object('type', 'string', 'description', 'for none: what to say back to them, one to three short spoken sentences; for an edit: ""')))
  FROM c WHERE c.c IS NOT NULL $$;

-- The time a request is asked at, in words, for the model.
CREATE OR REPLACE FUNCTION verse.voice_edit_now() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT 'It is now ' || to_char(now() AT TIME ZONE 'America/Chicago', 'FMDay, FMMonth FMDD, YYYY, FMHH12:MI AM') || ' in Chicago (Central time).' $$;

CREATE OR REPLACE FUNCTION verse.voice_edit_ask(realm text, body jsonb, pubkey text, cap int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  sentence text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'sentence', 300) ELSE '' END;
  row_id bigint;
BEGIN
  IF verse.voice_edit_catalogue(realm) IS NULL THEN RETURN '{"status":400,"error":"this realm takes no voice edits"}'; END IF;
  IF sentence = '' THEN RETURN '{"status":400,"error":"say what to change"}'; END IF;
  IF EXISTS (SELECT 1 FROM verse.voice_edits v WHERE v.pubkey = voice_edit_ask.pubkey AND v.ms > verse.now_ms() - 30000 AND v.ms_taken IS NULL) THEN
    RETURN '{"status":429,"error":"one edit at a time"}';
  END IF;
  IF (SELECT count(*) FROM verse.voice_edits WHERE ms > (extract(epoch FROM date_trunc('day', now())) * 1000)::bigint) >= cap THEN
    RETURN '{"status":503,"error":"the day''s voice edits are used up"}';
  END IF;
  INSERT INTO verse.voice_edits (ms, realm, pubkey, sentence) VALUES (verse.now_ms(), realm, pubkey, sentence) RETURNING id INTO row_id;
  RETURN jsonb_build_object('status', 200, 'id', row_id, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text',
      verse.voice_edit_prompt(realm) || E'\n' || verse.voice_edit_now()))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text', sentence)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'responseSchema', verse.voice_edit_schema(realm),
      'temperature', 0, 'maxOutputTokens', 800, 'thinkingConfig', jsonb_build_object('thinkingBudget', 0))));
END $$;

-- A reply as the page can show and say it: one line, no quotes or backslashes, at most 300 characters.
CREATE OR REPLACE FUNCTION verse.voice_edit_reply(d jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(d->'reply') = 'string'
    THEN left(btrim(regexp_replace(translate(d->>'reply', E'"\\', E'''/'), '\s+', ' ', 'g')), 300) ELSE '' END $$;

CREATE OR REPLACE FUNCTION verse.voice_edit_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
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
    ELSIF jsonb_typeof(d) = 'object' AND d->>'edit' = 'none' THEN
      IF verse.voice_edit_reply(d) <> '' THEN held := jsonb_build_object('edit', 'chat', 'reply', verse.voice_edit_reply(d));
      ELSE status := 422; err := 'none fit';
      END IF;
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
