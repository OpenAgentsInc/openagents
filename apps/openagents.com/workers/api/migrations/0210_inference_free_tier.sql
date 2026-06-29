-- Inference gateway free-tier enablement (EPIC #5474): Gemini 3.5 Flash free
-- allowance, Sybil-resistant per-owner-identity pool, premium-model allowlist,
-- and earned-allowance accrual.
--
-- All three tables are PUBLIC-SAFE: they carry owner/account refs, model ids,
-- USD-micros tallies, and request ids only — never prompts, completions,
-- wallet/payment material, addresses, or secrets. Free usage is "we eat the
-- cost": it is NOT a credit ledger movement (no msat decrement) — it only
-- accrues against the free pool, so it lives outside agent_balances/pay_ins.
--
-- INERT until INFERENCE_GATEWAY_ENABLED is on: no code path writes these tables
-- on the flag-off route.

-- ---------------------------------------------------------------------------
-- Free-usage tally — one cumulative row per VERIFIED OWNER IDENTITY.
--
-- The $10 free pool keys to the owner-claim identity (the approved-X owner
-- claim), NOT the per-Autopilot account/instance, so all accounts/autopilots
-- under one verified owner share ONE pool (Sybil resistance). Unclaimed /
-- unverified accounts key to a synthetic `account:<accountRef>` owner key and
-- get only a tiny taste before being blocked. `owner_key` is the resolved
-- identity ref (`owner:<userId>` for a verified claim, `account:<ref>` for an
-- unclaimed account). `cumulative_free_usd_micros` is the running free spend in
-- USD micros (1e-6 USD) so sub-cent requests accrue precisely.
CREATE TABLE IF NOT EXISTS inference_free_usage_tally (
  owner_key TEXT PRIMARY KEY,
  -- 'verified' when keyed to a verified owner-claim identity; 'unclaimed' when
  -- keyed to a bare account (taste-only). Drives which cap applies.
  identity_kind TEXT NOT NULL,
  -- Cumulative free (we-eat-the-cost) usage in USD micros.
  cumulative_free_usd_micros INTEGER NOT NULL DEFAULT 0
    CHECK (cumulative_free_usd_micros >= 0),
  -- Count of free requests accrued (diagnostics only).
  free_request_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Per-request idempotency for the free-usage accrual. One row per served
-- request id, so a retried/replayed settle for the SAME request never
-- double-accrues against the pool. UNIQUE on request_id is the guard.
CREATE TABLE IF NOT EXISTS inference_free_usage_events (
  request_id TEXT PRIMARY KEY,
  owner_key TEXT NOT NULL,
  account_ref TEXT NOT NULL,
  served_model TEXT NOT NULL,
  free_usd_micros INTEGER NOT NULL CHECK (free_usd_micros >= 0),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_inference_free_usage_events_owner
  ON inference_free_usage_events(owner_key, created_at DESC);

-- ---------------------------------------------------------------------------
-- Premium-model allowlist — owner-controlled set keyed by owner identity.
--
-- Premium models (Claude Opus/Sonnet/Haiku, GPT, etc.) are OWNER-GRANT ONLY. A
-- non-allowlisted owner requesting a premium model is DENIED (no auto un-gate).
-- One row per (owner_key) grants the owner the premium tier. `granted_by` is
-- the owner/admin actor ref that granted it (audit). `scope` reserves room for
-- per-model-class grants later; today a row grants the whole premium tier.
CREATE TABLE IF NOT EXISTS inference_premium_allowlist (
  owner_key TEXT PRIMARY KEY,
  scope TEXT NOT NULL DEFAULT 'all_premium',
  granted_by TEXT,
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- ---------------------------------------------------------------------------
-- Earned free allowance — additional free pool earned via RL-1-spine
-- contribution (e.g. each referred signup adds $X to the owner's free pool).
--
-- One cumulative row per owner identity. `earned_free_usd_micros` ADDS to the
-- base free cap so the effective free allowance = base cap + earned. Per-source
-- accrual idempotency lives in inference_earned_allowance_events.
CREATE TABLE IF NOT EXISTS inference_earned_allowance (
  owner_key TEXT PRIMARY KEY,
  earned_free_usd_micros INTEGER NOT NULL DEFAULT 0
    CHECK (earned_free_usd_micros >= 0),
  accrual_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Per-accrual idempotency for earned allowance. One row per accrual event ref
-- (e.g. `referred_signup:<referredUserId>`), so the same contribution never
-- grants the bonus twice. UNIQUE on accrual_event_ref is the guard.
CREATE TABLE IF NOT EXISTS inference_earned_allowance_events (
  accrual_event_ref TEXT PRIMARY KEY,
  owner_key TEXT NOT NULL,
  accrual_kind TEXT NOT NULL,
  earned_usd_micros INTEGER NOT NULL CHECK (earned_usd_micros >= 0),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_inference_earned_allowance_events_owner
  ON inference_earned_allowance_events(owner_key, created_at DESC);
