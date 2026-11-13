-- Worlds with an ID (#13412, the browser editor's E3; docs/editor/2026-11-12-browser-editor.md,
-- docs/protocol/world-edit-v1.md): a world's document, its undo log and its outbox.
-- coax/web/api/worlds.c reads and writes these rows; the rules are Coax (world-policy.coax, and the
-- world's own code: Splice's is metaverse/splice/world-api.coax). SQL stores the rows, holds the row
-- lock a write takes, and keeps one outbox row for each saved event; it decides nothing.
--
-- A world has two revisions (metaverse-engine/editor/world-live.coax): `dev`, which edits change, and
-- `live`, which a publish copies from dev with the pins of the build that reads it (engine, rules,
-- content, wire). The live revision before the last pin change stays as `prev` for one release.
-- A document is world.doc.v1 text (metaverse-engine/editor/world-doc.coax); '' is the world before its
-- first edit, which its own code writes. Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS verse.world_docs (
  id text PRIMARY KEY CHECK (id ~ '^[a-z][a-z0-9-]{0,31}$'),
  kind text NOT NULL,                       -- whose Coax checks its edits: 'splice'
  dev_rev bigint NOT NULL DEFAULT 0 CHECK (dev_rev >= 0),
  dev_doc text NOT NULL DEFAULT '',
  done int NOT NULL DEFAULT 0 CHECK (done >= 0),      -- steps of the undo log that stand
  steps int NOT NULL DEFAULT 0 CHECK (steps >= done), -- steps the log holds
  live_rev bigint NOT NULL DEFAULT 0 CHECK (live_rev >= 0),   -- 0: nothing is published
  live_from bigint NOT NULL DEFAULT -1,     -- the dev revision live was published from
  live_doc text NOT NULL DEFAULT '',
  live_when int NOT NULL DEFAULT 0 CHECK (live_when IN (0, 1)),   -- 0 now, 1 next session
  live_engine int NOT NULL DEFAULT 0, live_rules int NOT NULL DEFAULT 0, live_content int NOT NULL DEFAULT 0, live_wire int NOT NULL DEFAULT 0,
  prev_rev bigint NOT NULL DEFAULT 0 CHECK (prev_rev >= 0),   -- 0: no revision before a pin change is kept
  prev_doc text NOT NULL DEFAULT '',
  prev_engine int NOT NULL DEFAULT 0, prev_rules int NOT NULL DEFAULT 0, prev_content int NOT NULL DEFAULT 0, prev_wire int NOT NULL DEFAULT 0
);
-- The undo log: step n's batch as it was taken, and the batch that takes it back (world.edit.v1).
CREATE TABLE IF NOT EXISTS verse.world_steps (
  world text NOT NULL REFERENCES verse.world_docs (id) ON DELETE CASCADE,
  n int NOT NULL CHECK (n >= 0),
  did text NOT NULL,
  inv text NOT NULL,
  PRIMARY KEY (world, n)
);
-- The realm-authoring context's outbox (proto/README.md "Domain events"): one row for each saved
-- event, written in the transaction that changes the document. `body` is the event's bytes as the
-- codec wrote them (metaverse-engine/editor/world-edit.coax): realm-authoring.world.edited.v1 on the
-- dev lane, one a revision; realm-authoring.world.published.v1 on the live lane.
CREATE TABLE IF NOT EXISTS verse.world_outbox (
  seq bigserial PRIMARY KEY,
  event text NOT NULL,
  world text NOT NULL REFERENCES verse.world_docs (id) ON DELETE CASCADE,
  lane text NOT NULL CHECK (lane IN ('dev', 'live')),
  rev bigint NOT NULL CHECK (rev >= 1),
  body text NOT NULL,
  at_ms bigint NOT NULL CHECK (at_ms > 0),
  UNIQUE (world, lane, rev)
);
-- A reader asks for a lane's events after the last revision it took, in order.
CREATE OR REPLACE FUNCTION verse.world_outbox_since(w text, l text, after bigint, n int) RETURNS SETOF verse.world_outbox
LANGUAGE sql STABLE AS $$ SELECT * FROM verse.world_outbox WHERE world = w AND lane = l AND rev > after ORDER BY rev LIMIT least(greatest(n, 0), 500) $$;
-- Splice is the first world with an ID.
INSERT INTO verse.world_docs (id, kind) VALUES ('splice', 'splice') ON CONFLICT (id) DO NOTHING;
