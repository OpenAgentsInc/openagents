-- KS-8.9 (#8320): inference entitlements and quotas -- Postgres twins for
-- the free-tier/entitlement accounting that rides the inference serving
-- path, plus the agent rate-limit recovery and agent-search
-- payment/entitlement sub-families.
--
-- Source D1 tables (apps/openagents.com/workers/api/migrations):
--   0210_inference_free_tier.sql          inference_free_usage_tally,
--                                         inference_free_usage_events,
--                                         inference_premium_allowlist,
--                                         inference_earned_allowance,
--                                         inference_earned_allowance_events
--   0217/0223_inference_batch_jobs*.sql   inference_batch_jobs (+ wait timing)
--   0227_inference_operator_exemption.sql inference_operator_exemption
--   0231_inference_free_tier_keys.sql     inference_free_tier_keys,
--                                         inference_free_tier_usage,
--                                         inference_free_tier_usage_events,
--                                         inference_free_key_mints
--   0235/0256 privacy                     inference_privacy_entitlements,
--                                         inference_privacy_entitlement_receipts,
--                                         inference_confidential_compute_execution_receipts
--   0257 referral splits                  inference_referral_margin_splits
--   0189 builtin compute                  builtin_compute_agent_quota_events
--   0150 orange check                     orange_check_entitlements
--   0109 agent rate limit                 agent_rate_limit_challenges/receipts/
--                                         entitlements/redemptions
--   0116/0117 agent search                agent_search_requests/sources/
--                                         quota_events/cache_entries +
--                                         payment_challenges/payment_receipts/
--                                         entitlements/payment_redemptions
--
-- EXCLUDED BY DESIGN: `agent_search_metric_events`. Per MIGRATION_PLAN §3.6
-- the `*_metric_events` observability streams are Analytics Engine
-- candidates, not Postgres rows — enforcement tallies stay relational,
-- observability streams do not. The metric stream is NOT mirrored and NOT
-- backfilled; its D1 table retires with the decommission follow-up once the
-- Analytics Engine sink lands.
--
-- ENFORCEMENT COUNTERS ARE EVENT-KEYED (the §3.6 risk note): a lost
-- increment is a free-tier leak, a doubled one is a false denial. Every
-- tally with an event twin (`inference_free_usage_tally`,
-- `inference_earned_allowance`, `inference_free_tier_usage`) is maintained
-- ONLY behind its event table's unique key (request_id /
-- accrual_event_ref): the mirror inserts the event ON CONFLICT DO NOTHING
-- and increments the tally only for a FRESH event row, in one transaction —
-- the same discipline as the D1 batches. Backfill verifies
-- tally = SUM(events) per key. `inference_free_key_mints` is the one
-- counter WITHOUT an event key upstream (a bounded per-IP-per-day abuse
-- guard, not billing enforcement); the backfill converges it to the D1
-- snapshot and --verify flags drift.
--
-- TYPE FIDELITY (v1, the 0009 rule): keep D1 byte-compatible
-- representations where exact reconciliation depends on it — TEXT
-- timestamps / JSON-as-text, bigint for D1 INTEGER counters, double
-- precision for D1 REAL dollar amounts.
--
-- IDEMPOTENCY KEYS PORT EXACTLY: every UNIQUE natural key in D1 is
-- preserved so the mirror and backfill can use INSERT ... ON CONFLICT DO
-- NOTHING with the same duplicate semantics.
--
-- NO CROSS-TABLE FOREIGN KEYS DURING MIGRATION (the 0009 rule): D1's FKs
-- (agent_rate_limit_* chains, agent_search_* chains,
-- inference_referral_margin_splits -> referral_attributions /
-- site_referral_sources) are intentionally NOT ported — referenced tables
-- may migrate in other waves, rows are mirrored/backfilled per table, and
-- set membership is verified by the backfill tooling before any read
-- cutover. ID references remain as plain text columns.
--
-- INDEXES ARE PORTED FROM ACTUAL READS ONLY; each index carries the read
-- that justifies it.

