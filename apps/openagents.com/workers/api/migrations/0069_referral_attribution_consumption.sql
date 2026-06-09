CREATE TABLE IF NOT EXISTS user_referral_attributions (
  user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  referral_attribution_id TEXT NOT NULL UNIQUE
    REFERENCES referral_attributions(id) ON DELETE RESTRICT,
  referral_source_id TEXT NOT NULL
    REFERENCES site_referral_sources(id) ON DELETE RESTRICT,
  referral_invite_id TEXT REFERENCES referral_invites(id) ON DELETE SET NULL,
  capture_path TEXT NOT NULL CHECK (capture_path IN ('human', 'agent')),
  target TEXT NOT NULL CHECK (target IN ('home', 'order', 'agent_claim')),
  first_verified_at TEXT NOT NULL,
  policy_state TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN ('active', 'disputed', 'archived')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_user_referral_attributions_source
  ON user_referral_attributions(referral_source_id, policy_state, created_at DESC);

CREATE TABLE IF NOT EXISTS order_referral_attributions (
  software_order_id TEXT PRIMARY KEY NOT NULL
    REFERENCES software_orders(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  referral_attribution_id TEXT NOT NULL
    REFERENCES referral_attributions(id) ON DELETE RESTRICT,
  referral_source_id TEXT NOT NULL
    REFERENCES site_referral_sources(id) ON DELETE RESTRICT,
  referral_invite_id TEXT REFERENCES referral_invites(id) ON DELETE SET NULL,
  capture_path TEXT NOT NULL CHECK (capture_path IN ('human', 'agent')),
  target TEXT NOT NULL CHECK (target IN ('home', 'order', 'agent_claim')),
  linked_at TEXT NOT NULL,
  policy_state TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN ('active', 'disputed', 'archived')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_order_referral_attributions_user
  ON order_referral_attributions(user_id, policy_state, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_referral_attributions (
  agent_user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  owner_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  referral_attribution_id TEXT NOT NULL
    REFERENCES referral_attributions(id) ON DELETE RESTRICT,
  referral_source_id TEXT NOT NULL
    REFERENCES site_referral_sources(id) ON DELETE RESTRICT,
  referral_invite_id TEXT REFERENCES referral_invites(id) ON DELETE SET NULL,
  capture_path TEXT NOT NULL CHECK (capture_path IN ('human', 'agent')),
  target TEXT NOT NULL CHECK (target IN ('home', 'order', 'agent_claim')),
  claimed_at TEXT NOT NULL,
  policy_state TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN ('active', 'disputed', 'archived')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_agent_referral_attributions_owner
  ON agent_referral_attributions(owner_user_id, policy_state, created_at DESC)
  WHERE owner_user_id IS NOT NULL;
