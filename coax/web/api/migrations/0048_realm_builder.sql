-- Build your own realm (realm-builder; docs/verse/realm-builder.md; realms.c answers):
--
--   POST /api/playtest/realms/plan {prompt, board?}   the board: Gemini on Vertex AI reads what the
--        visitor said and answers a realm under a response schema whose enums are this catalogue's
--        words; this file holds the answer to the catalogue and writes each feature's few words
--   POST /api/playtest/realms {plan?, board}          MAKE IT: the checked features become a realm
--        owned by the visitor's player key (and its account), at /realms/<slug>
--   GET  /api/playtest/realms/<slug>                  the realm's spec, a line each (myrealm.coax reads it)
--   POST /api/playtest/realms/<slug>/settings {tools} the owner only: what visitors get
--   POST /api/playtest/realms/<slug>/place {thing, x, z, yaw}   the owner places; anyone else proposes
--   GET  /api/playtest/realms/<slug>/proposals        the owner only: what waits, a line each
--   POST /api/playtest/realms/<slug>/proposals/<id>/accept|decline   the owner only
--
-- No words of a prompt are matched here: the model reads them and only its typed answer is
-- checked. A realm is made from the board the page sends back, held to the catalogue again, so a
-- page can only switch features on or off; what is off is never kept. A made realm is a world
-- (verse.worlds) owned like any other, so logging in later (/api/playtest/signin) claims a guest's.

-- What the made-realm engine (apps/openagents-web/myrealm.coax) draws today. Each feature: what it
-- is (the model reads it), its fields and their words (the default first), a count [min, max,
-- default], and how many a board may hold.
CREATE FUNCTION verse.builder_catalogue() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$ SELECT '{
  "water":     {"says": "a pond or small lake", "size": ["medium", "small", "large"], "most": 1},
  "trees":     {"says": "a grove of trees", "variant": ["round", "pine", "dead", "mixed"], "count": [3, 20, 9], "most": 1},
  "rocks":     {"says": "rocks scattered about", "count": [2, 16, 6], "most": 1},
  "plants":    {"says": "ferns and low plants", "count": [4, 24, 10], "most": 1},
  "lanterns":  {"says": "a ring of glowing lanterns", "colour": ["gold", "orange", "red", "green", "blue", "violet", "white"], "count": [4, 16, 8], "most": 1},
  "flock":     {"says": "a flock of birds that circles whoever walks", "variant": ["crow", "bat"], "count": [3, 12, 7], "most": 1},
  "props":     {"says": "things standing about: crates, barrels, a wagon, a fence, or a camp (a chest and an anvil)", "variant": ["crates", "barrels", "wagon", "fence", "camp"], "most": 5},
  "character": {"says": "a character who lives here, with a name and a role; visitors can talk to them", "most": 4},
  "tool":      {"says": "something visitors may do here", "tool": ["build", "fly", "jump", "map", "talk", "voice"], "most": 6}
}'::jsonb $$;
CREATE FUNCTION verse.builder_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['water', 'trees', 'rocks', 'plants', 'lanterns', 'flock', 'props', 'character', 'tool'] $$;
CREATE FUNCTION verse.builder_grounds() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['meadow', 'forest', 'desert', 'snow', 'stone', 'beach'] $$;
CREATE FUNCTION verse.builder_times() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['day', 'dawn', 'dusk', 'night'] $$;
-- The styles the made-realm engine draws (docs/verse/art-styles.md); photoreal isn't built.
CREATE FUNCTION verse.builder_styles() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['lowpoly', 'painterly'] $$;
CREATE FUNCTION verse.builder_tools() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['build', 'fly', 'jump', 'map', 'talk', 'voice'] $$;
-- What a visitor (or the owner) may place in a made realm with build.
CREATE FUNCTION verse.builder_things() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['crate', 'barrel', 'tree', 'rock', 'plant', 'lantern', 'fence', 'chest'] $$;
-- Realm pages the site already has: a made realm never takes their address.
CREATE FUNCTION verse.builder_reserved() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['new', 'built', 'drop', 'fixture', 'sandbox', 'garden', 'characters', 'haunted-house', 'the-grid', 'everglade',
               'fairy-forest', 'dead-gears', 'ice-cream-man', 'hecates-cemetery', 'assets', 'made', 'mine', 'index', 'realm'] $$;

