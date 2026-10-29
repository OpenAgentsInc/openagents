-- Build your own realm's setup questions (docs/verse/realm-builder.md; realms.c answers): before the
-- board, /realms/new asks a few things one at a time, each with tap choices or SAY IT: 2D or 3D,
-- the camera (third person, first person, top-down), the art style, the ground and the time of
-- day. What was answered is fixed: the planner is told so and its board is held to it; the model
-- fills only the rest from the prompt. Each answer can be changed later from the board.
--
--   POST /api/playtest/realms/plan {prompt, setup?, plan?, on?}   setup {camera, style, ground, time}: fixed
--   POST /api/playtest/realms {plan, on, setup?}                  the setup as the board shows it now
--   POST /api/playtest/realms/pick {field, words}                 SAY IT for one question: the model
--        picks one of that question's words (or "none") under a schema of just those words
--
-- The camera is the made realm's (myrealm.coax): third (behind you, today's), first (at eye
-- height, you unseen), top (high above at a fixed angle) and side (2D: a side-on view, walking
-- left and right, and jumping). "Second person" isn't a game camera, so it isn't offered.

-- The cameras the made-realm engine draws.
CREATE FUNCTION verse.builder_cameras() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['third', 'first', 'top', 'side'] $$;
ALTER TABLE verse.made_realms ADD COLUMN camera text NOT NULL DEFAULT 'third';

-- The setup questions and their words (the page shows them in this order; dims is 3d or 2d, and
-- 2d means the side camera).
CREATE FUNCTION verse.builder_questions() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$ SELECT jsonb_build_object(
  'dims',   jsonb_build_object('asks', 'Is the realm 2D (seen side-on) or 3D?', 'words', jsonb_build_array('3d', '2d')),
  'camera', jsonb_build_object('asks', 'Which camera: third person (behind you), first person (through your eyes) or top-down (from high above)?',
                               'words', jsonb_build_array('third', 'first', 'top')),
  'style',  jsonb_build_object('asks', 'Which art style: low poly (facets, flat colours) or painterly (soft, painted)?', 'words', to_jsonb(verse.builder_styles())),
  'ground', jsonb_build_object('asks', 'What is the ground: ' || array_to_string(verse.builder_grounds(), ', ') || '?', 'words', to_jsonb(verse.builder_grounds())),
  'time',   jsonb_build_object('asks', 'What time of day: ' || array_to_string(verse.builder_times(), ', ') || '?', 'words', to_jsonb(verse.builder_times()))) $$;

