-- KS-8.7 (#8318): billing / credits / Stripe / pay-ins domain — Postgres
-- twins of the 22 live D1 tables (worker migrations 0016/0018/0019/0031/
-- 0052/0114/0125/0160/0169/0170/0211/0226/0290).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.4 (Wave B) + §1 universal
-- porting rules; templates: 0005_pylon_dispatch.sql (KS-8.1) and
-- 0008_token_usage_ledger.sql (KS-8.2).
--
-- MONEY-DOMAIN DISCIPLINE (the §3.4 risk list):
--   * D1 stays the SOLE write authority for this domain. These tables are
--     a best-effort dual-write mirror + backfill target only; reads default
--     to D1 and any read cutover is an epic-gated ops decision (#8282).
--   * Amounts keep D1's exact integer representations: cents and msat are
--     bigint (msat sums exceed int4), never numeric/float — the acceptance
--     is balance = SUM(amount_cents) reconciling TO THE CENT per account
--     and SUM(cost_msat)/SUM(amount_msat) reconciling exactly per
--     type/state/direction.
--   * IDEMPOTENCY KEYS PORT EXACTLY: every D1 UNIQUE surface below is the
--     same key set — `stripe_webhook_events.event_id` (the webhook dedupe
--     gate for everything downstream), every `idempotency_key` /
--     `idempotency_key_hash`, the buyer redemption `challenge_ref`, the
--     spend-limit `(actor_ref, scope_ref, window_ref)` window, the
--     reconciliation `(provider_ref, external_event_ref)` pair, the Stripe
--     `(stripe_customer_id, livemode)` / `(stripe_payment_method_id,
--     livemode)` pairs, and the paid-plan intent uniques. Keys round-trip
--     byte-exact (TEXT → text).
--   * The mirror/backfill write mode is a CONVERGE upsert to the
--     authoritative D1 row bytes (`ON CONFLICT (pk) DO UPDATE`). This never
--     re-makes an idempotency decision: D1's INSERT OR IGNORE already made
--     it, and Postgres only ever receives rows D1 accepted. Converge (not
--     DO NOTHING) is required because webhook status, checkout fulfillment,
--     pay-in state machines, auto-top-up policies, and notification rows
--     are legitimately UPDATEd in place on D1.
--   * Side-effectful evaluators (auto-top-up, sweeps, Stripe API calls)
--     run against exactly ONE store: D1. Nothing in Postgres triggers or
--     feeds an evaluator in this lane.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): TEXT ISO-8601 timestamps stay
-- text (sort correctly, hash byte-exact), 0/1 booleans stay smallint with
-- CHECK (enabled, livemode, replayed), JSON stays text (row-hash
-- reconciliation compares exact bytes). Tightening to native types is a
-- post-retirement cleanup, never mid-migration.
--
-- REWRITE ARTIFACT: `billing_ledger_entries_next` (worker 0031/0170) was a
-- rename-swap scratch table; both migrations renamed it back to
-- `billing_ledger_entries`, so no `_next` table exists live and nothing is
-- created for it here. The decommission follow-up verifies its absence.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL POSTGRES READS (the KS-8.2 rule).
-- This lane routes exactly ONE read to Postgres behind
-- KHALA_SYNC_BILLING_READS: the per-user balance
-- (SELECT SUM(amount_cents) FROM billing_ledger_entries WHERE user_id = ?).
-- KEPT beyond PKs/uniques:
--   * billing_ledger_entries_user_created_idx — the routed balance SUM and
--     the recent-entries projection both filter on user_id first (and the
--     decommission lane's recent-entries read orders by created_at DESC).
-- DROPPED D1 indexes (no Postgres read in this lane uses them; the
-- decommission follow-up moves the remaining reads WITH their re-derived
-- indexes): idx_billing_ledger_entries_run, idx_billing_usage_cursors_user,
-- idx_billing_credit_notifications_status,
-- idx_billing_auto_top_up_events_user_created,
-- idx_stripe_saved_payment_methods_customer,
-- idx_stripe_checkout_sessions_user_created / _fulfillment,
-- idx_stripe_webhook_events_received, idx_pay_ins_state / _payer /
-- _public_receipt_ref, idx_pay_in_legs_pay_in / _party, the seven
-- buyer_payment_* partial actor/history indexes,
-- first_batch_payment_policies_assignment_idx / _site_idx, and
-- idx_khala_code_paid_plan_intents_account / _status.
-- PARTIAL UNIQUE indexes that are CONSTRAINTS (not read accelerators) are
-- ported verbatim: first_batch_payment_policies_order_active_idx.
--
-- NO FOREIGN KEYS (same discipline as KS-8.1/8.2): dual-write mirrors and
-- the backfill land per-row and per-table; referential integrity is
-- verified by reconciliation, not enforced mid-migration (D1's FKs to
-- users/teams/agent_runs/software_orders reference tables outside this
-- domain anyway).

-- ---------------------------------------------------------------------------
-- billing_* (worker 0016/0018/0019/0031/0170)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS billing_accounts (
  user_id    text NOT NULL PRIMARY KEY,
  currency   text NOT NULL DEFAULT 'USD',
  status     text NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'suspended')),
  created_at text NOT NULL,
  updated_at text NOT NULL
);

