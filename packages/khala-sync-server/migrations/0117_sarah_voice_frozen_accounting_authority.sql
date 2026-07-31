-- #9285: Bind provider accounting to the exact admitted rate and provider
-- session. Existing in-flight rows stay nullable and fail closed in the
-- application because their original rate cannot be reconstructed safely.

ALTER TABLE sarah_voice_admissions
  ADD COLUMN IF NOT EXISTS credit_rate_msat_per_million_tokens bigint CHECK (
    credit_rate_msat_per_million_tokens IS NULL
    OR credit_rate_msat_per_million_tokens > 0
  ),
  ADD CONSTRAINT sarah_voice_admissions_credit_rate_required CHECK (
    credit_rate_msat_per_million_tokens IS NOT NULL
  ) NOT VALID;

ALTER TABLE sarah_realtime_voice_sessions
  ADD COLUMN IF NOT EXISTS credit_rate_msat_per_million_tokens bigint CHECK (
    credit_rate_msat_per_million_tokens IS NULL
    OR credit_rate_msat_per_million_tokens > 0
  ),
  ADD CONSTRAINT sarah_realtime_voice_sessions_credit_rate_required CHECK (
    credit_rate_msat_per_million_tokens IS NOT NULL
  ) NOT VALID;

ALTER TABLE sarah_livekit_accounting_reconciliations
  ADD COLUMN IF NOT EXISTS provider_session_ref_digest text CHECK (
    provider_session_ref_digest IS NULL
    OR provider_session_ref_digest ~ '^[0-9a-f]{64}$'
  ),
  ADD CONSTRAINT sarah_livekit_reconciliations_provider_session_required CHECK (
    provider_session_ref_digest IS NOT NULL
  ) NOT VALID;
