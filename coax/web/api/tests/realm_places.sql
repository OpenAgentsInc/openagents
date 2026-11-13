-- Places as world edits (migrations/0190_realm_places.sql, GitHub #13506): spawn and despawn on a made realm and on
-- a hand-built one, IDs that never shift, the gone mark each keeps, the spec's lines and one undo log. One
-- transaction, rolled back. Each check raises on failure and prints an "ok" line; the last line is "done N". Run by
-- realm_engine_pin.coax (PIN_SQL names this file):
--   PGURL=... PSQL=psql PIN_SQL=coax/web/api/tests/realm_places.sql coax/web/tool.sh coax/web/api/tests/realm_engine_pin.coax
\set ON_ERROR_STOP 1
BEGIN;
INSERT INTO verse.players (id, key_sha256, created_ms) VALUES ('p-places', encode(sha256(convert_to('places-player-key-0001', 'UTF8')), 'hex'), 1),
  ('p-places-2', encode(sha256(convert_to('places-player-key-0002', 'UTF8')), 'hex'), 1);
INSERT INTO verse.worlds (id, owner_player, created_ms) VALUES ('made-places', 'p-places', 1);
INSERT INTO verse.made_realms (slug, world, name, style, ground, time, features, tools, made_ms, visibility, placed)
  VALUES ('places', 'made-places', 'Places', 'lowpoly', 'grass', 'day', '[]', '{}', 1, 'public', '[{"thing":"crate","x":1,"z":2,"yaw":0}]');

DO $$
DECLARE
  out jsonb;
  m verse.made_realms;
  spec text;
BEGIN
  -- the world reads what a made realm places; its own spec does not change
  spec := verse.realm_world_spec('places', 'places-player-key-0001', false);
  IF strpos(spec, E'\nplaced crate 1 2 0\n') = 0 OR strpos(spec, E'thing crate\nthing barrel\nthing tree\n') = 0 THEN RAISE EXCEPTION 'world spec: %', spec; END IF;
  IF strpos(verse.edit_spec('places', 'places-player-key-0001', false), 'thing crate') > 0 THEN RAISE EXCEPTION 'the realm''s own spec gained thing lines'; END IF;
  -- who may: the owner; a visitor may not; no such realm
  IF verse.realm_spawn('places', 0, 'barrel', 0, 0, 0, 'places-player-key-0002', false)->>'status' <> '403'
     OR verse.realm_despawn('places', 1, 'places-player-key-0002', false)->>'status' <> '403'
     OR verse.realm_spawn('no-such-places', 0, 'barrel', 0, 0, 0, 'places-player-key-0001', false)->>'status' <> '404' THEN
    RAISE EXCEPTION 'who may';
  END IF;
  -- a spawn appends: its ID is the list's new length, in metres and radians as /place writes them
  PERFORM set_config('verse.world_edit', 'on', true);
  out := verse.realm_spawn('places', 0, 'barrel', 150, -250, 90, 'places-player-key-0001', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF out->>'status' <> '200' OR (out->>'id')::int <> 2 OR m.placed -> 1 IS DISTINCT FROM '{"thing":"barrel","x":1.5,"z":-2.5,"yaw":1.571}'::jsonb THEN RAISE EXCEPTION 'spawn: % %', out, m.placed; END IF;
  -- one undo log: the world's edit is no step of the realm's own, though it is a revision
  IF jsonb_array_length(m.undo) <> 0 OR m.rev < 2 THEN RAISE EXCEPTION 'a world edit pushed a step of the realm''s own: %', m.undo; END IF;
  -- what it may not place, nor where
  IF verse.realm_spawn('places', 0, 'dragon', 0, 0, 0, 'places-player-key-0001', false)->>'status' <> '422'
     OR verse.realm_spawn('places', 0, 'crate', 4001, 0, 0, 'places-player-key-0001', false)->>'status' <> '422'
     OR verse.realm_spawn('places', 0, 'crate', 0, -4001, 0, 'places-player-key-0001', false)->>'status' <> '422'
     OR verse.realm_spawn('places', 0, 'crate', 4000, -4000, 0, 'places-player-key-0001', false)->>'status' <> '200' THEN
    RAISE EXCEPTION 'what and where';
  END IF;
  -- a despawn marks it gone; nothing moves up, and the next spawn is 4
  out := verse.realm_despawn('places', 1, 'places-player-key-0001', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF out->>'status' <> '200' OR jsonb_array_length(m.placed) <> 3 OR m.shaped IS DISTINCT FROM '[{"entity":"placed:0","yaw":0,"scale":100,"gone":true}]'::jsonb THEN RAISE EXCEPTION 'despawn: % %', out, m.shaped; END IF;
  IF verse.realm_despawn('places', 1, 'places-player-key-0001', false)->>'error' <> 'thing 1 is gone already'
     OR verse.realm_despawn('places', 9, 'places-player-key-0001', false)->>'error' <> 'no thing 9' THEN RAISE EXCEPTION 'despawn refusals'; END IF;
  IF (verse.realm_spawn('places', 0, 'tree', 0, 0, 0, 'places-player-key-0001', false)->>'id')::int <> 4 THEN RAISE EXCEPTION 'the next ID'; END IF;
  -- the spec says it is gone, as a made realm's shapes do
  spec := verse.realm_world_spec('places', 'places-player-key-0001', false);
  IF strpos(spec, E'shaped placed:0 0 100 1\n') = 0 THEN RAISE EXCEPTION 'gone in the spec: %', spec; END IF;
  -- a spawn of thing 1 brings it back as it was: its kind must be its own, and it must be gone
  IF verse.realm_spawn('places', 1, 'barrel', 0, 0, 0, 'places-player-key-0001', false)->>'error' <> 'thing 1 is a crate'
     OR verse.realm_spawn('places', 2, 'barrel', 0, 0, 0, 'places-player-key-0001', false)->>'error' <> 'thing 2 stands already'
     OR verse.realm_spawn('places', 7, 'crate', 0, 0, 0, 'places-player-key-0001', false)->>'error' <> 'no thing 7' THEN RAISE EXCEPTION 'bring-back refusals'; END IF;
  out := verse.realm_spawn('places', 1, 'crate', 0, 0, 0, 'places-player-key-0001', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF out->>'status' <> '200' OR (out->>'id')::int <> 1 OR m.shaped <> '[]' OR m.placed -> 0 IS DISTINCT FROM '{"thing":"crate","x":1,"z":2,"yaw":0}'::jsonb THEN RAISE EXCEPTION 'back: % %', out, m.shaped; END IF;
  -- a turned thing keeps its turn through a despawn and its spawn
  UPDATE verse.made_realms SET shaped = '[{"entity":"placed:1","yaw":45,"scale":150,"gone":false}]' WHERE slug = 'places';
  PERFORM verse.realm_despawn('places', 2, 'places-player-key-0001', false);
  PERFORM verse.realm_spawn('places', 2, 'barrel', 0, 0, 0, 'places-player-key-0001', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF m.shaped IS DISTINCT FROM '[{"entity":"placed:1","yaw":45,"scale":150,"gone":false}]'::jsonb THEN RAISE EXCEPTION 'turn kept: %', m.shaped; END IF;
  -- without the world's mark, a change is a step of the realm's own, as before
  PERFORM set_config('verse.world_edit', '', true);
  PERFORM verse.realm_despawn('places', 3, 'places-player-key-0001', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF jsonb_array_length(m.undo) <> 1 THEN RAISE EXCEPTION 'the realm''s own steps: %', m.undo; END IF;
END $$;
SELECT 'ok a made realm: spawn appends, despawn marks gone, IDs never shift, and the world''s edits are its log''s';

DO $$
DECLARE
  out jsonb;
  spec text;
  p jsonb;
BEGIN
  -- a hand-built realm keeps gone on the entry; its spec says so as a made realm's does
  INSERT INTO verse.realm_rights (slug, owner_account, things) VALUES ('test-places-hand', 'n-test-places', ARRAY['crate', 'bench']);
  out := verse.realm_spawn('test-places-hand', 0, 'bench', 1000, 2000, 0, '', true);
  IF out->>'status' <> '200' OR (out->>'id')::int <> 1 THEN RAISE EXCEPTION 'hand spawn: %', out; END IF;
  IF verse.realm_spawn('test-places-hand', 0, 'barrel', 0, 0, 0, '', true)->>'status' <> '422'
     OR verse.realm_spawn('test-places-hand', 0, 'crate', 40001, 0, 0, '', true)->>'status' <> '422' THEN RAISE EXCEPTION 'hand what and where'; END IF;
  PERFORM verse.realm_spawn('test-places-hand', 0, 'crate', -500, 0, 180, '', true);
  out := verse.realm_despawn('test-places-hand', 1, '', true);
  SELECT placed INTO p FROM verse.realm_edit WHERE slug = 'test-places-hand';
  IF out->>'status' <> '200' OR p IS DISTINCT FROM '[{"thing":"bench","x":10,"z":20,"yaw":0,"gone":true},{"thing":"crate","x":-5,"z":0,"yaw":3.142}]'::jsonb THEN RAISE EXCEPTION 'hand despawn: % %', out, p; END IF;
  spec := verse.realm_world_spec('test-places-hand', '', true);
  IF strpos(spec, E'thing crate\nthing bench\n') = 0 OR strpos(spec, E'placed bench 10 20 0\nplaced crate -5 0 3.142\n') = 0 OR strpos(spec, E'shaped placed:0 0 100 1\n') = 0 OR strpos(spec, 'placed:1 ') > 0 THEN
    RAISE EXCEPTION 'hand spec: %', spec;
  END IF;
  out := verse.realm_spawn('test-places-hand', 1, 'bench', 0, 0, 0, '', true);
  SELECT placed INTO p FROM verse.realm_edit WHERE slug = 'test-places-hand';
  IF out->>'status' <> '200' OR p -> 0 IS DISTINCT FROM '{"thing":"bench","x":10,"z":20,"yaw":0}'::jsonb OR strpos(verse.edit_spec('test-places-hand', '', true), 'shaped') > 0 THEN RAISE EXCEPTION 'hand back: % %', out, p; END IF;
  IF verse.realm_spawn('test-places-hand', 0, 'crate', 0, 0, 0, '', false)->>'status' <> '403' THEN RAISE EXCEPTION 'hand who may'; END IF;
END $$;
SELECT 'ok a hand-built realm: gone on the entry, and the spec''s shaped lines say it';
SELECT 'done 2';
ROLLBACK;
