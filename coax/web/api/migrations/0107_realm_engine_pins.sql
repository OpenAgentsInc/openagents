-- A published made realm plays the engine build and assets it was published with (GitHub #12629,
-- Aethyr book chapter 54, docs/verse/aethyrforge.md slice 8).
--
-- Until now publication froze the document revision but the realm's page played whatever engine
-- /realms/built served that day: a later release silently changed every published realm. The
-- engine a made realm plays is /realms/built's app script, whose files (host script, wasm,
-- shaders, voice engine, render package, asset packs) are named by their content and never deleted
-- from the site's bucket (coax/web/deploy.sh, pin.sh). Publishing now reads that script from the
-- bucket and keeps it, as an engine manifest (aethyr/forge/publish.coax has the format and the
-- same rule, as an independent check), beside the revision it froze:
--
--   aethyr-engine 1
--   NAME VALUE          one line an attribute, in the page's order, white space runs one space
--
-- A manifest names nothing that isn't named by its content; its SHA-256 is the engine's identity.
-- The realm's page carries the manifest (data-engine) and builds the engine's script from it, so
-- a visitor gets the same bytes after any later release; ?engine=current tries today's engine.
--
--   POST .../<slug>/publish and .../edit-v1/publish   -> 200 {..., engine: SHA256 | null}; 503 when
--        the site has a bucket and its engine can't be read or named by content: nothing is published
--   GET  .../<slug>/published                         -> 200 {slug, n, rev, sha256, engine, engine_sha256, why, ms}
--        (the latest publish record); 404 when it was never pinned
-- The spec gains "engine SHA256" for a pinned made realm. A fork starts on its parent's engine; a
-- made realm's first revision (MAKE IT, a fork without a pinned parent) and every realm published
-- before this migration are pinned to the engine served when its page is next written (realms.c).
-- Built-in realms' forks (base set) are hand-built engines and are not pinned here.

