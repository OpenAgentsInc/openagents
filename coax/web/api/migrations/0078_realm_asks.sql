-- What an owner asked for, kept per realm (realm-asks; docs/verse/realm-edit.md "What you asked
-- for"; realms.c answers). Every realm's edit mode lists its features: the board's rows (what its
-- code switches) and every ask its owner and admins made, each APPLIED or NOT APPLIED, with DELETE,
-- and ADD for a new one.
--
-- An ask goes the way talking to the board always went (0050's revise, 0075's for a fork): the
-- model answers under a schema whose enums are this realm's rows (a made realm: the builder's
-- catalogue), with the same daily cap and pace. What that answer changed is kept with the ask
-- (did: each changed row as it was before and after, and a made realm's ground and time), so:
--   applied   the answer changed the realm; DELETE puts back each row the ask changed that is still
--             as the ask left it (a later change owns the rest), and the ask is gone
--   waiting   the realm's code can't do it yet (or the model wasn't reached): NOT APPLIED, kept for
--             whoever builds it (verse/SKILL.md "Asks waiting on code"); DELETE takes it off
--   removing  built in code (done with its commit), then deleted: its code comes out the way it went
--             in, and the ask leaves when that commit lands (done again)
-- DELETE on a board's row: a made realm's feature is data, and leaves its features and off at once
-- (an ask whose only change it was goes with it); a hand-built realm's row is code, so it is
-- switched off now and a "remove" ask waits for its code to come out (DELETE on that ask puts the
-- row back).
--
--   GET  /api/playtest/realms/<slug>/asks                -> text: "ask ID applied|waiting|removing ask|remove WORDS", newest first ("" for a visitor)
--   POST /api/playtest/realms/<slug>/asks {words}        -> 200 lines: "ask ID STATE", "plan N", "row ..."; 400, 403, 404, 429
--   POST /api/playtest/realms/<slug>/asks/<id>/delete    -> 200 {deleted, reverted} | {state: removing}; 403, 404
--   POST /api/playtest/realms/<slug>/rows/<key>/delete   -> 200 {deleted} | {ask, state}; 403, 404
--   POST /api/playtest/realms/<slug>/asks/<id>/done {commit}   a site admin (an agent signing NIP-98): built
--        in code -> applied; a remove or a removing one -> gone. 400, 403, 404, 409
--
-- No words are matched here: the model reads them and only its typed answer is checked.
-- Additive: one table and its functions.

CREATE TABLE verse.realm_asks (
  id bigserial PRIMARY KEY,
  slug text NOT NULL,
  ms bigint NOT NULL,
  player text,                    -- who asked (verse.players)
  words text NOT NULL CHECK (length(words) BETWEEN 1 AND 400 AND words !~ '[[:cntrl:]]'),
  kind text NOT NULL DEFAULT 'ask' CHECK (kind IN ('ask', 'remove')),
  row_key text,                   -- a remove: the board's row it takes out
  state text NOT NULL CHECK (state IN ('asking', 'applied', 'waiting', 'removing')),
  plan bigint,                    -- verse.realm_plans: the model's revision
  did jsonb NOT NULL DEFAULT '{}',   -- {"before": {KEY: V}, "after": {KEY: V}, "ground": [B, A], "time": [B, A]}
  commit_hash text CHECK (commit_hash ~ '^[0-9a-f]{40}$'),   -- built in code
  done_ms bigint
);
CREATE INDEX realm_asks_slug ON verse.realm_asks (slug, id DESC);

-- What an ask can change, now: a made realm's board (KEY -> the feature with on), ground and time;
-- a hand-built realm's rows (KEY -> {on, raw: what realm_edit.switched holds, null for nothing}).
CREATE FUNCTION verse.ask_state(s text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  m verse.made_realms;
  e verse.realm_edit;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NOT NULL THEN
    RETURN jsonb_build_object('made', true, 'ground', m.ground, 'time', m.time,
      'rows', coalesce((SELECT jsonb_object_agg(verse.edit_key(f), f) FROM jsonb_array_elements(verse.edit_made_board(m)) f), '{}'));
  END IF;
  SELECT * INTO e FROM verse.realm_edit WHERE slug = s;
  RETURN jsonb_build_object('made', false,
    'rows', coalesce((SELECT jsonb_object_agg(b->>'id', jsonb_build_object('on', (b->>'on')::boolean, 'raw', e.switched->(b->>'id')))
                      FROM jsonb_array_elements(verse.edit_hand_board(s)) b), '{}'));
END $$;

-- What changed between two states: only the rows that differ (a hand-built row by whether it is on).
CREATE FUNCTION verse.ask_diff(b jsonb, a jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('before', coalesce(jsonb_object_agg(k, b->'rows'->k), '{}'), 'after', coalesce(jsonb_object_agg(k, a->'rows'->k), '{}'))
    || CASE WHEN b->>'ground' IS DISTINCT FROM a->>'ground' THEN jsonb_build_object('ground', jsonb_build_array(b->'ground', a->'ground')) ELSE '{}' END
    || CASE WHEN b->>'time' IS DISTINCT FROM a->>'time' THEN jsonb_build_object('time', jsonb_build_array(b->'time', a->'time')) ELSE '{}' END
  FROM (SELECT k FROM (SELECT jsonb_object_keys(coalesce(b->'rows', '{}')) k UNION SELECT jsonb_object_keys(coalesce(a->'rows', '{}'))) u
        WHERE CASE WHEN coalesce((a->>'made')::boolean, false) THEN b->'rows'->k IS DISTINCT FROM a->'rows'->k
                   ELSE b->'rows'->k->'on' IS DISTINCT FROM a->'rows'->k->'on' END) x $$;
CREATE FUNCTION verse.ask_changed(d jsonb) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(d->'after', '{}') <> '{}' OR d ? 'ground' OR d ? 'time' $$;

-- Put back what an ask changed, where it is still as the ask left it. -> how many rows went back.
CREATE FUNCTION verse.ask_revert(s text, did jsonb) RETURNS int LANGUAGE plpgsql AS $$
DECLARE
  cur jsonb := verse.ask_state(s);
  made boolean := coalesce((cur->>'made')::boolean, false);
  back jsonb := '{}';
  k text;
  was jsonb;
  left_as jsonb;
  n int := 0;
BEGIN
  FOR k IN SELECT jsonb_object_keys(coalesce(did->'after', '{}')) LOOP
    left_as := nullif(did->'after'->k, 'null'::jsonb);
    was := did->'before'->k;
    IF made AND (cur->'rows'->k) IS NOT DISTINCT FROM left_as THEN back := back || jsonb_build_object(k, was); END IF;
    IF NOT made AND cur->'rows' ? k AND (cur->'rows'->k->'on') IS NOT DISTINCT FROM (left_as->'on') THEN back := back || jsonb_build_object(k, was); END IF;
  END LOOP;
  n := (SELECT count(*) FROM jsonb_object_keys(back));
  IF made AND (n > 0 OR did ? 'ground' OR did ? 'time') THEN
    UPDATE verse.made_realms SET
      features = coalesce((SELECT jsonb_agg(f ORDER BY o) FROM jsonb_array_elements(features) WITH ORDINALITY AS x(f, o) WHERE NOT back ? verse.edit_key(f)), '[]')
              || coalesce((SELECT jsonb_agg(v - 'on') FROM jsonb_each(back) x(kk, v) WHERE jsonb_typeof(v) = 'object' AND (v->>'on')::boolean), '[]'),
      off = coalesce((SELECT jsonb_agg(f ORDER BY o) FROM jsonb_array_elements(off) WITH ORDINALITY AS x(f, o) WHERE NOT back ? verse.edit_key(f)), '[]')
         || coalesce((SELECT jsonb_agg(v - 'on') FROM jsonb_each(back) x(kk, v) WHERE jsonb_typeof(v) = 'object' AND NOT (v->>'on')::boolean), '[]'),
      ground = CASE WHEN did ? 'ground' AND ground IS NOT DISTINCT FROM did->'ground'->>1 THEN coalesce(did->'ground'->>0, ground) ELSE ground END,
      time = CASE WHEN did ? 'time' AND time IS NOT DISTINCT FROM did->'time'->>1 THEN coalesce(did->'time'->>0, time) ELSE time END,
      settings_ms = verse.now_ms()
    WHERE slug = s;
  ELSIF n > 0 THEN
    UPDATE verse.realm_edit SET
      switched = (switched - ARRAY(SELECT jsonb_object_keys(back)))
              || coalesce((SELECT jsonb_object_agg(kk, v->'raw') FROM jsonb_each(back) x(kk, v) WHERE jsonb_typeof(v->'raw') = 'boolean'), '{}'),
      ms = verse.now_ms()
    WHERE slug = s;
  END IF;
  RETURN n;
END $$;

-- ADD: {words} from an owner or admin. The revision is asked as 0050's is; the ask is kept even when
-- the model can't be asked today (it waits, NOT APPLIED). -> {status, error} | {status, id, ask, request} | {status, ask, lines}
CREATE FUNCTION verse.ask_add(s text, body jsonb, cap int, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  said text := btrim(regexp_replace(CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'words', 400) ELSE '' END, '[[:cntrl:]]+', ' ', 'g'));
  x jsonb;
  aid bigint;
  st int;
BEGIN
  x := verse.edit_revise_ask(s, jsonb_build_object('prompt', said), cap, k, site_admin);
  st := (x->>'status')::int;
  IF st IN (400, 401, 403, 404, 429) THEN RETURN x; END IF;
  IF said = '' THEN RETURN '{"status":400,"error":"say what you want"}'; END IF;
  INSERT INTO verse.realm_asks (slug, ms, player, words, state, plan)
  VALUES (s, verse.now_ms(), (verse.builder_player(k)).id, said, CASE WHEN st = 200 THEN 'asking' ELSE 'waiting' END, CASE WHEN st = 200 THEN (x->>'id')::bigint END)
  RETURNING id INTO aid;
  IF st <> 200 THEN
    RETURN jsonb_build_object('status', 200, 'ask', aid, 'lines', 'ask ' || aid || E' waiting\n' || verse.edit_rows(s));
  END IF;
  RETURN x || jsonb_build_object('ask', aid);
END $$;

-- The model's answer to an ask: the revision as always, and what it changed kept with the ask.
CREATE FUNCTION verse.ask_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  a verse.realm_asks;
  before jsonb;
  res jsonb;
  d jsonb;
  st text;
BEGIN
  SELECT * INTO a FROM verse.realm_asks WHERE id = (asked->>'ask')::bigint FOR UPDATE;
  IF a.id IS NULL THEN   -- deleted while the model thought: its answer changes nothing
    RETURN jsonb_build_object('status', 200, 'lines', E'ask 0 gone\n' || verse.edit_rows((SELECT slug FROM verse.realm_plans WHERE id = (asked->>'id')::bigint)));
  END IF;
  before := verse.ask_state(a.slug);
  res := verse.fork_revise_answer(asked, http, response, model, taken);
  d := verse.ask_diff(before, verse.ask_state(a.slug));
  -- a hand-built realm's answer names every row; the ones it didn't change keep what they held
  IF NOT coalesce((before->>'made')::boolean, false) THEN
    UPDATE verse.realm_edit SET
      switched = (switched - ARRAY(SELECT kk FROM jsonb_object_keys(before->'rows') kk WHERE NOT coalesce(d->'after', '{}') ? kk))
              || coalesce((SELECT jsonb_object_agg(kk, v->'raw') FROM jsonb_each(before->'rows') x(kk, v)
                           WHERE NOT coalesce(d->'after', '{}') ? kk AND jsonb_typeof(v->'raw') = 'boolean'), '{}')
    WHERE slug = a.slug;
  END IF;
  st := CASE WHEN (res->>'status')::int = 200 AND verse.ask_changed(d) THEN 'applied' ELSE 'waiting' END;
  UPDATE verse.realm_asks SET state = st, did = CASE WHEN st = 'applied' THEN d ELSE '{}' END WHERE id = a.id;
  RETURN jsonb_build_object('status', 200, 'ask', a.id, 'state', st,
    'lines', 'ask ' || a.id || ' ' || st || E'\n' || coalesce(res->>'lines', verse.edit_rows(a.slug)));
END $$;

-- The list, newest first: "ask ID STATE KIND WORDS" ("asking" reads as waiting). "" for a visitor.
CREATE FUNCTION verse.ask_list(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg('ask ' || id || ' ' || CASE WHEN state = 'asking' THEN 'waiting' ELSE state END || ' ' || kind || ' ' || words, E'\n' ORDER BY id DESC) || E'\n', '')
  FROM (SELECT * FROM verse.realm_asks WHERE slug = s AND verse.edit_may(s, k, site_admin) ORDER BY id DESC LIMIT 40) x $$;

-- DELETE an ask: what it changed goes back and it is gone; one built in code waits for its code to come out.
CREATE FUNCTION verse.ask_delete(s text, aid bigint, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  a verse.realm_asks;
  n int;
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  SELECT * INTO a FROM verse.realm_asks WHERE id = aid AND slug = s FOR UPDATE;
  IF a.id IS NULL THEN RETURN '{"status":404,"error":"no such ask"}'; END IF;
  IF a.commit_hash IS NOT NULL AND a.kind = 'ask' THEN
    UPDATE verse.realm_asks SET state = 'removing' WHERE id = aid;
    RETURN jsonb_build_object('status', 200, 'ask', aid, 'state', 'removing');
  END IF;
  n := verse.ask_revert(s, a.did);
  DELETE FROM verse.realm_asks WHERE id = aid;
  RETURN jsonb_build_object('status', 200, 'deleted', aid, 'reverted', n);
END $$;

-- DELETE a board's row. A made realm's feature leaves its board (and an ask whose only change it was
-- goes too); a hand-built realm's row is switched off and a remove ask waits for its code to come out.
CREATE FUNCTION verse.ask_row_delete(s text, key text, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  m verse.made_realms;
  label text;
  before jsonb;
  aid bigint;
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF m.slug IS NOT NULL THEN
    IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(verse.edit_made_board(m)) f WHERE verse.edit_key(f) = key) THEN RETURN '{"status":404,"error":"no such row"}'; END IF;
    UPDATE verse.made_realms SET
      features = coalesce((SELECT jsonb_agg(f ORDER BY o) FROM jsonb_array_elements(features) WITH ORDINALITY AS x(f, o) WHERE verse.edit_key(f) <> key), '[]'),
      off = coalesce((SELECT jsonb_agg(f ORDER BY o) FROM jsonb_array_elements(off) WITH ORDINALITY AS x(f, o) WHERE verse.edit_key(f) <> key), '[]'),
      settings_ms = verse.now_ms()
    WHERE slug = s;
    -- the asks that changed it no longer can: their part goes, and an ask with nothing left goes too
    UPDATE verse.realm_asks SET did = jsonb_set(jsonb_set(did, '{before}', (did->'before') - key), '{after}', (did->'after') - key)
    WHERE slug = s AND did->'after' ? key;
    DELETE FROM verse.realm_asks WHERE slug = s AND state = 'applied' AND commit_hash IS NULL AND NOT verse.ask_changed(did);
    RETURN jsonb_build_object('status', 200, 'deleted', key);
  END IF;
  SELECT r->>'label' INTO label FROM verse.realm_rights h, jsonb_array_elements(h.rows) r WHERE h.slug = s AND r->>'id' = key;
  IF label IS NULL THEN RETURN '{"status":404,"error":"no such row"}'; END IF;
  SELECT id INTO aid FROM verse.realm_asks WHERE slug = s AND kind = 'remove' AND row_key = key LIMIT 1;
  IF aid IS NOT NULL THEN RETURN jsonb_build_object('status', 200, 'ask', aid, 'state', 'waiting'); END IF;
  before := verse.ask_state(s);
  INSERT INTO verse.realm_edit (slug, switched, ms, by_player) VALUES (s, jsonb_build_object(key, false), verse.now_ms(), (verse.builder_player(k)).id)
  ON CONFLICT (slug) DO UPDATE SET switched = verse.realm_edit.switched || jsonb_build_object(key, false), ms = excluded.ms, by_player = excluded.by_player;
  INSERT INTO verse.realm_asks (slug, ms, player, words, kind, row_key, state, did)
  VALUES (s, verse.now_ms(), (verse.builder_player(k)).id, left('Take out ' || label, 400), 'remove', key, 'waiting', verse.ask_diff(before, verse.ask_state(s)))
  RETURNING id INTO aid;
  RETURN jsonb_build_object('status', 200, 'ask', aid, 'state', 'waiting');
END $$;

-- Built in code (a site admin: the agent that built it, signing NIP-98, names the pushed commit). An
-- ask waiting -> applied with its commit; a remove, or an ask whose code came out -> gone.
CREATE FUNCTION verse.ask_done(s text, aid bigint, body jsonb, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  a verse.realm_asks;
  hash text := CASE WHEN jsonb_typeof(body->'commit') = 'string' AND body->>'commit' ~ '^[0-9a-f]{40}$' THEN body->>'commit' END;
BEGIN
  IF NOT site_admin THEN RETURN '{"status":403,"error":"a site admin"}'; END IF;
  IF hash IS NULL THEN RETURN '{"status":400,"error":"the commit, 40 hex digits"}'; END IF;
  SELECT * INTO a FROM verse.realm_asks WHERE id = aid AND slug = s FOR UPDATE;
  IF a.id IS NULL THEN RETURN '{"status":404,"error":"no such ask"}'; END IF;
  IF a.kind = 'remove' OR a.state = 'removing' THEN
    DELETE FROM verse.realm_asks WHERE id = aid;
    RETURN jsonb_build_object('status', 200, 'ask', aid, 'state', 'gone');
  END IF;
  IF a.state = 'applied' THEN RETURN jsonb_build_object('status', 409, 'error', 'already applied'); END IF;
  UPDATE verse.realm_asks SET state = 'applied', commit_hash = hash, done_ms = verse.now_ms() WHERE id = aid;
  RETURN jsonb_build_object('status', 200, 'ask', aid, 'state', 'applied');
END $$;