CREATE TABLE verse.realm_plans (
  id bigserial PRIMARY KEY,
  ms bigint NOT NULL,
  player text,                   -- verse.players id
  prompt text NOT NULL,
  board_in jsonb,                -- the board revised, if any
  board jsonb,                   -- what the page got
  error text,
  model text,
  ms_taken int
);
CREATE INDEX realm_plans_player ON verse.realm_plans (player, ms);
CREATE INDEX realm_plans_ms ON verse.realm_plans (ms);

CREATE TABLE verse.made_realms (
  slug text PRIMARY KEY,
  world text NOT NULL UNIQUE REFERENCES verse.worlds (id),
  name text NOT NULL,
  style text NOT NULL,
  ground text NOT NULL,
  time text NOT NULL,
  features jsonb NOT NULL,       -- only the checked ones, held to the catalogue (tools apart)
  tools jsonb NOT NULL,          -- {"build": bool, ...}: what visitors get; the owner changes it
  placed jsonb NOT NULL DEFAULT '[]',   -- [{thing, x, z, yaw}] placed by the owner, or accepted
  plan bigint,
  made_ms bigint NOT NULL,
  settings_ms bigint,
  page_ms bigint,                -- its page is in the site's bucket
  page_note text
);
CREATE TABLE verse.realm_proposals (
  id bigserial PRIMARY KEY,
  slug text NOT NULL REFERENCES verse.made_realms (slug),
  ms bigint NOT NULL,
  by_player text,
  change jsonb NOT NULL,         -- {kind: place, thing, x, z, yaw}
  state text NOT NULL DEFAULT 'waiting' CHECK (state IN ('waiting', 'accepted', 'declined')),
  decided_ms bigint
);
CREATE INDEX realm_proposals_slug ON verse.realm_proposals (slug, state, id);

CREATE FUNCTION verse.builder_player(k text) RETURNS verse.players LANGUAGE sql STABLE AS $$
  SELECT * FROM verse.players WHERE k ~ '^[A-Za-z0-9_-]{20,128}$' AND key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex') $$;