-- ---------------------------------------------------------------------------
-- Free-usage pool (owner-keyed, Sybil-resistant; EPIC #5474)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS inference_free_usage_tally (
  owner_key                  text PRIMARY KEY,
  identity_kind              text NOT NULL,
  cumulative_free_usd_micros bigint NOT NULL DEFAULT 0
    CHECK (cumulative_free_usd_micros >= 0),
  free_request_count         bigint NOT NULL DEFAULT 0,
  created_at                 text NOT NULL,
  updated_at                 text NOT NULL
);
-- Reads are single-row by owner_key (the free-allowance gate) — the PK
-- covers them; no secondary index needed.

CREATE TABLE IF NOT EXISTS inference_free_usage_events (
  request_id      text PRIMARY KEY,
  owner_key       text NOT NULL,
  account_ref     text NOT NULL,
  served_model    text NOT NULL,
  free_usd_micros bigint NOT NULL CHECK (free_usd_micros >= 0),
  created_at      text NOT NULL
);

-- Justified by: the D1 twin's owner listing index and the backfill/verify
-- tally = SUM(events) GROUP BY owner_key sweep.
CREATE INDEX IF NOT EXISTS idx_inference_free_usage_events_owner
  ON inference_free_usage_events (owner_key, created_at DESC);

CREATE TABLE IF NOT EXISTS inference_premium_allowlist (
  owner_key  text PRIMARY KEY,
  scope      text NOT NULL DEFAULT 'all_premium',
  granted_by text,
  note       text,
  created_at text NOT NULL,
  updated_at text NOT NULL
);
-- Reads are single-row by owner_key (the premium gate) — PK only.

CREATE TABLE IF NOT EXISTS inference_earned_allowance (
  owner_key              text PRIMARY KEY,
  earned_free_usd_micros bigint NOT NULL DEFAULT 0
    CHECK (earned_free_usd_micros >= 0),
  accrual_count          bigint NOT NULL DEFAULT 0,
  created_at             text NOT NULL,
  updated_at             text NOT NULL
);

CREATE TABLE IF NOT EXISTS inference_earned_allowance_events (
  accrual_event_ref text PRIMARY KEY,
  owner_key         text NOT NULL,
  accrual_kind      text NOT NULL,
  earned_usd_micros bigint NOT NULL CHECK (earned_usd_micros >= 0),
  created_at        text NOT NULL
);

-- Justified by: the D1 twin's owner listing index and the verify
-- tally = SUM(events) GROUP BY owner_key sweep.
CREATE INDEX IF NOT EXISTS idx_inference_earned_allowance_events_owner
  ON inference_earned_allowance_events (owner_key, created_at DESC);

-- ---------------------------------------------------------------------------
-- Batch jobs (#6086 wait-timing columns included)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS inference_batch_jobs (
  job_id             text PRIMARY KEY,
  account_ref        text NOT NULL,
  status             text NOT NULL,
  charge_receipt_ref text NOT NULL,
  dataset_size       bigint NOT NULL,
  processed_items    bigint NOT NULL DEFAULT 0,
  failed_items       bigint NOT NULL DEFAULT 0,
  results_r2_key     text,
  created_at         text NOT NULL,
  updated_at         text NOT NULL,
  enqueued_at        text,
  started_at         text
);

-- Justified by: batch-job route "list my jobs" read
-- (WHERE account_ref = ? ORDER BY created_at DESC).
CREATE INDEX IF NOT EXISTS idx_inference_batch_jobs_account
  ON inference_batch_jobs (account_ref, created_at DESC);

-- Justified by: the batch consumer's oldest-first status scan
-- (WHERE status = ? ORDER BY created_at ASC).
CREATE INDEX IF NOT EXISTS idx_inference_batch_jobs_status
  ON inference_batch_jobs (status, created_at ASC);

-- ---------------------------------------------------------------------------
-- Operator exemption (#6180)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS inference_operator_exemption (
  owner_key  text PRIMARY KEY,
  scope      text NOT NULL DEFAULT 'own_infra_non_premium',
  granted_by text,
  note       text,
  created_at text NOT NULL,
  updated_at text NOT NULL
);
-- Reads are single-row by owner_key (the exemption gate) — PK only.

-- ---------------------------------------------------------------------------
-- Free API tier (#6228)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS inference_free_tier_keys (
  account_ref text PRIMARY KEY,
  scope       text NOT NULL DEFAULT 'free_khala_daily',
  mint_source text NOT NULL DEFAULT 'self_serve_anonymous',
  note        text,
  created_at  text NOT NULL,
  updated_at  text NOT NULL
);
-- Reads are single-row by account_ref (the free-tier gate) — PK only.

