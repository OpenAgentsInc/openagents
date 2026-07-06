-- AIUR-2 manual owner credit grant (#8500, epic #8467).
--
-- The first Khala Code mobile MVP build ships WITHOUT RevenueCat/IAP (#8481
-- postponed) — credits are assigned manually by the owner through the Aiur
-- admin panel (apps/aiur/) instead. The credit itself lands through the SAME
-- shared USD-origin ledger primitive the card-funded USD->msat bridge and the
-- $10 GitHub-signup grant both use (usd-credit-bridge.ts's
-- usdCreditGrantStatements): pay_ins.pay_in_type = 'usd_credit_grant' plus
-- agent_balances.usd_credit_msat, so it is inference-spendable (Pool B,
-- balance_msat) but never Bitcoin-withdrawable (RL-3 — the Lightning sweep
-- subtracts usd_credit_msat from the sweepable amount).
--
-- This table is the receipted, directly-queryable-per-user grant record for
-- admin-initiated grants specifically: one row per admin grant action
-- (grant_ref is the caller-supplied idempotency key, never auto-generated
-- server-side, so a retried request from the Aiur UI is guaranteed to be a
-- no-op rather than a double grant), never mutated after insert. `reason` is
-- owner-authored free text that lands in the receipt trail (never used for
-- anything but display/audit). `granted_by_user_id` is the verified OpenAuth
-- user id of the admin who made the grant (never a shared static token).
CREATE TABLE IF NOT EXISTS admin_credit_grants (
  grant_ref TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  account_ref TEXT NOT NULL,
  amount_usd_cents INTEGER NOT NULL CHECK (amount_usd_cents > 0),
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  reason TEXT NOT NULL,
  granted_by_user_id TEXT NOT NULL,
  credit_receipt_ref TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_admin_credit_grants_user
  ON admin_credit_grants(user_id, created_at DESC);

-- Clawback of an admin grant reuses the existing generic
-- `clawbackInferenceCredits` primitive (inference-abuse-controls.ts), which is
-- funding-source-agnostic and writes its own `pay_ins`/`pay_in_legs` rows — no
-- new table needed for the clawback event itself. This table records only the
-- ORIGINAL grant.
