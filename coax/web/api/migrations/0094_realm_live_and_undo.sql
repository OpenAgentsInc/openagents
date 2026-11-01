-- Visitors see the published revision; every edit is one undo step (Aethyr book chapters 52 and 54;
-- GitHub #12610).
--
-- The owner keeps editing and saving; a visitor's spec, document and fork are the revision last
-- published. MAKE IT and a fork publish their first revision, so a new realm is public as it was
-- made, and every realm made before this migration has the revision it is at now published.
--
-- Every change to a made realm's document (a save, a row switched, a thing placed, a card
-- accepted, a revision by voice, the settings) is kept as a revision and pushed on the realm's undo
-- list: its revision before, and the card it settled, if any. UNDO puts the document back as it
-- was at that revision, as a new revision, and a card settled by that step waits again. A card
-- declined is a step too, though the document doesn't change.
--
--   POST /api/playtest/realms/<slug>/undo {rev}   -> 200 {rev, undo}; 400, 403, 404, 409 {error, rev}
--   GET  /api/playtest/realms/<slug>/doc          -> the owner's: now; anyone else's: the revision published
-- The spec gains "undo N" (steps that can be undone) for its owner and admins. Nothing here reads words.

ALTER TABLE verse.realm_revisions DROP CONSTRAINT realm_revisions_why_check;
ALTER TABLE verse.realm_revisions ADD CONSTRAINT realm_revisions_why_check CHECK (why IN ('save', 'publish', 'fork', 'made', 'edit', 'undo'));
-- A realm's kept revisions go with it (realms aren't deleted; checks clean up the ones they make).
ALTER TABLE verse.realm_revisions DROP CONSTRAINT realm_revisions_slug_fkey;
ALTER TABLE verse.realm_revisions ADD CONSTRAINT realm_revisions_slug_fkey FOREIGN KEY (slug) REFERENCES verse.made_realms (slug) ON DELETE CASCADE;
ALTER TABLE verse.made_realms ADD COLUMN undo jsonb NOT NULL DEFAULT '[]';   -- [{to: REV, card: ID|null}], the newest last, at most 64

-- A kept revision says what made it; one first kept as an edit takes the save's or publish's name.
CREATE OR REPLACE FUNCTION verse.realm_keep_rev(m verse.made_realms, why text, k text) RETURNS text LANGUAGE sql AS $$
  INSERT INTO verse.realm_revisions AS r (slug, rev, doc, sha256, ms, by_player, why)
  VALUES (m.slug, m.rev, verse.realm_doc(m), verse.realm_doc_sha(verse.realm_doc(m)), verse.now_ms(), (verse.builder_player(k)).id, why)
  ON CONFLICT (slug, rev) DO UPDATE SET why = excluded.why, by_player = coalesce(excluded.by_player, r.by_player)
    WHERE r.why = 'edit' AND excluded.why <> 'edit';
  SELECT verse.realm_doc_sha(verse.realm_doc(m)) $$;

-- Every realm's revision now is kept; a made realm never published has it published.
INSERT INTO verse.realm_revisions (slug, rev, doc, sha256, ms, why)
SELECT m.slug, m.rev, verse.realm_doc(m), verse.realm_doc_sha(verse.realm_doc(m)), verse.now_ms(), 'edit' FROM verse.made_realms m
ON CONFLICT (slug, rev) DO NOTHING;
UPDATE verse.made_realms m SET published_rev = m.rev, published_sha256 = verse.realm_doc_sha(verse.realm_doc(m))
WHERE m.base IS NULL AND m.published_rev IS NULL;

-- A new made realm (MAKE IT, a fork) has its first revision published and kept.
CREATE FUNCTION verse.made_realms_first() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.base IS NULL AND NEW.published_rev IS NULL THEN
    NEW.published_rev := NEW.rev;
    NEW.published_sha256 := verse.realm_doc_sha(verse.realm_doc(NEW));
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER made_realms_first BEFORE INSERT ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_first();
CREATE FUNCTION verse.made_realms_made() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO verse.realm_revisions (slug, rev, doc, sha256, ms, why)
  VALUES (NEW.slug, NEW.rev, verse.realm_doc(NEW), verse.realm_doc_sha(verse.realm_doc(NEW)), verse.now_ms(), 'made')
  ON CONFLICT (slug, rev) DO NOTHING;
  RETURN NULL;
