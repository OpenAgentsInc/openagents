-- Places as world edits (migrations/0190_realm_places.sql, GitHub #13506; 0193, #13514): the rows a spawn and a despawn
-- write on a made realm and on a hand-built one, IDs that never shift, the gone mark each keeps, the spec's lines and one
-- undo log. Whether one may stand is realm-place-policy.coax's, checked in its own file and by world_checks.coax. One
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
  out int;
  m verse.made_realms;
  spec text;
BEGIN
  -- the world reads what a made realm places, and that it is a made realm; its own spec does not change
  spec := verse.realm_world_spec('places', 'places-player-key-0001', false);
  IF strpos(spec, E'\nplaced crate 1 2 0\n') = 0 OR strpos(spec, E'made 1\nthing crate\nthing barrel\nthing tree\n') = 0 THEN RAISE EXCEPTION 'world spec: %', spec; END IF;
  IF strpos(spec, E'\nrole owner\n') = 0 OR strpos(verse.realm_world_spec('places', 'places-player-key-0002', false), E'\nrole visitor\n') = 0 THEN RAISE EXCEPTION 'the role the world reads: %', spec; END IF;
  IF strpos(verse.edit_spec('places', 'places-player-key-0001', false), 'thing crate') > 0 OR strpos(verse.edit_spec('places', 'places-player-key-0001', false), 'made 1') > 0 THEN
    RAISE EXCEPTION 'the realm''s own spec gained the world''s lines';
  END IF;
  -- no such realm: nothing written
  IF verse.realm_thing_add('no-such-places', 'barrel', 0, 0, 0, '') <> 0 OR verse.realm_thing_mark('no-such-places', 1, true) <> 0 THEN RAISE EXCEPTION 'no realm'; END IF;
  -- an add appends: its ID is the list's new length, in metres and radians as the page writes them
  PERFORM set_config('verse.world_edit', 'on', true);
  out := verse.realm_thing_add('places', 'barrel', 150, -250, 90, 'places-player-key-0001');
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF out <> 2 OR m.placed -> 1 IS DISTINCT FROM '{"thing":"barrel","x":1.5,"z":-2.5,"yaw":1.571}'::jsonb THEN RAISE EXCEPTION 'add: % %', out, m.placed; END IF;
  -- one undo log: the world's edit is no step of the realm's own, though it is a revision
  IF jsonb_array_length(m.undo) <> 0 OR m.rev < 2 THEN RAISE EXCEPTION 'a world edit pushed a step of the realm''s own: %', m.undo; END IF;
  -- a mark gone: nothing moves up, and the next add is 3; a thing the list hasn't is no mark
  out := verse.realm_thing_mark('places', 1, true);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF out <> 1 OR jsonb_array_length(m.placed) <> 2 OR m.shaped IS DISTINCT FROM '[{"entity":"placed:0","yaw":0,"scale":100,"gone":true}]'::jsonb THEN RAISE EXCEPTION 'gone: % %', out, m.shaped; END IF;
  IF verse.realm_thing_mark('places', 9, true) <> 0 OR verse.realm_thing_mark('places', 0, true) <> 0 THEN RAISE EXCEPTION 'no such thing'; END IF;
  IF verse.realm_thing_add('places', 'tree', 0, 0, 0, 'places-player-key-0001') <> 3 THEN RAISE EXCEPTION 'the next ID'; END IF;
  -- the spec says it is gone, as a made realm's shapes do
  spec := verse.realm_world_spec('places', 'places-player-key-0001', false);
  IF strpos(spec, E'shaped placed:0 0 100 1\n') = 0 THEN RAISE EXCEPTION 'gone in the spec: %', spec; END IF;
  -- thing 1 back as it was: the entry goes, since it keeps no turn or size of its own
  out := verse.realm_thing_mark('places', 1, false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF out <> 1 OR m.shaped <> '[]' OR m.placed -> 0 IS DISTINCT FROM '{"thing":"crate","x":1,"z":2,"yaw":0}'::jsonb THEN RAISE EXCEPTION 'back: % %', out, m.shaped; END IF;
  -- a turned thing keeps its turn through a mark gone and back
  UPDATE verse.made_realms SET shaped = '[{"entity":"placed:1","yaw":45,"scale":150,"gone":false}]' WHERE slug = 'places';
  PERFORM verse.realm_thing_mark('places', 2, true);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF m.shaped IS DISTINCT FROM '[{"entity":"placed:1","yaw":45,"scale":150,"gone":true}]'::jsonb THEN RAISE EXCEPTION 'turn kept gone: %', m.shaped; END IF;
  PERFORM verse.realm_thing_mark('places', 2, false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF m.shaped IS DISTINCT FROM '[{"entity":"placed:1","yaw":45,"scale":150,"gone":false}]'::jsonb THEN RAISE EXCEPTION 'turn kept: %', m.shaped; END IF;
  -- without the world's mark, a change is a step of the realm's own, as before
  PERFORM set_config('verse.world_edit', '', true);
  PERFORM verse.realm_thing_mark('places', 3, true);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'places';
  IF jsonb_array_length(m.undo) <> 1 THEN RAISE EXCEPTION 'the realm''s own steps: %', m.undo; END IF;
END $$;
SELECT 'ok a made realm: an add appends, a mark keeps it in the list, IDs never shift, and the world''s edits are its log''s';

DO $$
DECLARE
  out int;
  spec text;
  p jsonb;
BEGIN
  -- a hand-built realm keeps gone on the entry; its spec says so as a made realm's does, and it is no made realm
  INSERT INTO verse.realm_rights (slug, owner_account, things) VALUES ('test-places-hand', 'n-test-places', ARRAY['crate', 'bench']);
  out := verse.realm_thing_add('test-places-hand', 'bench', 1000, 2000, 0, '');
  IF out <> 1 THEN RAISE EXCEPTION 'hand add: %', out; END IF;
  PERFORM verse.realm_thing_add('test-places-hand', 'crate', -500, 0, 180, '');
  out := verse.realm_thing_mark('test-places-hand', 1, true);
  SELECT placed INTO p FROM verse.realm_edit WHERE slug = 'test-places-hand';
  IF out <> 1 OR p IS DISTINCT FROM '[{"thing":"bench","x":10,"z":20,"yaw":0,"gone":true},{"thing":"crate","x":-5,"z":0,"yaw":3.142}]'::jsonb THEN RAISE EXCEPTION 'hand gone: % %', out, p; END IF;
  spec := verse.realm_world_spec('test-places-hand', '', true);
  IF strpos(spec, E'thing crate\nthing bench\n') = 0 OR strpos(spec, E'placed bench 10 20 0\nplaced crate -5 0 3.142\n') = 0 OR strpos(spec, E'shaped placed:0 0 100 1\n') = 0
     OR strpos(spec, 'placed:1 ') > 0 OR strpos(spec, 'made 1') > 0 THEN
    RAISE EXCEPTION 'hand spec: %', spec;
  END IF;
  out := verse.realm_thing_mark('test-places-hand', 1, false);
  SELECT placed INTO p FROM verse.realm_edit WHERE slug = 'test-places-hand';
  IF out <> 1 OR p -> 0 IS DISTINCT FROM '{"thing":"bench","x":10,"z":20,"yaw":0}'::jsonb OR strpos(verse.edit_spec('test-places-hand', '', true), 'shaped') > 0 THEN RAISE EXCEPTION 'hand back: % %', out, p; END IF;
  IF verse.realm_thing_mark('test-places-hand', 3, true) <> 0 THEN RAISE EXCEPTION 'hand no such thing'; END IF;
END $$;
SELECT 'ok a hand-built realm: gone on the entry, and the spec''s shaped lines say it';
SELECT 'done 2';
ROLLBACK;
