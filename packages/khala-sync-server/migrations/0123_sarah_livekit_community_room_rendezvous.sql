CREATE TABLE IF NOT EXISTS sarah_livekit_community_room_rendezvous (
  presence_lease_ref   text PRIMARY KEY
    REFERENCES sarah_livekit_room_authorities (presence_lease_ref) ON DELETE CASCADE,
  community_ref       text NOT NULL,
  channel_ref         text NOT NULL,
  membership_revision text NOT NULL CHECK (membership_revision ~ '^[0-9a-f]{64}$'),
  room_ref             text NOT NULL,
  room_epoch           bigint NOT NULL CHECK (room_epoch >= 1),
  session_ref          text NOT NULL,
  generation           integer NOT NULL CHECK (generation >= 1),
  expires_at           text NOT NULL,
  state                text NOT NULL CHECK (state IN ('active', 'retired')),
  retired_at           text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  UNIQUE (room_ref, room_epoch),
  CHECK (
    (state = 'active' AND retired_at IS NULL)
    OR (state = 'retired' AND retired_at IS NOT NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS sarah_livekit_community_room_rendezvous_active_idx
  ON sarah_livekit_community_room_rendezvous (community_ref, channel_ref)
  WHERE state = 'active';
