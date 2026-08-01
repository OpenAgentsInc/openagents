-- #9285: Provider-accounting uncertainty must preserve the complete credit
-- hold, but it must not deny the owner all later voice sessions forever.
-- After the bounded reconciliation window the maintenance worker records a
-- durable escalation and removes only the voice-concurrency lock. Exact
-- provider evidence is still required before charge or hold settlement.

ALTER TABLE sarah_realtime_voice_sessions
  ADD COLUMN IF NOT EXISTS accounting_escalated_at text,
  ADD COLUMN IF NOT EXISTS accounting_escalation_ref text UNIQUE,
  ADD CONSTRAINT sarah_realtime_voice_accounting_escalation_shape_check CHECK (
    (
      accounting_escalated_at IS NULL
      AND accounting_escalation_ref IS NULL
    )
    OR
    (
      state = 'accounting_uncertain'
      AND accounting_escalated_at IS NOT NULL
      AND accounting_escalation_ref IS NOT NULL
    )
    OR
    (
      state IN ('settled', 'released')
      AND accounting_escalated_at IS NOT NULL
      AND accounting_escalation_ref IS NOT NULL
    )
  );

DROP INDEX IF EXISTS sarah_realtime_voice_owner_active_idx;

CREATE UNIQUE INDEX sarah_realtime_voice_owner_active_idx
  ON sarah_realtime_voice_sessions (owner_user_id)
  WHERE state IN ('reserved', 'connected')
    OR (
      state = 'accounting_uncertain'
      AND accounting_escalated_at IS NULL
    );

CREATE INDEX IF NOT EXISTS sarah_realtime_voice_accounting_escalation_due_idx
  ON sarah_realtime_voice_sessions (updated_at, session_ref)
  WHERE state = 'accounting_uncertain'
    AND accounting_escalated_at IS NULL;