CREATE TABLE IF NOT EXISTS billing_ledger_entries (
  id              text NOT NULL PRIMARY KEY,
  user_id         text NOT NULL,
  team_id         text,
  run_id          text,
  source          text NOT NULL CHECK (
    source IN (
      'trial_grant', 'coupon', 'credit_card_placeholder', 'stripe_checkout',
      'stripe_auto_top_up', 'container_usage', 'codex_usage',
      'manual_adjustment'
    )
  ),
  description     text NOT NULL,
  amount_cents    bigint NOT NULL,
  currency        text NOT NULL DEFAULT 'USD',
  quantity        bigint,
  unit            text,
  unit_rate_cents bigint,
  metadata_json   text NOT NULL DEFAULT '{}',
  idempotency_key text NOT NULL UNIQUE,
  created_at      text NOT NULL
);

-- The routed balance SUM + recent-entries projection filter on user_id.
CREATE INDEX IF NOT EXISTS billing_ledger_entries_user_created_idx
  ON billing_ledger_entries (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS billing_usage_cursors (
  run_id                text NOT NULL,
  meter                 text NOT NULL,
  user_id               text NOT NULL,
  team_id               text,
  last_billed_at        text NOT NULL,
  total_billed_quantity bigint NOT NULL DEFAULT 0,
  updated_at            text NOT NULL,
  PRIMARY KEY (run_id, meter)
);

CREATE TABLE IF NOT EXISTS billing_coupon_redemptions (
  user_id         text NOT NULL,
  coupon_code     text NOT NULL,
  ledger_entry_id text NOT NULL,
  redeemed_at     text NOT NULL,
  PRIMARY KEY (user_id, coupon_code)
);

CREATE TABLE IF NOT EXISTS billing_credit_notifications (
  user_id         text NOT NULL,
  kind            text NOT NULL CHECK (kind IN ('out_of_credits')),
  email           text,
  display_name    text NOT NULL,
  balance_cents   bigint NOT NULL,
  status          text NOT NULL CHECK (status IN ('pending', 'sent', 'failed')),
  resend_email_id text,
  error_message   text,
  idempotency_key text NOT NULL UNIQUE,
  created_at      text NOT NULL,
  updated_at      text NOT NULL,
  PRIMARY KEY (user_id, kind)
);

CREATE TABLE IF NOT EXISTS billing_auto_top_up_policies (
  user_id                text NOT NULL,
  currency               text NOT NULL DEFAULT 'USD',
  enabled                smallint NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
  threshold_cents        bigint NOT NULL,
  amount_cents           bigint NOT NULL,
  monthly_cap_cents      bigint NOT NULL,
  spent_this_month_cents bigint NOT NULL DEFAULT 0,
  cap_period_yyyymm      text NOT NULL,
  status                 text NOT NULL
    CHECK (status IN ('active', 'disabled', 'paused')),
  pause_reason           text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  PRIMARY KEY (user_id, currency)
);

CREATE TABLE IF NOT EXISTS billing_auto_top_up_events (
  id                       text NOT NULL PRIMARY KEY,
  user_id                  text NOT NULL,
  status                   text NOT NULL CHECK (
    status IN (
      'succeeded', 'declined', 'cap_reached', 'skipped',
      'requires_payment_method'
    )
  ),
  amount_cents             bigint NOT NULL,
  currency                 text NOT NULL DEFAULT 'USD',
  balance_before_cents     bigint,
  balance_after_cents      bigint,
  stripe_payment_intent_id text,
  ledger_entry_id          text,
  reason                   text,
  idempotency_key          text NOT NULL UNIQUE,
  created_at               text NOT NULL
);

-- ---------------------------------------------------------------------------
-- stripe_* (worker 0031/0170)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS stripe_customers (
  user_id            text NOT NULL,
  currency           text NOT NULL DEFAULT 'USD',
  stripe_customer_id text NOT NULL,
  livemode           smallint NOT NULL DEFAULT 0 CHECK (livemode IN (0, 1)),
  email_snapshot     text,
  created_at         text NOT NULL,
  updated_at         text NOT NULL,
  PRIMARY KEY (user_id, currency, livemode),
  UNIQUE (stripe_customer_id, livemode)
);

CREATE TABLE IF NOT EXISTS stripe_checkout_sessions (
  session_id         text NOT NULL PRIMARY KEY,
  user_id            text NOT NULL,
  package_id         text NOT NULL,
  amount_cents       bigint NOT NULL,
  currency           text NOT NULL DEFAULT 'USD',
  payment_status     text NOT NULL,
  fulfillment_status text NOT NULL CHECK (
    fulfillment_status IN (
      'pending', 'fulfilled', 'unpaid', 'expired', 'mismatched'
    )
  ),
  ledger_entry_id    text,
  stripe_customer_id text NOT NULL,
  checkout_url       text,
  created_at         text NOT NULL,
  updated_at         text NOT NULL
);

-- The Stripe webhook replay dedupe gate: event_id is the PRIMARY KEY on
-- both sides; D1's INSERT OR IGNORE and the Postgres converge upsert share
-- the identical key, byte-exact.
CREATE TABLE IF NOT EXISTS stripe_webhook_events (
  event_id            text NOT NULL PRIMARY KEY,
  type                text NOT NULL,
  processing_status   text NOT NULL CHECK (
    processing_status IN ('received', 'processed', 'ignored', 'failed')
  ),
  checkout_session_id text,
  received_at         text NOT NULL,
  processed_at        text
);

CREATE TABLE IF NOT EXISTS stripe_saved_payment_methods (
  user_id                  text NOT NULL,
  currency                 text NOT NULL DEFAULT 'USD',
  livemode                 smallint NOT NULL DEFAULT 0
    CHECK (livemode IN (0, 1)),
  stripe_customer_id       text NOT NULL,
  stripe_payment_method_id text NOT NULL,
  setup_intent_id          text,
  brand                    text,
  last4                    text,
  exp_month                bigint,
  exp_year                 bigint,
  status                   text NOT NULL CHECK (
    status IN ('active', 'detached', 'failed', 'requires_action')
  ),
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  PRIMARY KEY (user_id, currency, livemode),
  UNIQUE (stripe_payment_method_id, livemode)
);

-- ---------------------------------------------------------------------------
-- pay_ins / pay_in_legs (worker 0160/0169/0211/0226)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS pay_ins (
  id                 text NOT NULL PRIMARY KEY,
  pay_in_type        text NOT NULL CHECK (
    pay_in_type IN (
      'tip', 'sweep', 'buffer_funding', 'reward', 'adjustment',
      'usd_credit_grant', 'lightning_charge'
    )
  ),
  payer_ref          text NOT NULL,
  cost_msat          bigint NOT NULL CHECK (cost_msat > 0),
  state              text NOT NULL CHECK (
    state IN ('pending', 'forwarding', 'paid', 'failed')
  ),
  failure_reason     text,
  rung               text CHECK (rung IN ('credited', 'direct_bolt12') OR rung IS NULL),
  context_ref        text,
  idempotency_key    text NOT NULL UNIQUE,
  genesis_id         text,
  successor_id       text,
  created_at         text NOT NULL,
  state_changed_at   text NOT NULL,
  public_receipt_ref text
);

CREATE TABLE IF NOT EXISTS pay_in_legs (
  id                     text NOT NULL PRIMARY KEY,
  pay_in_id              text NOT NULL,
  direction              text NOT NULL CHECK (direction IN ('in', 'out')),
  kind                   text NOT NULL CHECK (kind IN ('balance', 'lightning')),
  party_ref              text NOT NULL,
  amount_msat            bigint NOT NULL CHECK (amount_msat > 0),
  resulting_balance_msat bigint,
  external_ref           text,
  refund_of_leg_id       text,
  created_at             text NOT NULL
);

-- The mirror reads back a pay-in's legs by pay_in_id (and the decommission
-- lane's per-pay-in leg reads use the same access path).
CREATE INDEX IF NOT EXISTS pay_in_legs_pay_in_idx
  ON pay_in_legs (pay_in_id);

-- ---------------------------------------------------------------------------
-- buyer_payment_* (worker 0114/0125)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS buyer_payment_challenges (
  id                           text NOT NULL PRIMARY KEY,
  challenge_ref                text NOT NULL UNIQUE,
  idempotency_key_hash         text NOT NULL UNIQUE,
  actor_ref                    text NOT NULL,
  owner_user_id                text,
  product_id                   text NOT NULL,
  surface                      text NOT NULL CHECK (
    surface IN (
      'agent_api', 'billing', 'forum_paid_action', 'runner', 'site_checkout'
    )
  ),
  method                       text NOT NULL
    CHECK (method IN ('GET', 'POST', 'PUT', 'PATCH', 'DELETE')),
  path                         text NOT NULL,
  request_body_digest          text NOT NULL,
  price_asset                  text NOT NULL
    CHECK (price_asset IN ('bitcoin', 'credits', 'usd')),
  price_denomination           text NOT NULL,
  price_amount_minor_units     bigint NOT NULL
    CHECK (price_amount_minor_units >= 0),
  spend_cap_asset              text NOT NULL
    CHECK (spend_cap_asset IN ('bitcoin', 'credits', 'usd')),
  spend_cap_denomination       text NOT NULL,
  spend_cap_amount_minor_units bigint NOT NULL
    CHECK (spend_cap_amount_minor_units >= 0),
  status                       text NOT NULL
    CHECK (status IN ('issued', 'expired', 'cancelled')),
  expires_at                   text NOT NULL,
  metadata_refs_json           text NOT NULL DEFAULT '[]',
  public_projection_json       text NOT NULL DEFAULT '{}',
  created_at                   text NOT NULL,
  archived_at                  text
);

CREATE TABLE IF NOT EXISTS buyer_payment_receipts (
  id                     text NOT NULL PRIMARY KEY,
  receipt_ref            text NOT NULL UNIQUE,
  challenge_ref          text NOT NULL,
  actor_ref              text NOT NULL,
  owner_user_id          text,
  product_id             text NOT NULL,
  surface                text NOT NULL CHECK (
    surface IN (
      'agent_api', 'billing', 'forum_paid_action', 'runner', 'site_checkout'
    )
  ),
  amount_asset           text NOT NULL
    CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination    text NOT NULL,
  amount_minor_units     bigint NOT NULL CHECK (amount_minor_units >= 0),
  entitlement_ref        text NOT NULL UNIQUE,
  redacted_payment_ref   text NOT NULL,
  status                 text NOT NULL CHECK (status IN ('issued', 'voided')),
  metadata_refs_json     text NOT NULL DEFAULT '[]',
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);

CREATE TABLE IF NOT EXISTS buyer_payment_entitlements (
  id              text NOT NULL PRIMARY KEY,
  entitlement_ref text NOT NULL UNIQUE,
  challenge_ref   text NOT NULL,
  receipt_ref     text NOT NULL,
  actor_ref       text NOT NULL,
  owner_user_id   text,
  product_id      text NOT NULL,
  surface         text NOT NULL CHECK (
    surface IN (
      'agent_api', 'billing', 'forum_paid_action', 'runner', 'site_checkout'
    )
  ),
  scope_refs_json text NOT NULL DEFAULT '[]',
  status          text NOT NULL
    CHECK (status IN ('active', 'consumed', 'expired', 'revoked')),
  expires_at      text,
  created_at      text NOT NULL,
  consumed_at     text,
  archived_at     text
);

CREATE TABLE IF NOT EXISTS buyer_payment_redemptions (
  id                     text NOT NULL PRIMARY KEY,
  redemption_ref         text NOT NULL UNIQUE,
  idempotency_key_hash   text NOT NULL UNIQUE,
  challenge_ref          text NOT NULL UNIQUE,
  actor_ref              text NOT NULL,
  proof_ref              text NOT NULL,
  entitlement_ref        text NOT NULL,
  receipt_ref            text NOT NULL,
  status                 text NOT NULL
    CHECK (status IN ('redeemed', 'replayed', 'rejected')),
  replayed               smallint NOT NULL DEFAULT 0 CHECK (replayed IN (0, 1)),
  metadata_refs_json     text NOT NULL DEFAULT '[]',
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);

CREATE TABLE IF NOT EXISTS buyer_payment_spend_limits (
  id                  text NOT NULL PRIMARY KEY,
  spend_limit_ref     text NOT NULL UNIQUE,
  actor_ref           text NOT NULL,
  owner_user_id       text,
  product_id          text,
  scope_ref           text NOT NULL,
  window_ref          text NOT NULL,
  amount_asset        text NOT NULL
    CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination text NOT NULL,
  amount_minor_units  bigint NOT NULL CHECK (amount_minor_units >= 0),
  status              text NOT NULL
    CHECK (status IN ('active', 'exhausted', 'revoked')),
  metadata_refs_json  text NOT NULL DEFAULT '[]',
  created_at          text NOT NULL,
  updated_at          text NOT NULL,
  archived_at         text,
  UNIQUE (actor_ref, scope_ref, window_ref)
);

CREATE TABLE IF NOT EXISTS buyer_payment_credit_debits (
  id                       text NOT NULL PRIMARY KEY,
  debit_ref                text NOT NULL UNIQUE,
  idempotency_key_hash     text NOT NULL UNIQUE,
  actor_ref                text NOT NULL,
  owner_user_id            text,
  product_id               text NOT NULL,
  amount_asset             text NOT NULL CHECK (amount_asset IN ('credits')),
  amount_denomination      text NOT NULL,
  amount_minor_units       bigint NOT NULL CHECK (amount_minor_units >= 0),
  billing_ledger_entry_ref text,
  receipt_ref              text,
  status                   text NOT NULL
    CHECK (status IN ('reserved', 'captured', 'released', 'voided')),
  metadata_refs_json       text NOT NULL DEFAULT '[]',
  public_projection_json   text NOT NULL DEFAULT '{}',
  created_at               text NOT NULL,
  archived_at              text
);

CREATE TABLE IF NOT EXISTS buyer_payment_reconciliation_events (
  id                     text NOT NULL PRIMARY KEY,
  event_ref              text NOT NULL UNIQUE,
  idempotency_key_hash   text NOT NULL UNIQUE,
  provider_ref           text NOT NULL,
  external_event_ref     text NOT NULL,
  challenge_ref          text,
  receipt_ref            text,
  product_id             text,
  status                 text NOT NULL
    CHECK (status IN ('observed', 'matched', 'replayed', 'rejected')),
  result_ref             text NOT NULL,
  metadata_refs_json     text NOT NULL DEFAULT '[]',
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text,
  UNIQUE (provider_ref, external_event_ref)
);

-- ---------------------------------------------------------------------------
-- first_batch_payment_policies (worker 0052)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS first_batch_payment_policies (
  id                    text NOT NULL PRIMARY KEY,
  software_order_id     text NOT NULL,
  assignment_id         text,
  site_id               text,
  policy_mode           text NOT NULL
    CHECK (policy_mode IN ('public_beta_free', 'operator_grant')),
  applied_by_user_id    text,
  reason                text NOT NULL CHECK (length(reason) > 0),
  customer_safe_summary text NOT NULL CHECK (length(customer_safe_summary) > 0),
  created_at            text NOT NULL,
  updated_at            text NOT NULL,
  archived_at           text
);

-- CONSTRAINT (not a read accelerator): at most one ACTIVE policy per
-- software order — ported verbatim from worker 0052.
CREATE UNIQUE INDEX IF NOT EXISTS first_batch_payment_policies_order_active_idx
  ON first_batch_payment_policies (software_order_id)
  WHERE archived_at IS NULL;

-- ---------------------------------------------------------------------------
-- khala_code_paid_plan_payment_intents (worker 0290)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS khala_code_paid_plan_payment_intents (
  purchase_ref                 text NOT NULL PRIMARY KEY,
  account_ref                  text NOT NULL,
  idempotency_key              text NOT NULL UNIQUE,
  rail                         text NOT NULL
    CHECK (rail IN ('stripe_checkout', 'lightning_mpp')),
  status                       text NOT NULL CHECK (
    status IN ('requires_payment', 'fulfilled', 'failed', 'expired')
  ),
  plan_id                      text NOT NULL,
  amount_cents                 bigint,
  amount_sats                  bigint,
  stripe_checkout_session_id   text UNIQUE,
  stripe_checkout_url          text,
  lightning_payment_hash       text UNIQUE,
  lightning_invoice            text,
  lightning_network            text CHECK (
    lightning_network IS NULL
    OR lightning_network IN ('mainnet', 'regtest', 'signet')
  ),
  lightning_invoice_expires_at text,
  entitlement_receipt_ref      text,
  failure_reason               text,
  created_at                   text NOT NULL,
  updated_at                   text NOT NULL,
  fulfilled_at                 text
);
