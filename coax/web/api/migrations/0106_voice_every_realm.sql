-- Every realm's voice (after 0105, openagents#12643). Only a realm with a voice catalogue (Everglade)
-- could be spoken to; now any realm with rows takes the shared verbs, hand-built (verse.realm_rights)
-- or made (verse.made_realms, forks too):
--   switch {row, on}  one of the realm's rows: a hand-built realm's rights rows, a made realm's board
--                     (verse.edit_made_board, keyed as the board is: verse.edit_key)
--   show {panel}      the editing screen's panes
--   look              a question about the screen (the page asks again at .../look with the frame)
--   ask               a change no switch does ("add a dragon by the lake"): the page puts the words on
--                     the realm's list (POST /api/playtest/realms/<slug>/asks, the board's ADD)
--   none              chat: a short answer
-- A realm with a catalogue keeps its own verbs (place, move, ...) and 0105's prompt, schema and hold;
-- ask is only for a realm without one. A fork speaks with its base's catalogue (the C route used to
-- swap the slug; now SQL does, so the fork's own rows are the ones switched).
-- Nothing reads the words: the model picks from a typed enum, and SQL holds the answer to it.

-- A fork's catalogue is its base's.
ALTER FUNCTION verse.voice_edit_catalogue(text) RENAME TO voice_edit_catalogue_0051;
CREATE FUNCTION verse.voice_edit_catalogue(realm text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT verse.voice_edit_catalogue_0051(coalesce((SELECT base FROM verse.made_realms WHERE slug = realm AND base IS NOT NULL), realm)) $$;

-- Does a realm take a voice: a catalogue, rights rows, or a made realm.
CREATE FUNCTION verse.voice_realm(realm text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT verse.voice_edit_catalogue(realm) IS NOT NULL OR EXISTS (SELECT 1 FROM verse.realm_rights WHERE slug = realm)
      OR EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = realm) $$;
CREATE FUNCTION verse.voice_realm_name(realm text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT name FROM verse.made_realms WHERE slug = realm), initcap(replace(realm, '-', ' '))) $$;

-- A realm's rows: a made realm's board (what is in it and what was switched off), else its rights rows.
CREATE OR REPLACE FUNCTION verse.voice_edit_rows(realm text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = realm) THEN
    (SELECT coalesce(jsonb_agg(jsonb_build_object('id', verse.edit_key(f), 'label', f->>'label', 'says', f->>'label') ORDER BY o), '[]'::jsonb)
     FROM verse.made_realms m, jsonb_array_elements(verse.edit_made_board(m)) WITH ORDINALITY AS t(f, o)
     WHERE m.slug = realm AND jsonb_typeof(f->'label') = 'string' AND verse.edit_key(f) IS NOT NULL)
  ELSE coalesce((SELECT jsonb_agg(r) FROM verse.realm_rights x, jsonb_array_elements(x.rows) r
                 WHERE x.slug = realm AND jsonb_typeof(r->'id') = 'string'), '[]'::jsonb) END $$;

-- The verbs a realm takes: its catalogue's, else the shared ones (switch only with rows to switch).
CREATE FUNCTION verse.voice_edit_verbs(realm text) RETURNS text[] LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN verse.voice_edit_catalogue(realm) IS NOT NULL THEN verse.voice_edit_order()
    ELSE CASE WHEN jsonb_array_length(verse.voice_edit_rows(realm)) > 0 THEN ARRAY['switch'] ELSE '{}'::text[] END || ARRAY['show', 'look', 'ask'] END $$;

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
                        "what''s that building over there? => look", "why is the water black? => look", "can you see that glitch? => look"]},
  "ask": {"says": "they want the realm changed in a way none of its switches does: something added, taken away, moved or made different; their words go on the realm''s list of changes",
          "examples": ["add a dragon by the lake => ask", "put a castle on the hill => ask", "make the trees taller => ask", "can we have more lanterns => ask",
                       "take the bridge away => ask (when no switch is the bridge)"]}
}'::jsonb $$;

ALTER FUNCTION verse.voice_edit_prompt(text) RENAME TO voice_edit_prompt_0105;
ALTER FUNCTION verse.voice_edit_schema(text) RENAME TO voice_edit_schema_0105;
ALTER FUNCTION verse.voice_edit_hold(text, jsonb) RENAME TO voice_edit_hold_0105;

