-- #9285: Bind provider accounting to the exact admitted rate and provider
-- session. Pre-migration rows cannot be assigned a rate without inventing
-- accounting authority, so quarantine active legacy rows while preserving
-- their holds. New rows default to admitted_v1 and require a frozen rate.

ALTER TABLE sarah_voice_admissions
  ADD COLUMN IF NOT EXISTS credit_rate_msat_per_million_tokens bigint CHECK (
    credit_rate_msat_per_million_tokens IS NULL
    OR credit_rate_msat_per_million_tokens > 0
  ),
  ADD COLUMN IF NOT EXISTS accounting_rate_authority text NOT NULL
    DEFAULT 'legacy_unresolved';

ALTER TABLE sarah_voice_admissions
  ALTER COLUMN accounting_rate_authority SET DEFAULT 'admitted_v1',
  ADD CONSTRAINT sarah_voice_admissions_credit_rate_authority_check CHECK (
    (
      accounting_rate_authority = 'legacy_unresolved'
      AND credit_rate_msat_per_million_tokens IS NULL
      AND state = 'expired'
    )
    OR
    (
      accounting_rate_authority = 'admitted_v1'
      AND credit_rate_msat_per_million_tokens IS NOT NULL
    )
  ) NOT VALID;

ALTER TABLE sarah_realtime_voice_sessions
  ADD COLUMN IF NOT EXISTS credit_rate_msat_per_million_tokens bigint CHECK (
    credit_rate_msat_per_million_tokens IS NULL
    OR credit_rate_msat_per_million_tokens > 0
  ),
  ADD COLUMN IF NOT EXISTS accounting_rate_authority text NOT NULL
    DEFAULT 'legacy_unresolved';

ALTER TABLE sarah_realtime_voice_sessions
  ALTER COLUMN accounting_rate_authority SET DEFAULT 'admitted_v1',
  ADD CONSTRAINT sarah_realtime_voice_sessions_credit_rate_authority_check CHECK (
    (
      accounting_rate_authority = 'legacy_unresolved'
      AND credit_rate_msat_per_million_tokens IS NULL
      AND (
        state = 'accounting_uncertain'
        OR (
          state = 'released'
          AND connected_at IS NULL
          AND charged_msat = 0
        )
      )
    )
    OR
    (
      accounting_rate_authority = 'admitted_v1'
      AND credit_rate_msat_per_million_tokens IS NOT NULL
    )
  ) NOT VALID;

WITH legacy_active AS (
  SELECT session_ref
  FROM sarah_realtime_voice_sessions
  WHERE accounting_rate_authority = 'legacy_unresolved'
    AND state = 'connected'
),
clock AS (
  SELECT
    to_char(
      CURRENT_TIMESTAMP AT TIME ZONE 'UTC',
      'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'
    ) AS requested_at,
    to_char(
      (CURRENT_TIMESTAMP + INTERVAL '15 seconds') AT TIME ZONE 'UTC',
      'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'
    ) AS deadline_at
)
UPDATE sarah_livekit_room_bindings AS binding
SET worker_stop_reason = COALESCE(binding.worker_stop_reason, 'operator_stop'),
    worker_stop_close_reason = COALESCE(
      binding.worker_stop_close_reason,
      'legacy_accounting_authority_unavailable'
    ),
    worker_stop_requested_at = COALESCE(
      binding.worker_stop_requested_at,
      clock.requested_at
    ),
    worker_stop_deadline_at = COALESCE(
      binding.worker_stop_deadline_at,
      clock.deadline_at
    ),
    updated_at = clock.requested_at
FROM legacy_active, clock
WHERE binding.session_ref = legacy_active.session_ref
  AND binding.worker_closed_at IS NULL;

UPDATE sarah_voice_admissions
SET state = 'expired'
WHERE accounting_rate_authority = 'legacy_unresolved'
  AND state = 'active';

ALTER TABLE sarah_livekit_accounting_reconciliations
  ADD COLUMN IF NOT EXISTS provider_session_ref_digest text CHECK (
    provider_session_ref_digest IS NULL
    OR provider_session_ref_digest ~ '^[0-9a-f]{64}$'
  ),
  ADD CONSTRAINT sarah_livekit_reconciliations_provider_session_required CHECK (
    provider_session_ref_digest IS NOT NULL
  ) NOT VALID;
