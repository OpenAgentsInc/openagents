-- EP263-LK H4 follow-up (#9282): give `sarah_livekit_provisioning_intents` the
-- same bounded retry that `sarah_livekit_room_bindings` received in 0125.
--
-- 0125 fixed the room-binding cleanup loop and left its sibling untouched.
-- `claimLiveKitProvisioningIntents` re-selects every `pending`, `reconciling`,
-- and `cleanup_failed` intent behind a flat staleness gate with no attempt
-- count, no next-attempt time, and no terminal give-up state, and
-- `markLiveKitProvisioningIntent` writes `cleanup_failed` straight back into
-- that same claim pool. An intent whose broker key can never be cleaned is
-- therefore retried for as long as the service runs. This table is failing
-- every tick in production today.
--
-- The columns, the terminal state, and the shape constraint deliberately match
-- the room-binding spelling exactly. Two sibling tables that converge for the
-- same reason should be readable — and queryable by an operator — with one
-- vocabulary, not two.
--
-- `cleanup_abandoned` stays outside the `state IN ('pending','reconciling',
-- 'bound')` capacity and authorization predicates exactly as `cleanup_failed`
-- already did, so this changes resource behavior only.

ALTER TABLE sarah_livekit_provisioning_intents
  ADD COLUMN IF NOT EXISTS cleanup_attempt_count integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS cleanup_next_attempt_at text,
  ADD COLUMN IF NOT EXISTS cleanup_abandoned_at text,
  ADD CONSTRAINT sarah_livekit_provisioning_intents_cleanup_attempt_count_check CHECK (
    cleanup_attempt_count >= 0
  );

ALTER TABLE sarah_livekit_provisioning_intents
  DROP CONSTRAINT IF EXISTS sarah_livekit_provisioning_intents_state_check;

-- The original CHECK was inline and unnamed, so it carries a generated name.
-- Drop it by discovery rather than by guess, then re-add the widened set under
-- an explicit name the next migration can find.
DO $$
DECLARE
  constraint_name text;
BEGIN
  SELECT conname INTO constraint_name
  FROM pg_constraint
  WHERE conrelid = 'sarah_livekit_provisioning_intents'::regclass
    AND contype = 'c'
    AND pg_get_constraintdef(oid) LIKE '%reconciling%'
    AND pg_get_constraintdef(oid) NOT LIKE '%cleanup_abandoned%'
  LIMIT 1;
  IF constraint_name IS NOT NULL THEN
    EXECUTE format(
      'ALTER TABLE sarah_livekit_provisioning_intents DROP CONSTRAINT %I',
      constraint_name
    );
  END IF;
END $$;

ALTER TABLE sarah_livekit_provisioning_intents
  ADD CONSTRAINT sarah_livekit_provisioning_intents_state_check CHECK (
    state IN (
      'pending',
      'reconciling',
      'bound',
      'cleanup_failed',
      'cleanup_abandoned',
      'cleaned'
    )
  );

-- The give-up must be legible from the row alone: abandoned rows carry the
-- moment we stopped, and no other state may claim one.
ALTER TABLE sarah_livekit_provisioning_intents
  ADD CONSTRAINT sarah_livekit_provisioning_intents_abandoned_shape_check CHECK (
    (state = 'cleanup_abandoned' AND cleanup_abandoned_at IS NOT NULL)
    OR (state <> 'cleanup_abandoned' AND cleanup_abandoned_at IS NULL)
  );

-- The reconciler claim reads by state and next-attempt time, never by
-- `created_at` alone.
CREATE INDEX IF NOT EXISTS sarah_livekit_provisioning_intents_cleanup_idx
  ON sarah_livekit_provisioning_intents (state, cleanup_next_attempt_at, created_at);
