-- A project realm's published "path" tag names its source folder. The realms' folder moved from
-- verse/ to metaverse/ (docs/naming/2026-11-12-metaverse-rename.md, #13405), so the next publish
-- of a project realm says metaverse/SLUG/. Events already published keep the path they were
-- signed with.
--
-- The function's stored definition is rewritten in place, as 0178 does, so the rest of it stays
-- byte for byte what the earlier migrations made it. If the old prefix isn't found the migration
-- stops: a change that changed nothing would otherwise pass silently.
DO $$
DECLARE
  def text;
  old text := 'drops.tag(''path'', rc, ''verse/'' || p.slug || ''/'')';
  new text := 'drops.tag(''path'', rc, ''metaverse/'' || p.slug || ''/'')';
BEGIN
  def := pg_get_functiondef('drops.publish_next(text)'::regprocedure);
  IF position(old IN def) = 0 THEN RAISE EXCEPTION '0180: drops.publish_next does not build the path from verse/'; END IF;
  EXECUTE replace(def, old, new);
END $$;