-- The visitor's fixed answers, held: only known fields, each one of its words.
CREATE FUNCTION verse.builder_setup(d jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(jsonb_object_agg(k, d->>k), '{}') FROM unnest(ARRAY['camera', 'style', 'ground', 'time']) k
  WHERE jsonb_typeof(d) = 'object' AND jsonb_typeof(d->k) = 'string'
    AND (d->>k) = ANY (CASE k WHEN 'camera' THEN verse.builder_cameras() WHEN 'style' THEN verse.builder_styles()
                              WHEN 'ground' THEN verse.builder_grounds() ELSE verse.builder_times() END) $$;

-- A board held to the catalogue, now with its camera.
CREATE OR REPLACE FUNCTION verse.builder_hold(d jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  nm text := CASE WHEN jsonb_typeof(d) = 'object' THEN verse.builder_line(d->'name', 30) ELSE '' END;
  f jsonb;
  h jsonb;
  kept jsonb := '[]';
  seen text[] := '{}';
  key text;
  counts jsonb := '{}';
BEGIN
  IF nm = '' THEN RETURN NULL; END IF;
  FOR f IN SELECT e FROM jsonb_array_elements(CASE WHEN jsonb_typeof(d->'features') = 'array' THEN d->'features' ELSE '[]' END) e LIMIT 40 LOOP
    h := verse.builder_hold_feature(f);
    CONTINUE WHEN h IS NULL;
    key := h->>'feature' || ':' || coalesce(h->>'variant', h->>'tool', lower(h->>'name'), '');
    CONTINUE WHEN key = ANY (seen) AND h->>'feature' IN ('props', 'tool', 'character');
    CONTINUE WHEN coalesce((counts->>(h->>'feature'))::int, 0) >= (verse.builder_catalogue()->(h->>'feature')->>'most')::int;
    seen := seen || key;
    counts := counts || jsonb_build_object(h->>'feature', coalesce((counts->>(h->>'feature'))::int, 0) + 1);
    kept := kept || jsonb_build_array(h);
  END LOOP;
  kept := coalesce((SELECT jsonb_agg(e ORDER BY array_position(verse.builder_order(), e->>'feature'), o)
                    FROM jsonb_array_elements(kept) WITH ORDINALITY AS t(e, o)), '[]');
  RETURN jsonb_build_object('name', nm,
    'camera', verse.builder_pick(d->'camera', verse.builder_cameras()),
    'style', verse.builder_pick(d->'style', verse.builder_styles()),
    'ground', verse.builder_pick(d->'ground', verse.builder_grounds()),
    'time', verse.builder_pick(d->'time', verse.builder_times()),
    'features', kept);
END $$;

-- What the model is told: the catalogue, the cameras, and that what the visitor fixed stays.
CREATE OR REPLACE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'You plan a realm for the Verse, a small world in the browser that a visitor is making for themselves. '
    || 'They said, out loud or typed, what world they want. Answer the realm as a short board they will check features on or off: '
    || 'a name for it (at most 30 characters, in their words where they gave one), its camera, its style, its ground and its time of day, '
    || 'and the features from this catalogue that make what they asked for. Use only the catalogue. '
    || 'The realm starts empty: add only features that fit what they said. Add a character only when they asked for people, '
    || 'characters or someone living there; give each a short name and a role of a few words. '
    || 'Add the visitor tools that fit (talk when there are characters; jump and map are usually wanted); put "on" false for '
    || 'a feature that is a good suggestion they did not ask for, true for what they asked for. '
    || 'If they gave the current board, change it as they now say and keep everything else, including what is on or off. '
    || 'If they fixed some answers in setup, use exactly those and choose only the rest. '
    || E'\nCameras: third (third person, behind you), first (first person, through your eyes), top (top-down, from high above), '
    || 'side (a 2D realm seen side-on: walk left and right and jump). Use side only for a 2D or side-scrolling world.'
    || E'\nStyles: lowpoly (facets and flat colours), painterly (soft, painted).'
    || E'\nGrounds: ' || array_to_string(verse.builder_grounds(), ', ') || '. Times: ' || array_to_string(verse.builder_times(), ', ') || '.'
    || E'\nThe features:\n'
    || string_agg('- ' || n || ': ' || (c->>'says')
         || CASE WHEN c ? 'variant' THEN '. variant: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c->'variant') k) ELSE '' END
         || CASE WHEN c ? 'colour' THEN '. colour: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c->'colour') k) ELSE '' END
         || CASE WHEN c ? 'size' THEN '. size: small, medium or large' ELSE '' END
         || CASE WHEN c ? 'tool' THEN '. tool: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c->'tool') k) ELSE '' END
         || CASE WHEN c ? 'count' THEN '. count ' || (c->'count'->>0) || ' to ' || (c->'count'->>1) ELSE '' END
         || CASE WHEN n = 'character' THEN '. name, role' ELSE '' END
         || '. At most ' || (c->>'most') || '.', E'\n' ORDER BY o)
  FROM unnest(verse.builder_order()) WITH ORDINALITY AS t(n, o), LATERAL (SELECT verse.builder_catalogue()->n AS c) x $$;

-- The response schema, now with the camera.
CREATE OR REPLACE FUNCTION verse.builder_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('type', 'object', 'required', jsonb_build_array('name', 'camera', 'style', 'ground', 'time', 'features'), 'properties', jsonb_build_object(
    'name', jsonb_build_object('type', 'string'),
    'camera', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.builder_cameras())),
    'style', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.builder_styles())),
    'ground', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.builder_grounds())),
    'time', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.builder_times())),
    'features', jsonb_build_object('type', 'array', 'items', jsonb_build_object('type', 'object', 'required', jsonb_build_array('feature', 'on'),
      'properties', jsonb_build_object(
        'feature', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.builder_order())),
        'on', jsonb_build_object('type', 'boolean'),
        'variant', jsonb_build_object('type', 'string', 'enum', (SELECT jsonb_agg(DISTINCT k) FROM jsonb_each(verse.builder_catalogue()) e, jsonb_array_elements_text(coalesce(e.value->'variant', '[]')) k)),
        'colour', jsonb_build_object('type', 'string', 'enum', verse.builder_catalogue()->'lanterns'->'colour'),
        'size', jsonb_build_object('type', 'string', 'enum', jsonb_build_array('small', 'medium', 'large')),
        'count', jsonb_build_object('type', 'integer'),
        'tool', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.builder_tools())),
        'name', jsonb_build_object('type', 'string'),
        'role', jsonb_build_object('type', 'string')))))) $$;

