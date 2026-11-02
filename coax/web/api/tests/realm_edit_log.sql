-- The server's edit log (migrations/0110_realm_edit_log.sql, GitHub #12629): every change to a made
-- realm is one step of per-slot inverses, UNDO and REDO walk them, lore and quests are in the
-- document, and older steps and documents keep working. One transaction, rolled back. Each check
-- raises on failure and prints an "ok" line; the last line is "done N". Run by realm_engine_pin.coax:
--   PGURL=... PSQL=psql PIN_SQL=coax/web/api/tests/realm_edit_log.sql coax/web/tool.sh coax/web/api/tests/realm_engine_pin.coax
\set ON_ERROR_STOP 1
BEGIN;
INSERT INTO verse.players (id, key_sha256, created_ms) VALUES ('p-log', encode(sha256(convert_to('elog-player-key-00001', 'UTF8')), 'hex'), 1),
  ('p-log-2', encode(sha256(convert_to('elog-player-key-00002', 'UTF8')), 'hex'), 1);
INSERT INTO verse.worlds (id, owner_player, created_ms) VALUES ('made-elog', 'p-log', 1), ('made-elog-old', 'p-log', 1);
INSERT INTO verse.made_realms (slug, world, name, style, ground, time, features, tools, made_ms, visibility)
VALUES ('elog', 'made-elog', 'Edit Log', 'lowpoly', 'grass', 'day', '[{"feature":"trees","size":"some"},{"feature":"rocks"}]', '{"fly":false}', 1, 'public');
-- The realm's document now, without its revision: what two states are compared by.
CREATE FUNCTION pg_temp.now_doc(s text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.realm_doc(r) - 'rev' FROM verse.made_realms r WHERE r.slug = s $$;
CREATE FUNCTION pg_temp.rev(s text) RETURNS bigint LANGUAGE sql AS $$ SELECT rev FROM verse.made_realms WHERE slug = s $$;
CREATE FUNCTION pg_temp.k1() RETURNS text LANGUAGE sql AS $$ SELECT 'elog-player-key-00001' $$;
CREATE FUNCTION pg_temp.undo(s text) RETURNS jsonb LANGUAGE sql AS $$ SELECT verse.realm_undo(s, jsonb_build_object('rev', pg_temp.rev(s)), pg_temp.k1(), false) $$;
CREATE FUNCTION pg_temp.redo(s text) RETURNS jsonb LANGUAGE sql AS $$ SELECT verse.realm_redo(s, jsonb_build_object('rev', pg_temp.rev(s)), pg_temp.k1(), false) $$;
CREATE TEMP TABLE states (n int PRIMARY KEY, doc jsonb) ON COMMIT DROP;

DO $$
DECLARE
  m verse.made_realms;
  ops jsonb;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'elog';
  IF verse.realm_doc(m) ? 'lore' OR verse.realm_doc(m) ? 'quests' THEN RAISE EXCEPTION 'no lore, no key: older documents keep their bytes'; END IF;
  IF verse.realm_doc_sha(verse.realm_doc(m)) <> (SELECT sha256 FROM verse.realm_revisions WHERE slug = 'elog' AND rev = m.rev) THEN RAISE EXCEPTION 'the kept revision is the document'; END IF;
  -- the slots, read and written
  m.moved := '[{"entity":"trees:1","x":1,"z":2}]'; m.shaped := '[{"entity":"trees:1","yaw":90,"scale":150,"gone":false},{"entity":"rocks:0","yaw":0,"scale":100,"gone":true}]';
  IF verse.realm_slot(m, 'trees:1') IS DISTINCT FROM '{"x":1,"z":2,"yaw":90,"scale":150,"gone":false}'::jsonb
     OR verse.realm_slot(m, 'rocks:0') IS DISTINCT FROM '{"yaw":0,"scale":100,"gone":true}'::jsonb OR verse.realm_slot(m, 'rocks:1') IS NOT NULL
     OR verse.realm_slot(m, 'set:time') IS DISTINCT FROM '"day"'::jsonb OR verse.realm_slot(m, 'tool:fly') IS DISTINCT FROM 'false'::jsonb
     OR verse.realm_slot(m, 'feature:1') IS DISTINCT FROM '{"feature":"rocks"}'::jsonb OR verse.realm_slot(m, 'feature:2') IS NOT NULL THEN
    RAISE EXCEPTION 'slots read';
  END IF;
  m := verse.realm_slot_put(m, 'trees:1', '{"yaw":45,"scale":100,"gone":false}');
  IF m.moved <> '[]' OR m.shaped -> 0 IS DISTINCT FROM '{"entity":"trees:1","yaw":45,"scale":100,"gone":false}'::jsonb THEN RAISE EXCEPTION 'an entity replaced in place: % %', m.moved, m.shaped; END IF;
  m := verse.realm_slot_put(m, 'place:2', '{"thing":"crate","x":0,"z":0,"yaw":0}');
  IF jsonb_array_length(m.placed) <> 3 THEN RAISE EXCEPTION 'place:2 fills to it'; END IF;
  m := verse.realm_slot_put(m, 'place:2', NULL);
  IF m.placed <> '[]' THEN RAISE EXCEPTION 'taking the last away takes the holes: %', m.placed; END IF;
  -- an edit's ops are the slots that differ, before and after
  ops := verse.realm_ops((SELECT r FROM verse.made_realms r WHERE r.slug = 'elog'), verse.realm_slot_put(verse.realm_slot_put((SELECT r FROM verse.made_realms r WHERE r.slug = 'elog'), 'set:time', '"night"'), 'lore:mara', '"Keeps the lamp."'));
  IF ops IS DISTINCT FROM '[{"slot":"set:time","before":"day","after":"night"},{"slot":"lore:mara","before":null,"after":"Keeps the lamp."}]'::jsonb THEN RAISE EXCEPTION 'ops: %', ops; END IF;
END $$;
SELECT 'ok a slot is read and written as aethyr/forge/edit.coax names it; an edit''s ops are the slots that differ';

-- A run of edits through every revision path, each a step; each state kept to compare.
DO $$
DECLARE
  out jsonb;
  pid bigint;
BEGIN
  INSERT INTO states VALUES (0, pg_temp.now_doc('elog'));
  out := verse.realm_save('elog', jsonb_build_object('rev', pg_temp.rev('elog'), 'moved', '[{"entity":"trees:1","x":1.5,"z":-2}]'::jsonb,
                                                     'shaped', '[{"entity":"rocks:0","yaw":30,"scale":120,"gone":false}]'::jsonb), pg_temp.k1(), false);
  IF out->>'status' <> '200' THEN RAISE EXCEPTION 'save: %', out; END IF;
  INSERT INTO states VALUES (1, pg_temp.now_doc('elog'));
  UPDATE verse.made_realms SET time = 'night', tools = tools || '{"fly":true}' WHERE slug = 'elog';   -- settings
  INSERT INTO states VALUES (2, pg_temp.now_doc('elog'));
  UPDATE verse.made_realms SET off = off || '["rocks"]' WHERE slug = 'elog';   -- a row switched off
  INSERT INTO states VALUES (3, pg_temp.now_doc('elog'));
  out := verse.edit_lore('elog', '{"who":"mara","text":"She keeps the lighthouse."}', pg_temp.k1(), false);
  IF out->>'status' <> '200' OR NOT (pg_temp.now_doc('elog')->'lore' ? 'mara') THEN RAISE EXCEPTION 'lore: %', out; END IF;
  IF EXISTS (SELECT 1 FROM verse.realm_lore WHERE slug = 'elog') THEN RAISE EXCEPTION 'a made realm''s lore is in its document, not the table'; END IF;
  INSERT INTO states VALUES (4, pg_temp.now_doc('elog'));
  out := verse.edit_quest('elog', '{"giver":"mara","ask":"Light the lamp","steps":["Find oil","","Climb"],"reward":"A map"}', pg_temp.k1(), false);
  IF out->>'status' <> '200' THEN RAISE EXCEPTION 'quest: %', out; END IF;
  INSERT INTO states VALUES (5, pg_temp.now_doc('elog'));
  INSERT INTO verse.realm_proposals (slug, ms, by_player, change) VALUES ('elog', 1, 'p-log-2', '{"kind":"place","thing":"crate","x":3,"z":4,"yaw":0}') RETURNING id INTO pid;
  out := verse.edit_decide('elog', pid, true, pg_temp.k1(), false);
  IF out->>'status' <> '200' OR jsonb_array_length((SELECT placed FROM verse.made_realms WHERE slug = 'elog')) <> 1 THEN RAISE EXCEPTION 'accept: %', out; END IF;
  INSERT INTO states VALUES (6, pg_temp.now_doc('elog'));
  IF (SELECT jsonb_array_length(undo) FROM verse.made_realms WHERE slug = 'elog') <> 6 THEN RAISE EXCEPTION 'six steps: %', (SELECT undo FROM verse.made_realms WHERE slug = 'elog'); END IF;
  IF (SELECT undo -> -1 -> 'ops' -> 0 ->> 'slot' FROM verse.made_realms WHERE slug = 'elog') <> 'place:0'
     OR (SELECT undo -> -1 ->> 'card' FROM verse.made_realms WHERE slug = 'elog')::bigint <> pid THEN RAISE EXCEPTION 'the accept''s step names its slot and card'; END IF;
END $$;
SELECT 'ok a save, the settings, a row, lore, a quest and a card accepted are one step each, with their ops';

DO $$
DECLARE
  out jsonb;
  i int;
  pid bigint := (SELECT max(id) FROM verse.realm_proposals WHERE slug = 'elog');
BEGIN
  FOR i IN REVERSE 5..0 LOOP
    out := pg_temp.undo('elog');
    IF out->>'status' <> '200' THEN RAISE EXCEPTION 'undo to %: %', i, out; END IF;
    IF pg_temp.now_doc('elog') IS DISTINCT FROM (SELECT doc FROM states WHERE n = i) THEN
      RAISE EXCEPTION 'undo to % gave %, not %', i, pg_temp.now_doc('elog'), (SELECT doc FROM states WHERE n = i);
    END IF;
    IF i = 5 AND (SELECT state FROM verse.realm_proposals WHERE id = pid) <> 'waiting' THEN RAISE EXCEPTION 'the card waits again'; END IF;
  END LOOP;
  IF (pg_temp.undo('elog'))->>'error' <> 'nothing to undo' THEN RAISE EXCEPTION 'past the first step'; END IF;
  FOR i IN 1..6 LOOP
    out := pg_temp.redo('elog');
    IF out->>'status' <> '200' OR pg_temp.now_doc('elog') IS DISTINCT FROM (SELECT doc FROM states WHERE n = i) THEN RAISE EXCEPTION 'redo to %: %', i, out; END IF;
  END LOOP;
  IF (SELECT state FROM verse.realm_proposals WHERE id = pid) <> 'accepted' THEN RAISE EXCEPTION 'redo settles the card again'; END IF;
  IF (pg_temp.redo('elog'))->>'error' <> 'nothing to redo' THEN RAISE EXCEPTION 'past the last step'; END IF;
  IF strpos(verse.edit_spec('elog', pg_temp.k1(), false), E'\nredo 0\n') = 0 OR strpos(verse.edit_spec('elog', pg_temp.k1(), false), E'\nundo 6\n') = 0 THEN
    RAISE EXCEPTION 'spec: %', verse.edit_spec('elog', pg_temp.k1(), false);
  END IF;
  IF strpos(verse.edit_spec('elog', 'elog-player-key-00002', false), 'redo ') > 0 THEN RAISE EXCEPTION 'a visitor has no redo line'; END IF;
END $$;
SELECT 'ok undo walks every step back to the first state, redo walks forward to the last, cards with them';

DO $$
DECLARE
  out jsonb;
  r0 bigint;
  pid bigint;
BEGIN
  -- undo two, then a new edit drops what was undone
  PERFORM pg_temp.undo('elog'); PERFORM pg_temp.undo('elog');
  IF (SELECT jsonb_array_length(redo) FROM verse.made_realms WHERE slug = 'elog') <> 2 THEN RAISE EXCEPTION 'two to redo'; END IF;
  UPDATE verse.made_realms SET palette = 'autumn' WHERE slug = 'elog';
  IF (SELECT jsonb_array_length(redo) FROM verse.made_realms WHERE slug = 'elog') <> 0 THEN RAISE EXCEPTION 'a new edit drops the redo'; END IF;
  -- undo is per edit: a slot no longer as its step left it refuses (409) and changes nothing
  PERFORM set_config('verse.restoring', 'on', true);
  UPDATE verse.made_realms SET palette = 'winter' WHERE slug = 'elog';   -- changed with no step
  PERFORM set_config('verse.restoring', '', true);
  r0 := pg_temp.rev('elog');
  out := pg_temp.undo('elog');
  IF out->>'status' <> '409' OR pg_temp.rev('elog') <> r0 OR (SELECT palette FROM verse.made_realms WHERE slug = 'elog') <> 'winter' THEN RAISE EXCEPTION 'stale undo: %', out; END IF;
  -- stale revision, someone else, no revision
  IF (verse.realm_undo('elog', jsonb_build_object('rev', r0 - 1), pg_temp.k1(), false))->>'status' <> '409'
     OR (verse.realm_redo('elog', jsonb_build_object('rev', r0), 'elog-player-key-00002', false))->>'status' <> '403'
     OR (verse.realm_redo('elog', '{}', pg_temp.k1(), false))->>'status' <> '400' THEN RAISE EXCEPTION 'refusals'; END IF;
  -- a declined card is a step with no ops: undo has it wait, redo declines it again; the document stays
  UPDATE verse.made_realms SET palette = 'natural' WHERE slug = 'elog';
  INSERT INTO verse.realm_proposals (slug, ms, by_player, change) VALUES ('elog', 1, 'p-log-2', '{"kind":"place","thing":"barrel","x":1,"z":1,"yaw":0}') RETURNING id INTO pid;
  out := verse.edit_decide('elog', pid, false, pg_temp.k1(), false);
  IF out->>'status' <> '200' OR (SELECT undo -> -1 FROM verse.made_realms WHERE slug = 'elog') IS DISTINCT FROM jsonb_build_object('to', pg_temp.rev('elog'), 'card', pid, 'ops', '[]'::jsonb) THEN
    RAISE EXCEPTION 'decline step: %', (SELECT undo -> -1 FROM verse.made_realms WHERE slug = 'elog');
  END IF;
  out := pg_temp.undo('elog');
  IF out->>'status' <> '200' OR (SELECT state FROM verse.realm_proposals WHERE id = pid) <> 'waiting' THEN RAISE EXCEPTION 'undo a decline: %', out; END IF;
  out := pg_temp.redo('elog');
  IF out->>'status' <> '200' OR (SELECT state FROM verse.realm_proposals WHERE id = pid) <> 'declined' THEN RAISE EXCEPTION 'redo a decline: %', out; END IF;
END $$;
SELECT 'ok a new edit drops the redo; a step whose slots changed under it is refused whole; a declined card goes back and forth';

DO $$
DECLARE
  out jsonb;
  before jsonb;
  r1 bigint;
BEGIN
  -- a step kept before 0110 (no ops) still undoes its whole revision, and redoes it
  r1 := pg_temp.rev('elog');
  UPDATE verse.made_realms SET ground = 'sand' WHERE slug = 'elog';
  before := pg_temp.now_doc('elog');
  UPDATE verse.made_realms SET undo = jsonb_set(undo, '{-1}', (undo -> -1) - 'ops') WHERE slug = 'elog';
  out := pg_temp.undo('elog');
  IF out->>'status' <> '200' OR (SELECT ground FROM verse.made_realms WHERE slug = 'elog') <> 'grass' THEN RAISE EXCEPTION 'an older step: %', out; END IF;
  out := pg_temp.redo('elog');
  IF out->>'status' <> '200' OR pg_temp.now_doc('elog') IS DISTINCT FROM before THEN RAISE EXCEPTION 'redo an older step: % %', out, pg_temp.now_doc('elog'); END IF;
END $$;
SELECT 'ok a step kept before the log undoes and redoes its whole revision';

DO $$
DECLARE
  out jsonb;
  f verse.made_realms;
  m verse.made_realms;
  panes text;
BEGIN
  -- (the quest was undone above: give it again) publish with lore; then change it: a visitor reads the lore published, the owner the lore now
  PERFORM verse.edit_quest('elog', '{"giver":"mara","ask":"Light the lamp","steps":["Find oil","","Climb"],"reward":"A map"}', pg_temp.k1(), false);
  out := verse.made_publish('elog', jsonb_build_object('rev', pg_temp.rev('elog')), pg_temp.k1(), false);
  IF out->>'status' <> '200' THEN RAISE EXCEPTION 'publish: %', out; END IF;
  IF NOT ((SELECT doc FROM verse.realm_revisions WHERE slug = 'elog' AND rev = (out->>'rev')::bigint) -> 'lore' ? 'mara') THEN RAISE EXCEPTION 'the published revision holds the lore'; END IF;
  PERFORM verse.edit_lore('elog', '{"who":"mara","text":"She sold the lighthouse."}', pg_temp.k1(), false);
  panes := verse.edit_panes('elog', 'elog-player-key-00002', false);
  IF panes <> E'lore mara|She keeps the lighthouse.\nquest mara|Light the lamp|Find oil||Climb|A map\n' THEN RAISE EXCEPTION 'visitor panes: %', panes; END IF;
  IF verse.edit_panes('elog', pg_temp.k1(), false) NOT LIKE 'lore mara|She sold the lighthouse.%' THEN RAISE EXCEPTION 'owner panes'; END IF;
  -- undo brings the lore back (0055's tables never could)
  out := pg_temp.undo('elog');
  IF out->>'status' <> '200' OR verse.edit_panes('elog', pg_temp.k1(), false) NOT LIKE 'lore mara|She keeps the lighthouse.%' THEN RAISE EXCEPTION 'undo lore: %', out; END IF;
  -- a fork carries the lore and quests of its revision; its edits stay its own
  out := verse.made_fork('elog', '{"name":"Edit Log Visitor"}', 'elog-player-key-00002', 12);
  SELECT * INTO f FROM verse.made_realms WHERE slug = out->>'slug';
  IF out->>'status' <> '200' OR f.lore IS DISTINCT FROM '{"mara":"She keeps the lighthouse."}'::jsonb OR NOT (f.quests ? 'mara') THEN RAISE EXCEPTION 'fork: % %', out, f.lore; END IF;
  IF f.published_sha256 <> verse.realm_doc_sha(verse.realm_doc(f)) OR NOT (verse.realm_doc(f) ? 'lore') THEN RAISE EXCEPTION 'the fork''s hash covers its lore'; END IF;
  PERFORM verse.edit_lore(f.slug, '{"who":"mara","text":""}', 'elog-player-key-00002', false);
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'elog';
  IF NOT (m.lore ? 'mara') OR (SELECT lore FROM verse.made_realms WHERE slug = f.slug) <> '{}' THEN RAISE EXCEPTION 'a fork''s lore edit touched its parent'; END IF;
  -- limits and refusals as 0055's
  IF (verse.edit_lore('elog', '{"who":"Bad Name","text":"x"}', pg_temp.k1(), false))->>'status' <> '400'
     OR (verse.edit_lore('elog', '{"who":"mara","text":"x"}', 'elog-player-key-00002', false))->>'status' <> '403'
     OR (verse.edit_quest('elog', '{"giver":"mara","ask":"x"}', 'elog-player-key-00002', false))->>'status' <> '403' THEN RAISE EXCEPTION 'lore refusals'; END IF;
  PERFORM verse.edit_lore('elog', jsonb_build_object('who', 'c' || i, 'text', 'x'), pg_temp.k1(), false) FROM generate_series(1, 39) i;
  IF (verse.edit_lore('elog', '{"who":"one-more","text":"x"}', pg_temp.k1(), false))->>'status' <> '409' THEN RAISE EXCEPTION 'forty characters'; END IF;
END $$;
SELECT 'ok lore and quests are published, undone and forked with the document; a visitor reads the revision published';

DO $$
DECLARE
  m verse.made_realms;
  sha_old text;
  rev_old bigint;
  pub bigint;
BEGIN
  -- a realm made before 0110 with lore and quests in 0055's tables, published as it is
  INSERT INTO verse.made_realms (slug, world, name, style, ground, time, features, tools, made_ms, visibility) VALUES ('elog-old', 'made-elog-old', 'Old', 'lowpoly', 'grass', 'day', '[]', '{}', 1, 'public');
  INSERT INTO verse.realm_lore (slug, who, text, ms) VALUES ('elog-old', 'bram', 'Bakes at dawn.', 1);
  INSERT INTO verse.realm_quests (slug, giver, ask, steps, reward, ms) VALUES ('elog-old', 'bram', 'Fetch flour', '{"Mill","Cart"}', 'Bread', 1);
  SELECT rev, published_rev INTO rev_old, pub FROM verse.made_realms WHERE slug = 'elog-old';
  sha_old := (SELECT sha256 FROM verse.realm_revisions WHERE slug = 'elog-old' AND rev = rev_old);
  IF NOT verse.realm_lore_into_doc('elog-old') OR verse.realm_lore_into_doc('elog-old') THEN RAISE EXCEPTION 'moved in once'; END IF;
  SELECT * INTO m FROM verse.made_realms WHERE slug = 'elog-old';
  IF m.rev <> rev_old + 1 OR m.lore_since <> m.rev OR m.lore <> '{"bram":"Bakes at dawn."}' OR m.quests -> 'bram' <> '{"ask":"Fetch flour","steps":["Mill","Cart"],"reward":"Bread"}'
     OR jsonb_array_length(m.undo) <> 0 THEN RAISE EXCEPTION 'migrated: %', to_jsonb(m); END IF;
  IF (SELECT sha256 FROM verse.realm_revisions WHERE slug = 'elog-old' AND rev = rev_old) <> sha_old
     OR (SELECT why FROM verse.realm_revisions WHERE slug = 'elog-old' AND rev = m.rev) <> 'edit'
     OR (SELECT sha256 FROM verse.realm_revisions WHERE slug = 'elog-old' AND rev = m.rev) <> verse.realm_doc_sha(verse.realm_doc(m)) THEN
    RAISE EXCEPTION 'the older revision keeps its bytes and hash; the new one is kept';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM verse.realm_lore WHERE slug = 'elog-old') THEN RAISE EXCEPTION 'the tables keep their rows'; END IF;
  -- a visitor still reads the lore live until a revision at or after lore_since is published
  IF m.published_rev <> pub OR verse.edit_panes('elog-old', 'elog-player-key-00002', false) <> E'lore bram|Bakes at dawn.\nquest bram|Fetch flour|Mill|Cart||Bread\n' THEN
    RAISE EXCEPTION 'visitor panes after the move: %', verse.edit_panes('elog-old', 'elog-player-key-00002', false);
  END IF;
  PERFORM verse.made_publish('elog-old', jsonb_build_object('rev', m.rev), pg_temp.k1(), false);
  PERFORM verse.edit_lore('elog-old', '{"who":"bram","text":"Bakes at noon."}', pg_temp.k1(), false);
  IF verse.edit_panes('elog-old', 'elog-player-key-00002', false) NOT LIKE 'lore bram|Bakes at dawn.%' THEN RAISE EXCEPTION 'published lore after the move'; END IF;
  -- a hand-built realm keeps 0055's tables
  IF verse.made_lore_realm('nowhere') THEN RAISE EXCEPTION 'not a made realm'; END IF;
END $$;
SELECT 'ok lore and quests kept before move into the document as one revision; older bytes and hashes stay; visitors see no change';
SELECT 'done 7';
ROLLBACK;
