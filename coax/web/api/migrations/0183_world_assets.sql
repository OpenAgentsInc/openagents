-- A world's imported assets on the API (GitHub #13420, the browser editor's E11;
-- docs/protocol/world-assets-v1.md). A file dropped in the editor goes straight to storage, named
-- by its SHA-256, and the API packs it into the world's import pack
-- (metaverse-engine/editor/pack-import.coax). coax/web/api/worlds.c reads and writes these rows;
-- the rules are Coax (world-policy.coax, pack-import-policy.coax). SQL stores the rows and
-- decides nothing.
--
-- A lane names its pack by hash, as it holds its document: dev's changes with an import, live's
-- is dev's at a publish, and the one before the last pin change stays for one release. '' means
-- the world has imported nothing.
ALTER TABLE verse.world_docs
  ADD COLUMN IF NOT EXISTS dev_pack text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS live_pack text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS prev_pack text NOT NULL DEFAULT '';
-- A pack's bytes (a VPK1 file), by the SHA-256 of them. One megabyte at most: Splice's budget
-- (metaverse/splice/pieces.coax SP_IMPORT_PACK). A pack no lane names is deleted by worlds.c.
CREATE TABLE IF NOT EXISTS verse.world_packs (
  world text NOT NULL REFERENCES verse.world_docs (id) ON DELETE CASCADE,
  hash text NOT NULL CHECK (hash ~ '^[0-9a-f]{64}$'),
  bytes bytea NOT NULL CHECK (octet_length(bytes) BETWEEN 1 AND 1048576),
  at_ms bigint NOT NULL CHECK (at_ms > 0),
  PRIMARY KEY (world, hash)
);
-- A file an import took, as it was dropped: its bytes are the object
-- PREFIX/worlds/WORLD/files/HASH in the private bucket (drops.c). kind: 1 mesh, 2 texture,
-- 3 sound. licence: its number in the importer's list (pack-import-policy.coax PIP_LICENCE).
CREATE TABLE IF NOT EXISTS verse.world_files (
  world text NOT NULL REFERENCES verse.world_docs (id) ON DELETE CASCADE,
  hash text NOT NULL CHECK (hash ~ '^[0-9a-f]{64}$'),
  name text NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
  kind int NOT NULL CHECK (kind BETWEEN 1 AND 3),
  licence int NOT NULL CHECK (licence >= 0),
  bytes bigint NOT NULL CHECK (bytes >= 1),
  at_ms bigint NOT NULL CHECK (at_ms > 0),
  PRIMARY KEY (world, hash)
);
