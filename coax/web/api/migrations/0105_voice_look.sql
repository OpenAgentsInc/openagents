-- The voice looks at the screen (after 0104). "What's this dark strip along the sky?" used to come back
-- "I can't tell what that is." Now the selector answers look for a question about what they see, and the
-- page asks again at POST /api/playtest/realm/<realm>/look with the same words and the frame on screen
-- (the host's flag 32: the next frame drawn, as base64 JPEG, in the body's "image"). Gemini answers
-- from the picture in one to three spoken sentences:
--   200 {edit: "chat", reply, model}
-- Looks are paced and counted with voice edits (verse.voice_edits; the sentence kept as "look: ...").
-- The picture itself is never stored.
--
-- The verbs every realm shares (switch, show, look) are described here once, in voice_edit_generic;
-- the prompt lists them after a realm's own catalogue, so a later shared verb needs no prompt rewrite.

CREATE OR REPLACE FUNCTION verse.voice_edit_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['place', 'remove', 'move', 'resize', 'rotate', 'restore', 'list', 'undo', 'switch', 'show', 'look'] $$;

CREATE OR REPLACE FUNCTION verse.voice_edit_generic() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$ SELECT '{
  "switch": {"says": "turn one of the realm''s switches on or off: row is its id, on is true to turn it on, false to turn it off, disable, stop or get rid of it",
             "examples": ["turn off the guards => switch, row guards, on false", "stop the guards chasing me => switch, row guards, on false",
                          "make me invincible => switch, row invincible, on true", "I want to be able to die again => switch, row invincible, on false",
                          "no more ducks => switch, row floats, on false", "keep it daytime => switch, row daycycle, on false"]},
  "show": {"says": "open or close the editing screen: panel edit (the edit panel, the board, the list of features, edit mode), settings (the realm''s settings, tools visitors get), ideas (visitors'' ideas, proposals, cards), close (close it, done editing)",
           "examples": ["open the edit panel => show, panel edit", "edit mode => show, panel edit", "show me the settings => show, panel settings",
                        "what have visitors suggested => show, panel ideas", "close that => show, panel close", "I''m done editing => show, panel close"]},
  "look": {"says": "they ask about something they see on their screen right now: what it is, why it looks like that, where something is, what is wrong with the picture; the screen is looked at next",
           "examples": ["there''s some weird dark strip along the sky, what is this? => look", "what am I looking at? => look",
                        "what''s that building over there? => look", "why is the water black? => look", "can you see that glitch? => look"]}
}'::jsonb $$;

-- 0104's prompt, its edits from the realm's catalogue or the shared ones, in the order above.
CREATE OR REPLACE FUNCTION verse.voice_edit_prompt(realm text) RETURNS text LANGUAGE sql STABLE AS $$
  WITH c AS (SELECT verse.voice_edit_catalogue(realm) AS c)
  SELECT 'You turn what an admin said out loud into one edit of ' || initcap(realm) || ', a realm in the Verse (a small 3D world). '
    || 'They stand in the realm and speak one sentence to change it. Choose the one edit below that does what they asked, '
    || 'and fill its fields from what they said; every field the edit does not use gets its "not said" value. '
    || 'Speech recognition may mishear a word; pick what they most likely meant. When the sentence is unclear, still pick the edit it most likely means. '
    || 'Answer edit "none" only when it is not an edit at all (a question, small talk, thanks); a question about what they see on screen is look. '
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
    || E'\nThe realm''s switches (its features, each on or off for everyone):\n'
    || coalesce((SELECT string_agg('- ' || (r->>'id') || ': ' || coalesce(r->>'label', '') || ' (' || coalesce(r->>'says', '') || ')', E'\n')
                 FROM jsonb_array_elements(verse.voice_edit_rows(realm)) r), '(none)')
    || E'\nThe edits:\n'
    || (SELECT string_agg('- ' || n || ': ' || (e->>'says')
         || E'\n  for example: ' || (SELECT string_agg(k, '; ') FROM jsonb_array_elements_text(e->'examples') k), E'\n' ORDER BY o)
        FROM unnest(verse.voice_edit_order()) WITH ORDINALITY AS t(n, o),
             LATERAL (SELECT coalesce(c.c->'edits'->n, verse.voice_edit_generic()->n) AS e) x WHERE e IS NOT NULL)
    || E'\n- none: not an edit; reply answers them. for example: what time is it => none, reply with the time; '
    || 'is this working? => none, reply yes, I can hear you; hello => none, reply with a greeting and what they can ask for; '
    || 'who lives here? => none, reply naming a few of the city''s people'
  FROM c WHERE c.c IS NOT NULL $$;

