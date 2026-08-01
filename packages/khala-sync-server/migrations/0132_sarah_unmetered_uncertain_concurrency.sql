-- #9285: Owner-waived sessions have no credit hold to preserve. Their durable
-- unmetered authority capture remains available while a later session starts.

DROP INDEX IF EXISTS sarah_realtime_voice_owner_active_idx;

CREATE UNIQUE INDEX sarah_realtime_voice_owner_active_idx
  ON sarah_realtime_voice_sessions (owner_user_id)
  WHERE state IN ('reserved', 'connected')
    OR (
      state = 'accounting_uncertain'
      AND credit_mode <> 'owner_waived_unmetered'
      AND accounting_escalated_at IS NULL
    );

DROP INDEX IF EXISTS sarah_realtime_voice_accounting_escalation_due_idx;

CREATE INDEX sarah_realtime_voice_accounting_escalation_due_idx
  ON sarah_realtime_voice_sessions (updated_at, session_ref)
  WHERE state = 'accounting_uncertain'
    AND credit_mode <> 'owner_waived_unmetered'
    AND accounting_escalated_at IS NULL;
