CREATE TABLE IF NOT EXISTS referral_attributions (
  id TEXT PRIMARY KEY NOT NULL,
  referral_source_id TEXT NOT NULL
    REFERENCES site_referral_sources(id) ON DELETE CASCADE,
  referral_invite_id TEXT
    REFERENCES referral_invites(id) ON DELETE SET NULL,
  public_source_ref TEXT NOT NULL,
  public_invite_ref TEXT,
  capture_path TEXT NOT NULL CHECK (capture_path IN ('human', 'agent')),
  target TEXT NOT NULL CHECK (target IN ('home', 'order', 'agent_claim')),
  policy_state TEXT NOT NULL DEFAULT 'pending' CHECK (
    policy_state IN (
      'pending',
      'claimed',
      'expired',
      'disabled',
      'disputed',
      'archived'
    )
  ),
  first_verified_at TEXT,
  claimed_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_referral_attributions_source
  ON referral_attributions(referral_source_id, policy_state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_referral_attributions_invite
  ON referral_attributions(referral_invite_id, policy_state, created_at DESC)
  WHERE referral_invite_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_referral_attributions_expiry
  ON referral_attributions(expires_at, policy_state);