CREATE TABLE IF NOT EXISTS inference_free_tier_usage (
  account_ref        text NOT NULL,
  usage_day          text NOT NULL,
  free_request_count bigint NOT NULL DEFAULT 0,
  free_total_tokens  bigint NOT NULL DEFAULT 0,
  created_at         text NOT NULL,
  updated_at         text NOT NULL,
  PRIMARY KEY (account_ref, usage_day)
);
-- Reads are single-row by (account_ref, usage_day) — the composite PK
-- covers the daily quota gate.

CREATE TABLE IF NOT EXISTS inference_free_tier_usage_events (
  request_id   text PRIMARY KEY,
  account_ref  text NOT NULL,
  usage_day    text NOT NULL,
  served_model text NOT NULL,
  total_tokens bigint NOT NULL DEFAULT 0,
  created_at   text NOT NULL
);

-- Justified by: the verify tally = SUM(events) GROUP BY
-- (account_ref, usage_day) sweep; the serving path never scans events.
CREATE INDEX IF NOT EXISTS idx_inference_free_tier_usage_events_account_day
  ON inference_free_tier_usage_events (account_ref, usage_day);

CREATE TABLE IF NOT EXISTS inference_free_key_mints (
  ip_hash    text NOT NULL,
  mint_day   text NOT NULL,
  mint_count bigint NOT NULL DEFAULT 0,
  created_at text NOT NULL,
  updated_at text NOT NULL,
  PRIMARY KEY (ip_hash, mint_day)
);
-- Reads are single-row by (ip_hash, mint_day) — composite PK only.

-- ---------------------------------------------------------------------------
-- Paid privacy / confidential compute (#6295, #6293)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS inference_privacy_entitlements (
  account_ref  text PRIMARY KEY,
  privacy_tier text NOT NULL DEFAULT 'paid_privacy',
  note         text,
  created_at   text NOT NULL,
  updated_at   text NOT NULL
);
-- Reads are single-row by account_ref (the capture opt-out check) — PK only.

CREATE TABLE IF NOT EXISTS inference_privacy_entitlement_receipts (
  receipt_ref      text PRIMARY KEY,
  entitlement_ref  text NOT NULL UNIQUE,
  account_ref      text NOT NULL,
  purchase_ref     text NOT NULL UNIQUE,
  idempotency_key  text NOT NULL UNIQUE,
  privacy_tier     text NOT NULL DEFAULT 'paid_privacy',
  capture_excluded bigint NOT NULL DEFAULT 1 CHECK (capture_excluded IN (0, 1)),
  reason_ref       text NOT NULL,
  created_at       text NOT NULL,
  updated_at       text NOT NULL
);