-- look is held as list and undo are: the verb alone.
DO $$
DECLARE src text := pg_get_functiondef('verse.voice_edit_hold(text, jsonb)'::regprocedure);
BEGIN
  IF position('''list'', ''undo'', ''look''' IN src) = 0 THEN
    src := replace(src, 'IF verb IN (''list'', ''undo'') THEN RETURN out; END IF;', 'IF verb IN (''list'', ''undo'', ''look'') THEN RETURN out; END IF;');
    IF position('''list'', ''undo'', ''look''' IN src) = 0 THEN RAISE EXCEPTION 'voice_edit_hold: list/undo line not found'; END IF;
    EXECUTE src;
  END IF;
END $$;

-- A look: the words and the frame, to Gemini as a picture and a question.
CREATE OR REPLACE FUNCTION verse.voice_look_ask(realm text, body jsonb, pubkey text, cap int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  question text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'question', 300) ELSE '' END;
  image text := CASE WHEN jsonb_typeof(body->'image') = 'string' THEN body->>'image' ELSE '' END;
  row_id bigint;
BEGIN
  IF verse.voice_edit_catalogue(realm) IS NULL AND NOT EXISTS (SELECT 1 FROM verse.realm_rights WHERE slug = realm) THEN
    RETURN '{"status":400,"error":"this realm takes no voice"}';
  END IF;
  IF question = '' THEN RETURN '{"status":400,"error":"ask what to look at"}'; END IF;
  IF image = '' THEN RETURN '{"status":400,"error":"no picture"}'; END IF;
  IF length(image) > 700000 THEN RETURN '{"status":413,"error":"the picture is too large"}'; END IF;
  IF image !~ '^[A-Za-z0-9+/]+=*$' THEN RETURN '{"status":400,"error":"not a picture"}'; END IF;
  IF EXISTS (SELECT 1 FROM verse.voice_edits v WHERE v.pubkey = voice_look_ask.pubkey AND v.ms > verse.now_ms() - 30000 AND v.ms_taken IS NULL) THEN
    RETURN '{"status":429,"error":"one at a time"}';
  END IF;
  IF (SELECT count(*) FROM verse.voice_edits WHERE ms > (extract(epoch FROM date_trunc('day', now())) * 1000)::bigint) >= cap THEN
    RETURN '{"status":503,"error":"the day''s voice edits are used up"}';
  END IF;
  INSERT INTO verse.voice_edits (ms, realm, pubkey, sentence) VALUES (verse.now_ms(), realm, pubkey, 'look: ' || question) RETURNING id INTO row_id;
  RETURN jsonb_build_object('status', 200, 'id', row_id, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text',
      'The picture is the screen of someone in ' || initcap(realm) || ', a realm in the Verse: a small 3D world drawn live in their browser, '
      || 'which they own and are building. Answer their question about what is on the screen the way a friendly builder looking over their '
      || 'shoulder would, out loud: one to three short plain sentences, no lists or markdown. Say what the thing most likely is (part of the '
      || 'scene, the sky, the water, the interface, or a drawing fault such as a gap, a seam, a band or a missing texture) and, when it looks '
      || 'like a fault, say so plainly. When you cannot tell, say what you can see and that you are not sure.'))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(
      jsonb_build_object('inlineData', jsonb_build_object('mimeType', 'image/jpeg', 'data', image)),
      jsonb_build_object('text', question)))),
    'generationConfig', jsonb_build_object('temperature', 0.2, 'maxOutputTokens', 400,
      'thinkingConfig', jsonb_build_object('thinkingBudget', 0))));
END $$;

CREATE OR REPLACE FUNCTION verse.voice_look_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  r jsonb;
  t text;
  reply text;
  err text;
  status int := 200;
BEGIN
  BEGIN r := response::jsonb; EXCEPTION WHEN others THEN r := NULL; END;
  IF http = 200 AND r IS NOT NULL THEN
    SELECT string_agg(p->>'text', '') INTO t
    FROM jsonb_array_elements(coalesce(r->'candidates'->0->'content'->'parts', '[]')) p
    WHERE coalesce((p->>'thought')::boolean, false) = false AND p ? 'text';
    reply := verse.voice_edit_reply(jsonb_build_object('reply', coalesce(t, '')));
    IF reply = '' THEN status := 503; err := 'no answer'; END IF;
  ELSE
    status := 503;
    err := CASE WHEN http = 429 THEN 'busy' WHEN http = 0 THEN 'model unreachable' ELSE 'model error ' || http END;
  END IF;
  UPDATE verse.voice_edits SET edit = CASE WHEN err IS NULL THEN jsonb_build_object('edit', 'look', 'reply', reply) END,
    error = err, model = voice_look_answer.model, ms_taken = taken WHERE id = (asked->>'id')::bigint;
  IF err IS NOT NULL THEN RETURN jsonb_build_object('status', status, 'error', err); END IF;
  RETURN jsonb_build_object('status', 200, 'edit', 'chat', 'reply', reply, 'model', model);
END $$;
