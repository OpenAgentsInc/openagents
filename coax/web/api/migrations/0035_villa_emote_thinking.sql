-- Planetary Villa: emotes checked against the real model (hc-emotes-live).
-- Asked eight times with "She always spun twice before lying down", Gemini
-- thought for nearly 4,000 tokens and ran out of its 4,096 three times, so
-- the answer came back cut off and the page said "we could not read the
-- answer". Thinking LOW answers every time in about six seconds, and the
-- budget doubles for the motion itself. The prompt said "pelvis any" while the
-- engine turns the pelvis only -180..180, so a spin written as 360 and 540 was
-- refused: it now says so, and how to go round twice.

CREATE OR REPLACE FUNCTION villa.emote_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'You write one short animation ("motion") for a pet''s avatar in a gentle memorial garden, from what its owner says it '
    || 'used to do. Answer name (a-z, 0-9, _ only, at most 24, a word for the move), label (one or two words in capitals for its '
    || E'button) and text: the motion, statements separated by "; ". The format:\n'
    || E'motion NAME; rig animal; seconds S (0.1 to 30); once | loop | hold (after the last key: fade out, repeat, keep the pose); '
    || E'fade F (0 to 2); still (only if the pet stops walking to do it); then keys: at T (seconds from the start, each later than '
    || E'the last); under each key, JOINT PITCH YAW ROLL in degrees, and lift X Y Z in metres for the whole body.\n'
    || E'Joints and their ranges (pitch, yaw, roll): pelvis -180..180 for each; spine and chest -40..40, -40..40, -30..30; neck -70..80, -80..80, -40..40; '
    || E'head -60..60, -60..60, -45..45; left_ear and right_ear -70..70, -60..60, -70..70; tail, tail_2, tail_3 -80..100, -80..80, -40..40; '
    || E'front_left_shoulder and front_right_shoulder -130..80, -25..25, -40..40; front_*_elbow -10..150, -10..10, -10..10; '
    || E'front_*_wrist -40..130, -10..10, -10..10; front_*_paw and hind_*_paw -80..80, -20..20, -20..20; '
    || E'hind_left_hip and hind_right_hip -110..80, -25..25, -40..40; hind_*_knee -30..130, -10..10, -10..10; '
    || E'hind_*_hock -130..40, -10..10, -10..10. Never turn nose, ear tips, tail_tip, toes or eyes. Turning the whole body round is pelvis 0 YAW 0, '
    || E'YAW from -180 to 180 and never past it: a full spin is a key with pelvis 0 -180 0 and a later one with pelvis 0 180 0; '
    || E'to go round again, add a key 0.001 seconds after the 180 one with pelvis 0 -180 0 and turn on from there. Keep it under 6 seconds, gentle and true to what was said. Examples:\n'
    || E'motion sit; rig animal; seconds 1.2; hold; still; fade 0.5; at 1.2; pelvis -32 0 0; lift 0 -0.1 0; front_left_shoulder 32 0 0; front_right_shoulder 32 0 0; hind_left_hip -75 0 0; hind_right_hip -75 0 0; hind_left_knee 125 0 0; hind_right_knee 125 0 0; hind_left_hock -110 0 0; hind_right_hock -110 0 0; neck -5 0 0; head 15 0 0; tail 60 0 0\n'
    || E'motion wag; rig animal; seconds 0.5; loop; fade 0.3; at 0; tail -60 45 0; tail_2 0 25 0; pelvis 0 4 0; at 0.25; tail -60 -45 0; tail_2 0 -25 0; pelvis 0 -4 0\n'
    || E'motion play_bow; rig animal; seconds 2.4; once; still; fade 0.35; at 0.5; pelvis 22 0 0; lift 0 -0.02 0; front_left_shoulder -85 0 0; front_right_shoulder -85 0 0; front_left_elbow 15 0 0; front_right_elbow 15 0 0; neck -40 0 0; head -10 0 0; tail -70 40 0; at 1.1; tail -70 -40 0; at 2.4; pelvis 0 0 0; lift 0 0 0; tail -50 0 0\n'
    || 'If you are told your last motion was refused and why, write it again fixing exactly that.' $$;

CREATE OR REPLACE FUNCTION villa.emote_request(p villa.pets, said text, err text, last text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', villa.emote_prompt()))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text',
      'The pet: ' || p.name || ', a ' || p.species || E'.\nWhat they used to do: ' || said
      || CASE WHEN err <> '' THEN E'\nYour last motion was refused: ' || err || E'\nIt was: ' || last ELSE '' END)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json',
      'responseSchema', '{"type":"object","properties":{"name":{"type":"string"},"label":{"type":"string"},"text":{"type":"string"}},"required":["name","label","text"]}'::jsonb,
      'temperature', 0.4, 'maxOutputTokens', 8192,
      'thinkingConfig', jsonb_build_object('thinkingLevel', 'LOW'))) $$;
