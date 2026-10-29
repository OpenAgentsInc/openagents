-- Every realm edits the way Build your own realm does (byo-every-realm; docs/verse/realm-edit.md;
-- realms.c answers). A realm is a made one (verse.made_realms) or a hand-built one from
-- apps/openagents-web/realms.json (verse.realm_rights, seeded below; test_realm_rights.py checks the
-- two agree). Every realm has an owner account and admins, and the server decides here, for any
-- slug, who may change it:
--   owner   a made realm's maker (verse.owns), or the hand-built realm's owner account
--   admin   a site admin (admins.conf, by site login or NIP-98: realms.c passes it in) or an
--           account in the realm's admins
--   visitor anyone else
-- Owners and admins change the board (the realm's features on or off), the settings (the tools
-- visitors get), place things at once and settle what visitors propose. A visitor's thing, while
-- build is on, waits as a card.
--
--   GET  /api/playtest/realms/<slug>                  the spec, a line each (edit_spec)
--   POST /api/playtest/realms/<slug>/switch {row, on} one row of the board on or off
--   POST /api/playtest/realms/<slug>/revise {prompt}  talking revises the board: the model answers
--        under a schema whose enums are this realm's rows (or, for a made realm, the builder's
--        catalogue), and what it answers is the realm now
--   POST /api/playtest/realms/<slug>/settings {tools}, .../place, .../proposals, .../proposals/<id>/accept|decline
--
-- No words are matched here: the model reads them and only its typed answer is checked.