-- The board a request means, with the setup the page shows laid over it (fixed answers win).
CREATE OR REPLACE FUNCTION verse.builder_board_of(body jsonb, rec verse.players) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  b jsonb;
BEGIN
  IF jsonb_typeof(body) <> 'object' THEN RETURN NULL; END IF;
  IF jsonb_typeof(body->'plan') = 'number' AND jsonb_typeof(body->'on') = 'array' THEN
    SELECT p.board INTO b FROM verse.realm_plans p WHERE p.id = (body->>'plan')::bigint AND p.player = rec.id AND p.board IS NOT NULL;
    IF b IS NULL THEN RETURN NULL; END IF;
    RETURN b - 'features' || verse.builder_setup(body->'setup') || jsonb_build_object('features', coalesce((SELECT jsonb_agg(f || jsonb_build_object('on',
        CASE WHEN jsonb_typeof(body->'on'->((o - 1)::int)) = 'boolean' THEN (body->'on'->>((o - 1)::int))::boolean ELSE (f->>'on')::boolean END) ORDER BY o)
      FROM jsonb_array_elements(b->'features') WITH ORDINALITY AS t(f, o)), '[]'));
  END IF;
  b := verse.builder_hold(body->'board');
  RETURN CASE WHEN b IS NULL THEN NULL ELSE b || verse.builder_setup(body->'setup') END;
END $$;

-- The board's lines, now with "camera C" after the time.
CREATE OR REPLACE FUNCTION verse.builder_lines(plan bigint, b jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'plan ' || plan || E'\nname ' || (b->>'name') || E'\nstyle ' || (b->>'style') || E'\nground ' || (b->>'ground') || E'\ntime ' || (b->>'time')
      || E'\ncamera ' || coalesce(b->>'camera', 'third') || E'\n'
      || coalesce((SELECT string_agg('f ' || (f->>'feature') || ' ' || CASE WHEN (f->>'on')::boolean THEN '1' ELSE '0' END || ' ' || (f->>'label'), E'\n' ORDER BY o)
                   FROM jsonb_array_elements(b->'features') WITH ORDINALITY AS t(f, o)) || E'\n', '') $$;

-- A prompt asked, with the visitor's fixed setup: the model is told what is fixed, and the answer
-- carries the setup to builder_answer.
CREATE OR REPLACE FUNCTION verse.builder_ask(body jsonb, cap int, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  said text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'prompt', 600) ELSE '' END;
  rec verse.players := verse.builder_player(k);
  board jsonb := verse.builder_board_of(body, rec);
  setup jsonb := verse.builder_setup(CASE WHEN jsonb_typeof(body) = 'object' THEN body->'setup' END);
  row_id bigint;
  text_in text;
BEGIN
  IF said = '' THEN RETURN '{"status":400,"error":"say what world you want"}'; END IF;
  IF rec.id IS NOT NULL AND EXISTS (SELECT 1 FROM verse.realm_plans WHERE player = rec.id AND ms > verse.now_ms() - 2000) THEN
    RETURN '{"status":429,"error":"one at a time"}';
  END IF;
  IF (SELECT count(*) FROM verse.realm_plans WHERE ms > (extract(epoch FROM date_trunc('day', now())) * 1000)::bigint) >= cap THEN
    RETURN '{"status":503,"error":"the day''s plans are used up"}';
  END IF;
  INSERT INTO verse.realm_plans (ms, player, prompt, board_in) VALUES (verse.now_ms(), rec.id, said, board) RETURNING id INTO row_id;
  text_in := CASE WHEN board IS NULL THEN said
                  ELSE 'The current board: ' || (board - 'features' || jsonb_build_object('features',
                         (SELECT coalesce(jsonb_agg(f - 'label'), '[]') FROM jsonb_array_elements(board->'features') f)))::text
                       || E'\nThey now say: ' || said END;
  IF setup <> '{}' THEN text_in := 'Fixed in setup: ' || setup::text || E'\n' || text_in; END IF;
  RETURN jsonb_build_object('status', 200, 'id', row_id, 'setup', setup, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', verse.builder_prompt()))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text', text_in)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'responseSchema', verse.builder_schema(),
      'temperature', 0.4, 'maxOutputTokens', 4000, 'thinkingConfig', jsonb_build_object('thinkingLevel', 'LOW'))));
