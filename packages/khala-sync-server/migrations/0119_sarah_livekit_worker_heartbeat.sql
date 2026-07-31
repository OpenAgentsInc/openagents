ALTER TABLE sarah_livekit_room_bindings
  DROP CONSTRAINT IF EXISTS sarah_livekit_room_bindings_worker_stop_reason_check;

ALTER TABLE sarah_livekit_room_bindings
  ADD CONSTRAINT sarah_livekit_room_bindings_worker_stop_reason_check CHECK (
    worker_stop_reason IS NULL
    OR worker_stop_reason IN (
      'hold_exhausted',
      'membership_revoked',
      'operator_stop',
      'session_expired',
      'worker_unavailable'
    )
  );

ALTER TABLE sarah_livekit_worker_events
  DROP CONSTRAINT IF EXISTS sarah_livekit_worker_events_stop_reason_check;

ALTER TABLE sarah_livekit_worker_events
  ADD CONSTRAINT sarah_livekit_worker_events_stop_reason_check CHECK (
    stop_reason IS NULL
    OR stop_reason IN (
      'hold_exhausted',
      'membership_revoked',
      'operator_stop',
      'session_expired',
      'worker_unavailable'
    )
  );
