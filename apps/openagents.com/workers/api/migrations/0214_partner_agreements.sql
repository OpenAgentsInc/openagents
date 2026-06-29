-- autopilot_sites.partner_payout_ledger.v1 (#5524 follow-up): explicit partner
-- agreements that back the partner-attribution policy
-- (`partner-attribution-policy.ts`). Distinct from the referral rail BY
-- CONSTRUCTION: the referral feed INFERS its earner from last-touch click
-- attribution (`user_referral_attributions` -> `site_referral_sources`); the
-- partner rail requires an EXPLICIT, currently-active agreement that names the
-- covered customer, the credited partner, and the role. No row here means no
-- partner is ever credited for that customer (the policy's no-fallback rule).
--
-- `role` excludes `referral`: the referral role is owned by the referral rail
-- and is refused by the attribution policy to prevent cross-rail double-pay, so
-- it is not even storable here.
--
-- All identifiers are public-safe refs / user ids. This table holds NO payout
-- destinations, invoices, preimages, or provider payloads; settlement evidence
-- lives in the operator-gated ledger, never here.
CREATE TABLE IF NOT EXISTS partner_agreements (
  id TEXT PRIMARY KEY NOT NULL,
  agreement_ref TEXT NOT NULL UNIQUE,
  partner_ref TEXT NOT NULL,
  partner_user_id TEXT NOT NULL,
  customer_user_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (
    role IN (
      'design_partner',
      'affiliate'
    )
  ),
  effective_from TEXT NOT NULL,
  effective_until TEXT,
  policy_state TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN (
      'active',
      'archived'
    )
  ),
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS partner_agreements_customer_idx
  ON partner_agreements(customer_user_id, effective_from DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS partner_agreements_partner_idx
  ON partner_agreements(partner_user_id, role, effective_from DESC)
  WHERE archived_at IS NULL;
