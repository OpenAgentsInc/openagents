ALTER TABLE sarah_realtime_voice_usage
  ADD COLUMN IF NOT EXISTS provider_status text CHECK (
    provider_status IS NULL
    OR provider_status IN ('completed', 'cancelled', 'failed', 'incomplete')
  );

ALTER TABLE sarah_livekit_room_bindings
  ADD COLUMN IF NOT EXISTS provider_session_ref_digest text CHECK (
    provider_session_ref_digest IS NULL
    OR provider_session_ref_digest ~ '^[0-9a-f]{64}$'
  ),
  ADD COLUMN IF NOT EXISTS provider_configuration_digest text CHECK (
    provider_configuration_digest IS NULL
    OR provider_configuration_digest ~ '^[0-9a-f]{64}$'
  ),
  ADD COLUMN IF NOT EXISTS provider_admitted_at text,
  ADD CONSTRAINT sarah_livekit_room_bindings_provider_admission_shape_check CHECK (
    (
      provider_session_ref_digest IS NULL
      AND provider_configuration_digest IS NULL
      AND provider_admitted_at IS NULL
    )
    OR
    (
      provider_session_ref_digest IS NOT NULL
      AND provider_configuration_digest IS NOT NULL
      AND provider_admitted_at IS NOT NULL
    )
  );

ALTER TABLE sarah_livekit_worker_events
  DROP CONSTRAINT IF EXISTS sarah_livekit_worker_events_event_kind_check;

ALTER TABLE sarah_livekit_worker_events
  ADD CONSTRAINT sarah_livekit_worker_events_event_kind_check CHECK (
    event_kind IN (
      'worker_connected',
      'provider_admitted',
      'lease_check',
      'response_usage',
      'transcription_usage',
      'close'
    )
  );
