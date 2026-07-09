-- 0312_crm_sales_checkout_sessions.sql
--
-- OB-5 (#8562): pack-priced Stripe Checkout sessions issued from the CRM
-- reply/conversation flow, and their settled-webhook outcome.
--
-- Deliberately a SEPARATE table from `stripe_customers` / `stripe_checkout_sessions`
-- (migration 0031): those tables FK `user_id` to `users(id)` — an authenticated
-- OpenAgents account. A CRM prospect (`crm_contacts.id`) is not a `users` row,
-- so this table keys on `contact_id` instead and never touches the
-- authenticated-user billing tables. The Stripe Checkout Session itself is
-- created with `customer_email` (+ `customer_creation: always`), not a
-- pre-vended `stripe_customers` row, so no `users` FK is ever required for a
-- prospect who has not signed up.

CREATE TABLE IF NOT EXISTS crm_sales_checkout_sessions (
  session_id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  contact_id TEXT NOT NULL REFERENCES crm_contacts(id) ON DELETE CASCADE,
  opportunity_id TEXT NOT NULL REFERENCES crm_opportunities(id) ON DELETE CASCADE,
  package_id TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  source_ref TEXT NOT NULL DEFAULT 'unknown',
  payment_status TEXT NOT NULL DEFAULT 'unpaid',
  fulfillment_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (fulfillment_status IN ('pending', 'fulfilled', 'unpaid', 'expired', 'failed')),
  stripe_customer_id TEXT,
  checkout_url TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS crm_sales_checkout_sessions_contact_idx
  ON crm_sales_checkout_sessions(contact_id, created_at DESC);

CREATE INDEX IF NOT EXISTS crm_sales_checkout_sessions_opportunity_idx
  ON crm_sales_checkout_sessions(opportunity_id, created_at DESC);
