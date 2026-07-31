CREATE TABLE IF NOT EXISTS sarah_livekit_room_members (
  presence_lease_ref   text NOT NULL
    REFERENCES sarah_livekit_room_authorities (presence_lease_ref) ON DELETE CASCADE,
  owner_user_id        text NOT NULL,
  user_ref_digest      text NOT NULL CHECK (user_ref_digest ~ '^[0-9a-f]{64}$'),
  member_pubkey        text NOT NULL CHECK (member_pubkey ~ '^[0-9a-f]{64}$'),
  participant_ref      text NOT NULL,
  membership_revision text NOT NULL CHECK (membership_revision ~ '^[0-9a-f]{64}$'),
  room_ref             text NOT NULL,
  room_epoch           bigint NOT NULL CHECK (room_epoch >= 1),
  participant_grant_digest text NOT NULL CHECK (
    participant_grant_digest ~ '^[0-9a-f]{64}$'
  ),
  join_expires_at      text NOT NULL,
  state                text NOT NULL CHECK (state IN ('active', 'removed')),
  joined_at            text,
  removed_at           text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  PRIMARY KEY (presence_lease_ref, owner_user_id),
  UNIQUE (presence_lease_ref, user_ref_digest),
  UNIQUE (room_ref, room_epoch, participant_ref),
  CHECK (
    (state = 'active' AND removed_at IS NULL)
    OR (state = 'removed' AND removed_at IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS sarah_livekit_room_members_active_digest_idx
  ON sarah_livekit_room_members (presence_lease_ref, user_ref_digest)
  WHERE state = 'active';