-- Justified by: the receipts route's per-account listing read
-- (WHERE account_ref = ? ORDER BY created_at DESC).
CREATE INDEX IF NOT EXISTS idx_inference_privacy_entitlement_receipts_account
  ON inference_privacy_entitlement_receipts (account_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS inference_confidential_compute_execution_receipts (
  receipt_ref      text PRIMARY KEY,
  execution_ref    text NOT NULL UNIQUE,
  account_ref      text NOT NULL,
  request_ref      text NOT NULL UNIQUE,
  idempotency_key  text NOT NULL UNIQUE,
  capture_excluded bigint NOT NULL DEFAULT 1 CHECK (capture_excluded IN (0, 1)),
  reason_ref       text NOT NULL,
  created_at       text NOT NULL,
  updated_at       text NOT NULL
);

-- Justified by: the receipts route's per-account listing read.
CREATE INDEX IF NOT EXISTS idx_inference_confidential_compute_receipts_account
  ON inference_confidential_compute_execution_receipts (account_ref, created_at DESC);

-- ---------------------------------------------------------------------------
-- Referral margin splits (three-way inference revshare evidence)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS inference_referral_margin_splits (
  id                       text PRIMARY KEY,
  request_id               text NOT NULL UNIQUE,
  account_ref              text NOT NULL,
  referred_user_id         text NOT NULL,
  referrer_user_id         text NOT NULL,
  -- Plain text refs (D1 FKs to referral_attributions / site_referral_sources
  -- deliberately not ported — different migration wave).
  referral_attribution_id  text NOT NULL,
  referral_source_id       text NOT NULL,
  referral_invite_id       text,
  payout_ref               text NOT NULL,
  qualifying_event_ref     text NOT NULL,
  charge_receipt_ref       text NOT NULL,
  funding_kind             text NOT NULL,
  adapter_id               text NOT NULL,
  requested_model          text NOT NULL,
  served_model             text NOT NULL,
  served_by_contributor    bigint NOT NULL DEFAULT 0 CHECK (served_by_contributor IN (0, 1)),
  serving_node_count       bigint NOT NULL DEFAULT 0 CHECK (serving_node_count >= 0),
  charge_usd               double precision NOT NULL CHECK (charge_usd >= 0),
  cost_usd                 double precision NOT NULL CHECK (cost_usd >= 0),
  margin_usd               double precision NOT NULL CHECK (margin_usd >= 0),
  margin_sats              bigint NOT NULL CHECK (margin_sats >= 0),
  openagents_usd           double precision NOT NULL CHECK (openagents_usd >= 0),
  openagents_sats          bigint NOT NULL CHECK (openagents_sats >= 0),
  serving_node_usd         double precision NOT NULL CHECK (serving_node_usd >= 0),
  serving_node_sats        bigint NOT NULL CHECK (serving_node_sats >= 0),
  referrer_usd             double precision NOT NULL CHECK (referrer_usd >= 0),
  referrer_sats            bigint NOT NULL CHECK (referrer_sats >= 0),
  created_at               text NOT NULL,
  archived_at              text
);

-- Justified by: the referral dashboard's per-referrer earnings read
-- (WHERE referrer_user_id = ? ORDER BY created_at DESC).
CREATE INDEX IF NOT EXISTS idx_inference_referral_margin_splits_referrer
  ON inference_referral_margin_splits (referrer_user_id, created_at DESC);

-- Justified by: attribution-scoped audit reads over one referral chain.
CREATE INDEX IF NOT EXISTS idx_inference_referral_margin_splits_attribution
  ON inference_referral_margin_splits (referral_attribution_id, created_at DESC);

-- ---------------------------------------------------------------------------
-- Built-in hosted-compute agent quota events
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS builtin_compute_agent_quota_events (
  id                     text PRIMARY KEY,
  actor_user_id          text NOT NULL,
  grant_ref              text NOT NULL UNIQUE,
  provider               text NOT NULL CHECK (provider IN ('google_gemini')),
  budget_class           text NOT NULL CHECK (budget_class IN ('free_tier')),
  session_units          bigint NOT NULL CHECK (session_units >= 0),
  session_budget_seconds bigint NOT NULL CHECK (session_budget_seconds >= 0),
  token_ceiling          bigint NOT NULL CHECK (token_ceiling >= 0),
  created_at             text NOT NULL
);

-- Justified by: countSessionsSince (SUM(session_units) WHERE
-- actor_user_id = ? AND created_at >= ?) — the daily budget gate.
CREATE INDEX IF NOT EXISTS builtin_compute_agent_quota_actor_created_idx
  ON builtin_compute_agent_quota_events (actor_user_id, created_at DESC);

-- ---------------------------------------------------------------------------
-- Orange check entitlements
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS orange_check_entitlements (
  id                text PRIMARY KEY,
  agent_user_id     text NOT NULL UNIQUE,
  actor_ref         text NOT NULL UNIQUE,
  state             text NOT NULL CHECK (state IN ('active', 'revoked')),
  receipt_ref       text NOT NULL UNIQUE,
  action_ref        text,
  paid_amount_cents bigint NOT NULL DEFAULT 500,
  created_at        text NOT NULL,
  updated_at        text NOT NULL
);

-- Justified by: countActiveOrangeChecks (WHERE state = 'active') and the
-- moderation listing read (state, updated_at DESC).
CREATE INDEX IF NOT EXISTS orange_check_entitlements_state_idx
  ON orange_check_entitlements (state, updated_at DESC);

-- ---------------------------------------------------------------------------
-- Agent rate-limit recovery (pay-to-recover challenge/receipt/entitlement)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS agent_rate_limit_challenges (
  id                              text PRIMARY KEY,
  idempotency_key_hash            text NOT NULL UNIQUE,
  actor_ref                       text NOT NULL,
  owner_user_id                   text NOT NULL,
  route_key                       text NOT NULL,
  method                          text NOT NULL,
  path                            text NOT NULL,
  submission_idempotency_key_hash text NOT NULL,
  client_fingerprint_hash         text NOT NULL,
  request_body_digest             text NOT NULL,
  price_asset                     text NOT NULL CHECK (price_asset IN ('bitcoin', 'credits', 'usd')),
  price_denomination              text NOT NULL,
  price_value                     bigint NOT NULL CHECK (price_value >= 0),
  spend_cap_asset                 text NOT NULL CHECK (spend_cap_asset IN ('bitcoin', 'credits', 'usd')),
  spend_cap_denomination          text NOT NULL,
  spend_cap_value                 bigint NOT NULL CHECK (spend_cap_value >= 0),
  entitlement_kind                text NOT NULL,
  expires_at                      text NOT NULL,
  public_projection_json          text NOT NULL DEFAULT '{}',
  created_at                      text NOT NULL,
  archived_at                     text
);

-- Justified by: the recovery route's per-actor recent-challenge listing
-- (actor_ref, route_key, created_at DESC), same as the D1 twin.
CREATE INDEX IF NOT EXISTS agent_rate_limit_challenges_actor_route_idx
  ON agent_rate_limit_challenges (actor_ref, route_key, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_rate_limit_receipts (
  id                     text PRIMARY KEY,
  receipt_ref            text NOT NULL UNIQUE,
  challenge_id           text NOT NULL UNIQUE,
  actor_ref              text NOT NULL,
  owner_user_id          text NOT NULL,
  route_key              text NOT NULL,
  amount_asset           text NOT NULL CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination    text NOT NULL,
  amount_value           bigint NOT NULL CHECK (amount_value >= 0),
  entitlement_ref        text NOT NULL UNIQUE,
  redacted_payment_ref   text NOT NULL,
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);

-- Justified by: per-actor receipt listing (actor_ref, route_key,
-- created_at DESC), same as the D1 twin.
CREATE INDEX IF NOT EXISTS agent_rate_limit_receipts_actor_route_idx
  ON agent_rate_limit_receipts (actor_ref, route_key, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_rate_limit_entitlements (
  id                              text PRIMARY KEY,
  entitlement_ref                 text NOT NULL UNIQUE,
  challenge_id                    text NOT NULL UNIQUE,
  receipt_ref                     text NOT NULL,
  actor_ref                       text NOT NULL,
  owner_user_id                   text NOT NULL,
  route_key                       text NOT NULL,
  method                          text NOT NULL,
  path                            text NOT NULL,
  submission_idempotency_key_hash text NOT NULL,
  client_fingerprint_hash         text NOT NULL,
  request_body_digest             text NOT NULL,
  entitlement_kind                text NOT NULL,
  status                          text NOT NULL CHECK (status IN ('active', 'consumed', 'expired')),
  expires_at                      text NOT NULL,
  created_at                      text NOT NULL,
  consumed_at                     text,
  archived_at                     text
);

-- Justified by: the consume path's active-entitlement lookup
-- (actor_ref, route_key, status, expires_at), same as the D1 twin.
CREATE INDEX IF NOT EXISTS agent_rate_limit_entitlements_actor_route_idx
  ON agent_rate_limit_entitlements (actor_ref, route_key, status, expires_at);

CREATE TABLE IF NOT EXISTS agent_rate_limit_redemptions (
  id                     text PRIMARY KEY,
  idempotency_key_hash   text NOT NULL UNIQUE,
  challenge_id           text NOT NULL UNIQUE,
  actor_ref              text NOT NULL,
  proof_ref              text NOT NULL,
  entitlement_ref        text NOT NULL,
  receipt_ref            text NOT NULL,
  replayed               bigint NOT NULL DEFAULT 0 CHECK (replayed IN (0, 1)),
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);
-- Reads are single-row by idempotency_key_hash / challenge_id — the
-- UNIQUE constraints cover them; no secondary index needed.

-- ---------------------------------------------------------------------------
-- Agent search (requests, quota, cache, payments)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS agent_search_requests (
  id                     text PRIMARY KEY,
  receipt_ref            text NOT NULL UNIQUE,
  actor_ref              text NOT NULL,
  agent_user_id          text NOT NULL,
  credential_id          text NOT NULL,
  token_prefix           text NOT NULL,
  idempotency_key_hash   text NOT NULL UNIQUE,
  request_body_digest    text NOT NULL,
  query_hash             text NOT NULL,
  query_text             text,
  mode                   text NOT NULL CHECK (mode IN ('basic')),
  provider               text NOT NULL CHECK (provider IN ('exa')),
  provider_request_id    text,
  status                 text NOT NULL CHECK (status IN ('succeeded', 'failed')),
  cache_status           text NOT NULL CHECK (cache_status IN ('hit', 'miss')),
  charge_state           text NOT NULL CHECK (charge_state IN ('free_allowance', 'paid_entitlement')),
  product_id             text,
  entitlement_ref        text,
  provider_cost_dollars  double precision,
  public_projection_json text NOT NULL,
  created_at             text NOT NULL,
  completed_at           text,
  archived_at            text
);

-- Justified by: per-actor request history reads (actor_ref,
-- created_at DESC), same as the D1 twin.
CREATE INDEX IF NOT EXISTS agent_search_requests_actor_created_idx
  ON agent_search_requests (actor_ref, created_at DESC);

-- Justified by: per-credential request history reads (credential_id,
-- created_at DESC), same as the D1 twin.
CREATE INDEX IF NOT EXISTS agent_search_requests_credential_created_idx
  ON agent_search_requests (credential_id, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_search_sources (
  id                 text PRIMARY KEY,
  search_request_id  text NOT NULL,
  source_ref         text NOT NULL UNIQUE,
  title              text NOT NULL,
  url                text NOT NULL,
  domain             text NOT NULL,
  published_date     text,
  score              double precision,
  highlight_text     text,
  selected_text_hash text,
  public_safe        bigint NOT NULL DEFAULT 1 CHECK (public_safe IN (0, 1)),
  created_at         text NOT NULL
);

-- Justified by: source expansion for one request (WHERE
-- search_request_id = ?), same as the D1 twin.
CREATE INDEX IF NOT EXISTS agent_search_sources_request_idx
  ON agent_search_sources (search_request_id);

CREATE TABLE IF NOT EXISTS agent_search_quota_events (
  id              text PRIMARY KEY,
  actor_ref       text NOT NULL,
  credential_id   text NOT NULL,
  event_kind      text NOT NULL CHECK (event_kind IN ('search_request', 'provider_request')),
  mode            text NOT NULL CHECK (mode IN ('basic')),
  units           bigint NOT NULL CHECK (units > 0),
  product_id      text,
  entitlement_ref text,
  created_at      text NOT NULL
);

-- Justified by: countQuotaEventsSince (actor_ref, credential_id,
-- event_kind, created_at >= ?) — the free-quota denial gate. The D1 twin
-- indexes (actor_ref, event_kind, created_at); the credential column is
-- included here because the enforcing read filters on it too.
CREATE INDEX IF NOT EXISTS agent_search_quota_actor_kind_created_idx
  ON agent_search_quota_events (actor_ref, credential_id, event_kind, created_at DESC);

-- Justified by: countProviderRequestsSince (event_kind = 'provider_request'
-- AND created_at >= ?) — the global provider-budget gate — and the D1
-- twin's per-credential listing.
CREATE INDEX IF NOT EXISTS agent_search_quota_kind_created_idx
  ON agent_search_quota_events (event_kind, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_search_cache_entries (
  id           text PRIMARY KEY,
  cache_key    text NOT NULL,
  mode         text NOT NULL CHECK (mode IN ('basic')),
  provider     text NOT NULL CHECK (provider IN ('exa')),
  results_json text NOT NULL CHECK (length(results_json) <= 12000),
  result_count bigint NOT NULL DEFAULT 0 CHECK (result_count >= 0),
  cost_dollars double precision,
  created_at   text NOT NULL,
  expires_at   text NOT NULL,
  archived_at  text
);

-- Justified by: readFreshCache (cache_key, expires_at > now, archived_at
-- IS NULL, newest first) — the cache-hit read; partial like the D1 twin.
CREATE INDEX IF NOT EXISTS agent_search_cache_fresh_idx
  ON agent_search_cache_entries (cache_key, expires_at DESC)
  WHERE archived_at IS NULL;

-- Port of the D1 twin's one-active-entry-per-key guard: storeCache
-- archives the previous entry and inserts the fresh one in one batch.
CREATE UNIQUE INDEX IF NOT EXISTS agent_search_cache_key_active_idx
  ON agent_search_cache_entries (cache_key)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS agent_search_payment_challenges (
  id                     text PRIMARY KEY,
  idempotency_key_hash   text NOT NULL UNIQUE,
  actor_ref              text NOT NULL,
  agent_user_id          text NOT NULL,
  credential_id          text NOT NULL,
  token_prefix           text NOT NULL,
  method                 text NOT NULL CHECK (method IN ('POST')),
  path                   text NOT NULL CHECK (path = '/api/agents/search'),
  mode                   text NOT NULL CHECK (mode IN ('basic')),
  request_body_digest    text NOT NULL,
  product_id             text NOT NULL,
  price_asset            text NOT NULL CHECK (price_asset IN ('credits')),
  price_denomination     text NOT NULL CHECK (price_denomination = 'credit'),
  price_value            bigint NOT NULL CHECK (price_value > 0),
  spend_cap_asset        text NOT NULL CHECK (spend_cap_asset IN ('credits')),
  spend_cap_denomination text NOT NULL CHECK (spend_cap_denomination = 'credit'),
  spend_cap_value        bigint NOT NULL CHECK (spend_cap_value >= 0),
  expires_at             text NOT NULL,
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);

-- Justified by: per-actor challenge listing (actor_ref, created_at DESC),
-- same as the D1 twin.
CREATE INDEX IF NOT EXISTS agent_search_payment_challenges_actor_idx
  ON agent_search_payment_challenges (actor_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_search_payment_receipts (
  id                     text PRIMARY KEY,
  receipt_ref            text NOT NULL UNIQUE,
  challenge_id           text NOT NULL UNIQUE,
  actor_ref              text NOT NULL,
  agent_user_id          text NOT NULL,
  credential_id          text NOT NULL,
  product_id             text NOT NULL,
  amount_asset           text NOT NULL CHECK (amount_asset IN ('credits')),
  amount_denomination    text NOT NULL CHECK (amount_denomination = 'credit'),
  amount_value           bigint NOT NULL CHECK (amount_value > 0),
  entitlement_ref        text NOT NULL UNIQUE,
  redacted_payment_ref   text NOT NULL,
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);

-- Justified by: per-actor receipt listing (actor_ref, created_at DESC),
-- same as the D1 twin.
CREATE INDEX IF NOT EXISTS agent_search_payment_receipts_actor_idx
  ON agent_search_payment_receipts (actor_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_search_entitlements (
  id                  text PRIMARY KEY,
  entitlement_ref     text NOT NULL UNIQUE,
  challenge_id        text NOT NULL UNIQUE,
  receipt_ref         text NOT NULL,
  actor_ref           text NOT NULL,
  agent_user_id       text NOT NULL,
  credential_id       text NOT NULL,
  product_id          text NOT NULL,
  scope_ref           text NOT NULL,
  method              text NOT NULL CHECK (method IN ('POST')),
  path                text NOT NULL CHECK (path = '/api/agents/search'),
  mode                text NOT NULL CHECK (mode IN ('basic')),
  request_body_digest text NOT NULL,
  status              text NOT NULL CHECK (status IN ('active', 'consumed', 'expired')),
  expires_at          text NOT NULL,
  created_at          text NOT NULL,
  consumed_at         text,
  archived_at         text
);

-- Justified by: the consume path's active-entitlement lookup
-- (actor_ref, status, expires_at), same as the D1 twin.
CREATE INDEX IF NOT EXISTS agent_search_entitlements_actor_status_idx
  ON agent_search_entitlements (actor_ref, status, expires_at);

CREATE TABLE IF NOT EXISTS agent_search_payment_redemptions (
  id                     text PRIMARY KEY,
  idempotency_key_hash   text NOT NULL UNIQUE,
  challenge_id           text NOT NULL UNIQUE,
  actor_ref              text NOT NULL,
  credential_id          text NOT NULL,
  proof_ref              text NOT NULL,
  entitlement_ref        text NOT NULL,
  receipt_ref            text NOT NULL,
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);
-- Reads are single-row by idempotency_key_hash / challenge_id — the
-- UNIQUE constraints cover them; no secondary index needed.
