ALTER TABLE sarah_livekit_provisioning_intents
  ADD COLUMN IF NOT EXISTS provisioning_owner_ref text,
  ADD COLUMN IF NOT EXISTS provisioning_claimed_at text,
  ADD CONSTRAINT sarah_livekit_provisioning_intents_owner_shape_check CHECK (
    (
      provisioning_owner_ref IS NULL
      AND provisioning_claimed_at IS NULL
    )
    OR
    (
      provisioning_owner_ref IS NOT NULL
      AND provisioning_claimed_at IS NOT NULL
    )
  );