END $$;
CREATE TRIGGER made_realms_made AFTER INSERT ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_made();

-- A change that made a new revision is one undo step (after made_realms_rev, by name), unless it
-- is an undo; the card it settled comes from verse.card (edit_decide sets it).
CREATE FUNCTION verse.made_realms_undo() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  st jsonb;
BEGIN
  IF NEW.rev <> OLD.rev AND coalesce(current_setting('verse.restoring', true), '') <> 'on' THEN
    st := NEW.undo || jsonb_build_array(jsonb_build_object('to', OLD.rev, 'card', nullif(coalesce(current_setting('verse.card', true), ''), '')::bigint));
    WHILE jsonb_array_length(st) > 64 LOOP st := st - 0; END LOOP;
    NEW.undo := st;
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER made_realms_undo BEFORE UPDATE ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_undo();
-- Each new revision is kept, so undo can put it back.
CREATE FUNCTION verse.made_realms_kept() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.rev <> OLD.rev THEN
    INSERT INTO verse.realm_revisions (slug, rev, doc, sha256, ms, why)
    VALUES (NEW.slug, NEW.rev, verse.realm_doc(NEW), verse.realm_doc_sha(verse.realm_doc(NEW)), verse.now_ms(),
            CASE WHEN current_setting('verse.restoring', true) = 'on' THEN 'undo' ELSE 'edit' END)
    ON CONFLICT (slug, rev) DO NOTHING;
  END IF;
  RETURN NULL;
END $$;
CREATE TRIGGER made_realms_kept AFTER UPDATE ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_kept();

-- The document a visitor sees: the revision published, when the realm has changed since; NULL
-- when that is the realm as it is now.
CREATE FUNCTION verse.realm_live_doc(m verse.made_realms) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN m.base IS NOT NULL OR m.published_rev IS NULL OR m.published_rev = m.rev THEN NULL
              ELSE (SELECT doc FROM verse.realm_revisions WHERE slug = m.slug AND rev = m.published_rev) END $$;
-- A made realm as one of its kept documents has it (its identity, owner and fork lines stay its own).
CREATE FUNCTION verse.realm_as_doc(m verse.made_realms, d jsonb) RETURNS verse.made_realms LANGUAGE plpgsql IMMUTABLE AS $$
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
  RETURN p;
END $$;

-- GET .../doc as the viewer: its owner and admins the document now, anyone else the revision published.
CREATE FUNCTION verse.realm_doc_of(s text, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT CASE WHEN live IS NOT NULL AND NOT verse.edit_may(s, k, site_admin)
                               THEN jsonb_build_object('status', 200, 'doc', live, 'sha256', verse.realm_doc_sha(live))
                               ELSE verse.realm_doc_of(s) END
                   FROM verse.made_realms m, LATERAL (SELECT verse.realm_live_doc(m) AS live) l WHERE m.slug = s),
                  '{"status":404,"error":"no such realm"}') $$;

-- A visitor's spec from the revision published: the lines builder_spec, edit_spec and 0093 write, in their order.
CREATE FUNCTION verse.realm_live_spec(m verse.made_realms, d jsonb) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  p verse.made_realms := verse.realm_as_doc(m, d);
  out text;