-- ---- the rule: a manifest from a page, or NULL (aethyr/forge/publish.coax, engine_manifest)
CREATE FUNCTION verse.engine_path_ok(p text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(length(p) BETWEEN 2 AND 200 AND p ~ '^/[a-z0-9._/-]+$' AND p !~ '//' AND right(p, 1) <> '/'
         AND p !~ '/\.\.?(/|$)' AND p ~ '/([^/]+\.[0-9a-f]{12}\.[^/]+|[0-9a-f]{64})(/|$)', false) $$;
CREATE FUNCTION verse.engine_word_ok(w text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN w IS NULL OR w = '' THEN false
              WHEN strpos(w, '/') = 0 THEN w ~ '^[A-Za-z0-9._:=-]+$'
              ELSE (strpos(w, '/') = 1 OR left(w, strpos(w, '/') - 1) ~ '^[a-z0-9_:=-]*[:=]$')
                   AND NOT EXISTS (SELECT 1 FROM unnest(string_to_array(substr(w, strpos(w, '/')), '|')) x(p) WHERE NOT verse.engine_path_ok(p)) END $$;
CREATE FUNCTION verse.engine_value_ok(v text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT v IS NOT NULL AND v <> '' AND NOT EXISTS (SELECT 1 FROM unnest(string_to_array(v, ' ')) x(w) WHERE NOT verse.engine_word_ok(w)) $$;
CREATE FUNCTION verse.engine_manifest(t text) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  open text := '<script type="module" src="';
  rest text := t;
  at int;
  e int;
  tag text;
  found text;
  a text[];
  v text;
  names text[] := '{}';
  out text := E'aethyr-engine 1\n';
  has_src boolean := false;
  has_wasm boolean := false;
  has_wgsl boolean := false;
BEGIN
  IF t IS NULL THEN RETURN NULL; END IF;
  LOOP   -- every script that opens like the engine's; exactly one of them is myrealm's
    at := strpos(rest, open);
    EXIT WHEN at = 0;
    e := strpos(substr(rest, at), '>');
    IF e = 0 THEN RETURN NULL; END IF;
    tag := substr(rest, at, e - 1);
    IF strpos(tag, ' data-app="myrealm"') > 0 THEN
      IF found IS NOT NULL THEN RETURN NULL; END IF;
      found := substr(tag, 8);
    END IF;
    rest := substr(rest, at + 1);
  END LOOP;
  IF found IS NULL OR found !~ E'^([ \n\t\r]*[^ \n\t\r=]+="[^"]*")*[ \n\t\r]*$' THEN RETURN NULL; END IF;
  FOR a IN SELECT regexp_matches(found, E'([^ \n\t\r=]+)="([^"]*)"', 'g') LOOP
    IF a[1] !~ '^[a-z][a-z-]{0,31}$' OR a[1] = ANY (names) OR cardinality(names) >= 24 THEN RETURN NULL; END IF;
    names := names || a[1];
    v := btrim(regexp_replace(a[2], E'[ \n\t\r]+', ' ', 'g'), ' ');
    IF NOT verse.engine_value_ok(v) THEN RETURN NULL; END IF;
    IF a[1] = 'src' THEN has_src := v ~ '^/coaxial/host\.[0-9a-f]{12}\.js$'; END IF;
    IF a[1] = 'data-app' AND v <> 'myrealm' THEN RETURN NULL; END IF;
    IF a[1] = 'data-wasm' THEN has_wasm := verse.engine_path_ok(v); END IF;
    IF a[1] = 'data-wgsl' THEN has_wgsl := verse.engine_path_ok(v); END IF;
    out := out || a[1] || ' ' || v || E'\n';
  END LOOP;
  IF NOT (has_src AND has_wasm AND has_wgsl) OR octet_length(out) > 4096 THEN RETURN NULL; END IF;
  RETURN out;
END $$;

-- ---- the record
ALTER TABLE verse.made_realms ADD COLUMN published_engine text;          -- the manifest it plays; NULL: today's engine
ALTER TABLE verse.made_realms ADD COLUMN published_engine_sha256 text;
-- Every pin, in order: what each publication froze. Nothing here is rewritten.
CREATE TABLE verse.realm_publications (
  slug text NOT NULL REFERENCES verse.made_realms (slug) ON DELETE CASCADE,
  n int NOT NULL,                       -- 1, 2, ...: this realm's records in order
  rev bigint NOT NULL,                  -- the document revision published
  doc_sha256 text NOT NULL,
  engine text NOT NULL,
  engine_sha256 text NOT NULL,
  why text NOT NULL CHECK (why IN ('publish', 'first', 'fork')),
  ms bigint NOT NULL,
  PRIMARY KEY (slug, n)
);

-- Pin a made realm's published revision to an engine manifest, and record it. NULL when it has no
-- published revision (or isn't a made realm).
CREATE FUNCTION verse.realm_pin(s text, engine text, why text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE
  sha text := encode(sha256(convert_to(engine, 'UTF8')), 'hex');
  m verse.made_realms;
BEGIN
  UPDATE verse.made_realms SET published_engine = engine, published_engine_sha256 = sha
  WHERE slug = s AND base IS NULL AND published_rev IS NOT NULL RETURNING * INTO m;
  IF m.slug IS NULL THEN RETURN NULL; END IF;
  INSERT INTO verse.realm_publications (slug, n, rev, doc_sha256, engine, engine_sha256, why, ms)
  VALUES (s, coalesce((SELECT max(n) FROM verse.realm_publications WHERE slug = s), 0) + 1,
          m.published_rev, m.published_sha256, engine, sha, why, verse.now_ms());
  RETURN sha;
END $$;

-- PUBLISH, pinned: template is the engine's page as the site serves it now, or NULL when this server
-- has no site bucket (a dev server: the realm then plays the current engine, as before). With a
-- bucket, a page that can't be read ('') or named by content refuses the whole publish (503), and
-- nothing it would have changed is kept.
CREATE FUNCTION verse.realm_publish_pinned(s text, body jsonb, k text, site_admin boolean, edit_v1 boolean, template text)
RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  made boolean := EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND base IS NULL);
  engine text := verse.engine_manifest(template);
  out jsonb;
BEGIN
  BEGIN
    out := CASE WHEN edit_v1 THEN verse.realm_edit_call(s, body, k, site_admin, 'publish', '') ELSE verse.fork_publish(s, body, k, site_admin) END;
    IF NOT made OR out->>'status' <> '200' THEN RETURN out; END IF;
    IF template IS NULL THEN
      UPDATE verse.made_realms SET published_engine = NULL, published_engine_sha256 = NULL WHERE slug = s;
      RETURN out || '{"engine":null}';
    END IF;
    IF engine IS NULL THEN RAISE EXCEPTION 'engine not pinned' USING ERRCODE = 'OA107'; END IF;
    RETURN out || jsonb_build_object('engine', verse.realm_pin(s, engine, 'publish'));
  EXCEPTION WHEN SQLSTATE 'OA107' THEN
    RETURN '{"status":503,"error":"the engine build can''t be pinned; nothing was published"}';
  END;
END $$;

-- Does writing its page need the engine read first: a made realm published but not pinned.
CREATE FUNCTION verse.realm_needs_pin(s text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND base IS NULL AND published_rev IS NOT NULL AND published_engine IS NULL) $$;
-- Its first pin: a made realm's first revision, or one published before pins (to the engine served
-- now). False when it needs none or the page doesn't name an engine by content.
CREATE FUNCTION verse.realm_pin_first(s text, template text) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE
  engine text := verse.engine_manifest(template);
BEGIN
  IF engine IS NULL OR NOT verse.realm_needs_pin(s) THEN RETURN false; END IF;
  RETURN verse.realm_pin(s, engine, 'first') IS NOT NULL;
END $$;

-- A fork of a made realm starts on its parent's engine: the build its parent's visitors play.
CREATE FUNCTION verse.made_realms_engine_from() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.base IS NULL AND NEW.forked_from IS NOT NULL AND NEW.published_engine IS NULL THEN
    SELECT published_engine, published_engine_sha256 INTO NEW.published_engine, NEW.published_engine_sha256
    FROM verse.made_realms WHERE slug = NEW.forked_from AND base IS NULL;
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER made_realms_engine_from BEFORE INSERT ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_engine_from();
CREATE FUNCTION verse.made_realms_engine_kept() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.published_engine IS NOT NULL AND NEW.published_rev IS NOT NULL THEN
    INSERT INTO verse.realm_publications (slug, n, rev, doc_sha256, engine, engine_sha256, why, ms)
    VALUES (NEW.slug, 1, NEW.published_rev, NEW.published_sha256, NEW.published_engine, NEW.published_engine_sha256, 'fork', verse.now_ms());
  END IF;
  RETURN NULL;
END $$;
CREATE TRIGGER made_realms_engine_kept AFTER INSERT ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_engine_kept();

-- GET .../published: the latest record (anyone who may see the realm; realms.c hides a private one).
CREATE FUNCTION verse.realm_published_of(s text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT jsonb_build_object('status', 200, 'slug', p.slug, 'n', p.n, 'rev', p.rev, 'sha256', p.doc_sha256,
                                             'engine', p.engine, 'engine_sha256', p.engine_sha256, 'why', p.why, 'ms', p.ms)
                   FROM verse.realm_publications p JOIN verse.made_realms m ON m.slug = p.slug
                   WHERE p.slug = s AND m.published_engine_sha256 = p.engine_sha256 AND m.published_rev = p.rev
                   ORDER BY p.n DESC LIMIT 1),
                  '{"status":404,"error":"not pinned"}') $$;

-- The spec gains "engine SHA256" for a pinned made realm.
ALTER FUNCTION verse.edit_spec(text, text, boolean) RENAME TO edit_spec_0099;
CREATE FUNCTION verse.edit_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN out IS NULL THEN NULL
              ELSE out || coalesce((SELECT 'engine ' || m.published_engine_sha256 || E'\n' FROM verse.made_realms m
                                    WHERE m.slug = s AND m.base IS NULL AND m.published_engine_sha256 IS NOT NULL), '') END
  FROM (SELECT verse.edit_spec_0099(s, k, site_admin) AS out) x $$;

-- ---- the page: the engine's script from its pin, else today's (/realms/built)
ALTER FUNCTION verse.builder_page(text, text) RENAME TO builder_page_0093;
CREATE FUNCTION verse.builder_page(s text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN m.base IS NOT NULL THEN p
              ELSE replace(replace(p,
                     '<script type="module" data-made="' || m.slug || '"',
                     '<script type="module" data-made="' || m.slug || '"'
                       || CASE WHEN m.published_engine IS NOT NULL
                               THEN ' data-engine="' || verse.builder_html(m.published_engine) || '" data-engine-sha256="' || m.published_engine_sha256 || '"'
                               ELSE '' END),
                   E'const t = await (await fetch("/realms/built", { cache: "no-store" })).text();\n'
                   || E'const from = new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]");\n'
                   || E'const s = document.createElement("script");\n'
                   || E'for (const a of from.attributes) s.setAttribute(a.name, a.value);\n',
                   E'const pin = me.dataset.engine, now = new URLSearchParams(location.search).get("engine") === "current";\n'
                   || E'let attrs;\n'
                   || E'if (pin && !now) attrs = pin.split("\\n").slice(1).filter(Boolean).map((l) => [l.slice(0, l.indexOf(" ")), l.slice(l.indexOf(" ") + 1)]);\n'
                   || E'else {\n'
                   || E'  const t = await (await fetch("/realms/built", { cache: "no-store" })).text();\n'
                   || E'  attrs = [...new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]").attributes].map((a) => [a.name, a.value]);\n'
                   || E'}\n'
                   || E'const s = document.createElement("script");\n'
                   || E'for (const [n, v] of attrs) s.setAttribute(n, v);\n') END
  FROM verse.made_realms m, LATERAL (SELECT verse.builder_page_0093(s, origin) AS p) x WHERE m.slug = s $$;
-- Every made realm's page is written again with this template (realms_refresh_pages), and pinned first.
CREATE OR REPLACE FUNCTION verse.builder_page_rev() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 2 $$;
