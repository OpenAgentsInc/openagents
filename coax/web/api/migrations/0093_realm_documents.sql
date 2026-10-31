-- A made realm is a document with an identity (Aethyr book chapters 51, 52, 54, 55 and 82; GitHub
-- #12610). Every change to what it is (its setup, features, rows, tools, placed things, moved
-- entities, name) is a new revision (`rev`, counted by a trigger). A save names the revision it
-- was made from, and a stale one is refused (409) with the revision that is there now: never
-- "newer wins". A publish names its revision too and records the document's hash. A fork of a
-- made realm records its parent and the parent's revision. Nothing here reads words.
--
--   GET  /api/playtest/realms/<slug>/doc                  -> {doc, sha256}: the document now; 404
--   POST /api/playtest/realms/<slug>/save {rev, moved: [{entity, x, z}]}
--        -> 200 {rev, sha256}; 400, 403 only its owner and admins, 404, 409 {error, rev} changed since
--   POST /api/playtest/realms/<slug>/publish {rev, listed?}   (a made realm; a fork of a built-in realm is 0075's)
--        -> 200 {published, rev, sha256, listed}; 400, 403, 404, 409 {error, rev}
--   POST /api/playtest/realms/<slug>/fork {name?}         (a made realm: its document, copied)
--        -> 200 {slug, path, name, from, rev}; 400, 401, 404, 429
-- The spec (edit_spec) gains, for a made realm: "rev N", "published N" (0: never), "listed 0|1",
-- "from SLUG REV|NAME" (a fork's parent), "moved ENTITY X Z" (metres).
-- A realm made or forked by a request with an automation mark (realms.c) is unlisted: off /og's
-- rows and noindex, so test realms never show as visitors' work.
-- Additive: columns, a table, a trigger, renamed functions behind dispatchers, new functions.

ALTER TABLE verse.made_realms ADD COLUMN rev bigint NOT NULL DEFAULT 1;
ALTER TABLE verse.made_realms ADD COLUMN moved jsonb NOT NULL DEFAULT '[]';    -- [{entity, x, z}]: where a laid-out entity stands instead
ALTER TABLE verse.made_realms ADD COLUMN forked_rev bigint;                    -- the parent's revision when it was forked
ALTER TABLE verse.made_realms ADD COLUMN unlisted boolean NOT NULL DEFAULT false;
ALTER TABLE verse.made_realms ADD COLUMN published_rev bigint;
ALTER TABLE verse.made_realms ADD COLUMN published_sha256 text;

-- Each saved, published or forked revision's document, kept as it was.
CREATE TABLE verse.realm_revisions (
  slug text NOT NULL REFERENCES verse.made_realms (slug),
  rev bigint NOT NULL,
  doc jsonb NOT NULL,
  sha256 text NOT NULL,
  ms bigint NOT NULL,
  by_player text,
  why text NOT NULL CHECK (why IN ('save', 'publish', 'fork', 'made')),
  PRIMARY KEY (slug, rev)
);

-- A new revision whenever what the realm is changes, unless the change names its own.
CREATE FUNCTION verse.made_realms_rev() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.rev = OLD.rev AND (NEW.name, NEW.style, NEW.ground, NEW.time, NEW.camera, NEW.palette, NEW.features, NEW.off, NEW.tools, NEW.placed, NEW.moved)
     IS DISTINCT FROM (OLD.name, OLD.style, OLD.ground, OLD.time, OLD.camera, OLD.palette, OLD.features, OLD.off, OLD.tools, OLD.placed, OLD.moved) THEN
    NEW.rev := OLD.rev + 1;
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER made_realms_rev BEFORE UPDATE ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_rev();

-- The document: what the realm is, nothing about who visited. Its hash is of this text.
CREATE FUNCTION verse.realm_doc(m verse.made_realms) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('doc', 'aethyr.realm', 'version', 1, 'slug', m.slug, 'rev', m.rev, 'name', m.name,
    'setup', jsonb_build_object('camera', m.camera, 'style', m.style, 'ground', m.ground, 'time', m.time, 'palette', m.palette),
    'features', m.features, 'off', m.off, 'tools', m.tools, 'placed', m.placed, 'moved', m.moved, 'base', m.base,
    'forked_from', CASE WHEN m.forked_from IS NULL THEN NULL ELSE jsonb_build_object('slug', m.forked_from, 'rev', m.forked_rev) END) $$;
CREATE FUNCTION verse.realm_doc_sha(d jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT encode(sha256(convert_to(d::text, 'UTF8')), 'hex') $$;
CREATE FUNCTION verse.realm_keep_rev(m verse.made_realms, why text, k text) RETURNS text LANGUAGE sql AS $$
  INSERT INTO verse.realm_revisions (slug, rev, doc, sha256, ms, by_player, why)
  VALUES (m.slug, m.rev, verse.realm_doc(m), verse.realm_doc_sha(verse.realm_doc(m)), verse.now_ms(), (verse.builder_player(k)).id, why)
  ON CONFLICT (slug, rev) DO NOTHING;
  SELECT verse.realm_doc_sha(verse.realm_doc(m)) $$;

-- GET .../doc -> {status, doc, sha256}
CREATE FUNCTION verse.realm_doc_of(s text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT jsonb_build_object('status', 200, 'doc', verse.realm_doc(m), 'sha256', verse.realm_doc_sha(verse.realm_doc(m)))
                   FROM verse.made_realms m WHERE m.slug = s), '{"status":404,"error":"no such realm"}') $$;

-- The moves a save sends, held: at most 64, each a feature:index name and a place within 40 m,
-- rounded to the centimetre, one per entity (the last wins within one save). NULL: not moves.
CREATE FUNCTION verse.realm_moves(v jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  out jsonb := '[]';
  e jsonb;
BEGIN
  IF jsonb_typeof(v) <> 'array' OR jsonb_array_length(v) > 64 THEN RETURN NULL; END IF;
  FOR e IN SELECT x FROM jsonb_array_elements(v) x LOOP
    IF jsonb_typeof(e) <> 'object' OR jsonb_typeof(e->'entity') <> 'string' OR (e->>'entity') !~ '^[a-z]{1,20}:[0-9]{1,3}$'
       OR jsonb_typeof(e->'x') <> 'number' OR jsonb_typeof(e->'z') <> 'number'
       OR abs((e->>'x')::numeric) > 40 OR abs((e->>'z')::numeric) > 40 THEN RETURN NULL; END IF;
    out := coalesce((SELECT jsonb_agg(o) FROM jsonb_array_elements(out) o WHERE o->>'entity' <> e->>'entity'), '[]')
        || jsonb_build_array(jsonb_build_object('entity', e->>'entity', 'x', round((e->>'x')::numeric, 2), 'z', round((e->>'z')::numeric, 2)));
  END LOOP;
  RETURN out;
END $$;

-- SAVE: only from the revision that is there now.
CREATE FUNCTION verse.realm_save(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms;
  base bigint := CASE WHEN jsonb_typeof(body->'rev') = 'number' THEN (body->>'rev')::bigint END;
  mv jsonb := verse.realm_moves(body->'moved');
  sha text;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF m.slug IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF NOT verse.edit_may(s, k, site_admin) THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF base IS NULL OR mv IS NULL THEN RETURN '{"status":400,"error":"a revision and the moves"}'; END IF;
  IF base <> m.rev THEN RETURN jsonb_build_object('status', 409, 'error', 'changed since', 'rev', m.rev); END IF;
  IF mv = m.moved THEN RETURN jsonb_build_object('status', 200, 'rev', m.rev, 'sha256', verse.realm_keep_rev(m, 'save', k)); END IF;
  UPDATE verse.made_realms SET moved = mv, rev = m.rev + 1, settings_ms = verse.now_ms() WHERE slug = s RETURNING * INTO m;
  sha := verse.realm_keep_rev(m, 'save', k);
  RETURN jsonb_build_object('status', 200, 'rev', m.rev, 'sha256', sha);
END $$;

-- PUBLISH a made realm: the revision it names, its hash kept; listed or not as asked (else as it was).
CREATE FUNCTION verse.made_publish(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms;
  at bigint := CASE WHEN jsonb_typeof(body->'rev') = 'number' THEN (body->>'rev')::bigint END;
  sha text;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF m.slug IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF NOT verse.edit_may(s, k, site_admin) THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF at IS NULL OR (body ? 'listed' AND jsonb_typeof(body->'listed') <> 'boolean') THEN RETURN '{"status":400,"error":"a revision"}'; END IF;
  IF at <> m.rev THEN RETURN jsonb_build_object('status', 409, 'error', 'changed since', 'rev', m.rev); END IF;
  sha := verse.realm_keep_rev(m, 'publish', k);
  UPDATE verse.made_realms SET published_rev = m.rev, published_sha256 = sha, published_ms = verse.now_ms(),
         unlisted = CASE WHEN body ? 'listed' THEN NOT (body->>'listed')::boolean ELSE unlisted END
  WHERE slug = s RETURNING * INTO m;
  RETURN jsonb_build_object('status', 200, 'published', true, 'rev', m.published_rev, 'sha256', sha, 'listed', NOT m.unlisted);
END $$;

ALTER FUNCTION verse.fork_publish(text, jsonb, text, boolean) RENAME TO fork_publish_0075;
CREATE FUNCTION verse.fork_publish(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT CASE WHEN EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND base IS NULL) THEN verse.made_publish(s, body, k, site_admin)
              ELSE verse.fork_publish_0075(s, body, k, site_admin) END $$;

-- FORK a made realm: a new made realm whose document is the parent's as it is now, its parent and
-- that revision recorded. The parent is only read.
CREATE FUNCTION verse.made_fork(s text, body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  rec verse.players := verse.builder_player(k);
  src verse.made_realms;
  nm text;
  slug_ text;
  m verse.made_realms;
BEGIN
  SELECT * INTO src FROM verse.made_realms WHERE slug = s AND base IS NULL;
  IF src.slug IS NULL THEN RETURN '{"status":404,"error":"this realm can''t be forked"}'; END IF;
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
  PERFORM verse.realm_keep_rev(src, 'fork', k);   -- the parent's revision it was forked at, kept
  PERFORM verse.realm_keep_rev(m, 'made', k);
  RETURN jsonb_build_object('status', 200, 'slug', slug_, 'path', '/realms/' || slug_, 'name', nm, 'from', src.slug, 'rev', src.rev);
END $$;

ALTER FUNCTION verse.fork_make(text, jsonb, text, int) RENAME TO fork_make_0075;
CREATE FUNCTION verse.fork_make(s text, body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE sql AS $$
  SELECT CASE WHEN EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND base IS NULL) THEN verse.made_fork(s, body, k, per_day)
              ELSE verse.fork_make_0075(s, body, k, per_day) END $$;

-- The spec: a made realm's document lines after 0050's.
ALTER FUNCTION verse.edit_spec(text, text, boolean) RENAME TO edit_spec_0092;
CREATE FUNCTION verse.edit_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  out text := verse.edit_spec_0092(s, k, site_admin);
  m verse.made_realms;
BEGIN
  IF out IS NULL THEN RETURN NULL; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NULL THEN RETURN out; END IF;
  RETURN out || 'rev ' || m.rev || E'\npublished ' || coalesce(m.published_rev, 0) || E'\nlisted ' || CASE WHEN m.unlisted THEN '0' ELSE '1' END || E'\n'
      || CASE WHEN m.forked_from IS NOT NULL AND m.base IS NULL
              THEN 'from ' || m.forked_from || ' ' || coalesce(m.forked_rev, 0) || '|' || verse.builder_one_line(to_jsonb(verse.fork_name_of(m.forked_from)), 60) || E'\n' ELSE '' END
      || coalesce((SELECT string_agg('moved ' || (v->>'entity') || ' ' || to_char((v->>'x')::numeric, 'FM990.00') || ' ' || to_char((v->>'z')::numeric, 'FM990.00'), E'\n' ORDER BY o)
                   FROM jsonb_array_elements(m.moved) WITH ORDINALITY AS t(v, o)) || E'\n', '');
END $$;

-- An unlisted realm is off /og's rows (0075's, without them); a page says noindex, and a fork of a
-- made realm names its parent.
CREATE OR REPLACE FUNCTION verse.builder_og_rows(n int) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(
    '/realms/' || m.slug || '|made||' || verse.builder_line(to_jsonb(m.name), 120) || '|' || verse.builder_line(to_jsonb(verse.builder_share_line(m)), 240)
    || '|https://openagents.com/realms/' || m.slug || '|' || verse.builder_card_path(m) || '|' || verse.builder_line(to_jsonb(m.name), 120)
    || '|' || CASE WHEN m.card_ms IS NULL THEN 'default' ELSE 'made' END || '|1200|630|' || coalesce(m.card_kb, 0)
    || '|' || CASE WHEN m.card_ms IS NULL THEN 'no picture of its own yet (its owner''s next visit takes one)' ELSE '' END
    || '|' || CASE WHEN m.description IS NULL THEN 'line written from what is in it' ELSE '' END || '|', E'\n' ORDER BY m.made_ms DESC) || E'\n', '')
  FROM (SELECT * FROM verse.made_realms WHERE (base IS NULL OR published_ms IS NOT NULL) AND NOT unlisted ORDER BY made_ms DESC LIMIT greatest(least(n, 200), 1)) m $$;

ALTER FUNCTION verse.builder_page(text, text) RENAME TO builder_page_0075;
CREATE FUNCTION verse.builder_page(s text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN m.base IS NOT NULL THEN p
              ELSE replace(replace(p,
                     E'<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">\n',
                     E'<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">\n' || CASE WHEN m.unlisted THEN E'<meta name="robots" content="noindex">\n' ELSE '' END),
                   E'</h1>\n',
                   E'</h1>\n' || CASE WHEN m.forked_from IS NOT NULL
                                      THEN '<p>Forked from <a href="/realms/' || m.forked_from || '">' || verse.builder_html(verse.fork_name_of(m.forked_from)) || '</a> at revision ' || coalesce(m.forked_rev, 0) || E'.</p>\n'
                                      ELSE '' END) END
  FROM verse.made_realms m, LATERAL (SELECT verse.builder_page_0075(s, origin) AS p) x WHERE m.slug = s $$;

ALTER FUNCTION verse.builder_md(text) RENAME TO builder_md_0075;
CREATE FUNCTION verse.builder_md(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.builder_md_0075(s)
      || CASE WHEN m.base IS NULL AND m.forked_from IS NOT NULL
              THEN E'\nForked from ' || verse.fork_name_of(m.forked_from) || ' (https://openagents.com/realms/' || m.forked_from || ') at revision ' || coalesce(m.forked_rev, 0) || E'.\n' ELSE '' END
  FROM verse.made_realms m WHERE m.slug = s $$;
