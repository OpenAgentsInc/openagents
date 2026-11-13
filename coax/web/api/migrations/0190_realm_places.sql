-- A realm's placed things are world edits (#13506, after #13422 and #13478): `spawn` and `despawn` of
-- world.edit.v1 on a realm's world (/api/worlds/<slug>), carried out here in the transaction that saves the
-- world's revision (coax/web/api/worlds.c realm_post; the realm's own code is metaverse/edit/realm-world.coax).
--
-- The entity model (the owner's choice, delegated, recorded on #13506 and in docs/protocol/world-edit-v1.md):
-- every placed thing has a stable integer ID with an exact inverse, and the world's undo log is the one log.
--   - A placed thing's world ID is its place in the realm's `placed` list, from 1: made realms'
--     `placed:N` (0109) is world ID N + 1. The list only grows, so no ID ever shifts.
--   - A thing taken away stays in the list, marked gone: a made realm's as 0109 already marks it
--     (a `shaped` entry `placed:N` with gone true), a hand-built realm's (verse.realm_edit.placed) with
--     `"gone": true` on its entry, new here. Nothing existing is rewritten.
--   - `spawn` with ID 0 appends a thing (its ID is the new length). `spawn` with ID N brings back thing N,
--     which must be gone: the inverse of `despawn N` and the redo of a spawn. `despawn N` marks it gone.
--   - The kind is the place of the thing's name in the realm's thing list, from 1: verse.builder_things()
--     for a made realm, verse.realm_rights.things for a hand-built one.
--   - One undo log: a change made through the world is a step of the world's log (verse.world_steps) and
--     not of a made realm's own UNDO (0094, 0111). While worlds.c carries a batch out it sets
--     `verse.world_edit`, and made_realms_undo pushes no step. The made realm's own UNDO keeps the steps
--     its page made (a move on /save, a place on /place); 0111's slot check refuses one a world edit has
--     since changed, in words.
-- verse.edit_spec gains a `shaped placed:N 0 100 1` line for each gone thing of a hand-built realm, as a made
-- realm's spec already has, so every reader of `placed` lines knows which are gone. verse.realm_world_spec is
-- the spec the world reads: edit_spec, with a made realm's thing list as `thing NAME` lines (a hand-built
-- realm's spec has them already).

-- ---- one undo log: a world's edit is no step of the made realm's own
CREATE OR REPLACE FUNCTION verse.made_realms_undo() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  st jsonb;
  ops jsonb;
  card bigint;
BEGIN
  IF NEW.rev <> OLD.rev AND coalesce(current_setting('verse.restoring', true), '') <> 'on'
     AND coalesce(current_setting('verse.world_edit', true), '') <> 'on' THEN
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

-- ---- the spec
ALTER FUNCTION verse.edit_spec(text, text, boolean) RENAME TO edit_spec_0190;
CREATE FUNCTION verse.edit_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN out IS NULL OR verse.edit_made(s) THEN out
              ELSE out || coalesce((SELECT string_agg('shaped placed:' || (o - 1) || E' 0 100 1', E'\n' ORDER BY o) || E'\n'
                                    FROM verse.realm_edit e, jsonb_array_elements(e.placed) WITH ORDINALITY AS t(p, o)
                                    WHERE e.slug = s AND jsonb_typeof(p) = 'object' AND coalesce((p->>'gone')::boolean, false)), '') END
  FROM (SELECT verse.edit_spec_0190(s, k, site_admin) AS out) x $$;

CREATE FUNCTION verse.realm_world_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN out IS NULL OR NOT verse.edit_made(s) THEN out
              ELSE out || coalesce((SELECT string_agg('thing ' || t, E'\n' ORDER BY o) || E'\n' FROM unnest(verse.builder_things()) WITH ORDINALITY AS x(t, o)), '') END
  FROM (SELECT verse.edit_spec(s, k, site_admin) AS out) x $$;

-- ---- spawn and despawn
-- A thing placed (id 0) or brought back (id N, gone), by its owner or an admin. x and z in centimetres, yaw in
-- whole degrees. -> {status, id} | {status, error}
CREATE FUNCTION verse.realm_spawn(s text, id int, thing text, x int, z int, yaw int, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  m verse.made_realms;
  h verse.realm_rights := verse.edit_hand(s);
  e verse.realm_edit;
  ent text := 'placed:' || (id - 1);
  p jsonb;
  sh jsonb;
  far int;
  n int;
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF id IS NULL OR id < 0 OR thing IS NULL OR x IS NULL OR z IS NULL OR yaw IS NULL THEN RETURN '{"status":400,"error":"a thing and where"}'; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF m.slug IS NOT NULL THEN
    IF id = 0 THEN
      IF NOT thing = ANY (verse.builder_things()) THEN RETURN jsonb_build_object('status', 422, 'error', 'a ' || thing || ' is not a thing this realm places'); END IF;
      IF abs(x) > 4000 OR abs(z) > 4000 THEN RETURN '{"status":422,"error":"a thing stands within 40 m of the middle"}'; END IF;
      IF jsonb_array_length(m.placed) >= 200 THEN RETURN '{"status":409,"error":"the realm is full: 200 things"}'; END IF;
      UPDATE verse.made_realms SET placed = placed || jsonb_build_array(jsonb_build_object('thing', thing, 'x', trim_scale(round(x / 100.0, 2)), 'z', trim_scale(round(z / 100.0, 2)),
                                                                                          'yaw', trim_scale(round(radians(yaw)::numeric, 3)))), settings_ms = verse.now_ms()
      WHERE slug = s RETURNING jsonb_array_length(placed) INTO n;
      RETURN jsonb_build_object('status', 200, 'id', n);
    END IF;
    p := nullif(m.placed -> (id - 1), 'null');
    IF p IS NULL THEN RETURN jsonb_build_object('status', 404, 'error', 'no thing ' || id); END IF;
    IF p->>'thing' IS DISTINCT FROM thing THEN RETURN jsonb_build_object('status', 422, 'error', 'thing ' || id || ' is a ' || coalesce(p->>'thing', 'thing')); END IF;
    SELECT v INTO sh FROM jsonb_array_elements(m.shaped) v WHERE v->>'entity' = ent LIMIT 1;
    IF NOT coalesce((sh->>'gone')::boolean, false) THEN RETURN jsonb_build_object('status', 409, 'error', 'thing ' || id || ' stands already'); END IF;
    UPDATE verse.made_realms SET shaped = verse.realm_entry(shaped, ent,
             CASE WHEN (sh->>'yaw')::int = 0 AND (sh->>'scale')::int = 100 THEN NULL ELSE sh || '{"gone": false}' END), settings_ms = verse.now_ms()
    WHERE slug = s;
    RETURN jsonb_build_object('status', 200, 'id', id);
  END IF;
  SELECT * INTO e FROM verse.realm_edit WHERE slug = s FOR UPDATE;
  IF id = 0 THEN
    IF NOT thing = ANY (h.things) THEN RETURN jsonb_build_object('status', 422, 'error', 'a ' || thing || ' is not a thing this realm places'); END IF;
    IF abs(x) > 40000 OR abs(z) > 40000 THEN RETURN '{"status":422,"error":"a thing stands within 400 m of the middle"}'; END IF;
    IF jsonb_array_length(coalesce(e.placed, '[]')) >= 200 THEN RETURN '{"status":409,"error":"the realm is full: 200 things"}'; END IF;
    p := jsonb_build_object('thing', thing, 'x', trim_scale(round(x / 100.0, 2)), 'z', trim_scale(round(z / 100.0, 2)), 'yaw', trim_scale(round(radians(yaw)::numeric, 3)));
    INSERT INTO verse.realm_edit (slug, placed, ms, by_player) VALUES (s, jsonb_build_array(p), verse.now_ms(), (verse.builder_player(k)).id)
    ON CONFLICT (slug) DO UPDATE SET placed = verse.realm_edit.placed || jsonb_build_array(p), ms = excluded.ms, by_player = excluded.by_player
    RETURNING jsonb_array_length(placed) INTO n;
    RETURN jsonb_build_object('status', 200, 'id', n);
  END IF;
  p := nullif(coalesce(e.placed, '[]') -> (id - 1), 'null');
  IF p IS NULL THEN RETURN jsonb_build_object('status', 404, 'error', 'no thing ' || id); END IF;
  IF p->>'thing' IS DISTINCT FROM thing THEN RETURN jsonb_build_object('status', 422, 'error', 'thing ' || id || ' is a ' || coalesce(p->>'thing', 'thing')); END IF;
  IF NOT coalesce((p->>'gone')::boolean, false) THEN RETURN jsonb_build_object('status', 409, 'error', 'thing ' || id || ' stands already'); END IF;
  UPDATE verse.realm_edit SET placed = jsonb_set(placed, ARRAY[(id - 1)::text], p - 'gone'), ms = verse.now_ms() WHERE slug = s;
  RETURN jsonb_build_object('status', 200, 'id', id);
END $$;

-- Thing `id` taken away (marked gone), by its owner or an admin. -> {status, id} | {status, error}
CREATE FUNCTION verse.realm_despawn(s text, id int, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  m verse.made_realms;
  e verse.realm_edit;
  ent text := 'placed:' || (id - 1);
  p jsonb;
  sh jsonb;
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF id IS NULL OR id < 1 THEN RETURN '{"status":400,"error":"a thing"}'; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF m.slug IS NOT NULL THEN
    p := nullif(m.placed -> (id - 1), 'null');
    IF p IS NULL THEN RETURN jsonb_build_object('status', 404, 'error', 'no thing ' || id); END IF;
    SELECT v INTO sh FROM jsonb_array_elements(m.shaped) v WHERE v->>'entity' = ent LIMIT 1;
    IF coalesce((sh->>'gone')::boolean, false) THEN RETURN jsonb_build_object('status', 409, 'error', 'thing ' || id || ' is gone already'); END IF;
    -- 0109's rule: at most 64 things moved, turned, resized or taken away
    IF (SELECT count(DISTINCT x) FROM (SELECT v->>'entity' x FROM jsonb_array_elements(m.moved || m.shaped) v UNION ALL SELECT ent) u) > 64 THEN
      RETURN '{"status":409,"error":"at most 64 things moved, turned, resized or taken away"}';
    END IF;
    UPDATE verse.made_realms SET shaped = verse.realm_entry(shaped, ent,
             jsonb_build_object('entity', ent, 'yaw', coalesce((sh->>'yaw')::int, 0), 'scale', coalesce((sh->>'scale')::int, 100), 'gone', true)), settings_ms = verse.now_ms()
    WHERE slug = s;
    RETURN jsonb_build_object('status', 200, 'id', id);
  END IF;
  SELECT * INTO e FROM verse.realm_edit WHERE slug = s FOR UPDATE;
  p := nullif(coalesce(e.placed, '[]') -> (id - 1), 'null');
  IF p IS NULL THEN RETURN jsonb_build_object('status', 404, 'error', 'no thing ' || id); END IF;
  IF coalesce((p->>'gone')::boolean, false) THEN RETURN jsonb_build_object('status', 409, 'error', 'thing ' || id || ' is gone already'); END IF;
  UPDATE verse.realm_edit SET placed = jsonb_set(placed, ARRAY[(id - 1)::text], p || '{"gone": true}'), ms = verse.now_ms() WHERE slug = s;
  RETURN jsonb_build_object('status', 200, 'id', id);
END $$;