END $$;

-- The model's answer, held to the catalogue, the fixed setup laid over it, and kept.
CREATE OR REPLACE FUNCTION verse.builder_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
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
    ELSE held := verse.builder_hold(d); IF held IS NULL THEN status := 502; err := 'not a board'; END IF;
    END IF;
  ELSE
    status := 503;
    err := CASE WHEN http = 429 THEN 'busy' WHEN http = 0 THEN 'model unreachable' ELSE 'model error ' || http END;
  END IF;
  IF held IS NOT NULL THEN held := held || verse.builder_setup(asked->'setup'); END IF;
  UPDATE verse.realm_plans SET board = held, error = err, model = builder_answer.model, ms_taken = taken WHERE id = (asked->>'id')::bigint;
  IF held IS NULL THEN RETURN jsonb_build_object('status', status, 'error', err); END IF;
  RETURN jsonb_build_object('status', 200, 'plan', (asked->>'id')::bigint, 'board', held, 'model', model,
                            'lines', verse.builder_lines((asked->>'id')::bigint, held));
END $$;

-- MAKE IT, keeping the camera.
CREATE OR REPLACE FUNCTION verse.builder_make(body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  rec verse.players := verse.builder_player(k);
  b jsonb := verse.builder_board_of(body, rec);
  feats jsonb;
  tools jsonb;
  s text;
  plan_id bigint := CASE WHEN jsonb_typeof(body->'plan') = 'number' THEN (body->>'plan')::bigint END;
BEGIN
  IF rec.id IS NULL THEN RETURN '{"status":401,"error":"no player"}'; END IF;
  IF b IS NULL THEN RETURN '{"status":400,"error":"no board"}'; END IF;
  IF (SELECT count(*) FROM verse.made_realms m JOIN verse.worlds w ON w.id = m.world
      WHERE w.owner_player = rec.id AND m.made_ms > verse.now_ms() - 86400000) >= per_day THEN
    RETURN '{"status":429,"error":"that''s today''s realms"}';
  END IF;
  feats := coalesce((SELECT jsonb_agg(f - 'on') FROM jsonb_array_elements(b->'features') f
                     WHERE (f->>'on')::boolean AND f->>'feature' <> 'tool'), '[]');
  tools := (SELECT jsonb_object_agg(t, EXISTS (SELECT 1 FROM jsonb_array_elements(b->'features') f
                                              WHERE f->>'feature' = 'tool' AND f->>'tool' = t AND (f->>'on')::boolean))
            FROM unnest(verse.builder_tools()) t);
  s := verse.builder_slug(b->>'name');
  INSERT INTO verse.worlds (id, owner_player, owner_account, created_ms) VALUES ('made-' || s, rec.id, rec.account, verse.now_ms());
  INSERT INTO verse.made_realms (slug, world, name, style, ground, time, camera, features, tools, plan, made_ms)
  VALUES (s, 'made-' || s, b->>'name', b->>'style', b->>'ground', b->>'time', verse.builder_pick(b->'camera', verse.builder_cameras()), feats, tools,
          (SELECT id FROM verse.realm_plans WHERE id = plan_id AND player = rec.id), verse.now_ms());
  RETURN jsonb_build_object('status', 200, 'slug', s, 'path', '/realms/' || s, 'name', b->>'name');
END $$;

-- The realm's spec, now with "camera C" after the time.
CREATE OR REPLACE FUNCTION verse.builder_spec(s text, k text) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  m verse.made_realms;
  mine boolean := verse.builder_owns(k, s);
  out text;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NULL THEN RETURN NULL; END IF;
  out := 'realm ' || m.slug || E'\nname ' || m.name || E'\nstyle ' || m.style || E'\nground ' || m.ground || E'\ntime ' || m.time
      || E'\ncamera ' || m.camera || E'\nowner ' || CASE WHEN mine THEN '1' ELSE '0' END || E'\n';
  out := out || coalesce((SELECT string_agg('tool ' || t || ' ' || CASE WHEN coalesce((m.tools->>t)::boolean, false) THEN '1' ELSE '0' END, E'\n' ORDER BY o)
                          FROM unnest(verse.builder_tools()) WITH ORDINALITY AS x(t, o)) || E'\n', '');
  out := out || coalesce((SELECT string_agg(CASE f->>'feature'
      WHEN 'character' THEN 'character ' || (f->>'name') || '|' || (f->>'role')
      ELSE 'feature ' || (f->>'feature') || concat(' ' || (f->>'size'), ' ' || (f->>'variant'), ' ' || (f->>'colour'), ' ' || (f->>'count')) END, E'\n' ORDER BY o)
    FROM jsonb_array_elements(m.features) WITH ORDINALITY AS x(f, o)) || E'\n', '');
  out := out || coalesce((SELECT string_agg('placed ' || (p->>'thing') || ' ' || trim_scale(round((p->>'x')::numeric, 2)) || ' ' || trim_scale(round((p->>'z')::numeric, 2))
                                            || ' ' || trim_scale(round((p->>'yaw')::numeric, 3)), E'\n' ORDER BY o)
                          FROM jsonb_array_elements(m.placed) WITH ORDINALITY AS x(p, o)) || E'\n', '');
  IF mine THEN
    out := out || 'waiting ' || (SELECT count(*) FROM verse.realm_proposals WHERE slug = s AND state = 'waiting') || E'\n';
  END IF;
  RETURN out;
END $$;

-- The realm's text, now naming its camera.
CREATE OR REPLACE FUNCTION verse.builder_md(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '# ' || m.name || E'\n\nA realm on the Verse at https://openagents.com/realms/' || m.slug
      || ', made by a visitor from one prompt (https://openagents.com/realms/new). '
      || CASE m.camera WHEN 'first' THEN '3D, first person' WHEN 'top' THEN '3D, top-down' WHEN 'side' THEN '2D, side-on' ELSE '3D, third person' END
      || '. Style: ' || m.style || '. Ground: ' || m.ground
      || '. Time of day: ' || m.time || E'.\n\n## What is here\n\n'
      || coalesce((SELECT string_agg('- ' || (f->>'label'), E'\n' ORDER BY o) FROM jsonb_array_elements(m.features) WITH ORDINALITY AS x(f, o)), '- Nothing yet: open ground')
      || E'\n\n## What visitors may do\n\n- Walk'
      || coalesce(E'\n' || (SELECT string_agg('- ' || initcap(t), E'\n' ORDER BY o) FROM unnest(verse.builder_tools()) WITH ORDINALITY AS x(t, o)
                            WHERE coalesce((m.tools->>t)::boolean, false)), '')
      || E'\n\nThe owner decides what visitors get, and accepts or declines what others build here.\n'
  FROM verse.made_realms m WHERE m.slug = s $$;

-- SAY IT for one setup question: {field, words} -> the model picks one of that question's words, or
-- "none" when the words name none of them. Kept in realm_plans like a plan (the same caps).
CREATE FUNCTION verse.builder_pick_ask(body jsonb, cap int, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  field text := CASE WHEN jsonb_typeof(body) = 'object' AND jsonb_typeof(body->'field') = 'string' THEN body->>'field' END;
  q jsonb := verse.builder_questions()->field;
  said text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'words', 300) ELSE '' END;
  rec verse.players := verse.builder_player(k);
  row_id bigint;
BEGIN
  IF q IS NULL THEN RETURN '{"status":400,"error":"no such question"}'; END IF;
  IF said = '' THEN RETURN '{"status":400,"error":"say one of them"}'; END IF;
  IF rec.id IS NOT NULL AND EXISTS (SELECT 1 FROM verse.realm_plans WHERE player = rec.id AND ms > verse.now_ms() - 2000) THEN
    RETURN '{"status":429,"error":"one at a time"}';
  END IF;
  IF (SELECT count(*) FROM verse.realm_plans WHERE ms > (extract(epoch FROM date_trunc('day', now())) * 1000)::bigint) >= cap THEN
    RETURN '{"status":503,"error":"the day''s plans are used up"}';
  END IF;
  INSERT INTO verse.realm_plans (ms, player, prompt, board_in) VALUES (verse.now_ms(), rec.id, said, jsonb_build_object('pick', field)) RETURNING id INTO row_id;
  RETURN jsonb_build_object('status', 200, 'id', row_id, 'field', field, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text',
      'A visitor is setting up a realm on the Verse, a small world in the browser. They were asked: ' || (q->>'asks')
      || ' Pick the option they mean from what they said. Answer "none" if they named none of them.'))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text', said)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json',
      'responseSchema', jsonb_build_object('type', 'object', 'required', jsonb_build_array('value'), 'properties', jsonb_build_object(
        'value', jsonb_build_object('type', 'string', 'enum', (q->'words') || '["none"]'::jsonb))),
      'temperature', 0.0, 'maxOutputTokens', 200, 'thinkingConfig', jsonb_build_object('thinkingLevel', 'LOW'))));
