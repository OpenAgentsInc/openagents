-- #9285: An admitted provider can remain billable after its worker disappears.
-- Preserve the owner's hold until exact provider accounting is reconciled
-- instead of fabricating a terminal settlement from the usage seen so far.

ALTER TABLE sarah_realtime_voice_sessions
  DROP CONSTRAINT IF EXISTS sarah_realtime_voice_sessions_state_check;

ALTER TABLE sarah_realtime_voice_sessions
  ADD CONSTRAINT sarah_realtime_voice_sessions_state_check CHECK (
    state IN (
      'reserved',
      'connected',
      'accounting_uncertain',
      'settled',
      'released',
      'failed'
    )
  );

DROP INDEX IF EXISTS sarah_realtime_voice_owner_active_idx;

CREATE UNIQUE INDEX sarah_realtime_voice_owner_active_idx
  ON sarah_realtime_voice_sessions (owner_user_id)
  WHERE state IN ('reserved', 'connected', 'accounting_uncertain');

ALTER TABLE sarah_livekit_room_bindings
  ADD COLUMN IF NOT EXISTS provider_accounting_status text NOT NULL DEFAULT 'pending',
  ADD COLUMN IF NOT EXISTS provider_accounting_terminal_at text,
  ADD COLUMN IF NOT EXISTS provider_accounting_uncertain_at text,
  ADD COLUMN IF NOT EXISTS provider_accounting_uncertain_reason text,
  ADD CONSTRAINT sarah_livekit_room_bindings_provider_accounting_status_check CHECK (
    provider_accounting_status IN ('pending', 'exact', 'uncertain')
  ),
  ADD CONSTRAINT sarah_livekit_room_bindings_provider_accounting_shape_check CHECK (
    (
      provider_accounting_status = 'pending'
      AND provider_accounting_terminal_at IS NULL
      AND provider_accounting_uncertain_at IS NULL
      AND provider_accounting_uncertain_reason IS NULL
    )
    OR
    (
      provider_accounting_status = 'exact'
      AND provider_accounting_terminal_at IS NOT NULL
      AND provider_accounting_uncertain_at IS NULL
      AND provider_accounting_uncertain_reason IS NULL
    )
    OR
    (
      provider_accounting_status = 'uncertain'
      AND provider_accounting_terminal_at IS NULL
      AND provider_accounting_uncertain_at IS NOT NULL
      AND provider_accounting_uncertain_reason IS NOT NULL
    )
  );
