-- Large trajectory R2 offload for the agent trace store (openagents #6221,
-- epic #6206). ADDITIVE. A full real agent session (e.g. a ~793-step Claude
-- Code session ≈ 2.5MB redacted ATIF) exceeds D1's ~1MB-per-value limit, so the
-- public-safe trajectory JSON is stored in R2 and only a pointer is kept in D1.
--
-- Invariants (see apps/openagents.com/INVARIANTS.md "Trace Upload Data Market"):
-- * When `trajectory_r2_key` is NON-NULL, the full public-safe trajectory JSON
--   lives in R2 at that key and `trajectory_json` holds only a placeholder
--   ('{}'). The read path rehydrates the trajectory from R2 transparently — the
--   public-safe read projection is identical either way.
-- * When `trajectory_r2_key` IS NULL, the trajectory is small and is stored
--   inline in `trajectory_json` as before (#6208 behaviour, unchanged).
-- * R2 stores ONLY the same public-safe, already-tripwired trajectory
--   projection that D1 would have stored. No raw prompts, logs, provider
--   payloads, secrets, wallet/payment material, or PII — the ingest tripwire
--   still rejects leaks before anything is written to D1 OR R2.
ALTER TABLE agent_traces
    ADD COLUMN trajectory_r2_key TEXT;
