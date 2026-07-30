ALTER TABLE sarah_realtime_voice_sessions
  ADD COLUMN IF NOT EXISTS transport_kind text NOT NULL
    DEFAULT 'custom_wss_v1'
    CHECK (transport_kind IN ('custom_wss_v1', 'livekit_room_v1'));

CREATE TABLE IF NOT EXISTS sarah_livekit_provisioning_intents (
  session_ref           text PRIMARY KEY
    REFERENCES sarah_realtime_voice_sessions (session_ref) ON DELETE CASCADE,
  generation            bigint NOT NULL CHECK (generation >= 1),
  idempotency_key       text NOT NULL UNIQUE,
  owner_user_id         text NOT NULL,
  device_ref            text NOT NULL,
  thread_ref            text NOT NULL,
  capability_profile    text NOT NULL,
  admission_ref         text NOT NULL,
  admission_digest      text NOT NULL CHECK (
    admission_digest ~ '^[0-9a-f]{64}$'
  ),
  room_context_kind     text NOT NULL CHECK (
    room_context_kind IN ('private', 'community')
  ),
  community_ref         text,
  channel_ref           text,
  membership_revision  text,
  state                 text NOT NULL CHECK (
    state IN (
      'pending',
      'reconciling',
      'bound',
      'cleanup_failed',
      'cleaned'
    )
  ),
  created_at            text NOT NULL,
  updated_at            text NOT NULL,
  CHECK (
    (
      room_context_kind = 'private'
      AND community_ref IS NULL
      AND channel_ref IS NULL
      AND membership_revision IS NULL
    )
    OR
    (
      room_context_kind = 'community'
      AND community_ref IS NOT NULL
      AND channel_ref IS NOT NULL
      AND membership_revision IS NOT NULL
    )
  )
);

CREATE TABLE IF NOT EXISTS sarah_livekit_room_bindings (
  session_ref                    text PRIMARY KEY
    REFERENCES sarah_realtime_voice_sessions (session_ref) ON DELETE CASCADE,
  owner_user_id                  text NOT NULL,
  device_ref                     text NOT NULL,
  thread_ref                     text NOT NULL,
  generation                     bigint NOT NULL CHECK (generation >= 1),
  capability_profile             text NOT NULL,
  admission_ref                  text NOT NULL,
  admission_digest               text NOT NULL CHECK (
    admission_digest ~ '^[0-9a-f]{64}$'
  ),
  room_context_kind              text NOT NULL CHECK (
    room_context_kind IN ('private', 'community')
  ),
  community_ref                  text,
  channel_ref                    text,
  membership_revision           text,
  room_ref                       text NOT NULL UNIQUE,
  room_epoch                     bigint NOT NULL CHECK (room_epoch >= 1),
  participant_ref                text NOT NULL UNIQUE,
  sarah_participant_ref          text NOT NULL UNIQUE,
  participant_grant_digest       text NOT NULL CHECK (
    participant_grant_digest ~ '^[0-9a-f]{64}$'
  ),
  join_expires_at                text NOT NULL,
  dispatch_ref                   text NOT NULL UNIQUE,
  sarah_presence_lease_ref       text NOT NULL UNIQUE,
  publish_allowed                boolean NOT NULL,
  subscribe_allowed              boolean NOT NULL,
  state                          text NOT NULL CHECK (
    state IN (
      'prepared',
      'active',
      'cleanup_ready',
      'cleanup_failed',
      'cleaned'
    )
  ),
  owner_joined_at                text,
  sarah_joined_at                text,
  cleanup_attempted_at           text,
  cleaned_at                     text,
  created_at                     text NOT NULL,
  updated_at                     text NOT NULL,
  CHECK (
    (
      room_context_kind = 'private'
      AND community_ref IS NULL
      AND channel_ref IS NULL
      AND membership_revision IS NULL
    )
    OR
    (
      room_context_kind = 'community'
      AND community_ref IS NOT NULL
      AND channel_ref IS NOT NULL
      AND membership_revision IS NOT NULL
    )
  ),
  CHECK (length(admission_ref) > 0)
);

CREATE INDEX IF NOT EXISTS sarah_livekit_room_bindings_context_idx
  ON sarah_livekit_room_bindings (
    room_context_kind,
    community_ref,
    channel_ref,
    membership_revision,
    state
  );

CREATE INDEX IF NOT EXISTS sarah_livekit_room_bindings_cleanup_idx
  ON sarah_livekit_room_bindings (state, updated_at);