BEGIN
  out := 'realm ' || p.slug || E'\nname ' || p.name || E'\nstyle ' || p.style || E'\nground ' || p.ground || E'\ntime ' || p.time
      || E'\ncamera ' || p.camera || E'\nowner 0\n';
  out := out || coalesce((SELECT string_agg('tool ' || t || ' ' || CASE WHEN coalesce((p.tools->>t)::boolean, false) THEN '1' ELSE '0' END, E'\n' ORDER BY o)
                          FROM unnest(verse.builder_tools()) WITH ORDINALITY AS x(t, o)) || E'\n', '');
  out := out || coalesce((SELECT string_agg(CASE f->>'feature'
      WHEN 'character' THEN 'character ' || (f->>'name') || '|' || (f->>'role')
      ELSE 'feature ' || (f->>'feature') || concat(' ' || (f->>'size'), ' ' || (f->>'variant'), ' ' || (f->>'colour'), ' ' || (f->>'count')) END, E'\n' ORDER BY o)
    FROM jsonb_array_elements(p.features) WITH ORDINALITY AS x(f, o)) || E'\n', '');
  out := out || coalesce((SELECT string_agg('placed ' || (q->>'thing') || ' ' || trim_scale(round((q->>'x')::numeric, 2)) || ' ' || trim_scale(round((q->>'z')::numeric, 2))
                                            || ' ' || trim_scale(round((q->>'yaw')::numeric, 3)), E'\n' ORDER BY o)
                          FROM jsonb_array_elements(p.placed) WITH ORDINALITY AS x(q, o)) || E'\n', '');
  out := out || 'palette ' || p.palette || E'\nrole visitor\n';
  out := out || coalesce((SELECT string_agg('row ' || verse.edit_key(f) || ' ' || CASE WHEN (f->>'on')::boolean THEN '1' ELSE '0' END || ' ' || (f->>'label'), E'\n' ORDER BY o)
                          FROM jsonb_array_elements(verse.edit_made_board(p)) WITH ORDINALITY AS t(f, o)) || E'\n', '');
  out := out || 'rev ' || p.rev || E'\npublished ' || m.published_rev || E'\nlisted ' || CASE WHEN m.unlisted THEN '0' ELSE '1' END || E'\n'
      || CASE WHEN m.forked_from IS NOT NULL
              THEN 'from ' || m.forked_from || ' ' || coalesce(m.forked_rev, 0) || '|' || verse.builder_one_line(to_jsonb(verse.fork_name_of(m.forked_from)), 60) || E'\n' ELSE '' END
      || coalesce((SELECT string_agg('moved ' || (v->>'entity') || ' ' || to_char((v->>'x')::numeric, 'FM990.00') || ' ' || to_char((v->>'z')::numeric, 'FM990.00'), E'\n' ORDER BY o)
                   FROM jsonb_array_elements(p.moved) WITH ORDINALITY AS t(v, o)) || E'\n', '');
  RETURN out;
END $$;

ALTER FUNCTION verse.edit_spec(text, text, boolean) RENAME TO edit_spec_0093;
CREATE FUNCTION verse.edit_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  m verse.made_realms;
  live jsonb;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s AND base IS NULL;
  IF m.slug IS NULL THEN RETURN verse.edit_spec_0093(s, k, site_admin); END IF;
  IF NOT verse.edit_may(s, k, site_admin) THEN
    live := verse.realm_live_doc(m);
    RETURN CASE WHEN live IS NOT NULL THEN verse.realm_live_spec(m, live) ELSE verse.edit_spec_0093(s, k, site_admin) END;
  END IF;
  RETURN verse.edit_spec_0093(s, k, site_admin) || 'undo ' || jsonb_array_length(m.undo) || E'\n';
END $$;

