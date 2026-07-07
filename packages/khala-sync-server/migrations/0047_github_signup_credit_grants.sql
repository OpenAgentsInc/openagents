-- #8515 (Cloudflare D1 exit): Postgres twin of the $10 GitHub-account-keyed
-- signup credit grant-tracking tables (worker D1 migration
-- 0305_github_signup_credit_grants.sql). The 401-dead D1 HTTP bridge made
-- grantGithubSignupCredit's `db` writes throw, so new GitHub signups silently
-- received $0 instead of the $10 grant (the error was swallowed by the
-- sign-in try/catch as `github_signup_credit_grant_failed`). Routing that `db`
-- handle to this Postgres twin through the Khala Code product-state adapter
-- (makePostgresD1Database) resumes the idempotency/audit + abuse-floor writes.
--
-- The credit itself already lands on the Postgres credits ledger
-- (pay_ins/pay_in_legs/agent_balances via usdCreditGrantStatements); these two
-- tables are the receipted per-user idempotency/audit record and the IP
-- abuse-floor bucket only. NO money is moved here.
--
-- TYPE FIDELITY (mirrors worker migration 0305): TEXT ISO-8601 timestamps,
-- money amounts as bigint (msat/cents, semantically < 2^53 — the adapter's
-- int8 parser reads them back as JS numbers, matching the D1 numeric shape),
-- the UNIQUE github_user_id + credit_receipt_ref idempotency guards, and the
-- (ip_hash, mint_day) abuse-floor bucket. No prompts, emails, IPs in the
-- clear, or payment material.
CREATE TABLE IF NOT EXISTS github_signup_credit_grants (
  grant_ref                 text PRIMARY KEY NOT NULL,
  github_user_id            text NOT NULL UNIQUE,
  user_id                   text NOT NULL,
  account_ref               text NOT NULL,
  amount_usd_cents          bigint NOT NULL CHECK (amount_usd_cents > 0),
  amount_msat               bigint NOT NULL CHECK (amount_msat > 0),
  credit_receipt_ref        text NOT NULL UNIQUE,
  github_account_created_at text,
  ip_hash                   text,
  created_at                text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_github_signup_credit_grants_user
  ON github_signup_credit_grants(user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS github_signup_credit_ip_mints (
  ip_hash     text NOT NULL,
  mint_day    text NOT NULL,
  mint_count  integer NOT NULL DEFAULT 0,
  created_at  text NOT NULL,
  updated_at  text NOT NULL,
  PRIMARY KEY (ip_hash, mint_day)
);