END $$;

-- The pick's answer: 200 {field, value, lines "field F\nvalue V\n"}; value "none" when nothing fits.
CREATE FUNCTION verse.builder_pick_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  r jsonb;
  t text;
  d jsonb;
  v text;
  field text := asked->>'field';
  words jsonb := verse.builder_questions()->field->'words';
BEGIN
  BEGIN r := response::jsonb; EXCEPTION WHEN others THEN r := NULL; END;
  IF http = 200 AND r IS NOT NULL THEN
    SELECT string_agg(p->>'text', '') INTO t
    FROM jsonb_array_elements(coalesce(r->'candidates'->0->'content'->'parts', '[]')) p
    WHERE coalesce((p->>'thought')::boolean, false) = false AND p ? 'text';
    BEGIN d := t::jsonb; EXCEPTION WHEN others THEN d := NULL; END;
    v := CASE WHEN jsonb_typeof(d->'value') = 'string' AND words ? (d->>'value') THEN d->>'value' ELSE 'none' END;
  END IF;
  UPDATE verse.realm_plans SET board = CASE WHEN v IS NULL THEN NULL ELSE jsonb_build_object('pick', field, 'value', v) END,
         error = CASE WHEN v IS NULL THEN 'model error ' || http END, model = builder_pick_answer.model, ms_taken = taken
  WHERE id = (asked->>'id')::bigint;
  IF v IS NULL THEN RETURN jsonb_build_object('status', 503, 'error', CASE WHEN http = 429 THEN 'busy' ELSE 'no answer' END); END IF;
  RETURN jsonb_build_object('status', 200, 'field', field, 'value', v, 'lines', 'field ' || field || E'\nvalue ' || v || E'\n');
END $$;

-- The builder's own routes are never a made realm's address either.
CREATE OR REPLACE FUNCTION verse.builder_reserved() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['new', 'built', 'drop', 'fixture', 'sandbox', 'garden', 'characters', 'haunted-house', 'the-grid', 'everglade',
               'fairy-forest', 'dead-gears', 'ice-cream-man', 'hecates-cemetery', 'assets', 'made', 'mine', 'index', 'realm', 'plan', 'pick'] $$;
