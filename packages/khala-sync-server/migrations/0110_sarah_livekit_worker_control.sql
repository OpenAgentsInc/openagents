ALTER TABLE sarah_livekit_provisioning_intents
  ADD COLUMN IF NOT EXISTS worker_control_token_digest text CHECK (
    worker_control_token_digest IS NULL
    OR worker_control_token_digest ~ '^[0-9a-f]{64}$'
  );

CREATE UNIQUE INDEX IF NOT EXISTS
  sarah_livekit_provisioning_intents_worker_token_idx
  ON sarah_livekit_provisioning_intents (worker_control_token_digest)
  WHERE worker_control_token_digest IS NOT NULL;

ALTER TABLE sarah_livekit_room_bindings
  DROP CONSTRAINT IF EXISTS
    sarah_livekit_room_bindings_participant_ref_key;

ALTER TABLE sarah_livekit_room_bindings
  DROP CONSTRAINT IF EXISTS
    sarah_livekit_room_bindings_sarah_participant_ref_key;

ALTER TABLE sarah_livekit_room_bindings
  ADD CONSTRAINT sarah_livekit_room_bindings_owner_participant_room_key
  UNIQUE (room_ref, participant_ref);

ALTER TABLE sarah_livekit_room_bindings
  ADD CONSTRAINT sarah_livekit_room_bindings_sarah_participant_room_key
  UNIQUE (room_ref, sarah_participant_ref);

ALTER TABLE sarah_livekit_room_bindings
  ADD COLUMN IF NOT EXISTS worker_control_token_digest text CHECK (
    worker_control_token_digest IS NULL
    OR worker_control_token_digest ~ '^[0-9a-f]{64}$'
  ),
  ADD COLUMN IF NOT EXISTS worker_job_ref text,
  ADD COLUMN IF NOT EXISTS worker_ref_digest text CHECK (
    worker_ref_digest IS NULL OR worker_ref_digest ~ '^[0-9a-f]{64}$'
  ),
  ADD COLUMN IF NOT EXISTS worker_room_sid text,
  ADD COLUMN IF NOT EXISTS worker_claimed_at text,
  ADD COLUMN IF NOT EXISTS worker_last_seen_at text,
  ADD COLUMN IF NOT EXISTS worker_closed_at text,
  ADD COLUMN IF NOT EXISTS worker_close_reason text;

CREATE UNIQUE INDEX IF NOT EXISTS
  sarah_livekit_room_bindings_worker_token_idx
  ON sarah_livekit_room_bindings (worker_control_token_digest)
  WHERE worker_control_token_digest IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS
  sarah_livekit_room_bindings_worker_job_idx
  ON sarah_livekit_room_bindings (worker_job_ref)
  WHERE worker_job_ref IS NOT NULL;

ALTER TABLE sarah_realtime_voice_usage
  ADD COLUMN IF NOT EXISTS usage_kind text NOT NULL DEFAULT 'response'
    CHECK (usage_kind IN ('response', 'transcription'));

CREATE TABLE IF NOT EXISTS sarah_livekit_worker_events (
  session_ref                 text NOT NULL
    REFERENCES sarah_realtime_voice_sessions (session_ref) ON DELETE CASCADE,
  generation                  bigint NOT NULL CHECK (generation >= 1),
  event_ref                   text NOT NULL CHECK (
    length(event_ref) BETWEEN 1 AND 256
  ),
  worker_job_ref              text NOT NULL CHECK (
    length(worker_job_ref) BETWEEN 1 AND 256
  ),
  worker_control_token_digest text NOT NULL CHECK (
    worker_control_token_digest ~ '^[0-9a-f]{64}$'
  ),
  event_kind                  text NOT NULL CHECK (
    event_kind IN (
      'worker_connected',
      'lease_check',
      'response_usage',
      'transcription_usage',
      'close'
    )
  ),
  event_payload_digest        text NOT NULL CHECK (
    event_payload_digest ~ '^[0-9a-f]{64}$'
  ),
  stop_reason                 text CHECK (
    stop_reason IS NULL
    OR stop_reason IN (
      'hold_exhausted',
      'membership_revoked',
      'operator_stop'
    )
  ),
  observed_at                 text NOT NULL,
  PRIMARY KEY (session_ref, generation, event_ref)
);

CREATE INDEX IF NOT EXISTS sarah_livekit_worker_events_job_idx
  ON sarah_livekit_worker_events (worker_job_ref, observed_at);
