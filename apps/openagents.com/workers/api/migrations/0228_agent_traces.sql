-- Agent trace store (openagents #6208, epic #6206). Persists the PUBLIC-SAFE
-- projection of an ATIF-v1.7 agent trajectory keyed by a uuid so the shareable
-- `/trace/{uuid}` surface can dereference real runs.
--
-- Invariants (see apps/openagents.com/INVARIANTS.md "Agent Trace Store"):
-- * `trajectory_json` holds ONLY the public-safe ATIF projection. It is
--   tripwired at ingest: secrets, tokens, wallet/payment material, PII, local
--   paths, and raw/split provider model ids are rejected before persistence
--   (only `openagents/khala`-class public ids are allowed). No raw prompts,
--   raw logs, or provider payloads are ever stored here.
-- * Large blobs (video/screenshots) live in R2; this table stores only
--   public-safe R2 reference keys in `blob_refs_json`, never the blob bytes.
-- * `visibility` is the share tier (public | unlisted | owner_only) enforced on
--   read. public/unlisted need no auth; owner_only requires the owner session.
-- * A trace is evidence only. It grants no accepted-work, payout, settlement,
--   or public-claim authority by itself.
CREATE TABLE IF NOT EXISTS agent_traces (
    trace_uuid TEXT PRIMARY KEY,
    owner_user_id TEXT NOT NULL,
    agent_ref TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    trajectory_id TEXT NOT NULL,
    session_id TEXT,
    visibility TEXT NOT NULL DEFAULT 'unlisted'
        CHECK (visibility IN ('public', 'unlisted', 'owner_only')),
    step_count INTEGER NOT NULL DEFAULT 0,
    trajectory_json TEXT NOT NULL DEFAULT '{}',
    blob_refs_json TEXT NOT NULL DEFAULT '[]',
    idempotency_key TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Owner-scoped listing (newest first).
CREATE INDEX IF NOT EXISTS idx_agent_traces_owner
    ON agent_traces (owner_user_id, created_at DESC);

-- Public discovery feed (newest first) over the `public` tier only.
CREATE INDEX IF NOT EXISTS idx_agent_traces_public
    ON agent_traces (visibility, created_at DESC);

-- Idempotent ingest: at most one stored trace per (owner, Idempotency-Key).
CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_traces_idempotency
    ON agent_traces (owner_user_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
