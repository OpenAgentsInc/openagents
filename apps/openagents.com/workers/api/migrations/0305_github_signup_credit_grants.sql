-- MM-D1 $10 GitHub-account-keyed signup credit grant (#8478, epic #8467).
--
-- The credit itself lands through the SAME shared USD-origin ledger primitive
-- the card-funded USD->msat bridge uses (usd-credit-bridge.ts's
-- usdCreditGrantStatements): pay_ins.pay_in_type = 'usd_credit_grant' plus
-- agent_balances.usd_credit_msat, so it is inference-spendable (Pool B,
-- balance_msat) but never Bitcoin-withdrawable (RL-3 — the Lightning sweep
-- subtracts usd_credit_msat from the sweepable amount).
--
-- This table is the receipted, directly-queryable-per-user grant record for
-- the signup path specifically: exactly one row per GitHub account
-- (UNIQUE github_user_id), never mutated after insert, so #8480's balance/
-- history UI (and any future clawback/audit tooling) can read "did this
-- GitHub account get its signup credit, and when" without parsing pay_ins
-- context_ref strings. No prompts, emails, or payment material stored.
CREATE TABLE IF NOT EXISTS github_signup_credit_grants (
  grant_ref TEXT PRIMARY KEY NOT NULL,
  github_user_id TEXT NOT NULL UNIQUE,
  user_id TEXT NOT NULL,
  account_ref TEXT NOT NULL,
  amount_usd_cents INTEGER NOT NULL CHECK (amount_usd_cents > 0),
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  credit_receipt_ref TEXT NOT NULL UNIQUE,
  github_account_created_at TEXT,
  ip_hash TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_github_signup_credit_grants_user
  ON github_signup_credit_grants(user_id, created_at DESC);

-- Anti-abuse floor (MM-D1 scope): bound how many DISTINCT GitHub accounts one
-- client IP can mint a signup grant for per UTC day. Mirrors the existing
-- `inference_free_key_mints` ip_hash + mint_day bucket pattern
-- (inference-free-tier-key.ts) rather than inventing a new shape. The raw IP
-- is hashed (SHA-256) before it ever reaches this table; never stored/logged
-- in the clear.
CREATE TABLE IF NOT EXISTS github_signup_credit_ip_mints (
  ip_hash TEXT NOT NULL,
  mint_day TEXT NOT NULL,
  mint_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (ip_hash, mint_day)
);
