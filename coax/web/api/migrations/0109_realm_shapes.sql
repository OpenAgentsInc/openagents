-- A made realm's entities turn, change size and are taken away, as well as move (GitHub #12629,
-- docs/verse/aethyrforge.md slice 3). aethyr/forge/edit.coax keeps one typed edit model for every
-- revision path; this is the document's side of rotate, resize and remove.
--
-- The document gains `shaped`: [{entity, yaw, scale, gone}], an entity's turn (whole degrees,
-- -179..180), size (whole percent of its laid-out size, 25..400) and whether it is taken away.
-- An entry with yaw 0, scale 100 and gone false is no entry. A document without any keeps the
-- exact bytes (and SHA-256) it had: `shaped` is in the document only when it has an entry.
-- "placed:N" names the N-th thing placed, so a placed thing moves, turns, resizes and goes too.
--
--   POST /api/playtest/realms/<slug>/save {rev, moved, shaped?}   -> as before; without `shaped` the
--        shapes stay as they are (a page before this keeps them), with it they are replaced. A
--        malformed `shaped`, or more than 64 entities overridden in all, is 400 and changes nothing.
-- The spec gains "shaped ENTITY YAW SCALE 0|1" lines (a visitor's from the revision published) and
-- "shapes 1" for a made realm, so a client knows the server keeps them. UNDO, forks and the
-- published revision carry shapes as they carry moves.

ALTER TABLE verse.made_realms ADD COLUMN shaped jsonb NOT NULL DEFAULT '[]';

-- The shapes a save sends, held: at most 64, each a FEATURE:N name, a whole-degree turn, a whole
-- percent size and gone; one per entity (the last wins); plain entries dropped. NULL: not shapes.
CREATE FUNCTION verse.realm_shapes(v jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  out jsonb := '[]';
  e jsonb;
BEGIN
  IF jsonb_typeof(v) IS DISTINCT FROM 'array' OR jsonb_array_length(v) > 64 THEN RETURN NULL; END IF;
  FOR e IN SELECT x FROM jsonb_array_elements(v) x LOOP
    IF jsonb_typeof(e) <> 'object' OR jsonb_typeof(e->'entity') IS DISTINCT FROM 'string' OR (e->>'entity') !~ '^[a-z]{1,20}:[0-9]{1,3}$'
       OR jsonb_typeof(e->'yaw') IS DISTINCT FROM 'number' OR jsonb_typeof(e->'scale') IS DISTINCT FROM 'number' OR jsonb_typeof(e->'gone') IS DISTINCT FROM 'boolean'
       OR (e->>'yaw')::numeric <> trunc((e->>'yaw')::numeric) OR (e->>'yaw')::numeric NOT BETWEEN -179 AND 180
       OR (e->>'scale')::numeric <> trunc((e->>'scale')::numeric) OR (e->>'scale')::numeric NOT BETWEEN 25 AND 400 THEN RETURN NULL; END IF;
    out := coalesce((SELECT jsonb_agg(o) FROM jsonb_array_elements(out) o WHERE o->>'entity' <> e->>'entity'), '[]');
    IF (e->>'yaw')::int <> 0 OR (e->>'scale')::int <> 100 OR (e->>'gone')::boolean THEN
      out := out || jsonb_build_array(jsonb_build_object('entity', e->>'entity', 'yaw', (e->>'yaw')::int, 'scale', (e->>'scale')::int, 'gone', (e->>'gone')::boolean));
    END IF;
  END LOOP;
  RETURN out;
END $$;

-- The document: 0093's, with `shaped` when it has any.
CREATE OR REPLACE FUNCTION verse.realm_doc(m verse.made_realms) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('doc', 'aethyr.realm', 'version', 1, 'slug', m.slug, 'rev', m.rev, 'name', m.name,
    'setup', jsonb_build_object('camera', m.camera, 'style', m.style, 'ground', m.ground, 'time', m.time, 'palette', m.palette),
    'features', m.features, 'off', m.off, 'tools', m.tools, 'placed', m.placed, 'moved', m.moved, 'base', m.base,
    'forked_from', CASE WHEN m.forked_from IS NULL THEN NULL ELSE jsonb_build_object('slug', m.forked_from, 'rev', m.forked_rev) END)
  || CASE WHEN coalesce(m.shaped, '[]') <> '[]' THEN jsonb_build_object('shaped', m.shaped) ELSE '{}' END $$;

-- A kept document as a realm (0094's): a document without `shaped` has none.
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
  RETURN p;
END $$;

-- A change of shapes is a new revision, as any change to what the realm is (0093's rule).
CREATE OR REPLACE FUNCTION verse.made_realms_rev() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.rev = OLD.rev AND (NEW.name, NEW.style, NEW.ground, NEW.time, NEW.camera, NEW.palette, NEW.features, NEW.off, NEW.tools, NEW.placed, NEW.moved, NEW.shaped)
     IS DISTINCT FROM (OLD.name, OLD.style, OLD.ground, OLD.time, OLD.camera, OLD.palette, OLD.features, OLD.off, OLD.tools, OLD.placed, OLD.moved, OLD.shaped) THEN
    NEW.rev := OLD.rev + 1;
  END IF;
  RETURN NEW;
END $$;

-- SAVE (0093's): the moves, and the shapes when sent; at most 64 entities overridden in all.
CREATE OR REPLACE FUNCTION verse.realm_save(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms;
  base bigint := CASE WHEN jsonb_typeof(body->'rev') = 'number' THEN (body->>'rev')::bigint END;
  mv jsonb := verse.realm_moves(body->'moved');
  sh jsonb := CASE WHEN body ? 'shaped' THEN verse.realm_shapes(body->'shaped') END;
  sha text;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF m.slug IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF NOT verse.edit_may(s, k, site_admin) THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF base IS NULL OR mv IS NULL OR (body ? 'shaped' AND sh IS NULL) THEN RETURN '{"status":400,"error":"a revision and the moves"}'; END IF;
  IF base <> m.rev THEN RETURN jsonb_build_object('status', 409, 'error', 'changed since', 'rev', m.rev); END IF;
  sh := coalesce(sh, m.shaped);
  IF (SELECT count(DISTINCT e) FROM (SELECT x->>'entity' e FROM jsonb_array_elements(mv) x UNION ALL SELECT y->>'entity' FROM jsonb_array_elements(sh) y) z) > 64 THEN
    RETURN '{"status":400,"error":"at most 64 things moved, turned, resized or taken away"}';
  END IF;
  IF mv = m.moved AND sh = m.shaped THEN RETURN jsonb_build_object('status', 200, 'rev', m.rev, 'sha256', verse.realm_keep_rev(m, 'save', k)); END IF;
  UPDATE verse.made_realms SET moved = mv, shaped = sh, rev = m.rev + 1, settings_ms = verse.now_ms() WHERE slug = s RETURNING * INTO m;
  sha := verse.realm_keep_rev(m, 'save', k);
  RETURN jsonb_build_object('status', 200, 'rev', m.rev, 'sha256', sha);
END $$;

-- UNDO (0094's): shapes come back with the rest of the revision.
CREATE OR REPLACE FUNCTION verse.realm_undo(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
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
  IF jsonb_array_length(m.undo) = 0 THEN RETURN jsonb_build_object('status', 409, 'error', 'nothing to undo', 'rev', m.rev); END IF;
  e := m.undo -> -1;
  SELECT doc INTO d FROM verse.realm_revisions WHERE slug = s AND rev = (e->>'to')::bigint;
  PERFORM set_config('verse.restoring', 'on', true);
  IF d IS NOT NULL THEN
    p := verse.realm_as_doc(m, d);
    UPDATE verse.made_realms SET name = p.name, style = p.style, ground = p.ground, time = p.time, camera = p.camera, palette = p.palette,
           features = p.features, off = p.off, tools = p.tools, placed = p.placed, moved = p.moved, shaped = p.shaped,
           undo = m.undo - (-1), settings_ms = verse.now_ms()
    WHERE slug = s RETURNING * INTO m;
  ELSE
    UPDATE verse.made_realms SET undo = m.undo - (-1) WHERE slug = s RETURNING * INTO m;
  END IF;
  IF jsonb_typeof(e->'card') = 'number' THEN
    UPDATE verse.realm_proposals SET state = 'waiting', decided_ms = NULL WHERE id = (e->>'card')::bigint AND slug = s;
  END IF;
  PERFORM set_config('verse.restoring', '', true);
  RETURN jsonb_build_object('status', 200, 'rev', m.rev, 'undo', jsonb_array_length(m.undo));
END $$;

-- A fork of a made realm copies the shapes of the revision it was forked at (made_fork copies the
-- other fields by name). Named to run before made_realms_first, which hashes the document.
CREATE FUNCTION verse.made_realms_doc_shaped() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.base IS NULL AND NEW.forked_from IS NOT NULL AND NEW.shaped = '[]' THEN
    NEW.shaped := coalesce((SELECT doc->'shaped' FROM verse.realm_revisions WHERE slug = NEW.forked_from AND rev = NEW.forked_rev),
                           (SELECT shaped FROM verse.made_realms WHERE slug = NEW.forked_from AND rev = NEW.forked_rev AND base IS NULL), '[]');
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER made_realms_doc_shaped BEFORE INSERT ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_doc_shaped();

-- The spec: shaped lines from the document the caller reads (a visitor's: the revision published),
-- and "shapes 1".
ALTER FUNCTION verse.edit_spec(text, text, boolean) RENAME TO edit_spec_0107;
CREATE FUNCTION verse.edit_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN out IS NULL OR NOT EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND base IS NULL) THEN out
              ELSE out || coalesce((SELECT string_agg('shaped ' || (v->>'entity') || ' ' || (v->>'yaw') || ' ' || (v->>'scale') || ' '
                                                      || CASE WHEN (v->>'gone')::boolean THEN '1' ELSE '0' END, E'\n' ORDER BY o) || E'\n'
                                    FROM jsonb_array_elements(coalesce(verse.realm_doc_of(s, k, site_admin)->'doc'->'shaped', '[]')) WITH ORDINALITY AS t(v, o)), '')
                       || E'shapes 1\n' END
  FROM (SELECT verse.edit_spec_0107(s, k, site_admin) AS out) x $$;
