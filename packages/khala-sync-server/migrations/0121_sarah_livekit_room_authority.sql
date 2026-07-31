CREATE TABLE IF NOT EXISTS sarah_livekit_room_authorities (
  presence_lease_ref  text PRIMARY KEY
    REFERENCES sarah_livekit_room_bindings (sarah_presence_lease_ref) ON DELETE CASCADE,
  session_ref         text NOT NULL UNIQUE
    REFERENCES sarah_livekit_room_bindings (session_ref) ON DELETE CASCADE,
  generation          bigint NOT NULL CHECK (generation >= 1),
  community_ref       text NOT NULL,
  channel_ref         text NOT NULL,
  membership_revision text NOT NULL CHECK (membership_revision ~ '^[0-9a-f]{64}$'),
  room_ref            text NOT NULL,
  room_epoch          bigint NOT NULL CHECK (room_epoch >= 1),
  revision            bigint NOT NULL CHECK (revision >= 1),
  snapshot_json       jsonb NOT NULL,
  created_at          text NOT NULL,
  updated_at          text NOT NULL,
  UNIQUE (room_ref, room_epoch)
);

CREATE INDEX IF NOT EXISTS sarah_livekit_room_authorities_context_idx
  ON sarah_livekit_room_authorities (
    community_ref,
    channel_ref,
    membership_revision
  );
