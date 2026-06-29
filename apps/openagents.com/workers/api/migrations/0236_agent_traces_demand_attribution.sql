-- Demand-origin attribution on captured agent traces (openagents #6298, epic
-- #6293/#6206). Auto-tags + segments a captured trace by demand origin so the
-- Khala trace corpus separates our own dogfood traffic (heartbeat / canary /
-- Terminal-Bench) from genuine external free-tier users with no manual step.
--
-- This mirrors the SAME demand-classification primitive already on the token
-- ledger (`token_usage_events`, migration 0232): the chat path resolves
-- `demandKind`/`demandSource` from the `x-openagents-demand-kind` /
-- `x-openagents-demand-source` headers for the recorder, and #6298 threads the
-- SAME resolved values into the trace emitter so trace and ledger always agree.
--
-- Invariants (see apps/openagents.com/INVARIANTS.md
-- "Captured Trace Demand-Origin Segmentation"):
-- * `demand_kind` is the bounded enum external | internal | own_capacity |
--   unlabeled. Forward-only: existing rows stay NULL and are treated as
--   `unlabeled` (the unclassified real-user default) on read.
-- * `demand_source` is a bounded attribution TOKEN (e.g. `heartbeat`,
--   `canary`, `harbor_terminal_bench`), not free-form content. Both columns are
--   bounded tokens, NOT trajectory content, so they are stored as dedicated
--   columns OUTSIDE the public-safe trajectory JSON and are never part of the
--   value-based tripwire's reject scan.
-- * Classification is fail-soft: a tagging error never blocks the completion or
--   the capture; an unclassifiable request defaults to `unlabeled`.
ALTER TABLE agent_traces
  ADD COLUMN demand_kind TEXT
    CHECK (
      demand_kind IS NULL
      OR demand_kind IN ('external', 'internal', 'own_capacity', 'unlabeled')
    );

ALTER TABLE agent_traces
  ADD COLUMN demand_source TEXT;

-- Corpus segmentation read: filter / count by demand origin (newest first).
CREATE INDEX IF NOT EXISTS idx_agent_traces_demand_kind
  ON agent_traces (demand_kind, created_at DESC);
