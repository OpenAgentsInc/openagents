-- Realms are worlds with an ID (#13422, the browser editor's E13; docs/editor/2026-11-12-browser-editor.md):
-- every realm with a board, a hand-built one (verse.realm_rights) or a made one (verse.made_realms), has a row
-- in verse.world_docs of kind 'realm', so /api/worlds/<slug> reads its board and settings as content tables
-- and takes a switch as a world.edit.v1 cell edit (coax/web/api/worlds.c; the realm's own code is
-- metaverse/edit/realm-world.coax). The row holds the world's revision, its undo log and its outbox; the
-- realm's state stays where it was (verse.realm_edit, verse.made_realms), changed by the realm's own SQL
-- (verse.edit_switch, verse.edit_settings). Who decides is Coax: the authority table's rule for a realm made
-- at run time (metaverse-engine/editor/authority.coax realm_authority_of) and the route's (world-policy.coax).
--
-- A world's ID is a realm's slug, so it takes the slug's form: a lowercase letter or a digit first, 48 bytes
-- at most (realms.c slug_ok; world-edit.coax we_world_ok). Splice and Everglade have rows of their own kind
-- and are never a realm's world. Additive, and safe to run twice.
ALTER TABLE verse.world_docs DROP CONSTRAINT IF EXISTS world_docs_id_check;
ALTER TABLE verse.world_docs ADD CONSTRAINT world_docs_id_check CHECK (id ~ '^[a-z0-9][a-z0-9-]{0,47}$');

CREATE OR REPLACE FUNCTION verse.realm_world_id_ok(s text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT s ~ '^[a-z0-9][a-z0-9-]{0,47}$' AND s NOT IN ('splice', 'everglade') $$;

INSERT INTO verse.world_docs (id, kind)
  SELECT slug, 'realm' FROM verse.realm_rights WHERE verse.realm_world_id_ok(slug)
  UNION SELECT slug, 'realm' FROM verse.made_realms WHERE verse.realm_world_id_ok(slug)
ON CONFLICT (id) DO NOTHING;

-- A realm made from now on is a world when it is made, and stops being one when it goes (its steps and
-- events go with it).
CREATE OR REPLACE FUNCTION verse.realm_world_made() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    IF verse.realm_world_id_ok(NEW.slug) THEN
      INSERT INTO verse.world_docs (id, kind) VALUES (NEW.slug, 'realm') ON CONFLICT (id) DO NOTHING;
    END IF;
    RETURN NEW;
  END IF;
  DELETE FROM verse.world_docs WHERE id = OLD.slug AND kind = 'realm';
  RETURN OLD;
END $$;
DROP TRIGGER IF EXISTS realm_world_made ON verse.made_realms;
CREATE TRIGGER realm_world_made AFTER INSERT OR DELETE ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.realm_world_made();
DROP TRIGGER IF EXISTS realm_world_made ON verse.realm_rights;
CREATE TRIGGER realm_world_made AFTER INSERT OR DELETE ON verse.realm_rights FOR EACH ROW EXECUTE FUNCTION verse.realm_world_made();
