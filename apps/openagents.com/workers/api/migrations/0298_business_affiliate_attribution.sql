-- LG-8 referral/affiliate attribution only (#8269).
--
-- Operator-issued codes map to public-safe `affiliate_<code>` sourceRef values.
-- Attribution rows link code -> business intake -> pipeline -> eventual payment
-- receipt without granting payout authority or storing raw UTM/contact data.

CREATE TABLE IF NOT EXISTS business_affiliate_codes (
  code TEXT PRIMARY KEY NOT NULL,
  source_ref TEXT NOT NULL UNIQUE,
  owner_ref TEXT NOT NULL,
  issued_by_ref TEXT NOT NULL,
  policy_state TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN ('active', 'paused', 'archived')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_business_affiliate_codes_owner
  ON business_affiliate_codes(owner_ref, policy_state, updated_at);

CREATE TABLE IF NOT EXISTS business_affiliate_attributions (
  attribution_ref TEXT PRIMARY KEY NOT NULL,
  code TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  business_signup_request_id TEXT NOT NULL UNIQUE,
  pipeline_ref TEXT,
  payment_receipt_ref TEXT,
  policy_state TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN ('active', 'archived')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (code) REFERENCES business_affiliate_codes(code),
  FOREIGN KEY (business_signup_request_id)
    REFERENCES business_signup_requests(id),
  FOREIGN KEY (pipeline_ref) REFERENCES business_pipeline_rows(pipeline_ref)
);

CREATE INDEX IF NOT EXISTS idx_business_affiliate_attributions_code
  ON business_affiliate_attributions(code, created_at);

CREATE INDEX IF NOT EXISTS idx_business_affiliate_attributions_source
  ON business_affiliate_attributions(source_ref, created_at);

CREATE INDEX IF NOT EXISTS idx_business_affiliate_attributions_pipeline
  ON business_affiliate_attributions(pipeline_ref)
  WHERE pipeline_ref IS NOT NULL;
