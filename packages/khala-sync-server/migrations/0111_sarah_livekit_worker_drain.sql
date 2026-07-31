ALTER TABLE sarah_livekit_room_bindings
  ADD COLUMN IF NOT EXISTS worker_stop_reason text CHECK (
    worker_stop_reason IS NULL
    OR worker_stop_reason IN (
      'hold_exhausted',
      'membership_revoked',
      'operator_stop',
      'session_expired'
    )
  ),
  ADD COLUMN IF NOT EXISTS worker_stop_close_reason text,
  ADD COLUMN IF NOT EXISTS worker_stop_requested_at text,
  ADD COLUMN IF NOT EXISTS worker_stop_deadline_at text,
  ADD CONSTRAINT sarah_livekit_room_bindings_worker_stop_shape_check CHECK (
    (
      worker_stop_reason IS NULL
      AND worker_stop_close_reason IS NULL
      AND worker_stop_requested_at IS NULL
      AND worker_stop_deadline_at IS NULL
    )
    OR
    (
      worker_stop_reason IS NOT NULL
      AND worker_stop_close_reason IS NOT NULL
      AND worker_stop_requested_at IS NOT NULL
      AND worker_stop_deadline_at IS NOT NULL
    )
  );

CREATE INDEX IF NOT EXISTS sarah_livekit_room_bindings_worker_stop_idx
  ON sarah_livekit_room_bindings (worker_stop_deadline_at)
  WHERE worker_stop_deadline_at IS NOT NULL
    AND worker_closed_at IS NULL;

ALTER TABLE sarah_livekit_worker_events
  DROP CONSTRAINT IF EXISTS sarah_livekit_worker_events_stop_reason_check;

ALTER TABLE sarah_livekit_worker_events
  ADD CONSTRAINT sarah_livekit_worker_events_stop_reason_check CHECK (
    stop_reason IS NULL
    OR stop_reason IN (
      'hold_exhausted',
      'membership_revoked',
      'operator_stop',
      'session_expired'
    )
  );
