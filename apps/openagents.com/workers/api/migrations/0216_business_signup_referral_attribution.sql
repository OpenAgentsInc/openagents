-- Business-signup referral attribution (issue #5809).
--
-- The public /business intake (business_signup_requests, migration 0191) only
-- recorded source_route '/business' and had no link into the referral
-- attribution spine. This migration:
--
--   1. Records the inbound referral code and the bound pending attribution id
--      on the business_signup_requests row itself (denormalized, audit-only).
--   2. Adds a dedicated consume-once binding table that mirrors
--      order_referral_attributions (migration 0069): one row per converted
--      business signup, keyed on business_signup_request_id, so a referral
--      source is credited exactly once for a given converted business signup.
--
-- This REUSES the existing spine (site_referral_sources -> referral_attributions
-- -> per-target consume-once tables). It does NOT introduce a parallel referral
-- path. The pending attribution still lives in referral_attributions with
-- target 'order' (a business signup is a conversion intent); this table is the
-- business-signup analogue of order_referral_attributions and enforces
-- consume-once via its PRIMARY KEY. It grants no spend, payout, or agent
-- authority -- it is attribution evidence only.

ALTER TABLE business_signup_requests
  ADD COLUMN referral_code TEXT;

-- Denormalized audit pointer to the bound pending attribution. We intentionally
-- do NOT add a column-level FK here: SQLite forbids ADD COLUMN with a REFERENCES
-- clause unless the target table already exists, and this column is audit-only.
-- The authoritative referential integrity for the binding lives on the
-- business_signup_referral_attributions table below (FK to referral_attributions
-- + site_referral_sources, ON DELETE RESTRICT for consume-once safety).
ALTER TABLE business_signup_requests
  ADD COLUMN referral_attribution_id TEXT;

CREATE INDEX IF NOT EXISTS business_signup_requests_referral_code_idx
  ON business_signup_requests(referral_code, created_at DESC)
  WHERE referral_code IS NOT NULL;

CREATE TABLE IF NOT EXISTS business_signup_referral_attributions (
  business_signup_request_id TEXT PRIMARY KEY NOT NULL
    REFERENCES business_signup_requests(id) ON DELETE CASCADE,
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

CREATE INDEX IF NOT EXISTS idx_business_signup_referral_attributions_source
  ON business_signup_referral_attributions(
    referral_source_id,
    policy_state,
    created_at DESC
  );

CREATE INDEX IF NOT EXISTS idx_business_signup_referral_attributions_attribution
  ON business_signup_referral_attributions(referral_attribution_id);
