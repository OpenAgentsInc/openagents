-- Referrals and Promo Credit system (docs/business/2026-11-27-referrals-and-promo-credit-spec.md).
--
-- Promo credit is strictly a non-withdrawable, non-transferable service discount in USD micros
-- (inference.promo_credit), separate from paid Bitcoin/Lightning credit (inference.credit in msat).
-- All policy decisions live in pay/referral-policy.coax (#authority, 0 surviving mutants);
-- this migration creates pure relational tables and indexes with zero new PL/pgSQL functions.

CREATE SCHEMA IF NOT EXISTS referral;

CREATE TABLE IF NOT EXISTS referral.config (
  id int PRIMARY KEY DEFAULT 1 CHECK (id = 1),
  github_connect_micros bigint NOT NULL DEFAULT 5000000 CHECK (github_connect_micros >= 0),
  invitee_micros bigint NOT NULL DEFAULT 10000000 CHECK (invitee_micros >= 0),
  inviter_micros bigint NOT NULL DEFAULT 5000000 CHECK (inviter_micros >= 0),
  kickback_bps int NOT NULL DEFAULT 100 CHECK (kickback_bps >= 0 AND kickback_bps <= 10000),
  gh_min_age_days int NOT NULL DEFAULT 90 CHECK (gh_min_age_days >= 0),
  gh_min_repos int NOT NULL DEFAULT 3 CHECK (gh_min_repos >= 0),
  gh_min_followers int NOT NULL DEFAULT 5 CHECK (gh_min_followers >= 0),
  inviter_cap_30d int NOT NULL DEFAULT 20 CHECK (inviter_cap_30d >= 0),
  promo_daily_cap_micros bigint NOT NULL DEFAULT 5000000 CHECK (promo_daily_cap_micros >= 0),
  promo_expiry_days int NOT NULL DEFAULT 90 CHECK (promo_expiry_days >= 1),
  kickback_inactivity_days int NOT NULL DEFAULT 365 CHECK (kickback_inactivity_days >= 1),
  redeem_window_days int NOT NULL DEFAULT 7 CHECK (redeem_window_days >= 1),
  activation_days_needed int NOT NULL DEFAULT 2 CHECK (activation_days_needed >= 1),
  activation_window_days int NOT NULL DEFAULT 30 CHECK (activation_window_days >= 1),
  daily_budget_micros bigint NOT NULL DEFAULT 500000000 CHECK (daily_budget_micros >= 0),
  total_budget_micros bigint NOT NULL DEFAULT 50000000000 CHECK (total_budget_micros >= 0),
  promo_models text NOT NULL DEFAULT 'google/gemini-4-neon,gemini-4-neon,openagents/router-decision',
  flag_redeem boolean NOT NULL DEFAULT true,
  flag_grants boolean NOT NULL DEFAULT true,
  flag_kickback boolean NOT NULL DEFAULT true,
  flag_github_connect boolean NOT NULL DEFAULT true,
  terms_version int NOT NULL DEFAULT 1 CHECK (terms_version >= 1),
  launch_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO referral.config (id) VALUES (1) ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS referral.codes (
  id bigserial PRIMARY KEY,
  code_hash text NOT NULL UNIQUE,
  code_text text UNIQUE,
  code_hint text NOT NULL DEFAULT '',
  kind text NOT NULL CHECK (kind IN ('personal', 'campaign')),
  owner_account_id text UNIQUE,
  vanity_handle text,
  vanity_enabled boolean NOT NULL DEFAULT false,
  bonus_micros bigint CHECK (bonus_micros IS NULL OR bonus_micros >= 0),
  max_uses int CHECK (max_uses IS NULL OR max_uses >= 1),
  uses_count int NOT NULL DEFAULT 0 CHECK (uses_count >= 0),
  expires_at timestamptz,
  revoked_at timestamptz,
  show_name boolean NOT NULL DEFAULT true,
  commit_footer_opt_in boolean NOT NULL DEFAULT false,
  inviter_cap_override int CHECK (inviter_cap_override IS NULL OR inviter_cap_override >= 0),
  note text NOT NULL DEFAULT '',
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS referral_codes_vanity_lower_idx
  ON referral.codes (lower(vanity_handle))
  WHERE vanity_handle IS NOT NULL;

CREATE TABLE IF NOT EXISTS referral.attributions (
  invitee_account_id text PRIMARY KEY,
  inviter_account_id text,
  code_id bigint NOT NULL REFERENCES referral.codes(id),
  code_text text NOT NULL DEFAULT '',
  source text NOT NULL DEFAULT 'typed' CHECK (source IN ('link', 'typed', 'cli')),
  show_to_inviter boolean NOT NULL DEFAULT true,
  terms_version int NOT NULL DEFAULT 1,
  bound_at timestamptz NOT NULL DEFAULT now(),
  ip_hash text NOT NULL DEFAULT '',
  ua_hash text NOT NULL DEFAULT '',
  device_hash text NOT NULL DEFAULT '',
  held boolean NOT NULL DEFAULT false,
  hold_reason text NOT NULL DEFAULT '',
  active_days_count int NOT NULL DEFAULT 0 CHECK (active_days_count >= 0),
  first_active_day date,
  second_active_day date,
  activated_at timestamptz
);

CREATE INDEX IF NOT EXISTS referral_attributions_inviter_idx
  ON referral.attributions (inviter_account_id, bound_at);
CREATE INDEX IF NOT EXISTS referral_attributions_code_idx
  ON referral.attributions (code_id, bound_at);
CREATE INDEX IF NOT EXISTS referral_attributions_code_ip_idx
  ON referral.attributions (code_id, ip_hash, bound_at);

CREATE TABLE IF NOT EXISTS referral.github_claims (
  github_id_key text PRIMARY KEY,
  account_id text NOT NULL,
  github_login text NOT NULL DEFAULT '',
  github_avatar text NOT NULL DEFAULT '',
  github_created_at timestamptz,
  repos_count int NOT NULL DEFAULT 0,
  followers_count int NOT NULL DEFAULT 0,
  orgs_count int NOT NULL DEFAULT 0,
  eligible boolean NOT NULL DEFAULT false,
  denied boolean NOT NULL DEFAULT false,
  claimed_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS referral_github_claims_account_idx
  ON referral.github_claims (account_id);
CREATE INDEX IF NOT EXISTS referral_github_claims_created_idx
  ON referral.github_claims (github_created_at);

CREATE TABLE IF NOT EXISTS referral.grants (
  id bigserial PRIMARY KEY,
  account_id text NOT NULL,
  attribution_id text,
  kind text NOT NULL CHECK (kind IN ('github_connect', 'referral_invitee', 'referral_inviter', 'kickback', 'milestone', 'campaign')),
  source_ref text NOT NULL DEFAULT '',
  amount_micros bigint NOT NULL CHECK (amount_micros >= 0),
  status text NOT NULL CHECK (status IN ('pending', 'granted', 'void', 'clawed_back')),
  reason text NOT NULL DEFAULT '',
  terms_version int NOT NULL DEFAULT 1,
  source_topup_id text,
  milestone_tier int CHECK (milestone_tier IS NULL OR milestone_tier IN (5, 25, 100)),
  created_at timestamptz NOT NULL DEFAULT now(),
  granted_at timestamptz,
  expires_at timestamptz,
  voided_at timestamptz,
  UNIQUE (kind, account_id, source_ref)
);

CREATE INDEX IF NOT EXISTS referral_grants_account_status_idx
  ON referral.grants (account_id, status, created_at);
CREATE INDEX IF NOT EXISTS referral_grants_status_created_idx
  ON referral.grants (status, created_at);
CREATE INDEX IF NOT EXISTS referral_grants_granted_at_idx
  ON referral.grants (granted_at)
  WHERE status = 'granted';

CREATE TABLE IF NOT EXISTS referral.clicks (
  id bigserial PRIMARY KEY,
  code_id bigint REFERENCES referral.codes(id),
  code_text text NOT NULL DEFAULT '',
  ip_hash text NOT NULL DEFAULT '',
  ua_hash text NOT NULL DEFAULT '',
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS referral_clicks_code_created_idx
  ON referral.clicks (code_id, created_at);

CREATE TABLE IF NOT EXISTS referral.milestones (
  account_id text NOT NULL,
  tier int NOT NULL CHECK (tier IN (5, 25, 100)),
  grant_id bigint REFERENCES referral.grants(id),
  awarded_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (account_id, tier)
);

CREATE TABLE IF NOT EXISTS referral.admin_audit (
  id bigserial PRIMARY KEY,
  admin_account_id text NOT NULL,
  action text NOT NULL,
  target_id text NOT NULL DEFAULT '',
  reason text NOT NULL DEFAULT '',
  payload_json jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS inference.promo_credit (
  grant_id bigint PRIMARY KEY REFERENCES referral.grants(id),
  account_id text NOT NULL,
  remaining_micros bigint NOT NULL CHECK (remaining_micros >= 0),
  original_micros bigint NOT NULL CHECK (original_micros >= 0),
  kind text NOT NULL,
  expires_at timestamptz,
  last_activity_at timestamptz NOT NULL DEFAULT now(),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS inference_promo_credit_acct_exp_idx
  ON inference.promo_credit (account_id, expires_at NULLS LAST, created_at)
  WHERE remaining_micros > 0;

CREATE TABLE IF NOT EXISTS inference.promo_spend_day (
  account_id text NOT NULL,
  utc_day date NOT NULL,
  spent_micros bigint NOT NULL DEFAULT 0 CHECK (spent_micros >= 0),
  PRIMARY KEY (account_id, utc_day)
);

CREATE TABLE IF NOT EXISTS inference.promo_calls (
  call_id bigint PRIMARY KEY,
  account_id text NOT NULL,
  promo_micros bigint NOT NULL DEFAULT 0 CHECK (promo_micros >= 0),
  paid_msat bigint NOT NULL DEFAULT 0 CHECK (paid_msat >= 0),
  created_at timestamptz NOT NULL DEFAULT now()
);

DO $$
DECLARE r text;
BEGIN
  FOREACH r IN ARRAY ARRAY['oa_api', 'oa_api_staging'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT USAGE ON SCHEMA referral TO %I', r);
      EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA referral TO %I', r);
      EXECUTE format('GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA referral TO %I', r);
      EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON inference.promo_credit, inference.promo_spend_day, inference.promo_calls TO %I', r);
    END IF;
  END LOOP;
  FOREACH r IN ARRAY ARRAY['oa_dump', 'oa_readonly'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT USAGE ON SCHEMA referral TO %I', r);
      EXECUTE format('GRANT SELECT ON ALL TABLES IN SCHEMA referral TO %I', r);
      EXECUTE format('GRANT SELECT ON ALL SEQUENCES IN SCHEMA referral TO %I', r);
      EXECUTE format('GRANT SELECT ON inference.promo_credit, inference.promo_spend_day, inference.promo_calls TO %I', r);
    END IF;
  END LOOP;
END $$;
