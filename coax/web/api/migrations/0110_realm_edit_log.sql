-- A made realm's lore and quests are in its document, and the server's UNDO and REDO walk one log of
-- per-edit inverses, the log aethyr/forge/edit.coax keeps (GitHub #12629; Aethyr book chapter 52).
--
-- Lore and quests: a made realm (base NULL) keeps them in the document, `lore` {WHO: TEXT} and
-- `quests` {GIVER: {ask, steps, reward}}, each present only when it has an entry, so a document
-- without any keeps the exact bytes (and SHA-256) it had. POST .../lore and .../quest write the
-- document: a new revision and an undo step, as any edit. Forks carry them, and a visitor reads
-- the revision published (GET .../panes). Hand-built realms and forks of built-in realms keep
-- 0055's tables, as before.
--   Existing data: a made realm with lore or quests in 0055's tables has them copied into its
--   document as one new revision (kept, not an undo step); every revision kept before keeps its
--   bytes and hash, and the tables keep their rows. `lore_since` is that revision: until the realm
--   publishes one at or after it, a visitor reads its lore and quests live (as 0055 always did), so
--   nothing a visitor sees changes with this migration.
--
-- The edit log: each undo step is {to, card, ops}, ops a list of {slot, before, after} (null: the
-- slot absent), the slots aethyr/forge/edit.coax names:
--   ENTITY ("trees:3", "placed:0")   {x, z} when moved and {yaw, scale, gone} when shaped
--   set:FIELD                         name, style, ground, time, camera, palette
--   tool:NAME                         a visitor tool, true or false
--   row:N, feature:N, place:N         the N-th entry of off, features and placed
--   lore:WHO, quest:GIVER             a character's lore; a giver's quest
-- UNDO applies the newest step's inverse (each slot from after back to before, newest op first),
-- refused (409) when a slot is no longer what the step left; REDO applies it again, refused when a
-- slot is no longer what the undo left. A new edit drops what was undone. Steps kept before this
-- migration (no ops) still undo by their whole revision, as 0094's did. A card's state goes back
-- and forth with its step.
--   POST /api/playtest/realms/<slug>/redo {rev}   -> 200 {rev, undo, redo}; 400, 403, 404, 409 {error, rev}
--   POST /api/playtest/realms/<slug>/undo {rev}   -> 200 {rev, undo, redo}; as 0094's
-- The spec gains a "redo N" line for its owner and admins.

ALTER TABLE verse.made_realms ADD COLUMN lore jsonb NOT NULL DEFAULT '{}';
ALTER TABLE verse.made_realms ADD COLUMN quests jsonb NOT NULL DEFAULT '{}';
ALTER TABLE verse.made_realms ADD COLUMN lore_since bigint;
ALTER TABLE verse.made_realms ADD COLUMN redo jsonb NOT NULL DEFAULT '[]';

-- The document: 0109's, with `lore` and `quests` when it has any.
CREATE OR REPLACE FUNCTION verse.realm_doc(m verse.made_realms) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('doc', 'aethyr.realm', 'version', 1, 'slug', m.slug, 'rev', m.rev, 'name', m.name,
    'setup', jsonb_build_object('camera', m.camera, 'style', m.style, 'ground', m.ground, 'time', m.time, 'palette', m.palette),
    'features', m.features, 'off', m.off, 'tools', m.tools, 'placed', m.placed, 'moved', m.moved, 'base', m.base,
    'forked_from', CASE WHEN m.forked_from IS NULL THEN NULL ELSE jsonb_build_object('slug', m.forked_from, 'rev', m.forked_rev) END)
  || CASE WHEN coalesce(m.shaped, '[]') <> '[]' THEN jsonb_build_object('shaped', m.shaped) ELSE '{}' END
  || CASE WHEN coalesce(m.lore, '{}') <> '{}' THEN jsonb_build_object('lore', m.lore) ELSE '{}' END
  || CASE WHEN coalesce(m.quests, '{}') <> '{}' THEN jsonb_build_object('quests', m.quests) ELSE '{}' END $$;

-- A kept document as a realm: one without `lore` or `quests` has none.
CREATE OR REPLACE FUNCTION verse.realm_as_doc(m verse.made_realms, d jsonb) RETURNS verse.made_realms LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  p verse.made_realms := m;
BEGIN
  p.rev := coalesce((d->>'rev')::bigint, m.rev);
  p.name := coalesce(d->>'name', m.name);
  p.style := coalesce(d->'setup'->>'style', m.style); p.ground := coalesce(d->'setup'->>'ground', m.ground);
  p.time := coalesce(d->'setup'->>'time', m.time); p.camera := coalesce(d->'setup'->>'camera', m.camera);
  p.palette := coalesce(d->'setup'->>'palette', m.palette);
  p.features := coalesce(d->'features', m.features); p.off := coalesce(d->'off', m.off); p.tools := coalesce(d->'tools', m.tools);
  p.placed := coalesce(d->'placed', m.placed); p.moved := coalesce(d->'moved', m.moved);
  p.shaped := CASE WHEN d IS NULL THEN m.shaped ELSE coalesce(d->'shaped', '[]') END;
  p.lore := CASE WHEN d IS NULL THEN m.lore ELSE coalesce(d->'lore', '{}') END;
  p.quests := CASE WHEN d IS NULL THEN m.quests ELSE coalesce(d->'quests', '{}') END;
  RETURN p;
END $$;

-- A change to lore or quests is a new revision too.
CREATE OR REPLACE FUNCTION verse.made_realms_rev() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.rev = OLD.rev AND (NEW.name, NEW.style, NEW.ground, NEW.time, NEW.camera, NEW.palette, NEW.features, NEW.off, NEW.tools, NEW.placed, NEW.moved, NEW.shaped, NEW.lore, NEW.quests)
     IS DISTINCT FROM (OLD.name, OLD.style, OLD.ground, OLD.time, OLD.camera, OLD.palette, OLD.features, OLD.off, OLD.tools, OLD.placed, OLD.moved, OLD.shaped, OLD.lore, OLD.quests) THEN
    NEW.rev := OLD.rev + 1;
  END IF;
  RETURN NEW;
END $$;

-- ---- the slots
-- An entity's value: {x, z} when moved, {yaw, scale, gone} when shaped; NULL when neither.
CREATE FUNCTION verse.realm_entity(m verse.made_realms, e text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT nullif(coalesce((SELECT jsonb_build_object('x', v->'x', 'z', v->'z') FROM jsonb_array_elements(m.moved) v WHERE v->>'entity' = e LIMIT 1), '{}')
             || coalesce((SELECT jsonb_build_object('yaw', v->'yaw', 'scale', v->'scale', 'gone', v->'gone') FROM jsonb_array_elements(m.shaped) v WHERE v->>'entity' = e LIMIT 1), '{}'),
                '{}') $$;
-- An array's N-th entry set (NULL takes it, and every NULL after the last entry, away).
CREATE FUNCTION verse.realm_at(a jsonb, n int, v jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  out jsonb := coalesce(a, '[]');
BEGIN
  WHILE jsonb_array_length(out) <= n LOOP out := out || 'null'::jsonb; END LOOP;
  out := jsonb_set(out, ARRAY[n::text], coalesce(v, 'null'::jsonb));
  WHILE jsonb_array_length(out) > 0 AND out -> -1 = 'null'::jsonb LOOP out := out - (-1); END LOOP;
  RETURN out;
END $$;
-- An entry replaced where it is, added at the end, or taken away (v NULL).
CREATE FUNCTION verse.realm_entry(a jsonb, e text, v jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE
    WHEN v IS NULL THEN coalesce((SELECT jsonb_agg(x ORDER BY o) FROM jsonb_array_elements(a) WITH ORDINALITY t(x, o) WHERE x->>'entity' <> e), '[]')
    WHEN EXISTS (SELECT 1 FROM jsonb_array_elements(a) x WHERE x->>'entity' = e)
      THEN (SELECT jsonb_agg(CASE WHEN x->>'entity' = e THEN v ELSE x END ORDER BY o) FROM jsonb_array_elements(a) WITH ORDINALITY t(x, o))
    ELSE a || jsonb_build_array(v) END $$;

-- A slot's value in m; NULL when absent.
CREATE FUNCTION verse.realm_slot(m verse.made_realms, slot text) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  kind text := split_part(slot, ':', 1);
  nm text := substr(slot, length(kind) + 2);
BEGIN
  RETURN CASE kind
    WHEN 'set' THEN CASE nm WHEN 'name' THEN to_jsonb(m.name) WHEN 'style' THEN to_jsonb(m.style) WHEN 'ground' THEN to_jsonb(m.ground)
                            WHEN 'time' THEN to_jsonb(m.time) WHEN 'camera' THEN to_jsonb(m.camera) WHEN 'palette' THEN to_jsonb(m.palette) END
    WHEN 'tool' THEN m.tools -> nm
    WHEN 'row' THEN nullif(m.off -> nm::int, 'null')
    WHEN 'feature' THEN nullif(m.features -> nm::int, 'null')
    WHEN 'place' THEN nullif(m.placed -> nm::int, 'null')
    WHEN 'lore' THEN m.lore -> nm
    WHEN 'quest' THEN m.quests -> nm
    ELSE verse.realm_entity(m, slot) END;
END $$;
-- m with a slot set to v (NULL: absent).
CREATE FUNCTION verse.realm_slot_put(m verse.made_realms, slot text, v jsonb) RETURNS verse.made_realms LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  kind text := split_part(slot, ':', 1);
  nm text := substr(slot, length(kind) + 2);
  p verse.made_realms := m;
BEGIN
  CASE kind
    WHEN 'set' THEN
      CASE nm WHEN 'name' THEN p.name := v #>> '{}'; WHEN 'style' THEN p.style := v #>> '{}'; WHEN 'ground' THEN p.ground := v #>> '{}';
              WHEN 'time' THEN p.time := v #>> '{}'; WHEN 'camera' THEN p.camera := v #>> '{}'; WHEN 'palette' THEN p.palette := v #>> '{}'; END CASE;
    WHEN 'tool' THEN p.tools := CASE WHEN v IS NULL THEN p.tools - nm ELSE jsonb_set(p.tools, ARRAY[nm], v) END;
    WHEN 'row' THEN p.off := verse.realm_at(p.off, nm::int, v);
    WHEN 'feature' THEN p.features := verse.realm_at(p.features, nm::int, v);
    WHEN 'place' THEN p.placed := verse.realm_at(p.placed, nm::int, v);
    WHEN 'lore' THEN p.lore := CASE WHEN v IS NULL THEN p.lore - nm ELSE jsonb_set(p.lore, ARRAY[nm], v) END;
    WHEN 'quest' THEN p.quests := CASE WHEN v IS NULL THEN p.quests - nm ELSE jsonb_set(p.quests, ARRAY[nm], v) END;
    ELSE
      p.moved := verse.realm_entry(p.moved, slot, CASE WHEN v ? 'x' THEN jsonb_build_object('entity', slot, 'x', v->'x', 'z', v->'z') END);
      p.shaped := verse.realm_entry(p.shaped, slot, CASE WHEN v ? 'yaw' THEN jsonb_build_object('entity', slot, 'yaw', v->'yaw', 'scale', v->'scale', 'gone', v->'gone') END);
  END CASE;
  RETURN p;
END $$;

-- The edit from o to n, one op a slot that differs, in slot order.
CREATE FUNCTION verse.realm_ops(o verse.made_realms, n verse.made_realms) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('slot', slot, 'before', b, 'after', a) ORDER BY grp, slot), '[]')
  FROM (
    SELECT grp, slot, verse.realm_slot(o, slot) b, verse.realm_slot(n, slot) a FROM (
      SELECT 1 grp, 'set:' || f slot FROM unnest(ARRAY['name', 'style', 'ground', 'time', 'camera', 'palette']) f
      UNION SELECT 2, 'tool:' || k FROM (SELECT jsonb_object_keys(o.tools) k UNION SELECT jsonb_object_keys(n.tools)) t
      UNION SELECT 3, 'row:' || i FROM generate_series(0, greatest(jsonb_array_length(o.off), jsonb_array_length(n.off)) - 1) i
      UNION SELECT 4, 'feature:' || i FROM generate_series(0, greatest(jsonb_array_length(o.features), jsonb_array_length(n.features)) - 1) i
      UNION SELECT 5, 'place:' || i FROM generate_series(0, greatest(jsonb_array_length(o.placed), jsonb_array_length(n.placed)) - 1) i
      UNION SELECT 6, x->>'entity' FROM (SELECT jsonb_array_elements(o.moved || o.shaped || n.moved || n.shaped) x) e
      UNION SELECT 7, 'lore:' || k FROM (SELECT jsonb_object_keys(o.lore) k UNION SELECT jsonb_object_keys(n.lore)) t
      UNION SELECT 8, 'quest:' || k FROM (SELECT jsonb_object_keys(o.quests) k UNION SELECT jsonb_object_keys(n.quests)) t
    ) s
  ) d WHERE b IS DISTINCT FROM a $$;

-- m with ops applied forward (each slot from before to after) or back (after to before, newest
-- first); NULL when a slot is not what the step says.
CREATE FUNCTION verse.realm_ops_apply(m verse.made_realms, ops jsonb, back boolean) RETURNS verse.made_realms LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  p verse.made_realms := m;
  op jsonb;
BEGIN
  FOR op IN SELECT x FROM jsonb_array_elements(ops) WITH ORDINALITY t(x, o) ORDER BY CASE WHEN back THEN -o ELSE o END LOOP
    IF verse.realm_slot(p, op->>'slot') IS DISTINCT FROM nullif(op->CASE WHEN back THEN 'after' ELSE 'before' END, 'null') THEN RETURN NULL; END IF;
    p := verse.realm_slot_put(p, op->>'slot', nullif(op->CASE WHEN back THEN 'before' ELSE 'after' END, 'null'));
  END LOOP;
  RETURN p;
END $$;

-- ---- every change is a step with its ops; a new one drops what was undone
CREATE OR REPLACE FUNCTION verse.made_realms_undo() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  st jsonb;
  ops jsonb;
  card bigint;
BEGIN
  IF NEW.rev <> OLD.rev AND coalesce(current_setting('verse.restoring', true), '') <> 'on' THEN
    ops := verse.realm_ops(OLD, NEW);
    card := nullif(coalesce(current_setting('verse.card', true), ''), '')::bigint;
    IF ops <> '[]' OR card IS NOT NULL THEN
      st := NEW.undo || jsonb_build_array(jsonb_build_object('to', OLD.rev, 'card', card, 'ops', ops));
      WHILE jsonb_array_length(st) > 64 LOOP st := st - 0; END LOOP;
      NEW.undo := st;
      NEW.redo := '[]';
    END IF;
  END IF;
  RETURN NEW;
END $$;

-- A realm set to p's document, with its undo and redo lists, as a restoring write (no step pushed).
CREATE FUNCTION verse.realm_restore(s text, p verse.made_realms, u jsonb, rd jsonb) RETURNS verse.made_realms LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms;
BEGIN
  PERFORM set_config('verse.restoring', 'on', true);
  UPDATE verse.made_realms SET name = p.name, style = p.style, ground = p.ground, time = p.time, camera = p.camera, palette = p.palette,
         features = p.features, off = p.off, tools = p.tools, placed = p.placed, moved = p.moved, shaped = p.shaped, lore = p.lore, quests = p.quests,
         undo = u, redo = rd, settings_ms = verse.now_ms()
  WHERE slug = s RETURNING * INTO m;
  PERFORM set_config('verse.restoring', '', true);
  RETURN m;
END $$;

-- UNDO: the newest step's inverse; a card it settled waits again.
CREATE OR REPLACE FUNCTION verse.realm_undo(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms;
  at bigint := CASE WHEN jsonb_typeof(body->'rev') = 'number' THEN (body->>'rev')::bigint END;
  e jsonb;
  d jsonb;
  p verse.made_realms;
  was text;
  r jsonb;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s AND base IS NULL FOR UPDATE;
  IF m.slug IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF NOT verse.edit_may(s, k, site_admin) THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF at IS NULL THEN RETURN '{"status":400,"error":"a revision"}'; END IF;
  IF at <> m.rev THEN RETURN jsonb_build_object('status', 409, 'error', 'changed since', 'rev', m.rev); END IF;
  IF jsonb_array_length(m.undo) = 0 THEN RETURN jsonb_build_object('status', 409, 'error', 'nothing to undo', 'rev', m.rev); END IF;
  e := m.undo -> -1;
  IF e ? 'ops' THEN
    p := verse.realm_ops_apply(m, e->'ops', true);
    IF p.slug IS NULL THEN RETURN jsonb_build_object('status', 409, 'error', 'changed since', 'rev', m.rev); END IF;
    r := e;
  ELSE   -- a step kept before 0110: its whole revision (one not kept: the step only goes)
    SELECT doc INTO d FROM verse.realm_revisions WHERE slug = s AND rev = (e->>'to')::bigint;
    p := CASE WHEN d IS NULL THEN m ELSE verse.realm_as_doc(m, d) END;
    r := jsonb_build_object('to', m.rev, 'card', e->'card');
  END IF;
  IF jsonb_typeof(e->'card') = 'number' THEN
    SELECT x.state INTO was FROM verse.realm_proposals x WHERE id = (e->>'card')::bigint AND slug = s;
    UPDATE verse.realm_proposals SET state = 'waiting', decided_ms = NULL WHERE id = (e->>'card')::bigint AND slug = s;
    r := r || jsonb_build_object('state', was);
  END IF;
  m := verse.realm_restore(s, p, m.undo - (-1), (SELECT CASE WHEN jsonb_array_length(x) > 64 THEN x - 0 ELSE x END FROM (SELECT m.redo || jsonb_build_array(r) x) y));
  RETURN jsonb_build_object('status', 200, 'rev', m.rev, 'undo', jsonb_array_length(m.undo), 'redo', jsonb_array_length(m.redo));
END $$;

-- REDO: the step last undone, again; its card settled again as it was.
CREATE FUNCTION verse.realm_redo(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms;
  at bigint := CASE WHEN jsonb_typeof(body->'rev') = 'number' THEN (body->>'rev')::bigint END;
  e jsonb;
  d jsonb;
  p verse.made_realms;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s AND base IS NULL FOR UPDATE;
  IF m.slug IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF NOT verse.edit_may(s, k, site_admin) THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF at IS NULL THEN RETURN '{"status":400,"error":"a revision"}'; END IF;
  IF at <> m.rev THEN RETURN jsonb_build_object('status', 409, 'error', 'changed since', 'rev', m.rev); END IF;
  IF jsonb_array_length(m.redo) = 0 THEN RETURN jsonb_build_object('status', 409, 'error', 'nothing to redo', 'rev', m.rev); END IF;
  e := m.redo -> -1;
  IF e ? 'ops' THEN
    p := verse.realm_ops_apply(m, e->'ops', false);
    IF p.slug IS NULL THEN RETURN jsonb_build_object('status', 409, 'error', 'changed since', 'rev', m.rev); END IF;
  ELSE
    SELECT doc INTO d FROM verse.realm_revisions WHERE slug = s AND rev = (e->>'to')::bigint;
    p := CASE WHEN d IS NULL THEN m ELSE verse.realm_as_doc(m, d) END;
  END IF;
  IF jsonb_typeof(e->'card') = 'number' AND e->>'state' IN ('accepted', 'declined') THEN
    UPDATE verse.realm_proposals SET state = e->>'state', decided_ms = verse.now_ms() WHERE id = (e->>'card')::bigint AND slug = s;
  END IF;
  m := verse.realm_restore(s, p, m.undo || jsonb_build_array(CASE WHEN e ? 'ops' THEN e - 'state' ELSE jsonb_build_object('to', m.rev, 'card', e->'card') END), m.redo - (-1));
  RETURN jsonb_build_object('status', 200, 'rev', m.rev, 'undo', jsonb_array_length(m.undo), 'redo', jsonb_array_length(m.redo));
END $$;

-- A declined card is a step with no ops (0094 pushed it with none at all).
CREATE OR REPLACE FUNCTION verse.edit_decide(s text, pid bigint, accept boolean, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb;
  r0 bigint;
BEGIN
  SELECT rev INTO r0 FROM verse.made_realms WHERE slug = s AND base IS NULL FOR UPDATE;
  PERFORM set_config('verse.card', pid::text, true);
  out := verse.edit_decide_0093(s, pid, accept, k, site_admin);
  PERFORM set_config('verse.card', '', true);
  IF r0 IS NOT NULL AND (out->>'status') = '200' AND NOT accept THEN
    UPDATE verse.made_realms SET redo = '[]', undo = (SELECT CASE WHEN jsonb_array_length(u) > 64 THEN u - 0 ELSE u END
                                         FROM (SELECT undo || jsonb_build_array(jsonb_build_object('to', r0, 'card', pid, 'ops', '[]'::jsonb)) AS u) x)
    WHERE slug = s;
  END IF;
  RETURN out;
END $$;

-- ---- lore and quests in a made realm's document
CREATE FUNCTION verse.made_lore_realm(s text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND base IS NULL) $$;

ALTER FUNCTION verse.edit_lore(text, jsonb, text, boolean) RENAME TO edit_lore_0055;
CREATE FUNCTION verse.edit_lore(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  v_who text := verse.pane_text(body->'who', 40);
  t text := verse.pane_text(body->'text', 400);
  m verse.made_realms;
BEGIN
  IF NOT verse.made_lore_realm(s) THEN RETURN verse.edit_lore_0055(s, body, k, site_admin); END IF;
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' OR NOT verse.pane_key_ok(v_who) THEN RETURN '{"status":400,"error":"say whose lore"}'; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF t = '' THEN
    UPDATE verse.made_realms SET lore = lore - v_who WHERE slug = s;
    RETURN jsonb_build_object('status', 200, 'who', v_who, 'saved', false);
  END IF;
  IF NOT (m.lore ? v_who) AND (SELECT count(*) FROM jsonb_object_keys(m.lore)) >= 40 THEN
    RETURN '{"status":409,"error":"this realm has lore for 40 characters"}';
  END IF;
  UPDATE verse.made_realms SET lore = jsonb_set(lore, ARRAY[v_who], to_jsonb(t)) WHERE slug = s;
  RETURN jsonb_build_object('status', 200, 'who', v_who, 'saved', true);
END $$;

ALTER FUNCTION verse.edit_quest(text, jsonb, text, boolean) RENAME TO edit_quest_0055;
CREATE FUNCTION verse.edit_quest(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  v_giver text := verse.pane_text(body->'giver', 40);
  v_ask text := verse.pane_text(body->'ask', 160);
  v_reward text := verse.pane_text(body->'reward', 100);
  v_steps jsonb;
  m verse.made_realms;
BEGIN
  IF NOT verse.made_lore_realm(s) THEN RETURN verse.edit_quest_0055(s, body, k, site_admin); END IF;
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' OR NOT verse.pane_key_ok(v_giver) THEN RETURN '{"status":400,"error":"say who gives it"}'; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF v_ask = '' THEN
    UPDATE verse.made_realms SET quests = quests - v_giver WHERE slug = s;
    RETURN jsonb_build_object('status', 200, 'giver', v_giver, 'saved', false);
  END IF;
  v_steps := coalesce((SELECT jsonb_agg(t ORDER BY i) FROM (
             SELECT verse.pane_text(x, 100) t, i FROM jsonb_array_elements(CASE WHEN jsonb_typeof(body->'steps') = 'array' THEN body->'steps' ELSE '[]' END)
             WITH ORDINALITY a(x, i) ORDER BY i LIMIT 3) z), '[]');
  IF NOT (m.quests ? v_giver) AND (SELECT count(*) FROM jsonb_object_keys(m.quests)) >= 40 THEN
    RETURN '{"status":409,"error":"this realm has 40 quests"}';
  END IF;
  UPDATE verse.made_realms SET quests = jsonb_set(quests, ARRAY[v_giver], jsonb_build_object('ask', v_ask, 'steps', v_steps, 'reward', v_reward)) WHERE slug = s;
  RETURN jsonb_build_object('status', 200, 'giver', v_giver, 'saved', true);
END $$;

-- The panes' lines from a document's lore and quests (0055's format and order).
CREATE FUNCTION verse.panes_of(lore jsonb, quests jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce((SELECT string_agg('lore ' || key || '|' || (value #>> '{}'), E'\n' ORDER BY key) || E'\n' FROM jsonb_each(coalesce(lore, '{}'))), '')
      || coalesce((SELECT string_agg('quest ' || key || '|' || (value->>'ask') || '|' || coalesce(value->'steps'->>0, '') || '|' || coalesce(value->'steps'->>1, '') || '|'
                                     || coalesce(value->'steps'->>2, '') || '|' || coalesce(value->>'reward', ''), E'\n' ORDER BY key) || E'\n'
                   FROM jsonb_each(coalesce(quests, '{}'))), '') $$;
-- GET .../panes: a made realm's from its document (a visitor's: the revision published; one
-- published before lore_since: live, as 0055's).
ALTER FUNCTION verse.edit_panes(text, text, boolean) RENAME TO edit_panes_0055;
CREATE FUNCTION verse.edit_panes(s text, k text, site_admin boolean) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  m verse.made_realms;
  live jsonb;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s AND base IS NULL;
  IF m.slug IS NULL OR verse.edit_role(s, k, site_admin) IS NULL THEN RETURN verse.edit_panes_0055(s, k, site_admin); END IF;
  IF NOT verse.edit_may(s, k, site_admin) AND (m.lore_since IS NULL OR m.published_rev >= m.lore_since) THEN
    live := verse.realm_live_doc(m);
    IF live IS NOT NULL THEN RETURN verse.panes_of(live->'lore', live->'quests'); END IF;
  END IF;
  RETURN verse.panes_of(m.lore, m.quests);
END $$;

-- A fork of a made realm carries the lore and quests of the revision it was forked at (before
-- made_realms_first, by name, which hashes the document).
CREATE FUNCTION verse.made_realms_doc_lore() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  d jsonb;
BEGIN
  IF NEW.base IS NULL AND NEW.forked_from IS NOT NULL AND NEW.lore = '{}' AND NEW.quests = '{}' THEN
    SELECT doc INTO d FROM verse.realm_revisions WHERE slug = NEW.forked_from AND rev = NEW.forked_rev;
    IF d IS NOT NULL THEN
      NEW.lore := coalesce(d->'lore', '{}'); NEW.quests := coalesce(d->'quests', '{}');
    ELSE
      SELECT coalesce(lore, '{}'), coalesce(quests, '{}') INTO NEW.lore, NEW.quests FROM verse.made_realms WHERE slug = NEW.forked_from AND rev = NEW.forked_rev AND base IS NULL;
      NEW.lore := coalesce(NEW.lore, '{}'); NEW.quests := coalesce(NEW.quests, '{}');
    END IF;
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER made_realms_doc_lore BEFORE INSERT ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_doc_lore();

-- The spec: a "redo N" line (0094's has "undo N"), for its owner and admins.
ALTER FUNCTION verse.edit_spec(text, text, boolean) RENAME TO edit_spec_0109;
CREATE FUNCTION verse.edit_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN out IS NULL OR NOT verse.edit_may(s, k, site_admin) THEN out
              ELSE out || coalesce((SELECT 'redo ' || jsonb_array_length(redo) || E'\n' FROM verse.made_realms WHERE slug = s AND base IS NULL), '') END
  FROM (SELECT verse.edit_spec_0109(s, k, site_admin) AS out) x $$;

-- ---- the existing data: a made realm's lore and quests from 0055's tables, as one new revision
-- (kept, not an undo step); `lore_since` is that revision. False when it has none there.
CREATE FUNCTION verse.realm_lore_into_doc(s text) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE
  lo jsonb := coalesce((SELECT jsonb_object_agg(l.who, l.text) FROM verse.realm_lore l WHERE l.slug = s), '{}');
  qu jsonb := coalesce((SELECT jsonb_object_agg(q.giver, jsonb_build_object('ask', q.ask, 'steps', to_jsonb(q.steps), 'reward', q.reward))
                        FROM verse.realm_quests q WHERE q.slug = s), '{}');
  m verse.made_realms;
BEGIN
  IF (lo = '{}' AND qu = '{}') OR NOT verse.made_lore_realm(s)
     OR EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND lore = lo AND quests = qu) THEN RETURN false; END IF;
  PERFORM set_config('verse.restoring', 'on', true);
  UPDATE verse.made_realms SET lore = lo, quests = qu WHERE slug = s RETURNING * INTO m;
  PERFORM set_config('verse.restoring', '', true);
  UPDATE verse.made_realms SET lore_since = m.rev WHERE slug = s;
  UPDATE verse.realm_revisions SET why = 'edit' WHERE slug = s AND rev = m.rev;
  RETURN true;
END $$;
SELECT count(*) FILTER (WHERE verse.realm_lore_into_doc(slug)) FROM verse.made_realms WHERE base IS NULL;
