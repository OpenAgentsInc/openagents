-- The pages, link previews and Markdown of made realms and forks say "the Metaverse" where they
-- said "the Verse" (docs/naming/2026-11-12-metaverse-rename.md, #13405). Names only: no other
-- word, link or rule changes. The model prompts that say "the Verse" are agent behaviour and are
-- not touched here.
--
-- Each function's stored definition is rewritten in place, so its other text stays byte for byte
-- what the earlier migrations made it. A phrase that isn't found stops the migration: a rename
-- that changed nothing would otherwise pass silently.
DO $$
DECLARE
  r record;
  def text;
BEGIN
  FOR r IN SELECT * FROM (VALUES
    -- the one-line description of a made realm (link previews, the realm list)
    ('verse.builder_line_of_0110(verse.made_realms)', ' A realm built on the Verse.', ' A realm built on the OpenAgents Metaverse.'),
    ('verse.builder_line_of(verse.made_realms)', ' A realm built on the Verse.', ' A realm built on the OpenAgents Metaverse.'),
    ('verse.builder_line_of(verse.made_realms)', ' A world built on the Verse.', ' A world built on the OpenAgents Metaverse.'),
    -- index.md of a made realm
    ('verse.builder_md_0071(text)', 'A realm on the Verse at ', 'A realm on the OpenAgents Metaverse at '),
    -- the page of a made realm: its title and its picture's words
    ('verse.builder_page_0071(text, text)', ' · a realm on the Verse · OpenAgents</title>', ' · a realm on the Metaverse · OpenAgents</title>'),
    ('verse.builder_page_0071(text, text)', ', a realm on the Verse">', ', a realm on the Metaverse">'),
    -- a fork's one-line description and its picture's words
    ('verse.fork_line(verse.made_realms)', ' || '' on the Verse''', ' || '' on the OpenAgents Metaverse'''),
    ('verse.fork_page_0110(text, text)', ' || '' on the Verse">', ' || '' on the Metaverse">')
  ) AS t(fn, old, new) LOOP
    def := pg_get_functiondef(r.fn::regprocedure);
    IF position(r.old IN def) = 0 THEN RAISE EXCEPTION '0178: % does not say %', r.fn, r.old; END IF;
    EXECUTE replace(def, r.old, r.new);
  END LOOP;
END $$;
