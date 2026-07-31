CREATE TABLE IF NOT EXISTS sarah_livekit_provider_disconnect_faults (
  request_ref                 text PRIMARY KEY CHECK (
    length(request_ref) BETWEEN 1 AND 256
  ),
  session_ref                 text NOT NULL
    REFERENCES sarah_realtime_voice_sessions (session_ref) ON DELETE CASCADE,
  generation                  bigint NOT NULL CHECK (generation >= 1),
  provider_session_ref_digest text NOT NULL CHECK (
    provider_session_ref_digest ~ '^[0-9a-f]{64}$'
  ),
  operator_actor_ref          text NOT NULL CHECK (
    length(operator_actor_ref) BETWEEN 1 AND 256
  ),
  requested_at                text NOT NULL,
  applied_at                  text,
  worker_job_ref              text,
  UNIQUE (session_ref, generation),
  CHECK (
    (applied_at IS NULL AND worker_job_ref IS NULL)
    OR (applied_at IS NOT NULL AND worker_job_ref IS NOT NULL)
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
      'interrupt_applied',
      'provider_disconnect_fault_applied',
      'response_usage',
      'transcription_usage',
      'close'
    )
  );
