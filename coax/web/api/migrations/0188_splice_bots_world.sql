-- Splice is Splice Bots (GitHub #13486; docs/naming/2026-11-13-splice-bots-rename.md): the world's ID
-- and kind become 'splice-bots'. Its row moves with everything that names it: the undo log, the
-- outbox (each event's "world" field too), the import pack, imported files, realm scripts, script
-- reports and its attachments list (the scope key and the list's first line, which world-files.coax
-- checks against the world). The documents' first line (world.doc.v1 ID) is rewritten; the world's
-- code would open them under the old ID too (metaverse/splice-bots/world.coax), and a document's
-- digest does not cover its ID. The API reads the old ID as the new one
-- (metaverse-engine/editor/world-alias.coax), so a saved link or a CLI call that names 'splice' keeps
-- working; the old ID stays reserved so no realm takes it.
--
-- Imported files' objects in the private bucket keep their names (PREFIX/worlds/splice/files/HASH).
-- worlds.c reads an object only while it imports it; the import pack it makes is the row in
-- verse.world_packs, which moves. An import that is uploading while this runs is asked again.
--
-- Safe to run twice: once 'splice' has no row, nothing moves. A 'splice-bots' row that holds nothing
-- (an API that ran before this migration made it) gives way to the moved one; one that holds edits
-- stops the migration, for a person to look at.
CREATE OR REPLACE FUNCTION verse.realm_world_id_ok(s text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT s ~ '^[a-z0-9][a-z0-9-]{0,47}$' AND s NOT IN ('splice', 'splice-bots', 'everglade') $$;

DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM verse.world_docs WHERE id = 'splice') THEN
    INSERT INTO verse.world_docs (id, kind) VALUES ('splice-bots', 'splice-bots') ON CONFLICT (id) DO NOTHING;
    RETURN;
  END IF;
  DELETE FROM verse.world_docs WHERE id = 'splice-bots' AND kind = 'splice-bots'
    AND dev_rev = 0 AND steps = 0 AND live_rev = 0 AND prev_rev = 0
    AND NOT EXISTS (SELECT 1 FROM verse.world_outbox WHERE world = 'splice-bots')
    AND NOT EXISTS (SELECT 1 FROM verse.world_files WHERE world = 'splice-bots')
    AND NOT EXISTS (SELECT 1 FROM verse.world_scripts WHERE world = 'splice-bots');
  IF EXISTS (SELECT 1 FROM verse.world_docs WHERE id = 'splice-bots') THEN
    RAISE EXCEPTION 'verse.world_docs has a splice-bots row that holds edits beside splice: look before moving';
  END IF;
  INSERT INTO verse.world_docs
    SELECT (jsonb_populate_record(NULL::verse.world_docs, to_jsonb(d) || '{"id": "splice-bots", "kind": "splice-bots"}'::jsonb)).*
    FROM verse.world_docs d WHERE d.id = 'splice';
  UPDATE verse.world_docs SET
    dev_doc = regexp_replace(dev_doc, '^world\.doc\.v1 splice\n', E'world.doc.v1 splice-bots\n'),
    live_doc = regexp_replace(live_doc, '^world\.doc\.v1 splice\n', E'world.doc.v1 splice-bots\n'),
    prev_doc = regexp_replace(prev_doc, '^world\.doc\.v1 splice\n', E'world.doc.v1 splice-bots\n')
  WHERE id = 'splice-bots';
  UPDATE verse.world_steps SET world = 'splice-bots' WHERE world = 'splice';
  UPDATE verse.world_outbox SET world = 'splice-bots', body = replace(body, '"world":"splice"', '"world":"splice-bots"') WHERE world = 'splice';
  UPDATE verse.world_packs SET world = 'splice-bots' WHERE world = 'splice';
  UPDATE verse.world_files SET world = 'splice-bots' WHERE world = 'splice';
  UPDATE verse.world_scripts SET world = 'splice-bots' WHERE world = 'splice';
  UPDATE verse.script_reports SET world = 'splice-bots' WHERE world = 'splice';
  UPDATE verse.attachment_lists SET key = 'world:splice-bots',
    list = regexp_replace(list, '^world\.files\.v1 splice ', 'world.files.v1 splice-bots ')
  WHERE key = 'world:splice' AND NOT EXISTS (SELECT 1 FROM verse.attachment_lists WHERE key = 'world:splice-bots');
  DELETE FROM verse.world_docs WHERE id = 'splice';
END $$;
