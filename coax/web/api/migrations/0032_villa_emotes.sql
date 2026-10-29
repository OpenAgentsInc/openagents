-- Planetary Villa: emotes an agent makes from a sentence about the pet
-- (hc-emotes; verse/planetary-villa/README.md). "She always spun twice before
-- lying down" goes to Gemini with the motion format (verse/motion.coax), the
-- dog's joints and how far each turns, and the stock emotes as examples; it
-- answers one motion. The page checks it with the engine's own reader
-- (verse_motion_read): if a turn is out of range it asks again with the
-- reason, and when it plays the page keeps it on the pet (at most eight).
--
--   POST /api/villa/pets/<id>/emote {said, error, last}   -> {motion: {name, label, text}}
--   POST /api/villa/pets/<id>/emote/keep {name, label, text, said}   -> the pet, with its emotes
--   POST /api/villa/pets/<id>/emote/<name>/delete

ALTER TABLE villa.pets ADD COLUMN emotes jsonb NOT NULL DEFAULT '[]';
ALTER TABLE villa.pets ADD COLUMN emote_ms bigint NOT NULL DEFAULT 0;

CREATE OR REPLACE FUNCTION villa.owned(p villa.pets) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT villa.public(p) || jsonb_build_object('id', p.id, 'status', p.status, 'choices', p.choices, 'agent', p.agent,
    'agent_note', p.agent_note, 'emotes', p.emotes,
    'files', coalesce((SELECT jsonb_agg(jsonb_build_object('id', f.id, 'name', f.name, 'kind', f.kind, 'bytes', f.bytes,
                         'status', f.status, 'note', f.note) ORDER BY f.created_ms)
                       FROM villa.files f WHERE f.pet = p.id AND f.status <> 'deleted'), '[]')) $$;

CREATE FUNCTION villa.emote_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'You write one short animation ("motion") for a pet''s avatar in a gentle memorial garden, from what its owner says it '
    || 'used to do. Answer name (a-z, 0-9, _ only, at most 24, a word for the move), label (one or two words in capitals for its '
    || E'button) and text: the motion, statements separated by "; ". The format:\n'
    || E'motion NAME; rig animal; seconds S (0.1 to 30); once | loop | hold (after the last key: fade out, repeat, keep the pose); '
    || E'fade F (0 to 2); still (only if the pet stops walking to do it); then keys: at T (seconds from the start, each later than '
    || E'the last); under each key, JOINT PITCH YAW ROLL in degrees, and lift X Y Z in metres for the whole body.\n'
    || E'Joints and their ranges (pitch, yaw, roll): pelvis any; spine and chest -40..40, -40..40, -30..30; neck -70..80, -80..80, -40..40; '
    || E'head -60..60, -60..60, -45..45; left_ear and right_ear -70..70, -60..60, -70..70; tail, tail_2, tail_3 -80..100, -80..80, -40..40; '
    || E'front_left_shoulder and front_right_shoulder -130..80, -25..25, -40..40; front_*_elbow -10..150, -10..10, -10..10; '
    || E'front_*_wrist -40..130, -10..10, -10..10; front_*_paw and hind_*_paw -80..80, -20..20, -20..20; '
    || E'hind_left_hip and hind_right_hip -110..80, -25..25, -40..40; hind_*_knee -30..130, -10..10, -10..10; '
    || E'hind_*_hock -130..40, -10..10, -10..10. Never turn nose, ear tips, tail_tip, toes or eyes. Turning the whole body round is pelvis 0 YAW 0, '
    || E'YAW from -180 to 180: a full spin is a key with pelvis 0 -180 0 and a later one with pelvis 0 180 0. Keep it under 6 seconds, gentle and true to what was said. Examples:\n'
    || E'motion sit; rig animal; seconds 1.2; hold; still; fade 0.5; at 1.2; pelvis -32 0 0; lift 0 -0.1 0; front_left_shoulder 32 0 0; front_right_shoulder 32 0 0; hind_left_hip -75 0 0; hind_right_hip -75 0 0; hind_left_knee 125 0 0; hind_right_knee 125 0 0; hind_left_hock -110 0 0; hind_right_hock -110 0 0; neck -5 0 0; head 15 0 0; tail 60 0 0\n'
    || E'motion wag; rig animal; seconds 0.5; loop; fade 0.3; at 0; tail -60 45 0; tail_2 0 25 0; pelvis 0 4 0; at 0.25; tail -60 -45 0; tail_2 0 -25 0; pelvis 0 -4 0\n'
    || E'motion play_bow; rig animal; seconds 2.4; once; still; fade 0.35; at 0.5; pelvis 22 0 0; lift 0 -0.02 0; front_left_shoulder -85 0 0; front_right_shoulder -85 0 0; front_left_elbow 15 0 0; front_right_elbow 15 0 0; neck -40 0 0; head -10 0 0; tail -70 40 0; at 1.1; tail -70 -40 0; at 2.4; pelvis 0 0 0; lift 0 0 0; tail -50 0 0\n'
    || 'If you are told your last motion was refused and why, write it again fixing exactly that.' $$;