-- ---- the hand-built realms: their owner, admins, the visitor tools their code honours, what may be
-- placed there, and the board's rows (what their code can switch: id, label, says)
CREATE TABLE verse.realm_rights (
  slug text PRIMARY KEY,
  owner_account text NOT NULL,
  admins text[] NOT NULL DEFAULT '{}',   -- accounts; site admins are admins of every realm
  tools text[] NOT NULL DEFAULT '{}',    -- of verse.builder_tools()
  things text[] NOT NULL DEFAULT '{}',   -- what may be placed (a visitor's waits as a card)
  rows jsonb NOT NULL DEFAULT '[]'       -- [{id, label, says}], each on until switched off
);
-- What the owner and admins changed in a hand-built realm.
CREATE TABLE verse.realm_edit (
  slug text PRIMARY KEY REFERENCES verse.realm_rights (slug),
  tools jsonb NOT NULL DEFAULT '{}',     -- {"jump": bool, ...}
  switched jsonb NOT NULL DEFAULT '{}',  -- {"robots": false, ...}: a row not named is on
  placed jsonb NOT NULL DEFAULT '[]',    -- [{thing, x, z, yaw}]
  ms bigint,
  by_player text
);
-- A made realm keeps the rows switched off, so they can come back on.
ALTER TABLE verse.made_realms ADD COLUMN off jsonb NOT NULL DEFAULT '[]';
-- Cards for any realm, not only made ones.
ALTER TABLE verse.realm_proposals DROP CONSTRAINT IF EXISTS realm_proposals_slug_fkey;
-- A revision is a plan of the realm it revises (the same daily cap and pace as the builder's).
ALTER TABLE verse.realm_plans ADD COLUMN slug text;

CREATE FUNCTION verse.edit_hand(s text) RETURNS verse.realm_rights LANGUAGE sql STABLE AS $$
  SELECT * FROM verse.realm_rights WHERE slug = s $$;
CREATE FUNCTION verse.edit_made(s text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s) $$;

-- owner, admin or visitor; NULL for no such realm.
CREATE FUNCTION verse.edit_role(s text, k text, site_admin boolean) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  rec verse.players := verse.builder_player(k);
  h verse.realm_rights := verse.edit_hand(s);
BEGIN
  IF verse.edit_made(s) THEN
    IF verse.builder_owns(k, s) THEN RETURN 'owner'; END IF;
    RETURN CASE WHEN site_admin THEN 'admin' ELSE 'visitor' END;
  END IF;
  IF h.slug IS NULL THEN RETURN NULL; END IF;
  IF rec.account IS NOT NULL AND rec.account = h.owner_account THEN RETURN 'owner'; END IF;
  IF site_admin OR (rec.account IS NOT NULL AND rec.account = ANY (h.admins)) THEN RETURN 'admin'; END IF;
  RETURN 'visitor';
END $$;
CREATE FUNCTION verse.edit_may(s text, k text, site_admin boolean) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT coalesce(verse.edit_role(s, k, site_admin) IN ('owner', 'admin'), false) $$;

-- A made realm's row key: its feature, and its variant or character's name (props:wagon, character:old-mott).
CREATE FUNCTION verse.edit_key(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT (f->>'feature') || coalesce(':' || CASE WHEN f->>'feature' = 'character' THEN regexp_replace(lower(f->>'name'), '[^a-z0-9]+', '-', 'g')
                                                WHEN f->>'feature' = 'props' THEN f->>'variant' END, '') $$;
-- A made realm's board: what's in it (on) and what was switched off, in the catalogue's order.
CREATE FUNCTION verse.edit_made_board(m verse.made_realms) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(jsonb_agg(e ORDER BY array_position(verse.builder_order(), e->>'feature'), o), '[]')
  FROM (SELECT f || '{"on": true}' AS e, o FROM jsonb_array_elements(m.features) WITH ORDINALITY AS a(f, o)
        UNION ALL SELECT f || '{"on": false}', 1000 + o FROM jsonb_array_elements(m.off) WITH ORDINALITY AS b(f, o)) x $$;
-- A hand-built realm's rows with what is on.
CREATE FUNCTION verse.edit_hand_board(s text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(r || jsonb_build_object('on', coalesce((e.switched->>(r->>'id'))::boolean, true)) ORDER BY o), '[]')
  FROM verse.realm_rights h LEFT JOIN verse.realm_edit e ON e.slug = h.slug, jsonb_array_elements(h.rows) WITH ORDINALITY AS t(r, o)
  WHERE h.slug = s $$;

-- The board's rows as lines: "row KEY 0|1 LABEL".
CREATE FUNCTION verse.edit_rows(s text) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  m verse.made_realms;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NOT NULL THEN
    RETURN coalesce((SELECT string_agg('row ' || verse.edit_key(f) || ' ' || CASE WHEN (f->>'on')::boolean THEN '1' ELSE '0' END || ' ' || (f->>'label'), E'\n' ORDER BY o)
                     FROM jsonb_array_elements(verse.edit_made_board(m)) WITH ORDINALITY AS t(f, o)) || E'\n', '');
  END IF;
  RETURN coalesce((SELECT string_agg('row ' || (r->>'id') || ' ' || CASE WHEN (r->>'on')::boolean THEN '1' ELSE '0' END || ' ' || (r->>'label'), E'\n' ORDER BY o)
                   FROM jsonb_array_elements(verse.edit_hand_board(s)) WITH ORDINALITY AS t(r, o)) || E'\n', '');
END $$;

-- The spec, a line each. A made realm's is builder_spec's (myrealm.coax reads it), with "owner 1" for
-- its owner and admins; any realm's ends with its role, its rows, and for an owner or admin what waits.
--   realm SLUG / role owner|admin|visitor / owner 0|1 / tool NAME 0|1 (the tools its code honours)
--   row KEY 0|1 LABEL / placed THING X Z YAW / waiting N
CREATE FUNCTION verse.edit_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  may boolean := role IN ('owner', 'admin');
  h verse.realm_rights;
  e verse.realm_edit;
  out text;
BEGIN
  IF role IS NULL THEN RETURN NULL; END IF;
  IF verse.edit_made(s) THEN
    out := verse.builder_spec(s, k);
    IF may AND role = 'admin' THEN
      out := replace(out, E'\nowner 0\n', E'\nowner 1\n')
          || 'waiting ' || (SELECT count(*) FROM verse.realm_proposals WHERE slug = s AND state = 'waiting') || E'\n';
    END IF;
    RETURN out || 'role ' || role || E'\n' || verse.edit_rows(s);
  END IF;
  h := verse.edit_hand(s);
  SELECT * INTO e FROM verse.realm_edit WHERE slug = s;
  out := 'realm ' || s || E'\nrole ' || role || E'\nowner ' || CASE WHEN may THEN '1' ELSE '0' END || E'\n';
  out := out || coalesce((SELECT string_agg('tool ' || t || ' ' || CASE WHEN coalesce((e.tools->>t)::boolean, true) THEN '1' ELSE '0' END, E'\n' ORDER BY o)
                          FROM unnest(h.tools) WITH ORDINALITY AS x(t, o)) || E'\n', '');
  out := out || coalesce((SELECT string_agg('thing ' || t, E'\n' ORDER BY o) FROM unnest(h.things) WITH ORDINALITY AS x(t, o)) || E'\n', '');
  out := out || verse.edit_rows(s);
  out := out || coalesce((SELECT string_agg('placed ' || (p->>'thing') || ' ' || (p->>'x') || ' ' || (p->>'z') || ' ' || (p->>'yaw'), E'\n' ORDER BY o)
                          FROM jsonb_array_elements(coalesce(e.placed, '[]')) WITH ORDINALITY AS x(p, o)) || E'\n', '');
  IF may THEN
    out := out || 'waiting ' || (SELECT count(*) FROM verse.realm_proposals WHERE slug = s AND state = 'waiting') || E'\n';
  END IF;
  RETURN out;
END $$;

-- One row on or off. -> {status, row, on} | {status, error}
CREATE FUNCTION verse.edit_switch(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  key text := CASE WHEN jsonb_typeof(body->'row') = 'string' THEN body->>'row' END;
  turn boolean := CASE WHEN jsonb_typeof(body->'on') = 'boolean' THEN (body->>'on')::boolean END;
  m verse.made_realms;
  f jsonb;
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF key IS NULL OR turn IS NULL THEN RETURN '{"status":400,"error":"a row and on"}'; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF m.slug IS NOT NULL THEN
    SELECT e INTO f FROM jsonb_array_elements(verse.edit_made_board(m)) e WHERE verse.edit_key(e) = key LIMIT 1;
    IF f IS NULL THEN RETURN '{"status":404,"error":"no such row"}'; END IF;
    UPDATE verse.made_realms SET
      features = coalesce((SELECT jsonb_agg(x) FROM jsonb_array_elements(features) x WHERE verse.edit_key(x) <> key), '[]')
                 || CASE WHEN turn THEN jsonb_build_array(f - 'on') ELSE '[]' END,
      off = coalesce((SELECT jsonb_agg(x) FROM jsonb_array_elements(off) x WHERE verse.edit_key(x) <> key), '[]')
            || CASE WHEN turn THEN '[]' ELSE jsonb_build_array(f - 'on') END,
      settings_ms = verse.now_ms()
    WHERE slug = s;
    RETURN jsonb_build_object('status', 200, 'row', key, 'on', turn);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM verse.realm_rights h, jsonb_array_elements(h.rows) r WHERE h.slug = s AND r->>'id' = key) THEN
    RETURN '{"status":404,"error":"no such row"}';
  END IF;
  INSERT INTO verse.realm_edit (slug, switched, ms, by_player) VALUES (s, jsonb_build_object(key, turn), verse.now_ms(), (verse.builder_player(k)).id)
  ON CONFLICT (slug) DO UPDATE SET switched = verse.realm_edit.switched || jsonb_build_object(key, turn), ms = excluded.ms, by_player = excluded.by_player;
  RETURN jsonb_build_object('status', 200, 'row', key, 'on', turn);
END $$;

-- The tools visitors get. -> {status, tools} | {status, error}
CREATE FUNCTION verse.edit_settings(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  t jsonb := CASE WHEN jsonb_typeof(body->'tools') = 'object' THEN body->'tools' ELSE '{}' END;
  m verse.made_realms;
  h verse.realm_rights := verse.edit_hand(s);
  e verse.realm_edit;
  next jsonb;
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NOT NULL THEN
    next := (SELECT jsonb_object_agg(x, CASE WHEN jsonb_typeof(t->x) = 'boolean' THEN (t->>x)::boolean ELSE coalesce((m.tools->>x)::boolean, false) END)
             FROM unnest(verse.builder_tools()) x);
    UPDATE verse.made_realms SET tools = next, settings_ms = verse.now_ms() WHERE slug = s;
    RETURN jsonb_build_object('status', 200, 'tools', next);
  END IF;
  SELECT * INTO e FROM verse.realm_edit WHERE slug = s;
  next := coalesce((SELECT jsonb_object_agg(x, CASE WHEN jsonb_typeof(t->x) = 'boolean' THEN (t->>x)::boolean ELSE coalesce((e.tools->>x)::boolean, true) END)
                    FROM unnest(h.tools) x), '{}');
  INSERT INTO verse.realm_edit (slug, tools, ms, by_player) VALUES (s, next, verse.now_ms(), (verse.builder_player(k)).id)
  ON CONFLICT (slug) DO UPDATE SET tools = excluded.tools, ms = excluded.ms, by_player = excluded.by_player;
  RETURN jsonb_build_object('status', 200, 'tools', next);
END $$;

-- A thing placed: an owner's or admin's goes in; a visitor's waits as a card (build must be on).
-- -> {status, placed: true} | {status, proposed: id} | {status, error}
CREATE FUNCTION verse.edit_place(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  rec verse.players := verse.builder_player(k);
  h verse.realm_rights := verse.edit_hand(s);
  e verse.realm_edit;
  x numeric := CASE WHEN jsonb_typeof(body->'x') = 'number' THEN (body->>'x')::numeric END;
  z numeric := CASE WHEN jsonb_typeof(body->'z') = 'number' THEN (body->>'z')::numeric END;
  yaw numeric := CASE WHEN jsonb_typeof(body->'yaw') = 'number' THEN (body->>'yaw')::numeric ELSE 0 END;
  thing text;
  change jsonb;
  pid bigint;
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF verse.edit_made(s) THEN
    IF role = 'admin' THEN   -- an admin places as its owner would
      thing := CASE WHEN jsonb_typeof(body->'thing') = 'string' AND (body->>'thing') = ANY (verse.builder_things()) THEN body->>'thing' END;
      IF thing IS NULL OR x IS NULL OR z IS NULL OR abs(x) > 40 OR abs(z) > 40 THEN RETURN '{"status":400,"error":"not a thing here"}'; END IF;
      UPDATE verse.made_realms SET placed = placed || jsonb_build_array(jsonb_build_object('thing', thing, 'x', trim_scale(round(x, 2)), 'z', trim_scale(round(z, 2)), 'yaw', trim_scale(round(yaw, 3))))
      WHERE slug = s AND jsonb_array_length(placed) < 200;
      RETURN '{"status":200,"placed":true}';
    END IF;
    RETURN verse.builder_place(s, body, k);
  END IF;
  IF rec.id IS NULL THEN RETURN '{"status":401,"error":"no player"}'; END IF;
  thing := CASE WHEN jsonb_typeof(body->'thing') = 'string' AND (body->>'thing') = ANY (h.things) THEN body->>'thing' END;
  IF thing IS NULL OR x IS NULL OR z IS NULL OR abs(x) > 400 OR abs(z) > 400 THEN RETURN '{"status":400,"error":"not a thing here"}'; END IF;
  change := jsonb_build_object('kind', 'place', 'thing', thing, 'x', trim_scale(round(x, 2)), 'z', trim_scale(round(z, 2)), 'yaw', trim_scale(round(yaw, 3)));
  SELECT * INTO e FROM verse.realm_edit WHERE slug = s;
  IF role IN ('owner', 'admin') THEN
    IF jsonb_array_length(coalesce(e.placed, '[]')) >= 200 THEN RETURN '{"status":409,"error":"the realm is full"}'; END IF;
    INSERT INTO verse.realm_edit (slug, placed, ms, by_player) VALUES (s, jsonb_build_array(change - 'kind'), verse.now_ms(), rec.id)
    ON CONFLICT (slug) DO UPDATE SET placed = verse.realm_edit.placed || jsonb_build_array(change - 'kind'), ms = excluded.ms, by_player = excluded.by_player;
    RETURN '{"status":200,"placed":true}';
  END IF;
  IF NOT ('build' = ANY (h.tools)) OR NOT coalesce((e.tools->>'build')::boolean, true) THEN
    RETURN '{"status":403,"error":"the owner hasn''t let visitors build"}';
  END IF;
  IF (SELECT count(*) FROM verse.realm_proposals WHERE slug = s AND state = 'waiting') >= 50 THEN RETURN '{"status":429,"error":"the owner has enough to look at"}'; END IF;
  INSERT INTO verse.realm_proposals (slug, ms, by_player, change) VALUES (s, verse.now_ms(), rec.id, change) RETURNING id INTO pid;
  RETURN jsonb_build_object('status', 200, 'proposed', pid);
END $$;

-- The cards, a line each: "P ID|WHO|THING|X|Z|YAW|BEFORE|AFTER", newest first; "" for a visitor.
CREATE FUNCTION verse.edit_proposals(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg('P ' || p.id || '|' || 'Visitor ' || right(coalesce(p.by_player, '----'), 4) || '|' || (p.change->>'thing') || '|'
         || (p.change->>'x') || '|' || (p.change->>'z') || '|' || (p.change->>'yaw') || '|' || n.before || '|' || (n.before + 1), E'\n' ORDER BY p.id DESC) || E'\n', '')
  FROM verse.realm_proposals p,
       LATERAL (SELECT count(*) AS before FROM jsonb_array_elements(coalesce((SELECT placed FROM verse.made_realms WHERE slug = s),
                                                                              (SELECT placed FROM verse.realm_edit WHERE slug = s), '[]')) e
                WHERE e->>'thing' = p.change->>'thing') n
  WHERE p.slug = s AND p.state = 'waiting' AND verse.edit_may(s, k, site_admin) $$;

-- ACCEPT or DECLINE, an owner or admin. -> {status, state}
CREATE FUNCTION verse.edit_decide(s text, pid bigint, accept boolean, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  p verse.realm_proposals;
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  SELECT * INTO p FROM verse.realm_proposals WHERE id = pid AND slug = s FOR UPDATE;
  IF p.id IS NULL THEN RETURN '{"status":404,"error":"no such proposal"}'; END IF;
  IF p.state <> 'waiting' THEN RETURN jsonb_build_object('status', 409, 'error', 'already ' || p.state); END IF;
  UPDATE verse.realm_proposals SET state = CASE WHEN accept THEN 'accepted' ELSE 'declined' END, decided_ms = verse.now_ms() WHERE id = pid;
  IF accept THEN
    IF verse.edit_made(s) THEN
      UPDATE verse.made_realms SET placed = placed || jsonb_build_array(p.change - 'kind') WHERE slug = s;
    ELSE
      INSERT INTO verse.realm_edit (slug, placed, ms) VALUES (s, jsonb_build_array(p.change - 'kind'), verse.now_ms())
      ON CONFLICT (slug) DO UPDATE SET placed = verse.realm_edit.placed || jsonb_build_array(p.change - 'kind'), ms = excluded.ms;
    END IF;
  END IF;
  RETURN jsonb_build_object('status', 200, 'state', CASE WHEN accept THEN 'accepted' ELSE 'declined' END);
END $$;

-- ---- talking revises the board

-- A hand-built realm's prompt and schema: its rows, and on or off for each.
CREATE FUNCTION verse.edit_hand_prompt(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT 'You edit a realm of the Verse, a 3D world in the browser, for its owner. The realm is built by hand; '
      || 'what can change is which of its features are on. They said, out loud or typed, how they want it. '
      || 'Answer every feature with on true or false: change only what they asked about and keep the rest as it is now. '
      || E'If what they asked for is not one of these features, keep everything as it is.\nThe features:\n'
      || string_agg('- ' || (r->>'id') || ': ' || (r->>'says'), E'\n' ORDER BY o)
  FROM verse.realm_rights h, jsonb_array_elements(h.rows) WITH ORDINALITY AS t(r, o) WHERE h.slug = s $$;
CREATE FUNCTION verse.edit_hand_schema(s text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('type', 'object', 'required', jsonb_build_array('features'), 'properties', jsonb_build_object(
    'features', jsonb_build_object('type', 'array', 'items', jsonb_build_object('type', 'object', 'required', jsonb_build_array('id', 'on'),
      'properties', jsonb_build_object(
        'id', jsonb_build_object('type', 'string', 'enum', (SELECT jsonb_agg(r->'id') FROM verse.realm_rights h, jsonb_array_elements(h.rows) r WHERE h.slug = s)),
        'on', jsonb_build_object('type', 'boolean')))))) $$;

-- {prompt} from an owner or admin: {"status", "error"} when it can't be asked, else the plan row and the request.
CREATE FUNCTION verse.edit_revise_ask(s text, body jsonb, cap int, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  said text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'prompt', 600) ELSE '' END;
  rec verse.players := verse.builder_player(k);
  m verse.made_realms;
  board jsonb;
  row_id bigint;
  sys text;
  schema jsonb;
  text_in text;
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF said = '' THEN RETURN '{"status":400,"error":"say how it should change"}'; END IF;
  IF EXISTS (SELECT 1 FROM verse.realm_plans WHERE slug = s AND ms > verse.now_ms() - 2000)
     OR (rec.id IS NOT NULL AND EXISTS (SELECT 1 FROM verse.realm_plans WHERE player = rec.id AND ms > verse.now_ms() - 2000)) THEN
    RETURN '{"status":429,"error":"one at a time"}';
  END IF;
  IF (SELECT count(*) FROM verse.realm_plans WHERE ms > (extract(epoch FROM date_trunc('day', now())) * 1000)::bigint) >= cap THEN
    RETURN '{"status":503,"error":"the day''s plans are used up"}';
  END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NOT NULL THEN
    board := jsonb_build_object('name', m.name, 'style', m.style, 'ground', m.ground, 'time', m.time,
                                'features', (SELECT coalesce(jsonb_agg(f - 'label'), '[]') FROM jsonb_array_elements(verse.edit_made_board(m)) f));
    sys := verse.builder_prompt();
    schema := verse.builder_schema();
  ELSE
    board := jsonb_build_object('features', (SELECT coalesce(jsonb_agg(jsonb_build_object('id', r->>'id', 'on', (r->>'on')::boolean)), '[]')
                                             FROM jsonb_array_elements(verse.edit_hand_board(s)) r));
    sys := verse.edit_hand_prompt(s);
    schema := verse.edit_hand_schema(s);
  END IF;
  INSERT INTO verse.realm_plans (ms, player, prompt, board_in, slug) VALUES (verse.now_ms(), rec.id, said, board, s) RETURNING id INTO row_id;
  text_in := 'The current board: ' || board::text || E'\nThey now say: ' || said;
  RETURN jsonb_build_object('status', 200, 'id', row_id, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', sys))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text', text_in)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'responseSchema', schema,
      'temperature', 0.3, 'maxOutputTokens', 4000, 'thinkingConfig', jsonb_build_object('thinkingLevel', 'LOW'))));
END $$;

-- The model's answer is the realm now: a made realm's features (on) and off, its ground and time (its
-- name, style, tools and placed things stay); a hand-built realm's rows. -> 200 {plan, changed, lines}
CREATE FUNCTION verse.edit_revise_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pid bigint := (asked->>'id')::bigint;
  s text := (SELECT slug FROM verse.realm_plans WHERE id = pid);
  r jsonb;
  t text;
  d jsonb;
  held jsonb;
  err text;
  status int := 200;
  changed int := 0;
  turn jsonb := '{}';
BEGIN
  BEGIN r := response::jsonb; EXCEPTION WHEN others THEN r := NULL; END;
  IF http = 200 AND r IS NOT NULL THEN
    SELECT string_agg(p->>'text', '') INTO t FROM jsonb_array_elements(coalesce(r->'candidates'->0->'content'->'parts', '[]')) p
    WHERE coalesce((p->>'thought')::boolean, false) = false AND p ? 'text';
    BEGIN d := t::jsonb; EXCEPTION WHEN others THEN d := NULL; END;
    IF d IS NULL THEN status := 503; err := 'no answer'; END IF;
  ELSE
    status := 503;
    err := CASE WHEN http = 429 THEN 'busy' WHEN http = 0 THEN 'model unreachable' ELSE 'model error ' || http END;
  END IF;
  IF status = 200 AND verse.edit_made(s) THEN
    held := verse.builder_hold(jsonb_build_object('name', 'x') || d);
    IF held IS NULL THEN status := 502; err := 'not a board';
    ELSE
      UPDATE verse.made_realms m SET
        ground = held->>'ground', time = held->>'time',
        features = coalesce((SELECT jsonb_agg(f - 'on') FROM jsonb_array_elements(held->'features') f WHERE (f->>'on')::boolean AND f->>'feature' <> 'tool'), '[]'),
        off = coalesce((SELECT jsonb_agg(f - 'on') FROM jsonb_array_elements(held->'features') f WHERE NOT (f->>'on')::boolean AND f->>'feature' <> 'tool'), '[]'),
        settings_ms = verse.now_ms()
      WHERE m.slug = s;
      changed := jsonb_array_length(held->'features');
    END IF;
  ELSIF status = 200 THEN
    SELECT coalesce(jsonb_object_agg(f->>'id', (f->>'on')::boolean), '{}') INTO turn
    FROM jsonb_array_elements(CASE WHEN jsonb_typeof(d->'features') = 'array' THEN d->'features' ELSE '[]' END) f, verse.realm_rights h, jsonb_array_elements(h.rows) rw
    WHERE h.slug = s AND rw->>'id' = f->>'id' AND jsonb_typeof(f->'on') = 'boolean';
    SELECT count(*) INTO changed FROM jsonb_each(turn) x JOIN jsonb_array_elements(verse.edit_hand_board(s)) b ON b->>'id' = x.key
    WHERE (b->>'on')::boolean <> (x.value #>> '{}')::boolean;
    INSERT INTO verse.realm_edit (slug, switched, ms) VALUES (s, turn, verse.now_ms())
    ON CONFLICT (slug) DO UPDATE SET switched = verse.realm_edit.switched || turn, ms = excluded.ms;
  END IF;
  UPDATE verse.realm_plans SET board = coalesce(held, CASE WHEN status = 200 THEN jsonb_build_object('switched', turn) END), error = err,
    model = edit_revise_answer.model, ms_taken = taken WHERE id = pid;
  IF status <> 200 THEN RETURN jsonb_build_object('status', status, 'error', err); END IF;
  RETURN jsonb_build_object('status', 200, 'plan', pid, 'changed', changed, 'lines', 'plan ' || pid || E'\n' || verse.edit_rows(s));
END $$;

-- ---- the hand-built realms (apps/openagents-web/realms.json "edit"; test_realm_rights.py checks they agree)
INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows) VALUES ('haunted-house', 'n-9f4b4c7e5944ddb3404651d3ba1a9985dc4261ccbca4e81d769a54e43614e27a', ARRAY['n-1e60e1b4820b482c593da4d7745f479fd1a5018ba374f50978b99bad9963c423']::text[], ARRAY['map','voice']::text[], ARRAY[]::text[],
  '[{"id": "others", "label": "Other visitors", "says": "other visitors walking the house"}, {"id": "sounds", "label": "The house''s sounds", "says": "the creaks, thunder and sounds of the house"}, {"id": "scares", "label": "Lightning and sparks", "says": "the lightning flashes and sparks the haunts throw"}, {"id": "chat", "label": "Room chat", "says": "the chat with others in the house"}]'::jsonb);
INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows) VALUES ('the-grid', 'n-9f4b4c7e5944ddb3404651d3ba1a9985dc4261ccbca4e81d769a54e43614e27a', ARRAY['n-1e60e1b4820b482c593da4d7745f479fd1a5018ba374f50978b99bad9963c423']::text[], ARRAY[]::text[], ARRAY[]::text[],
  '[{"id": "robots", "label": "Robots walking their rounds", "says": "the small robots patrolling the grid"}, {"id": "names", "label": "Place names in the air", "says": "the names floating over the stations"}]'::jsonb);
INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows) VALUES ('everglade', 'n-9f4b4c7e5944ddb3404651d3ba1a9985dc4261ccbca4e81d769a54e43614e27a', ARRAY['n-1e60e1b4820b482c593da4d7745f479fd1a5018ba374f50978b99bad9963c423']::text[], ARRAY['map','build']::text[], ARRAY['crate','barrel','bench','fence','stall']::text[],
  '[{"id": "flock", "label": "The flock that follows you", "says": "the birds earned on the river quest that follow whoever walks"}, {"id": "floats", "label": "Ducks and lily pads", "says": "ducks and lily pads floating on the river"}, {"id": "undergrowth", "label": "Bushes, ferns and rocks", "says": "bushes, ferns and rocks along the tree line"}, {"id": "daycycle", "label": "Day turning to night", "says": "the sky turning from day to night; off, it holds at noon"}]'::jsonb);
INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows) VALUES ('fairy-forest', 'n-9f4b4c7e5944ddb3404651d3ba1a9985dc4261ccbca4e81d769a54e43614e27a', ARRAY['n-1e60e1b4820b482c593da4d7745f479fd1a5018ba374f50978b99bad9963c423']::text[], ARRAY['fly']::text[], ARRAY[]::text[],
  '[{"id": "sparkles", "label": "Fairy dust sparkles", "says": "the fairy dust sparkling behind you"}, {"id": "grass", "label": "Wind-swept grass", "says": "the grass swaying in the wind"}, {"id": "ridges", "label": "The far ridges", "says": "the ridges far round the valley"}]'::jsonb);
INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows) VALUES ('dead-gears', 'n-9f4b4c7e5944ddb3404651d3ba1a9985dc4261ccbca4e81d769a54e43614e27a', ARRAY['n-1e60e1b4820b482c593da4d7745f479fd1a5018ba374f50978b99bad9963c423']::text[], ARRAY[]::text[], ARRAY[]::text[],
  '[{"id": "support", "label": "The support engineer", "says": "the support engineer who fights beside you and repairs the reactor"}, {"id": "alarms", "label": "Flashing wave alarms", "says": "the red alarm strips that flash when a wave comes"}, {"id": "radar", "label": "The radar", "says": "the radar in the corner of the screen"}, {"id": "sparks", "label": "Hit sparks", "says": "sparks and effects when something is hit"}]'::jsonb);
INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows) VALUES ('ice-cream-man', 'n-9f4b4c7e5944ddb3404651d3ba1a9985dc4261ccbca4e81d769a54e43614e27a', ARRAY['n-1e60e1b4820b482c593da4d7745f479fd1a5018ba374f50978b99bad9963c423']::text[], ARRAY[]::text[], ARRAY[]::text[],
  '[{"id": "minions", "label": "The Ice Cream Man''s minions", "says": "the minions who hunt you with him"}, {"id": "weapons", "label": "Weapons to earn and the spear puzzle", "says": "the weapons you can earn and the cabinet''s spear puzzle"}, {"id": "backrooms", "label": "The backrooms", "says": "the backrooms behind the house"}]'::jsonb);
INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows) VALUES ('hecates-cemetery', 'n-9f4b4c7e5944ddb3404651d3ba1a9985dc4261ccbca4e81d769a54e43614e27a', ARRAY['n-1e60e1b4820b482c593da4d7745f479fd1a5018ba374f50978b99bad9963c423']::text[], ARRAY[]::text[], ARRAY[]::text[],
  '[{"id": "flight", "label": "The flight in from space", "says": "the flight in from space at the start of a visit; off, a visit starts at the gate"}, {"id": "fireflies", "label": "Fireflies over the field", "says": "fireflies over the open field"}, {"id": "trail", "label": "A light trail behind you", "says": "the light trail and shimmer behind whoever walks"}, {"id": "tips", "label": "First-visit tips", "says": "the guiding tips and beacon for a first visit"}, {"id": "others", "label": "Other visitors and their dogs", "says": "other visitors walking here with their dogs"}]'::jsonb);
