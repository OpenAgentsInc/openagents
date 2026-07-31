ALTER TABLE sarah_livekit_room_bindings
  ADD COLUMN IF NOT EXISTS worker_interrupt_applied_sequence bigint NOT NULL DEFAULT 0
    CHECK (
      worker_interrupt_applied_sequence >= 0
      AND worker_interrupt_applied_sequence <= worker_interrupt_sequence
    ),
  ADD COLUMN IF NOT EXISTS worker_interrupt_applied_at text;

ALTER TABLE sarah_livekit_worker_events
  DROP CONSTRAINT IF EXISTS sarah_livekit_worker_events_event_kind_check;

ALTER TABLE sarah_livekit_worker_events
  ADD CONSTRAINT sarah_livekit_worker_events_event_kind_check CHECK (
    event_kind IN (
      'worker_connected',
      'provider_admitted',
      'lease_check',
      'interrupt_applied',
      'response_usage',
      'transcription_usage',
      'close'
    )
  );
