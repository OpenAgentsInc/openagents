CREATE TABLE IF NOT EXISTS site_referral_sources (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  site_version_id TEXT REFERENCES site_versions(id) ON DELETE SET NULL,
  referrer_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  public_source_ref TEXT NOT NULL UNIQUE,
  public_slug TEXT NOT NULL,
  campaign_ref TEXT,
  source_label TEXT,
  policy_state TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN ('active', 'disabled', 'disputed', 'expired', 'archived')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_site_referral_sources_site
  ON site_referral_sources(site_id, policy_state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_site_referral_sources_referrer
  ON site_referral_sources(referrer_user_id, policy_state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_site_referral_sources_slug
  ON site_referral_sources(public_slug, policy_state);

CREATE TABLE IF NOT EXISTS referral_invites (
  id TEXT PRIMARY KEY NOT NULL,
  referral_source_id TEXT NOT NULL
    REFERENCES site_referral_sources(id) ON DELETE CASCADE,
  public_invite_ref TEXT NOT NULL UNIQUE,
  token_hash TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (
    scope IN ('site_join', 'order_start', 'agent_claim')
  ),
  audience_path TEXT NOT NULL CHECK (audience_path IN ('human', 'agent')),
  policy_state TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN ('active', 'redeemed', 'expired', 'disabled', 'disputed')
  ),
  expires_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_referral_invites_source
  ON referral_invites(referral_source_id, policy_state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_referral_invites_expiry
  ON referral_invites(expires_at, policy_state)
  WHERE expires_at IS NOT NULL;
