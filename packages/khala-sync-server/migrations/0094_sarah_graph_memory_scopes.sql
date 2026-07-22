-- #9189 composition root: a durable Cloud SQL backing store for hosted Sarah
-- graph memory. The hosted Sarah runtime runs on Cloud Run's ephemeral
-- filesystem, so the SQLite-on-disk custody used by OpenAgents Desktop
-- (desktop-graph-memory-persistence.ts) is not durable there. This table is
-- the Postgres equivalent: one atomically-replaced, opaque, already-redacted
-- SDK envelope per owner+project scope, keyed by owner-hashed scope refs.
--
-- The portable `@openagentsinc/agent-experience-memory` `GraphMemoryStore`
-- owns ALL graph semantics (consent, redaction, generation ordering,
-- compare-and-set, receipts, envelope shape). This row holds ONLY durable
-- state: the owner-hashed scope refs, a monotonic revision the SDK's
-- compare-and-set is keyed on, and the redacted envelope JSON. It never holds
-- a raw owner id, secret, token, private path, or email -- the SDK redaction
-- boundary and the owner-hashed `sarahGraphMemoryScope` guarantee that
-- upstream (see apps/openagents.com/workers/api/src/sarah-graph-memory-store.ts
-- and sarah-graph-memory.ts).
--
-- Default OFF: nothing writes or reads this table until
-- SARAH_GRAPH_MEMORY_RECALL_ENABLED is turned on at the composition root.

CREATE TABLE IF NOT EXISTS sarah_graph_memory_scopes (
  owner_scope   text        NOT NULL,
  project_scope text        NOT NULL,
  revision      bigint      NOT NULL CHECK (revision >= 0),
  envelope_json jsonb       NOT NULL,
  updated_at    timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (owner_scope, project_scope),
  CONSTRAINT sarah_graph_memory_scopes_owner_shape
    CHECK (owner_scope ~ '^[A-Za-z0-9][A-Za-z0-9._:/-]{0,255}$'),
  CONSTRAINT sarah_graph_memory_scopes_project_shape
    CHECK (project_scope ~ '^[A-Za-z0-9][A-Za-z0-9._:/-]{0,255}$')
);