-- UNDO: the newest step back, from the revision there now; a card it settled waits again.
CREATE FUNCTION verse.realm_undo(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
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
           features = p.features, off = p.off, tools = p.tools, placed = p.placed, moved = p.moved, undo = m.undo - (-1), settings_ms = verse.now_ms()
    WHERE slug = s RETURNING * INTO m;
  ELSE
    UPDATE verse.made_realms SET undo = m.undo - (-1) WHERE slug = s RETURNING * INTO m;   -- that revision wasn't kept: the step only goes
  END IF;
  IF jsonb_typeof(e->'card') = 'number' THEN
    UPDATE verse.realm_proposals SET state = 'waiting', decided_ms = NULL WHERE id = (e->>'card')::bigint AND slug = s;
  END IF;
  PERFORM set_config('verse.restoring', '', true);
  RETURN jsonb_build_object('status', 200, 'rev', m.rev, 'undo', jsonb_array_length(m.undo));
END $$;

-- ACCEPT and DECLINE are steps: an accept's revision names its card; a decline, which changes no
-- document, is pushed as a step of its own.
ALTER FUNCTION verse.edit_decide(text, bigint, boolean, text, boolean) RENAME TO edit_decide_0093;
CREATE FUNCTION verse.edit_decide(s text, pid bigint, accept boolean, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb;
  r0 bigint;
BEGIN
  SELECT rev INTO r0 FROM verse.made_realms WHERE slug = s AND base IS NULL FOR UPDATE;
  PERFORM set_config('verse.card', pid::text, true);
  out := verse.edit_decide_0093(s, pid, accept, k, site_admin);
  PERFORM set_config('verse.card', '', true);
  IF r0 IS NOT NULL AND (out->>'status') = '200' AND NOT accept THEN
    UPDATE verse.made_realms SET undo = (SELECT CASE WHEN jsonb_array_length(u) > 64 THEN u - 0 ELSE u END
                                         FROM (SELECT undo || jsonb_build_array(jsonb_build_object('to', r0, 'card', pid)) AS u) x)
    WHERE slug = s;
  END IF;
  RETURN out;
END $$;

-- FORK as a visitor: the revision published, not the owner's latest.
ALTER FUNCTION verse.made_fork(text, jsonb, text, int) RENAME TO made_fork_0093;
CREATE FUNCTION verse.made_fork(s text, body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  rec verse.players := verse.builder_player(k);
  src verse.made_realms;
  live jsonb;
  nm text;
  slug_ text;
  m verse.made_realms;
BEGIN
  SELECT * INTO src FROM verse.made_realms WHERE slug = s AND base IS NULL;
  IF src.slug IS NULL OR verse.builder_owns(k, s) THEN RETURN verse.made_fork_0093(s, body, k, per_day); END IF;
  live := verse.realm_live_doc(src);
  IF live IS NULL THEN RETURN verse.made_fork_0093(s, body, k, per_day); END IF;
  src := verse.realm_as_doc(src, live);
  IF rec.id IS NULL THEN RETURN '{"status":401,"error":"no player"}'; END IF;
  IF jsonb_typeof(body->'name') = 'string' THEN
    nm := btrim(regexp_replace(verse.builder_one_line(body->'name', 40), '[<>&"]', '', 'g'));
    IF (body->>'name') <> '' AND nm = '' THEN RETURN '{"status":400,"error":"a name of plain words"}'; END IF;
  END IF;
  IF (SELECT count(*) FROM verse.made_realms x JOIN verse.worlds w ON w.id = x.world
      WHERE w.owner_player = rec.id AND x.made_ms > verse.now_ms() - 86400000) >= per_day THEN
    RETURN '{"status":429,"error":"that''s today''s realms"}';
  END IF;
  nm := coalesce(nullif(nm, ''), left(src.name, 34) || ' Fork');
  slug_ := verse.builder_slug(nm);
  INSERT INTO verse.worlds (id, owner_player, owner_account, created_ms) VALUES ('made-' || slug_, rec.id, rec.account, verse.now_ms());
  INSERT INTO verse.made_realms (slug, world, name, style, ground, time, camera, palette, features, off, tools, placed, moved, made_ms, forked_from, forked_rev)
  VALUES (slug_, 'made-' || slug_, nm, src.style, src.ground, src.time, src.camera, src.palette, src.features, src.off, src.tools, src.placed, src.moved,
          verse.now_ms(), src.slug, src.rev)
  RETURNING * INTO m;
  PERFORM verse.realm_keep_rev(m, 'made', k);
  RETURN jsonb_build_object('status', 200, 'slug', slug_, 'path', '/realms/' || slug_, 'name', nm, 'from', src.slug, 'rev', src.rev);
END $$;
