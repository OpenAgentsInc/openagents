-- Khala FREE API MODE (issue #6228, EPIC #5474). A self-serve FREE API key
-- (`POST /api/keys/free`) plus a FREE inference lane that calls the single
-- public model `openagents/khala` (own-infra GPT-OSS / Gemini Flash lanes)
-- WITHOUT a funded balance, within a per-key free quota. Beyond the quota — or
-- for premium lanes — credits/budget are still required (the existing balance +
-- 402 path), so paid Khala behavior for funded keys is unchanged.
--
-- "Free" is a property of the KEY/account + the lane, NOT a new model id: there
-- is exactly one public model `openagents/khala`. These tables mark which
-- accounts are free-tier and meter their free usage so the quota and abuse
-- limits are enforceable.
--
-- PUBLIC-SAFE: every row carries account refs, IP-hash refs, model ids, bounded
-- integer counters, and timestamps only — never prompts, completions,
-- wallet/payment material, raw tokens, raw IP addresses, or secrets. The free
-- API key itself is a normal `agent_credentials` row (hashed token, never
-- stored plaintext); these tables only mark the tier and tally usage.
--
-- INERT until INFERENCE_FREE_TIER_ENABLED is on AND the gateway is enabled: no
-- balance-gate bypass, no quota accrual, and no zero-debit metering happen on
-- the flag-off route. The mint endpoint is itself flag-gated.

-- 1. The set of accounts whose API key rides the FREE tier. Keyed by the
--    authenticated account ref (`agent:<userId>`), the SAME principal the
--    balance gate + metering hook key on. A row here means: a zero-balance
--    request for `openagents/khala` (own-infra / non-premium) is admitted within
--    the per-key free quota, recorded as a zero-debit free receipt. Premium
--    models and over-quota requests still hit the normal balance + 402 path.
CREATE TABLE IF NOT EXISTS inference_free_tier_keys (
  account_ref TEXT PRIMARY KEY,
  -- Reserves room for future tier scopes; today a row grants the standard free
  -- Khala lane within the daily quota.
  scope TEXT NOT NULL DEFAULT 'free_khala_daily',
  -- How the key was minted (audit), e.g. 'self_serve_anonymous' /
  -- 'self_serve_email'. Never a token or raw email.
  mint_source TEXT NOT NULL DEFAULT 'self_serve_anonymous',
  -- Optional public-safe note (audit). Never payment material or secrets.
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- 2. Per-key, per-UTC-day free-usage tally. Drives the daily free quota
--    (request count + served-token count). One row per (account_ref, usage_day).
--    Incremented idempotently per request by the free-tier metering wrapper.
CREATE TABLE IF NOT EXISTS inference_free_tier_usage (
  account_ref TEXT NOT NULL,
  -- UTC day bucket, e.g. '2026-06-24'. The quota resets each UTC day.
  usage_day TEXT NOT NULL,
  free_request_count INTEGER NOT NULL DEFAULT 0,
  free_total_tokens INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (account_ref, usage_day)
);

-- 3. Per-request idempotency guard for free-tier quota accrual. One UNIQUE row
--    per request id, so a retried/replayed settle for the SAME request hits the
--    constraint and is a no-op (never double-counts against the daily quota) —
--    exactly the discipline the credit ledger + free-allowance pool use.
CREATE TABLE IF NOT EXISTS inference_free_tier_usage_events (
  request_id TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  usage_day TEXT NOT NULL,
  served_model TEXT NOT NULL,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

-- 4. Abuse guard for the self-serve mint endpoint: per-IP-hash, per-UTC-day
--    mint counter so anonymous minting is bounded (no unbounded key minting).
--    `ip_hash` is a SHA-256 of the client IP — never the raw IP.
CREATE TABLE IF NOT EXISTS inference_free_key_mints (
  ip_hash TEXT NOT NULL,
  mint_day TEXT NOT NULL,
  mint_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (ip_hash, mint_day)
);
