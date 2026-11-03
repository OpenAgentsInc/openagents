-- Engine pins (migrations/0107_realm_engine_pins.sql, GitHub #12629) against PostgreSQL with every
-- migration applied. One transaction, rolled back: nothing it makes is kept. Each check raises on
-- failure; the run prints "ok ..." lines. Run by realm_engine_pin.coax:
--   PGURL=... PSQL=psql coax/web/tool.sh coax/web/api/tests/realm_engine_pin.coax
\set ON_ERROR_STOP 1
BEGIN;
-- The template as the live site served it on 2026-11-02; aethyr/forge/publish.coax keeps the same
-- fixture (ENGINE_FIXTURE) and its manifest's SHA-256 (ENGINE_FIXTURE_SHA), checked there with
-- the Coax rule and sha256sum: the two rules agree on it.
CREATE TEMP TABLE pin_fixture (k text PRIMARY KEY, t text) ON COMMIT DROP;
INSERT INTO pin_fixture VALUES ('a', E'<!doctype html>\n<html><body>\n<canvas id="coaxial"></canvas>\n<link rel="preload" href="/build/myrealm.0e7b9526d8a2.wasm" as="fetch" crossorigin><script type="module" src="/coaxial/host.9ca333f3d8b0.js" data-app="myrealm" data-realm="built" data-slot="built" data-voice="made-realm" data-style="lowpoly" data-clock="fixed:0.5" data-assets="\n  pack:/build/realms/fixture/assets/fixture.df9c027378c8.pack" data-voice-wasm="/build/voice.71d76f28966b.wasm" data-wasm="/build/myrealm.0e7b9526d8a2.wasm" data-wgsl="/build/myrealm.224131021ece.wgsl"></script>\n</body>\n</html>\n');
-- Two later releases: a new engine wasm, then new shaders.
INSERT INTO pin_fixture SELECT 'b', replace(t, 'myrealm.0e7b9526d8a2.wasm', 'myrealm.111111111111.wasm') FROM pin_fixture WHERE k = 'a';
INSERT INTO pin_fixture SELECT 'c', replace(t, 'myrealm.224131021ece.wgsl', 'myrealm.222222222222.wgsl') FROM pin_fixture WHERE k = 'b';
CREATE FUNCTION pg_temp.fx(k text) RETURNS text LANGUAGE sql AS $$ SELECT t FROM pin_fixture WHERE pin_fixture.k = fx.k $$;
CREATE FUNCTION pg_temp.sha(t text) RETURNS text LANGUAGE sql AS $$ SELECT encode(sha256(convert_to(t, 'UTF8')), 'hex') $$;

DO $$ BEGIN
  IF pg_temp.sha(verse.engine_manifest(pg_temp.fx('a'))) <> '1592e30f8f480ae5f99954d1733f6f57cb1b139b6d0d449ad6098e5e2336b006' THEN
    RAISE EXCEPTION 'the fixture''s manifest is not the one aethyr/forge/publish.coax derives: %', verse.engine_manifest(pg_temp.fx('a'));
  END IF;
  IF verse.engine_manifest(replace(pg_temp.fx('a'), E'data-assets="\n  pack:', 'data-assets="pack:')) <> verse.engine_manifest(pg_temp.fx('a')) THEN
    RAISE EXCEPTION 'white space in a value changed the manifest';
  END IF;
END $$;
SELECT 'ok the server''s manifest of the fixture is the Coax rule''s, byte for byte';

DO $$
DECLARE
  bad text[] := ARRAY[
    replace(pg_temp.fx('a'), 'data-wasm="/build/myrealm.0e7b9526d8a2.wasm"', 'data-wasm="/build/myrealm.wasm"'),
    replace(pg_temp.fx('a'), 'fixture.df9c027378c8.pack', 'fixture.pack'),
    replace(pg_temp.fx('a'), 'src="/coaxial/host.9ca333f3d8b0.js"', 'src="/coaxial/host.js"'),
    replace(pg_temp.fx('a'), 'src="/coaxial/host.9ca333f3d8b0.js"', 'src="https://evil.example/host.9ca333f3d8b0.js"'),
    replace(pg_temp.fx('a'), ' data-wgsl="/build/myrealm.224131021ece.wgsl"', ''),
    replace(pg_temp.fx('a'), ' data-wasm="/build/myrealm.0e7b9526d8a2.wasm"', ''),
    replace(pg_temp.fx('a'), 'data-voice="made-realm"', 'data-voice="made-realm" data-voice="made-realm"'),
    replace(pg_temp.fx('a'), 'data-voice="made-realm"', 'data-voice=made-realm'),
    replace(pg_temp.fx('a'), 'data-voice="made-realm"', 'data-voice=""'),
    replace(pg_temp.fx('a'), 'data-voice="made-realm"', 'data-voice crossorigin'),
    replace(pg_temp.fx('a'), 'data-voice="made-realm"', 'Data-voice="made-realm"'),
    replace(pg_temp.fx('a'), 'pack:/build/realms/fixture/assets/fixture.df9c027378c8.pack', 'pack:/build/../fixture.df9c027378c8.pack'),
    replace(pg_temp.fx('a'), '></script>' || E'\n</body>', ''),
    replace(pg_temp.fx('a'), '</body>', '<script type="module" src="/coaxial/host.9ca333f3d8b0.js" data-app="myrealm" data-wasm="/build/myrealm.0e7b9526d8a2.wasm" data-wgsl="/build/myrealm.224131021ece.wgsl"></script></body>'),
    '<html>no engine</html>', ''];
  i int;
BEGIN
  FOR i IN 1..cardinality(bad) LOOP
    IF verse.engine_manifest(bad[i]) IS NOT NULL THEN RAISE EXCEPTION 'template % should be refused', i; END IF;
  END LOOP;
  IF verse.engine_manifest(NULL) IS NOT NULL THEN RAISE EXCEPTION 'no template, no manifest'; END IF;
  IF verse.engine_manifest(replace(pg_temp.fx('a'), '<link', '<script type="module" src="/coaxial/host.9ca333f3d8b0.js" data-app="splash"></script><link'))
     IS DISTINCT FROM verse.engine_manifest(pg_temp.fx('a')) THEN RAISE EXCEPTION 'another app''s script is not the engine'; END IF;
END $$;
SELECT 'ok a template is refused when its engine is not named by content';

-- A made realm and its owner.
INSERT INTO verse.players (id, key_sha256, created_ms) VALUES ('p-engine-pin', encode(sha256(convert_to('engine-pin-player-key-0001', 'UTF8')), 'hex'), 1),
  ('p-engine-pin-2', encode(sha256(convert_to('engine-pin-player-key-0002', 'UTF8')), 'hex'), 1);
INSERT INTO verse.worlds (id, owner_player, created_ms) VALUES ('made-engine-pin', 'p-engine-pin', 1);
INSERT INTO verse.made_realms (slug, world, name, style, ground, time, features, tools, made_ms) VALUES ('engine-pin', 'made-engine-pin', 'Engine Pin', 'lowpoly', 'grass', 'day', '[]', '{}', 1);

DO $$
DECLARE
  m verse.made_realms;
  page text;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'engine-pin';
  IF m.published_rev IS DISTINCT FROM 1 OR m.published_engine IS NOT NULL OR NOT verse.realm_needs_pin('engine-pin') THEN RAISE EXCEPTION 'a new realm: published, not yet pinned'; END IF;
  IF verse.realm_published_of('engine-pin')->>'status' <> '404' THEN RAISE EXCEPTION 'no record before a pin'; END IF;
  IF verse.realm_pin_first('engine-pin', '<html>no engine</html>') OR NOT verse.realm_needs_pin('engine-pin') THEN RAISE EXCEPTION 'a refused template pins nothing'; END IF;
  IF NOT verse.realm_pin_first('engine-pin', pg_temp.fx('a')) THEN RAISE EXCEPTION 'first pin'; END IF;
  IF verse.realm_needs_pin('engine-pin') THEN RAISE EXCEPTION 'pinned once'; END IF;   -- its own statement: a later snapshot
  IF verse.realm_pin_first('engine-pin', pg_temp.fx('b')) THEN RAISE EXCEPTION 'a later release re-pinned a pinned realm'; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'engine-pin';
  IF m.published_engine <> verse.engine_manifest(pg_temp.fx('a')) OR m.published_engine_sha256 <> '1592e30f8f480ae5f99954d1733f6f57cb1b139b6d0d449ad6098e5e2336b006' THEN
    RAISE EXCEPTION 'the first pin is the engine served then';
  END IF;
  IF (SELECT count(*) FROM verse.realm_publications WHERE slug = 'engine-pin') <> 1
     OR verse.realm_published_of('engine-pin') @> '{"n":1,"rev":1,"why":"first"}' IS NOT TRUE
     OR verse.realm_published_of('engine-pin')->>'sha256' <> m.published_sha256 THEN RAISE EXCEPTION 'first record: %', verse.realm_published_of('engine-pin'); END IF;
  page := verse.builder_page('engine-pin', 'https://openagents.com');
  IF strpos(page, ' data-engine="' || m.published_engine || '" data-engine-sha256="' || m.published_engine_sha256 || '"') = 0
     OR strpos(page, 'const pin = me.dataset.engine') = 0 OR strpos(page, 'for (const a of from.attributes)') > 0
     OR strpos(page, 'for (const [n, v] of attrs) s.setAttribute(n, v);') = 0 THEN
    RAISE EXCEPTION 'the page plays its pin: %', page;
  END IF;
  IF strpos(verse.edit_spec('engine-pin', 'engine-pin-player-key-0001', false), E'\nengine ' || m.published_engine_sha256 || E'\n') = 0 THEN
    RAISE EXCEPTION 'the spec names the engine';
  END IF;
END $$;
SELECT 'ok a realm''s first revision is pinned to the engine served then; its page and spec name it';

-- PUBLISH after a change, through both route families, against a newer release.
UPDATE verse.made_realms SET name = 'Engine Pin Two' WHERE slug = 'engine-pin';
DO $$
DECLARE
  out jsonb;
  before verse.made_realms;
  m verse.made_realms;
BEGIN
  SELECT * INTO before FROM verse.made_realms WHERE slug = 'engine-pin';
  IF before.rev <> 2 THEN RAISE EXCEPTION 'the change is revision 2'; END IF;
  -- refused publishes change nothing: stale, someone else's, and an engine that can't be pinned
  out := verse.realm_publish_pinned('engine-pin', '{"rev":1}', 'engine-pin-player-key-0001', false, false, pg_temp.fx('b'));
  IF out->>'status' <> '409' THEN RAISE EXCEPTION 'stale: %', out; END IF;
  out := verse.realm_publish_pinned('engine-pin', '{"rev":2}', 'engine-pin-player-key-0002', false, false, pg_temp.fx('b'));
  IF out->>'status' <> '404' THEN RAISE EXCEPTION 'not the owner (a private realm is 404 to anyone else): %', out; END IF;
  out := verse.realm_publish_pinned('engine-pin', '{"rev":2,"listed":false}', 'engine-pin-player-key-0001', false, false, '');
  IF out->>'status' <> '503' THEN RAISE EXCEPTION 'unread engine: %', out; END IF;
  out := verse.realm_publish_pinned('engine-pin', '{"rev":2,"listed":false}', 'engine-pin-player-key-0001', false, true, replace(pg_temp.fx('b'), 'myrealm.111111111111.wasm', 'myrealm.wasm'));
  IF out->>'status' <> '503' THEN RAISE EXCEPTION 'unnamed engine (edit-v1): %', out; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'engine-pin';
  IF m.published_rev <> 1 OR m.published_engine_sha256 <> before.published_engine_sha256 OR m.visibility <> before.visibility OR m.rev <> 2
     OR (SELECT count(*) FROM verse.realm_publications WHERE slug = 'engine-pin') <> 1 THEN
    RAISE EXCEPTION 'a refused publish kept something';
  END IF;
  -- the publish: the revision and the engine served now
  out := verse.realm_publish_pinned('engine-pin', '{"rev":2}', 'engine-pin-player-key-0001', false, false, pg_temp.fx('b'));
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'engine-pin';
  IF out->>'status' <> '200' OR out->>'engine' <> pg_temp.sha(verse.engine_manifest(pg_temp.fx('b'))) OR m.published_rev <> 2
     OR m.published_engine <> verse.engine_manifest(pg_temp.fx('b')) OR m.rev <> 2 THEN RAISE EXCEPTION 'publish: %', out; END IF;
  IF verse.realm_published_of('engine-pin') @> jsonb_build_object('n', 2, 'rev', 2, 'why', 'publish', 'sha256', m.published_sha256, 'engine_sha256', out->>'engine') IS NOT TRUE THEN
    RAISE EXCEPTION 'second record: %', verse.realm_published_of('engine-pin');
  END IF;
  -- edit-v1, the same revision again, a newer release: a new record, the same document
  out := verse.realm_publish_pinned('engine-pin', '{"rev":2}', 'engine-pin-player-key-0001', false, true, pg_temp.fx('c'));
  IF out->>'status' <> '200' OR out->>'engine' <> pg_temp.sha(verse.engine_manifest(pg_temp.fx('c'))) OR (out->>'rev')::int <> 2 THEN RAISE EXCEPTION 'edit-v1 publish: %', out; END IF;
  IF (SELECT count(*) FROM verse.realm_publications WHERE slug = 'engine-pin') <> 3 THEN RAISE EXCEPTION 'every publish is a record'; END IF;
  IF (SELECT string_agg(engine_sha256, ',' ORDER BY n) FROM verse.realm_publications WHERE slug = 'engine-pin')
     <> '1592e30f8f480ae5f99954d1733f6f57cb1b139b6d0d449ad6098e5e2336b006,' || pg_temp.sha(verse.engine_manifest(pg_temp.fx('b'))) || ',' || pg_temp.sha(verse.engine_manifest(pg_temp.fx('c'))) THEN
    RAISE EXCEPTION 'the records keep each engine in order';
  END IF;
END $$;
SELECT 'ok publish pins the revision and today''s engine together; a refused publish keeps nothing';

-- A fork starts on its parent's engine, and stays apart from it.
DO $$
DECLARE
  out jsonb;
  f verse.made_realms;
  parent verse.made_realms;
BEGIN
  SELECT * INTO parent FROM verse.made_realms WHERE slug = 'engine-pin';
  out := verse.made_fork('engine-pin', '{"name":"Engine Pin Fork"}', 'engine-pin-player-key-0002', 12);
  IF out->>'status' <> '200' THEN RAISE EXCEPTION 'fork: %', out; END IF;
  SELECT * INTO f FROM verse.made_realms WHERE slug = out->>'slug';
  IF f.published_engine IS DISTINCT FROM parent.published_engine OR f.published_engine_sha256 IS DISTINCT FROM parent.published_engine_sha256
     OR f.forked_from <> 'engine-pin' OR verse.realm_needs_pin(f.slug) THEN RAISE EXCEPTION 'the fork plays its parent''s engine'; END IF;
  IF verse.realm_published_of(f.slug) @> jsonb_build_object('n', 1, 'why', 'fork', 'engine_sha256', parent.published_engine_sha256, 'rev', f.published_rev) IS NOT TRUE THEN
    RAISE EXCEPTION 'fork record: %', verse.realm_published_of(f.slug);
  END IF;
  UPDATE verse.made_realms SET name = 'Engine Pin Fork Two' WHERE slug = f.slug;
  SELECT * INTO f FROM verse.made_realms WHERE slug = f.slug;
  out := verse.realm_publish_pinned(f.slug, jsonb_build_object('rev', f.rev), 'engine-pin-player-key-0002', false, false, pg_temp.fx('a'));
  IF out->>'status' <> '200' OR out->>'engine' <> '1592e30f8f480ae5f99954d1733f6f57cb1b139b6d0d449ad6098e5e2336b006' THEN RAISE EXCEPTION 'fork publish: %', out; END IF;
  IF (SELECT published_engine_sha256 FROM verse.made_realms WHERE slug = 'engine-pin') <> parent.published_engine_sha256
     OR (SELECT count(*) FROM verse.realm_publications WHERE slug = 'engine-pin') <> 3
     OR (SELECT published_rev FROM verse.made_realms WHERE slug = 'engine-pin') <> parent.published_rev THEN
    RAISE EXCEPTION 'the fork''s publish touched its parent';
  END IF;
END $$;
SELECT 'ok a fork starts on its parent''s engine; its own publish leaves the parent''s record alone';

-- A server without a site bucket (a dev server) publishes as before, unpinned.
DO $$
DECLARE
  out jsonb;
  page text;
BEGIN
  out := verse.realm_publish_pinned('engine-pin', '{"rev":2}', 'engine-pin-player-key-0001', false, false, NULL);
  IF out->>'status' <> '200' OR out->'engine' <> 'null'::jsonb OR verse.realm_published_of('engine-pin')->>'status' <> '404' THEN RAISE EXCEPTION 'dev publish: %', out; END IF;
  page := verse.builder_page('engine-pin', 'https://openagents.com');
  IF strpos(page, 'data-engine=') > 0 OR strpos(page, 'fetch("/worlds/built"') = 0 OR strpos(verse.edit_spec('engine-pin', 'engine-pin-player-key-0001', false), E'\nengine ') > 0 THEN
    RAISE EXCEPTION 'unpinned: today''s engine';
  END IF;
  -- 0107 wrote no page again; 0110 (realms are worlds, /worlds/built) did, once.
  IF verse.builder_page_rev() <> 3 THEN RAISE EXCEPTION 'pages are written again'; END IF;
END $$;
SELECT 'ok without a bucket a publish is unpinned and the page plays today''s engine';
SELECT 'done 6';
ROLLBACK;
