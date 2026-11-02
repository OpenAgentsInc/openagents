-- Shapes (migrations/0109_realm_shapes.sql, GitHub #12629): rotate, resize and remove in the made
-- realm's document. One transaction, rolled back. Each check raises on failure and prints an "ok"
-- line; the last line is "done N". Run by realm_engine_pin.coax (PIN_SQL names this file):
--   PGURL=... PSQL=psql PIN_SQL=coax/web/api/tests/realm_shapes.sql coax/web/tool.sh coax/web/api/tests/realm_engine_pin.coax
\set ON_ERROR_STOP 1
BEGIN;
INSERT INTO verse.players (id, key_sha256, created_ms) VALUES ('p-shapes', encode(sha256(convert_to('shapes-player-key-0001', 'UTF8')), 'hex'), 1),
  ('p-shapes-2', encode(sha256(convert_to('shapes-player-key-0002', 'UTF8')), 'hex'), 1);
INSERT INTO verse.worlds (id, owner_player, created_ms) VALUES ('made-shapes', 'p-shapes', 1);
INSERT INTO verse.made_realms (slug, world, name, style, ground, time, features, tools, made_ms, visibility) VALUES ('shapes', 'made-shapes', 'Shapes', 'lowpoly', 'grass', 'day', '[]', '{}', 1, 'public');

DO $$
DECLARE
  v jsonb;
BEGIN
  v := verse.realm_shapes('[{"entity":"trees:1","yaw":90,"scale":150,"gone":false},{"entity":"rocks:0","yaw":0,"scale":100,"gone":false},{"entity":"trees:1","yaw":-90,"scale":150,"gone":true}]');
  IF v IS DISTINCT FROM '[{"entity":"trees:1","yaw":-90,"scale":150,"gone":true}]'::jsonb THEN RAISE EXCEPTION 'held: %', v; END IF;
  IF verse.realm_shapes('[{"entity":"trees:1","yaw":181,"scale":100,"gone":false}]') IS NOT NULL
     OR verse.realm_shapes('[{"entity":"trees:1","yaw":-180,"scale":100,"gone":false}]') IS NOT NULL
     OR verse.realm_shapes('[{"entity":"trees:1","yaw":1.5,"scale":100,"gone":false}]') IS NOT NULL
     OR verse.realm_shapes('[{"entity":"trees:1","yaw":0,"scale":24,"gone":false}]') IS NOT NULL
     OR verse.realm_shapes('[{"entity":"trees:1","yaw":0,"scale":401,"gone":false}]') IS NOT NULL
     OR verse.realm_shapes('[{"entity":"trees:1","yaw":0,"scale":100,"gone":"yes"}]') IS NOT NULL
     OR verse.realm_shapes('[{"entity":"Trees:1","yaw":0,"scale":100,"gone":false}]') IS NOT NULL
     OR verse.realm_shapes('[{"entity":"trees:1","yaw":0,"scale":100}]') IS NOT NULL
     OR verse.realm_shapes('{}') IS NOT NULL OR verse.realm_shapes(NULL) IS NOT NULL THEN
    RAISE EXCEPTION 'a malformed shape was held';
  END IF;
  IF verse.realm_shapes('[{"entity":"trees:1","yaw":180,"scale":25,"gone":false},{"entity":"placed:3","yaw":-179,"scale":400,"gone":false}]') IS NULL THEN RAISE EXCEPTION 'bounds'; END IF;
END $$;
SELECT 'ok a shape is a FEATURE:N name, a whole-degree turn, a whole-percent size and gone';

DO $$
DECLARE
  sha0 text := verse.realm_doc_sha(verse.realm_doc((SELECT r FROM verse.made_realms r WHERE r.slug = 'shapes')));
  out jsonb;
  m verse.made_realms;
  spec text;
BEGIN
  IF verse.realm_doc((SELECT r FROM verse.made_realms r WHERE r.slug = 'shapes')) ? 'shaped' THEN RAISE EXCEPTION 'no shapes, no key: old documents keep their bytes'; END IF;
  -- a page before 0109 saves moves only: shapes stay
  out := verse.realm_save('shapes', '{"rev":1,"moved":[],"shaped":[{"entity":"trees:2","yaw":45,"scale":200,"gone":false},{"entity":"rocks:1","yaw":0,"scale":100,"gone":true}]}', 'shapes-player-key-0001', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'shapes';
  IF out->>'status' <> '200' OR m.rev <> 2 OR jsonb_array_length(m.shaped) <> 2 OR out->>'sha256' = sha0 THEN RAISE EXCEPTION 'save shapes: %', out; END IF;
  out := verse.realm_save('shapes', '{"rev":2,"moved":[{"entity":"trees:2","x":1,"z":2}]}', 'shapes-player-key-0001', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'shapes';
  IF out->>'status' <> '200' OR m.rev <> 3 OR jsonb_array_length(m.shaped) <> 2 OR jsonb_array_length(m.moved) <> 1 THEN RAISE EXCEPTION 'moves only keep shapes: %', out; END IF;
  -- refusals change nothing: stale, malformed, someone else's, more than 64 overridden
  out := verse.realm_save('shapes', '{"rev":2,"moved":[],"shaped":[]}', 'shapes-player-key-0001', false);
  IF out->>'status' <> '409' THEN RAISE EXCEPTION 'stale: %', out; END IF;
  out := verse.realm_save('shapes', '{"rev":3,"moved":[],"shaped":[{"entity":"trees:2","yaw":999,"scale":100,"gone":false}]}', 'shapes-player-key-0001', false);
  IF out->>'status' <> '400' THEN RAISE EXCEPTION 'malformed: %', out; END IF;
  out := verse.realm_save('shapes', '{"rev":3,"moved":[],"shaped":[]}', 'shapes-player-key-0002', false);
  IF out->>'status' <> '403' THEN RAISE EXCEPTION 'not the owner: %', out; END IF;
  out := verse.realm_save('shapes', jsonb_build_object('rev', 3,
           'moved', (SELECT jsonb_agg(jsonb_build_object('entity', 'trees:' || i, 'x', 0, 'z', 0)) FROM generate_series(0, 39) i),
           'shaped', (SELECT jsonb_agg(jsonb_build_object('entity', 'rocks:' || i, 'yaw', 10, 'scale', 100, 'gone', false)) FROM generate_series(0, 24) i)),
           'shapes-player-key-0001', false);
  IF out->>'status' <> '400' THEN RAISE EXCEPTION 'more than 64: %', out; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'shapes';
  IF m.rev <> 3 OR jsonb_array_length(m.shaped) <> 2 OR jsonb_array_length(m.moved) <> 1 THEN RAISE EXCEPTION 'a refused save changed the document'; END IF;
  spec := verse.edit_spec('shapes', 'shapes-player-key-0001', false);
  IF strpos(spec, E'\nshaped trees:2 45 200 0\nshaped rocks:1 0 100 1\nshapes 1\n') = 0 THEN RAISE EXCEPTION 'spec: %', spec; END IF;
END $$;
SELECT 'ok a save keeps shapes with its moves; a page that sends moves only keeps them; refusals change nothing';

DO $$
DECLARE
  out jsonb;
  m verse.made_realms;
  f verse.made_realms;
  spec text;
BEGIN
  -- published at 3; then turned again at 4: a visitor sees 3's shapes
  out := verse.made_publish('shapes', '{"rev":3}', 'shapes-player-key-0001', false);
  IF out->>'status' <> '200' THEN RAISE EXCEPTION 'publish: %', out; END IF;
  out := verse.realm_save('shapes', '{"rev":3,"moved":[{"entity":"trees:2","x":1,"z":2}],"shaped":[{"entity":"trees:2","yaw":90,"scale":200,"gone":false}]}', 'shapes-player-key-0001', false);
  IF out->>'status' <> '200' OR (out->>'rev')::int <> 4 THEN RAISE EXCEPTION 'turn again: %', out; END IF;
  spec := verse.edit_spec('shapes', 'shapes-player-key-0002', false);
  IF strpos(spec, E'\nshaped trees:2 45 200 0\nshaped rocks:1 0 100 1\n') = 0 OR strpos(spec, 'shaped trees:2 90') > 0 THEN RAISE EXCEPTION 'visitor spec: %', spec; END IF;
  -- a visitor's fork copies the published revision's shapes; the owner's fork copies now's
  out := verse.made_fork('shapes', '{"name":"Shapes Visitor"}', 'shapes-player-key-0002', 12);
  SELECT * INTO f FROM verse.made_realms WHERE slug = out->>'slug';
  IF out->>'status' <> '200' OR f.forked_rev <> 3 OR f.shaped IS DISTINCT FROM (SELECT doc->'shaped' FROM verse.realm_revisions WHERE slug = 'shapes' AND rev = 3) THEN
    RAISE EXCEPTION 'visitor fork: % %', out, f.shaped;
  END IF;
  IF verse.realm_doc(f)->'shaped' IS DISTINCT FROM f.shaped OR f.published_sha256 <> verse.realm_doc_sha(verse.realm_doc(f)) THEN RAISE EXCEPTION 'the fork''s published hash covers its shapes'; END IF;
  out := verse.made_fork('shapes', '{"name":"Shapes Owner"}', 'shapes-player-key-0001', 12);
  SELECT * INTO f FROM verse.made_realms WHERE slug = out->>'slug';
  IF out->>'status' <> '200' OR f.shaped IS DISTINCT FROM '[{"entity":"trees:2","yaw":90,"scale":200,"gone":false}]'::jsonb THEN RAISE EXCEPTION 'owner fork: % %', out, f.shaped; END IF;
  -- the fork's edits stay its own
  out := verse.realm_save(f.slug, jsonb_build_object('rev', f.rev, 'moved', '[]'::jsonb, 'shaped', '[]'::jsonb), 'shapes-player-key-0001', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'shapes';
  IF out->>'status' <> '200' OR m.shaped IS DISTINCT FROM '[{"entity":"trees:2","yaw":90,"scale":200,"gone":false}]'::jsonb THEN RAISE EXCEPTION 'the fork''s save touched its parent'; END IF;
  -- UNDO brings 3's shapes back as a new revision
  out := verse.realm_undo('shapes', '{"rev":4}', 'shapes-player-key-0001', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'shapes';
  IF out->>'status' <> '200' OR m.rev <> 5 OR m.shaped IS DISTINCT FROM (SELECT doc->'shaped' FROM verse.realm_revisions WHERE slug = 'shapes' AND rev = 3) THEN
    RAISE EXCEPTION 'undo: % %', out, m.shaped;
  END IF;
  IF verse.realm_doc_sha(verse.realm_doc(m)) = verse.realm_doc_sha((SELECT doc FROM verse.realm_revisions WHERE slug = 'shapes' AND rev = 3)) THEN
    RAISE EXCEPTION 'a new revision is a new document (its rev differs)';
  END IF;
END $$;
SELECT 'ok visitors, forks and undo carry the shapes of their revision';
SELECT 'done 3';
ROLLBACK;
