-- EP263-LK H4/H5 (#9282): make LiveKit room cleanup and shared-room member
-- retirement converge instead of running forever.
--
-- H4. `claimLiveKitCleanups` re-selected every `cleanup_failed` binding behind
-- a flat 15 second gate with no attempt count and no terminal give-up state, so
-- a binding whose room the broker can never delete was retried for as long as
-- the service ran. Production carried 15 such rows, re-attempted continuously
-- for hours. Add a bounded attempt count, an explicit next-attempt time for
-- exponential backoff, and a visible terminal state.
--
-- `cleanup_abandoned` is deliberately a NEW state rather than a reuse of
-- `cleaned`: a room we gave up on is not a room we cleaned, and an operator
-- must be able to see the difference. It stays outside the
-- `state IN ('prepared','active')` authority predicates exactly as
-- `cleanup_failed` already did, so this changes resource behavior only.
--
-- H5. `sarah_livekit_room_members` rows were never retired: the worker-close
-- handler retired the community rendezvous beside them but nothing ever set
-- `removed_at`. Add the index the retirement sweep needs; the writers land in
-- the store.

ALTER TABLE sarah_livekit_room_bindings
  ADD COLUMN IF NOT EXISTS cleanup_attempt_count integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS cleanup_next_attempt_at text,
  ADD COLUMN IF NOT EXISTS cleanup_abandoned_at text,
  ADD CONSTRAINT sarah_livekit_room_bindings_cleanup_attempt_count_check CHECK (
    cleanup_attempt_count >= 0
  );

ALTER TABLE sarah_livekit_room_bindings
  DROP CONSTRAINT IF EXISTS sarah_livekit_room_bindings_state_check;

ALTER TABLE sarah_livekit_room_bindings
  ADD CONSTRAINT sarah_livekit_room_bindings_state_check CHECK (
    state IN (
      'prepared',
      'active',
      'cleanup_ready',
      'cleanup_failed',
      'cleanup_abandoned',
      'cleaned'
    )
  );

-- The give-up must be legible from the row alone: abandoned rows carry the
-- moment we stopped, and no other state may claim one.
ALTER TABLE sarah_livekit_room_bindings
  ADD CONSTRAINT sarah_livekit_room_bindings_cleanup_abandoned_shape_check CHECK (
    (state = 'cleanup_abandoned' AND cleanup_abandoned_at IS NOT NULL)
    OR (state <> 'cleanup_abandoned' AND cleanup_abandoned_at IS NULL)
  );

DROP INDEX IF EXISTS sarah_livekit_room_bindings_cleanup_idx;

CREATE INDEX IF NOT EXISTS sarah_livekit_room_bindings_cleanup_idx
  ON sarah_livekit_room_bindings (state, cleanup_next_attempt_at, updated_at);

-- Retirement sweeps read active members by expiry, never by lease alone.
CREATE INDEX IF NOT EXISTS sarah_livekit_room_members_expiry_idx
  ON sarah_livekit_room_members (join_expires_at)
  WHERE state = 'active';
