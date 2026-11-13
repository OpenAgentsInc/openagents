-- Attachments on a project and its worlds (GitHub #13468; docs/protocol/world-files-v1.md):
-- reference files a maker attaches (concept art, design documents, notes, links). They are never
-- in a pack and never reach a player. coax/web/api/worlds.c reads and writes these rows; the rules
-- and every byte of a list are Coax (world-files.coax, metaverse-engine/editor/file-store.coax,
-- the code the dev server runs too). SQL stores the rows and decides nothing.
--
-- One row a scope: a world's own attachments ('world:ID') and a project's own ('project:SLUG'),
-- each a world.files.v1 list. project: the slug the scope belongs to ('' for a world with none),
-- so a project's limits count all its worlds. count and bytes are the list's own, kept beside it.
-- A file's bytes are the object PREFIX/attachments/HASH in the private bucket that project drops
-- use (drops.c); it is deleted when no list names it. Nothing about a file is logged.
CREATE TABLE IF NOT EXISTS verse.attachment_lists (
  key text PRIMARY KEY CHECK (key ~ '^(world|project):[a-z][a-z0-9-]{0,47}$'),
  project text NOT NULL CHECK (project ~ '^([a-z][a-z0-9-]{0,47})?$'),
  list text NOT NULL CHECK (left(list, 15) = 'world.files.v1 ' AND length(list) <= 180000),
  count int NOT NULL CHECK (count BETWEEN 0 AND 200),
  bytes bigint NOT NULL CHECK (bytes BETWEEN 0 AND 1073741824),
  at_ms bigint NOT NULL CHECK (at_ms > 0)
);
CREATE INDEX IF NOT EXISTS attachment_lists_project ON verse.attachment_lists (project) WHERE project <> '';