-- One line of plain words: no line breaks or bars, at most n characters.
CREATE FUNCTION verse.builder_line(v jsonb, n int) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(btrim(regexp_replace(CASE WHEN jsonb_typeof(v) = 'string' THEN v #>> '{}' ELSE '' END, '[[:cntrl:]|]+', ' ', 'g')), n) $$;
CREATE FUNCTION verse.builder_pick(v jsonb, options jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(v) = 'string' AND options ? (v #>> '{}') THEN v #>> '{}' ELSE options->>0 END $$;
CREATE FUNCTION verse.builder_pick(v jsonb, options text[]) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(v) = 'string' AND (v #>> '{}') = ANY (options) THEN v #>> '{}' ELSE options[1] END $$;

-- A feature's few words, written here from its typed fields (never the model's own words, so the
-- board says exactly what will be built).
CREATE FUNCTION verse.builder_label(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE f->>'feature'
    WHEN 'water' THEN initcap(f->>'size') || ' pond'
    WHEN 'trees' THEN (f->>'count') || ' ' || CASE f->>'variant' WHEN 'mixed' THEN 'trees and bushes' ELSE (f->>'variant') || ' trees' END
    WHEN 'rocks' THEN (f->>'count') || ' rocks'
    WHEN 'plants' THEN (f->>'count') || ' ferns'
    WHEN 'lanterns' THEN (f->>'count') || ' ' || (f->>'colour') || ' lanterns'
    WHEN 'flock' THEN 'A flock of ' || (f->>'count') || ' ' || (f->>'variant') || 's'
    WHEN 'props' THEN CASE f->>'variant' WHEN 'crates' THEN 'Crates' WHEN 'barrels' THEN 'Barrels' WHEN 'wagon' THEN 'A wagon'
                                         WHEN 'fence' THEN 'A fence' ELSE 'A camp: chest and anvil' END
    WHEN 'character' THEN (f->>'name') || ', ' || (f->>'role')
    WHEN 'tool' THEN CASE f->>'tool' WHEN 'build' THEN 'Visitors can build' WHEN 'fly' THEN 'Visitors can fly'
                                     WHEN 'jump' THEN 'Visitors can jump' WHEN 'map' THEN 'A map for visitors'
                                     WHEN 'talk' THEN 'Visitors can talk to characters' ELSE 'Voice: talk out loud to characters' END
  END $$;

-- A feature held to the catalogue: only its own fields, each one of its words or in its range,
-- with its few words; NULL when it isn't one.
CREATE FUNCTION verse.builder_hold_feature(d jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  name text := CASE WHEN jsonb_typeof(d) = 'object' AND jsonb_typeof(d->'feature') = 'string' THEN d->>'feature' END;
  c jsonb := verse.builder_catalogue()->name;
  out jsonb;
  n numeric;
BEGIN
  IF c IS NULL OR name IS NULL THEN RETURN NULL; END IF;
  out := jsonb_build_object('feature', name, 'on', CASE WHEN jsonb_typeof(d->'on') = 'boolean' THEN (d->>'on')::boolean ELSE true END);
  IF c ? 'size' THEN out := out || jsonb_build_object('size', verse.builder_pick(d->'size', c->'size')); END IF;
  IF c ? 'variant' THEN out := out || jsonb_build_object('variant', verse.builder_pick(d->'variant', c->'variant')); END IF;
  IF c ? 'colour' THEN out := out || jsonb_build_object('colour', verse.builder_pick(d->'colour', c->'colour')); END IF;
  IF c ? 'tool' THEN
    IF NOT (jsonb_typeof(d->'tool') = 'string' AND c->'tool' ? (d->>'tool')) THEN RETURN NULL; END IF;
    out := out || jsonb_build_object('tool', d->>'tool');
  END IF;
  IF c ? 'count' THEN
    n := CASE WHEN jsonb_typeof(d->'count') = 'number' THEN round((d->>'count')::numeric) END;
    out := out || jsonb_build_object('count', CASE WHEN n IS NULL OR n <= 0 THEN (c->'count'->>2)::int
                                                   ELSE greatest((c->'count'->>0)::int, least((c->'count'->>1)::int, n))::int END);
  END IF;
  IF name = 'character' THEN
    IF verse.builder_line(d->'name', 24) = '' THEN RETURN NULL; END IF;
    out := out || jsonb_build_object('name', verse.builder_line(d->'name', 24),
                                     'role', coalesce(nullif(verse.builder_line(d->'role', 40), ''), 'lives here'));
  END IF;
  RETURN out || jsonb_build_object('label', verse.builder_label(out));
END $$;

-- A board held to the catalogue: its name, style, ground and time, and its features in the
-- catalogue's order, each kind at most its "most" (props once per variant, a tool once, a
-- character once per name). NULL when it has no name.
CREATE FUNCTION verse.builder_hold(d jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
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
    'style', verse.builder_pick(d->'style', verse.builder_styles()),
    'ground', verse.builder_pick(d->'ground', verse.builder_grounds()),
    'time', verse.builder_pick(d->'time', verse.builder_times()),
    'features', kept);
END $$;

-- What the model is told.
CREATE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'You plan a realm for the Verse, a small 3D world in the browser that a visitor is making for themselves. '
    || 'They said, out loud or typed, what world they want. Answer the realm as a short board they will check features on or off: '
    || 'a name for it (at most 30 characters, in their words where they gave one), its style, its ground and its time of day, '
    || 'and the features from this catalogue that make what they asked for. Use only the catalogue. '
    || 'The realm starts empty: add only features that fit what they said. Add a character only when they asked for people, '
    || 'characters or someone living there; give each a short name and a role of a few words. '
    || 'Add the visitor tools that fit (talk when there are characters; jump and map are usually wanted); put "on" false for '
    || 'a feature that is a good suggestion they did not ask for, true for what they asked for. '
    || 'If they gave the current board, change it as they now say and keep everything else, including what is on or off. '
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

-- The response schema: the catalogue's words as enums.
CREATE FUNCTION verse.builder_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('type', 'object', 'required', jsonb_build_array('name', 'style', 'ground', 'time', 'features'), 'properties', jsonb_build_object(
    'name', jsonb_build_object('type', 'string'),
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

-- The board a request means: {plan, on: [bool, ...]} is that plan's board (this player's) with each
-- feature switched as the page shows it, so a page only ever switches features; else {board}, held.
CREATE FUNCTION verse.builder_board_of(body jsonb, rec verse.players) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  b jsonb;
BEGIN
  IF jsonb_typeof(body) <> 'object' THEN RETURN NULL; END IF;
  IF jsonb_typeof(body->'plan') = 'number' AND jsonb_typeof(body->'on') = 'array' THEN
    SELECT p.board INTO b FROM verse.realm_plans p WHERE p.id = (body->>'plan')::bigint AND p.player = rec.id AND p.board IS NOT NULL;
    IF b IS NULL THEN RETURN NULL; END IF;
    RETURN b - 'features' || jsonb_build_object('features', coalesce((SELECT jsonb_agg(f || jsonb_build_object('on',
        CASE WHEN jsonb_typeof(body->'on'->((o - 1)::int)) = 'boolean' THEN (body->'on'->>((o - 1)::int))::boolean ELSE (f->>'on')::boolean END) ORDER BY o)
      FROM jsonb_array_elements(b->'features') WITH ORDINALITY AS t(f, o)), '[]'));
  END IF;
  RETURN verse.builder_hold(body->'board');
END $$;

-- A board as lines, for a page that reads lines (builder.coax): "plan ID", "name", "style", "ground",
-- "time", then "f FEATURE ON LABEL" a feature each, in the board's order.
CREATE FUNCTION verse.builder_lines(plan bigint, b jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'plan ' || plan || E'\nname ' || (b->>'name') || E'\nstyle ' || (b->>'style') || E'\nground ' || (b->>'ground') || E'\ntime ' || (b->>'time') || E'\n'
      || coalesce((SELECT string_agg('f ' || (f->>'feature') || ' ' || CASE WHEN (f->>'on')::boolean THEN '1' ELSE '0' END || ' ' || (f->>'label'), E'\n' ORDER BY o)
                   FROM jsonb_array_elements(b->'features') WITH ORDINALITY AS t(f, o)) || E'\n', '') $$;

-- A prompt asked: {"status", "error"} when it can't be, else the row it is kept in and the request.
CREATE FUNCTION verse.builder_ask(body jsonb, cap int, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  said text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'prompt', 600) ELSE '' END;
  rec verse.players := verse.builder_player(k);
  board jsonb := verse.builder_board_of(body, rec);
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
  RETURN jsonb_build_object('status', 200, 'id', row_id, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', verse.builder_prompt()))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text', text_in)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'responseSchema', verse.builder_schema(),
      'temperature', 0.4, 'maxOutputTokens', 4000, 'thinkingConfig', jsonb_build_object('thinkingLevel', 'LOW'))));
END $$;

-- The model's answer, held to the catalogue and kept: 200 {plan, board, model}.
CREATE FUNCTION verse.builder_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
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
  UPDATE verse.realm_plans SET board = held, error = err, model = builder_answer.model, ms_taken = taken WHERE id = (asked->>'id')::bigint;
  IF held IS NULL THEN RETURN jsonb_build_object('status', status, 'error', err); END IF;
  RETURN jsonb_build_object('status', 200, 'plan', (asked->>'id')::bigint, 'board', held, 'model', model,
                            'lines', verse.builder_lines((asked->>'id')::bigint, held));
END $$;

-- An address from a name: lower case, words joined by hyphens, never a page the site has, unique.
CREATE FUNCTION verse.builder_slug(nm text) RETURNS text LANGUAGE plpgsql VOLATILE AS $$
DECLARE
  base text := left(trim(BOTH '-' FROM regexp_replace(lower(nm), '[^a-z0-9]+', '-', 'g')), 40);
  s text;
  i int := 1;
BEGIN
  base := trim(BOTH '-' FROM base);
  IF base = '' OR base !~ '^[a-z0-9]' THEN base := 'realm'; END IF;
  s := base;
  WHILE s = ANY (verse.builder_reserved()) OR EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s) OR EXISTS (SELECT 1 FROM verse.worlds WHERE id = 'made-' || s) LOOP
    i := i + 1;
    s := left(base, 36) || '-' || i;
  END LOOP;
  RETURN s;
END $$;

-- MAKE IT: the board as the page sends it back, held again; only what is on is kept.
-- -> {status, slug, path} | {status, error}
CREATE FUNCTION verse.builder_make(body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE plpgsql AS $$
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
  INSERT INTO verse.made_realms (slug, world, name, style, ground, time, features, tools, plan, made_ms)
  VALUES (s, 'made-' || s, b->>'name', b->>'style', b->>'ground', b->>'time', feats, tools,
          (SELECT id FROM verse.realm_plans WHERE id = plan_id AND player = rec.id), verse.now_ms());
  RETURN jsonb_build_object('status', 200, 'slug', s, 'path', '/realms/' || s, 'name', b->>'name');
END $$;

CREATE FUNCTION verse.builder_owns(k text, s text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT coalesce(verse.owns(verse.builder_player(k), NULL, 'made-' || s), false) $$;

-- The realm's spec for myrealm.coax, a line each:
--   realm SLUG / name NAME / style S / ground G / time T / owner 0|1 / tool NAME 0|1 ...
--   feature water SIZE | trees VARIANT COUNT | rocks COUNT | plants COUNT | lanterns COLOUR COUNT
--           | flock VARIANT COUNT | props VARIANT
--   character NAME|ROLE / placed THING X Z YAW / waiting N (the owner only: proposals waiting)
CREATE FUNCTION verse.builder_spec(s text, k text) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  m verse.made_realms;
  mine boolean := verse.builder_owns(k, s);
  out text;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NULL THEN RETURN NULL; END IF;
  out := 'realm ' || m.slug || E'\nname ' || m.name || E'\nstyle ' || m.style || E'\nground ' || m.ground || E'\ntime ' || m.time
      || E'\nowner ' || CASE WHEN mine THEN '1' ELSE '0' END || E'\n';
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

-- The owner's settings: what visitors get. -> {status, tools} | {status, error}
CREATE FUNCTION verse.builder_settings(s text, body jsonb, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms;
  t jsonb := CASE WHEN jsonb_typeof(body->'tools') = 'object' THEN body->'tools' ELSE '{}' END;
  next jsonb;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF NOT verse.builder_owns(k, s) THEN RETURN '{"status":403,"error":"only its owner"}'; END IF;
  next := (SELECT jsonb_object_agg(x, CASE WHEN jsonb_typeof(t->x) = 'boolean' THEN (t->>x)::boolean ELSE coalesce((m.tools->>x)::boolean, false) END)
           FROM unnest(verse.builder_tools()) x);
  UPDATE verse.made_realms SET tools = next, settings_ms = verse.now_ms() WHERE slug = s;
  RETURN jsonb_build_object('status', 200, 'tools', next);
END $$;

-- A thing placed: the owner's goes in; anyone else's waits for the owner (build must be on for
-- visitors). -> {status, placed: true} | {status, proposed: id} | {status, error}
CREATE FUNCTION verse.builder_place(s text, body jsonb, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms;
  rec verse.players := verse.builder_player(k);
  thing text := CASE WHEN jsonb_typeof(body->'thing') = 'string' AND (body->>'thing') = ANY (verse.builder_things()) THEN body->>'thing' END;
  x numeric := CASE WHEN jsonb_typeof(body->'x') = 'number' THEN (body->>'x')::numeric END;
  z numeric := CASE WHEN jsonb_typeof(body->'z') = 'number' THEN (body->>'z')::numeric END;
  yaw numeric := CASE WHEN jsonb_typeof(body->'yaw') = 'number' THEN (body->>'yaw')::numeric ELSE 0 END;
  change jsonb;
  pid bigint;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF rec.id IS NULL THEN RETURN '{"status":401,"error":"no player"}'; END IF;
  IF thing IS NULL OR x IS NULL OR z IS NULL OR abs(x) > 40 OR abs(z) > 40 THEN RETURN '{"status":400,"error":"not a thing here"}'; END IF;
  change := jsonb_build_object('kind', 'place', 'thing', thing, 'x', trim_scale(round(x, 2)), 'z', trim_scale(round(z, 2)), 'yaw', trim_scale(round(yaw, 3)));
  IF verse.builder_owns(k, s) THEN
    IF jsonb_array_length(m.placed) >= 200 THEN RETURN '{"status":409,"error":"the realm is full"}'; END IF;
    UPDATE verse.made_realms SET placed = placed || jsonb_build_array(change - 'kind') WHERE slug = s;
    RETURN '{"status":200,"placed":true}';
  END IF;
  IF NOT coalesce((m.tools->>'build')::boolean, false) THEN RETURN '{"status":403,"error":"the owner hasn''t let visitors build"}'; END IF;
  IF (SELECT count(*) FROM verse.realm_proposals WHERE slug = s AND state = 'waiting') >= 50 THEN RETURN '{"status":429,"error":"the owner has enough to look at"}'; END IF;
  INSERT INTO verse.realm_proposals (slug, ms, by_player, change) VALUES (s, verse.now_ms(), rec.id, change) RETURNING id INTO pid;
  RETURN jsonb_build_object('status', 200, 'proposed', pid);
END $$;

-- The owner's cards, a line each: "P ID|WHO|THING|X|Z|YAW|BEFORE|AFTER", newest first.
CREATE FUNCTION verse.builder_proposals(s text, k text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg('P ' || p.id || '|' || 'Visitor ' || right(coalesce(p.by_player, '----'), 4) || '|' || (p.change->>'thing') || '|'
         || (p.change->>'x') || '|' || (p.change->>'z') || '|' || (p.change->>'yaw') || '|'
         || (SELECT count(*) FROM jsonb_array_elements(m.placed) e WHERE e->>'thing' = p.change->>'thing') || '|'
         || ((SELECT count(*) FROM jsonb_array_elements(m.placed) e WHERE e->>'thing' = p.change->>'thing') + 1), E'\n' ORDER BY p.id DESC) || E'\n', '')
  FROM verse.realm_proposals p JOIN verse.made_realms m ON m.slug = p.slug
  WHERE p.slug = s AND p.state = 'waiting' AND verse.builder_owns(k, s) $$;

-- ACCEPT or DECLINE, the owner only. -> {status, state}
CREATE FUNCTION verse.builder_decide(s text, pid bigint, accept boolean, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  p verse.realm_proposals;
BEGIN
  IF NOT EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s) THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF NOT verse.builder_owns(k, s) THEN RETURN '{"status":403,"error":"only its owner"}'; END IF;
  SELECT * INTO p FROM verse.realm_proposals WHERE id = pid AND slug = s FOR UPDATE;
  IF p.id IS NULL THEN RETURN '{"status":404,"error":"no such proposal"}'; END IF;
  IF p.state <> 'waiting' THEN RETURN jsonb_build_object('status', 409, 'error', 'already ' || p.state); END IF;
  UPDATE verse.realm_proposals SET state = CASE WHEN accept THEN 'accepted' ELSE 'declined' END, decided_ms = verse.now_ms() WHERE id = pid;
  IF accept THEN UPDATE verse.made_realms SET placed = placed || jsonb_build_array(p.change - 'kind') WHERE slug = s; END IF;
  RETURN jsonb_build_object('status', 200, 'state', CASE WHEN accept THEN 'accepted' ELSE 'declined' END);
END $$;

-- ---- the page at /realms/<slug>, written into the site's bucket (no deploy): it opens the made-realm
-- template's module (/realms/built, every build names its files) for this realm.
CREATE FUNCTION verse.builder_html(s text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT replace(replace(replace(replace(coalesce(s, ''), '&', '&amp;'), '<', '&lt;'), '>', '&gt;'), '"', '&quot;') $$;
CREATE FUNCTION verse.builder_page(s text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>' || verse.builder_html(m.name) || ' · a realm on the Verse · OpenAgents</title>
<meta name="description" content="' || verse.builder_html(m.name) || ', a realm someone built on the Verse from one prompt.">
<meta property="og:type" content="website">
<meta property="og:site_name" content="OpenAgents">
<meta property="og:title" content="' || verse.builder_html(m.name) || '">
<meta property="og:url" content="' || verse.builder_html(origin) || '/realms/' || m.slug || '">
<meta property="og:image" content="' || verse.builder_html(origin) || '/verse/og-verse.jpg">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/realms/' || m.slug || '/index.md">
<script>if ("gpu" in navigator) document.documentElement.classList.add("gpu")</script>
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;font-size:14px;line-height:1.5}
canvas{position:fixed;inset:0;width:100%;height:100%;touch-action:none;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none}
canvas[data-cursor=pointer]{cursor:pointer}
main{position:relative;max-width:640px;margin:0 auto;padding:24px 16px}
html:not(.gpu) body{overflow:auto}
a{color:#ff8c1f}
.gpu main{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;padding:0}
</style>
</head>
<body>
<canvas id="coaxial" aria-hidden="true"></canvas>
<main>
<h1>' || verse.builder_html(m.name) || '</h1>
<p><a href="/realms/' || m.slug || '/index.md">' || verse.builder_html(m.name) || '</a></p>
</main>
<script type="module" data-made="' || m.slug || '" data-style-of="' || m.style || '">
const me = document.querySelector("script[data-made]"), slug = me.dataset.made;
const t = await (await fetch("/realms/built", { cache: "no-store" })).text();
const from = new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]");
const s = document.createElement("script");
for (const a of from.attributes) s.setAttribute(a.name, a.value);
s.dataset.realm = "made"; s.dataset.slot = "made-" + slug; s.dataset.style = me.dataset.styleOf;
s.dataset.text = slug;
document.body.append(s);
</script>
</body>
</html>
' FROM verse.made_realms m WHERE m.slug = s $$;

CREATE FUNCTION verse.builder_md(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '# ' || m.name || E'\n\nA realm on the Verse at https://openagents.com/realms/' || m.slug
      || ', made by a visitor from one prompt (https://openagents.com/realms/new). Style: ' || m.style || '. Ground: ' || m.ground
      || '. Time of day: ' || m.time || E'.\n\n## What is here\n\n'
      || coalesce((SELECT string_agg('- ' || (f->>'label'), E'\n' ORDER BY o) FROM jsonb_array_elements(m.features) WITH ORDINALITY AS x(f, o)), '- Nothing yet: open ground')
      || E'\n\n## What visitors may do\n\n- Walk'
      || coalesce(E'\n' || (SELECT string_agg('- ' || initcap(t), E'\n' ORDER BY o) FROM unnest(verse.builder_tools()) WITH ORDINALITY AS x(t, o)
                            WHERE coalesce((m.tools->>t)::boolean, false)), '')
      || E'\n\nThe owner decides what visitors get, and accepts or declines what others build here.\n'
  FROM verse.made_realms m WHERE m.slug = s $$;

-- Every made realm's address, a line each (the site's bucket keeps realms/made-slugs.txt: a
-- staging deploy and a release leave these pages alone, coax/web/upload.py and release.sh).
CREATE FUNCTION verse.builder_slugs() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(slug, E'\n' ORDER BY made_ms, slug) || E'\n', '') FROM verse.made_realms $$;
CREATE FUNCTION verse.builder_paged(s text, note text) RETURNS boolean LANGUAGE sql AS $$
  UPDATE verse.made_realms SET page_ms = CASE WHEN note IS NULL THEN verse.now_ms() END, page_note = note WHERE slug = s RETURNING true $$;