-- A realm with a catalogue: 0105's prompt. Without one: its name, its rows and the shared verbs.
CREATE FUNCTION verse.voice_edit_prompt(realm text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN verse.voice_edit_catalogue(realm) IS NOT NULL THEN verse.voice_edit_prompt_0105(realm) ELSE
    'You turn what the owner of ' || verse.voice_realm_name(realm) || ' said out loud or typed into one action. '
    || verse.voice_realm_name(realm) || ' is a realm in the Verse (a small 3D world) and they stand in it. '
    || 'Choose the one action below that does what they asked, and fill its fields from what they said; every field the action does not use gets its "not said" value. '
    || 'Speech recognition may mishear a word; pick what they most likely meant. When the sentence is unclear, still pick the action it most likely means. '
    || 'Answer edit "none" only when it is not an action at all (a question, small talk, thanks); a question about what they see on screen is look. '
    || 'reply is "" for every action but none. For none, reply answers them the way a friendly builder standing beside them would, out loud: '
    || 'one to three short plain sentences, no lists or markdown. Answer the question itself when you can (what is in this realm comes from the '
    || 'list below; the current time is given); when you cannot know, say so briefly.'
    || E'\nThe realm''s switches (its features, each on or off for everyone):\n'
    || coalesce((SELECT string_agg('- ' || (r->>'id') || ': ' || coalesce(r->>'label', '') || CASE WHEN coalesce(r->>'says', '') NOT IN ('', r->>'label') THEN ' (' || (r->>'says') || ')' ELSE '' END, E'\n')
                 FROM jsonb_array_elements(verse.voice_edit_rows(realm)) r), '(none)')
    || E'\nThe actions:\n'
    || (SELECT string_agg('- ' || n || ': ' || (verse.voice_edit_generic()->n->>'says')
         || E'\n  for example: ' || (SELECT string_agg(k, '; ') FROM jsonb_array_elements_text(verse.voice_edit_generic()->n->'examples') k), E'\n' ORDER BY o)
        FROM unnest(verse.voice_edit_verbs(realm)) WITH ORDINALITY AS t(n, o))
    || E'\n- none: not an action; reply answers them. for example: what time is it => none, reply with the time; '
    || 'is this working? => none, reply yes, I can hear you; hello => none, reply with a greeting and what they can ask for; '
    || 'what''s in this realm? => none, reply naming a few of its features'
  END $$;

CREATE FUNCTION verse.voice_edit_schema(realm text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN verse.voice_edit_catalogue(realm) IS NOT NULL THEN verse.voice_edit_schema_0105(realm) ELSE
    jsonb_build_object('type', 'object',
      'required', '["edit", "row", "on", "panel", "reply"]'::jsonb,
      'propertyOrdering', '["edit", "row", "on", "panel", "reply"]'::jsonb,
      'properties', jsonb_build_object(
        'edit', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.voice_edit_verbs(realm) || ARRAY['none'])),
        'row', jsonb_build_object('type', 'string', 'description', 'for switch: the switch they mean; unnamed otherwise',
                                  'enum', '["unnamed"]'::jsonb || coalesce((SELECT jsonb_agg(r->'id') FROM jsonb_array_elements(verse.voice_edit_rows(realm)) r), '[]'::jsonb)),
        'on', jsonb_build_object('type', 'boolean', 'description', 'for switch: true to turn it on, false to turn it off; else false'),
        'panel', jsonb_build_object('type', 'string', 'description', 'for show: the pane to open, or close; unsaid otherwise',
                                    'enum', '["unsaid"]'::jsonb || verse.voice_edit_panels()),
        'reply', jsonb_build_object('type', 'string', 'description', 'for none: what to say back to them, one to three short spoken sentences; for an action: ""')))
  END $$;

-- Held: a realm with a catalogue as 0105 holds it; else the shared verbs, each with its own fields only.
CREATE FUNCTION verse.voice_edit_hold(realm text, d jsonb) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  verb text := CASE WHEN jsonb_typeof(d) = 'object' AND jsonb_typeof(d->'edit') = 'string' THEN d->>'edit' END;
BEGIN
  IF verse.voice_edit_catalogue(realm) IS NOT NULL THEN RETURN verse.voice_edit_hold_0105(realm, d); END IF;
  IF verb IS NULL OR NOT verb = ANY (verse.voice_edit_verbs(realm)) THEN RETURN NULL; END IF;
  IF verb IN ('look', 'ask') THEN RETURN jsonb_build_object('edit', verb); END IF;
  IF verb = 'switch' THEN
    IF NOT verse.voice_edit_rows(realm) @> jsonb_build_array(jsonb_build_object('id', coalesce(d->>'row', ''))) THEN RETURN NULL; END IF;
    RETURN jsonb_build_object('edit', 'switch', 'row', d->>'row', 'on', jsonb_typeof(d->'on') = 'boolean' AND (d->>'on')::boolean);
  END IF;
  IF NOT verse.voice_edit_panels() ? coalesce(d->>'panel', '') THEN RETURN NULL; END IF;
  RETURN jsonb_build_object('edit', 'show', 'panel', d->>'panel');
END $$;

-- 0056's ask, for every realm that takes a voice.
CREATE OR REPLACE FUNCTION verse.voice_edit_ask(realm text, body jsonb, pubkey text, cap int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  sentence text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'sentence', 300) ELSE '' END;
  row_id bigint;
BEGIN
  IF NOT verse.voice_realm(realm) THEN RETURN '{"status":400,"error":"this realm takes no voice edits"}'; END IF;
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

-- 0105's look, for every realm that takes a voice, by its name.
CREATE OR REPLACE FUNCTION verse.voice_look_ask(realm text, body jsonb, pubkey text, cap int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  question text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'question', 300) ELSE '' END;
  image text := CASE WHEN jsonb_typeof(body->'image') = 'string' THEN body->>'image' ELSE '' END;
  row_id bigint;
BEGIN
  IF NOT verse.voice_realm(realm) THEN RETURN '{"status":400,"error":"this realm takes no voice"}'; END IF;
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
      'The picture is the screen of someone in ' || verse.voice_realm_name(realm) || ', a realm in the Verse: a small 3D world drawn live in their browser, '
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