CREATE FUNCTION villa.emote_request(p villa.pets, said text, err text, last text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', villa.emote_prompt()))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text',
      'The pet: ' || p.name || ', a ' || p.species || E'.\nWhat they used to do: ' || said
      || CASE WHEN err <> '' THEN E'\nYour last motion was refused: ' || err || E'\nIt was: ' || last ELSE '' END)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json',
      'responseSchema', '{"type":"object","properties":{"name":{"type":"string"},"label":{"type":"string"},"text":{"type":"string"}},"required":["name","label","text"]}'::jsonb,
      'temperature', 0.4, 'maxOutputTokens', 4096)) $$;

CREATE FUNCTION villa.emote_done(pet_ text, http int, answer text, took int) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE txt text; d jsonb;
BEGIN
  IF http <> 200 THEN RETURN villa.err(502, 'we could not reach the agent: try again'); END IF;
  BEGIN
    SELECT string_agg(x->>'text', '' ORDER BY i) INTO txt
    FROM jsonb_array_elements(answer::jsonb#>'{candidates,0,content,parts}') WITH ORDINALITY AS z(x, i)
    WHERE coalesce((x->>'thought')::bool, false) = false AND x ? 'text';
    d := txt::jsonb;
  EXCEPTION WHEN others THEN d := NULL;
  END;
  IF jsonb_typeof(d) <> 'object' OR jsonb_typeof(d->'text') <> 'string' THEN RETURN villa.err(502, 'we could not read the answer: try again'); END IF;
  RETURN villa.r(200, jsonb_build_object('motion', jsonb_build_object(
    'name', left(regexp_replace(lower(coalesce(d->>'name', 'move')), '[^a-z0-9_]', '_', 'g'), 24),
    'label', upper(left(villa.clean(d->'label', 16), 16)),
    'text', left(regexp_replace(d->>'text', '[\x00-\x09\x0b-\x1f]', ' ', 'g'), 3000))));
END $$;

CREATE FUNCTION villa.emote_api(method text, parts text[], body jsonb, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE me verse.players := villa.player(k); p villa.pets; said text; nm text; t text; lb text;
BEGIN
  IF method <> 'POST' THEN RETURN villa.err(405, 'POST'); END IF;
  IF me.id IS NULL THEN RETURN villa.err(403, 'send a player key (POST /api/playtest/player)'); END IF;
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN body := '{}'; END IF;
  SELECT * INTO p FROM villa.pets WHERE id = parts[2] AND player = me.id AND status <> 'removed' FOR UPDATE;
  IF p.id IS NULL THEN RETURN villa.err(404, 'no such pet'); END IF;
  IF cardinality(parts) = 3 THEN
    said := villa.clean(body->'said', 300);
    IF said = '' THEN RETURN villa.err(400, 'tell us what they used to do'); END IF;
    IF p.emote_ms > verse.now_ms() - 4000 THEN RETURN villa.err(429, 'a little slower'); END IF;
    UPDATE villa.pets SET emote_ms = verse.now_ms() WHERE id = p.id;
    RETURN villa.r(200, '{}') || jsonb_build_object('pet', p.id, 'done', 'emote',
      'request', villa.emote_request(p, said, villa.clean(body->'error', 300), left(coalesce(body->>'last', ''), 3000)));
  END IF;
  IF cardinality(parts) = 4 AND parts[4] = 'keep' THEN
    nm := body->>'name'; t := body->>'text'; lb := upper(villa.clean(body->'label', 16));
    IF nm IS NULL OR nm !~ '^[a-z0-9_]{1,24}$' OR t IS NULL OR length(t) > 3000 OR lb = '' THEN RETURN villa.err(400, 'that motion isn''t whole'); END IF;
    UPDATE villa.pets SET emotes = (
      SELECT coalesce(jsonb_agg(e ORDER BY o), '[]') FROM (
        SELECT e, o FROM jsonb_array_elements(p.emotes) WITH ORDINALITY AS z(e, o) WHERE e->>'name' <> nm
        UNION ALL SELECT jsonb_build_object('name', nm, 'label', lb, 'text', t, 'said', villa.clean(body->'said', 300)), 1000
      ) w WHERE o > (SELECT greatest(0, jsonb_array_length(p.emotes) - 7)) OR o = 1000), updated_ms = verse.now_ms()
    WHERE id = p.id RETURNING * INTO p;
    RETURN villa.r(200, villa.owned(p));
  END IF;
  IF cardinality(parts) = 5 AND parts[5] = 'delete' THEN
    UPDATE villa.pets SET emotes = coalesce((SELECT jsonb_agg(e ORDER BY o) FROM jsonb_array_elements(p.emotes) WITH ORDINALITY AS z(e, o)
      WHERE e->>'name' <> parts[4]), '[]'), updated_ms = verse.now_ms() WHERE id = p.id RETURNING * INTO p;
    RETURN villa.r(200, villa.owned(p));
  END IF;
  RETURN villa.err(404, 'unknown');
END $$;
