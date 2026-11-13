-- A world's realm scripts on the API (GitHub #13467; docs/protocol/world-scripts-v1.md,
-- docs/research/2026-11-13-realm-scripts.md). A world names a script by its module's SHA-256, in a
-- row of its content tables (metaverse-engine/script/rows.coax). The API stores a module's bytes
-- under that hash only once the module check passes (metaverse-engine/script/wasm.coax), with the
-- manifest it wrote from the checked bytes. coax/web/api/scripts.c reads and writes these rows;
-- the rules are Coax (script-policy.coax). SQL stores the rows and decides nothing.

-- A stored module. bytes: 8 to 65,536, the module check's limit. manifest: the realm.script.v1
-- line the server wrote. author: the account (or the key no account holds) that stored it.
-- source: the Coax source it was compiled from, when the API's compile route made it; null for a
-- module that was uploaded. A world stores at most 64 (script-policy.coax); one that no lane's
-- rows name is collected a day after it was stored, when room is needed.
CREATE TABLE IF NOT EXISTS verse.world_scripts (
  world text NOT NULL REFERENCES verse.world_docs (id) ON DELETE CASCADE,
  hash text NOT NULL CHECK (hash ~ '^[0-9a-f]{64}$'),
  bytes bytea NOT NULL CHECK (octet_length(bytes) BETWEEN 8 AND 65536),
  manifest text NOT NULL CHECK (left(manifest, 1) = '{' AND length(manifest) <= 8192),
  author text NOT NULL CHECK (length(author) BETWEEN 1 AND 80),
  source text CHECK (source IS NULL OR length(source) BETWEEN 1 AND 65536),
  at_ms bigint NOT NULL CHECK (at_ms > 0),
  PRIMARY KEY (world, hash)
);

-- A module switched off everywhere, by hash: it is not served and no new edit names it. A world
-- whose published rows name it keeps its revision; its page and its room skip the script.
CREATE TABLE IF NOT EXISTS verse.script_off (
  hash text PRIMARY KEY CHECK (hash ~ '^[0-9a-f]{64}$'),
  why text NOT NULL CHECK (length(why) BETWEEN 1 AND 160),
  at_ms bigint NOT NULL CHECK (at_ms > 0),
  by text NOT NULL CHECK (length(by) BETWEEN 1 AND 80)
);

-- A report of a script, by any account that is logged in. status: open until an admin keeps the
-- script (kept) or switches it off (removed).
CREATE TABLE IF NOT EXISTS verse.script_reports (
  id bigserial PRIMARY KEY,
  world text NOT NULL CHECK (world ~ '^[a-z0-9][a-z0-9-]{0,47}$'),
  hash text NOT NULL CHECK (hash ~ '^[0-9a-f]{64}$'),
  reporter text NOT NULL CHECK (length(reporter) BETWEEN 1 AND 80),
  why text NOT NULL CHECK (length(why) BETWEEN 1 AND 160),
  at_ms bigint NOT NULL CHECK (at_ms > 0),
  status text NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'kept', 'removed'))
);
CREATE INDEX IF NOT EXISTS script_reports_open ON verse.script_reports (id) WHERE status = 'open';
CREATE INDEX IF NOT EXISTS script_reports_hash ON verse.script_reports (hash);
