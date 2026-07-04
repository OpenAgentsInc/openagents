-- KS-8.8 (#8319): Treasury, payouts, and tips settlement — Postgres twins of
-- the 27 live D1 money-domain tables (worker migrations 0101/0122/0128/0131/
-- 0143/0146/0147/0149/0151/0153/0159..0167/0184/0196..0199/0203/0204/0206/
-- 0211/0214/0224/0225/0261/0293). Plan: docs/khala-sync/MIGRATION_PLAN.md
-- §3.5 (Wave B). The HIGHEST-STAKES domain: D1 stays sole authority; these
-- twins receive best-effort dual-write mirrors + backfill only, and reads
-- stay on D1 until the epic-gated runbook cutover.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps (they sort correctly as text),
-- 0/1 booleans as smallint, JSON payloads as text (NOT jsonb: row-hash
-- reconciliation compares exact bytes), and every money amount (msat / sat /
-- cent / minor-unit) as bigint so millisat totals reconcile exactly.
-- Tightening to native types is a post-retirement cleanup, never
-- mid-migration.
--
-- IDEMPOTENCY / REPLAY KEYS PORT EXACTLY (MIGRATION_PLAN universal rule):
-- every D1 UNIQUE that a dedupe or replay guard rides ports as the SAME
-- Postgres unique constraint — payout intent/attempt idempotency_key_hash,
-- reconciliation (provider_ref, external_event_ref), forum money
-- idempotency_key + (provider_ref, external_ref), the L402 challenge_id
-- one-redemption rule, tip settlement one-claim-per-receipt, escrow receipt
-- refs + (escrow_id, transition_kind) once-only transitions, and the two
-- MPP replay-guard primary keys (payment_hash / spt) — so
-- `ON CONFLICT ... DO NOTHING / DO UPDATE` converges on the exact keys the
-- D1 authority dedupes on.
--
-- DELIBERATELY NOT PORTED (re-added at read cutover, per the KS-8.6
-- precedent for D1 partial uniques that can transiently mis-order under
-- per-row mirroring):
--   - agent_claim_reward_ledger's three partial uniques
--     ((campaign_ref, x_account_ref|owner_ref|agent_claim_ref) WHERE state
--     NOT IN ('rejected','reversed')): a replacement row can mirror before
--     its predecessor's rejected/reversed transition lands. The invariant
--     stays enforced by the D1 authority and is re-checked at
--     reconciliation.
--   - all cross-table FOREIGN KEYs (nexus attempt→intent, webhook→attempt,
--     redemption→challenge, receipts→escrow, ...): mirrors and backfill
--     land per-table/per-row; integrity is verified by set-membership at
--     reconciliation, not enforced mid-migration.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS in the owning worker
-- modules (treasury-page-routes.ts, nexus-treasury-payout-ledger.ts,
-- forum/paid-actions.ts, forum/repository.ts, tips-sweep.ts,
-- x-claim-reward-treasury-dispatcher.ts, agent-claim-reward-ledger.ts,
-- labor-escrow.ts, partner-payout-ledger.ts, site-referral-payout-ledger.ts,
-- revenue-event-provenance.ts, payments-ledger.ts) — NOT blind-ported from
-- D1. Each index below names the read it serves. Pure replay-guard tables
-- (mpp_*_replay) are point-lookup-by-PK only and get NO secondary indexes.

-- ---------------------------------------------------------------------------
-- Treasury wallet transactions (worker 0159→0197..0199 shape).
-- Reads: public treasury page latest-N (created_at DESC); the
-- TreasuryTransactions.reconcilePending cron scans state='pending' oldest
-- first; recipient attribution lists by recipient_ref.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS treasury_transactions (
  id                           text PRIMARY KEY,
  direction                    text NOT NULL CHECK (direction IN ('in', 'out')),
  amount_sat                   bigint NOT NULL DEFAULT 0,
  state                        text NOT NULL CHECK (state IN ('pending', 'settled', 'expired', 'failed')),
  bolt11                       text,
  payment_ref                  text,
  created_at                   text NOT NULL,
  settled_at                   text,
  expires_at                   text,
  failure_reason_ref           text,
  recipient_ref                text,
  redacted_destination_ref     text,
  owed_ref                     text,
  owed_sat                     bigint,
  recipient_confirmation_state text NOT NULL DEFAULT 'unconfirmed'
    CHECK (recipient_confirmation_state IN ('unconfirmed', 'confirmed_received')),
  recipient_confirmation_ref   text,
  recipient_confirmed_at       text
);

CREATE INDEX IF NOT EXISTS idx_pg_treasury_transactions_created
  ON treasury_transactions (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_pg_treasury_transactions_state_created
  ON treasury_transactions (state, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_pg_treasury_transactions_recipient
  ON treasury_transactions (recipient_ref, created_at DESC);

-- ---------------------------------------------------------------------------
-- Nexus payout authority (worker 0122 + 0203/0204). APPEND-ONLY ledgers —
-- the D1 store exposes only INSERT OR IGNORE creates + point reads by
-- ref/idempotency-key-hash, plus status/assignment scans on intents and
-- intent-scoped lists on attempts/receipts. The payout DISPATCHER reads
-- exactly one store (D1) during dual-write — these twins never drive
-- dispatch until the epic-gated cutover.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS nexus_payout_target_approvals (
  id                       text NOT NULL,
  approval_ref             text PRIMARY KEY,
  idempotency_key_hash     text NOT NULL UNIQUE,
  payout_target_ref        text NOT NULL,
  redacted_destination_ref text NOT NULL,
  owner_user_id            text,
  agent_ref                text,
  pylon_ref                text,
  status                   text NOT NULL CHECK (status IN ('active', 'expired', 'rejected', 'revoked')),
  approved_by_ref          text NOT NULL,
  approval_policy_ref      text NOT NULL,
  scope_refs_json          text NOT NULL DEFAULT '[]',
  public_projection_json   text NOT NULL DEFAULT '{}',
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  expires_at               text,
  archived_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_nexus_payout_target_approvals_target
  ON nexus_payout_target_approvals (payout_target_ref, status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_treasury_payout_intents (
  id                           text NOT NULL,
  payout_intent_ref            text PRIMARY KEY,
  idempotency_key_hash         text NOT NULL UNIQUE,
  actor_ref                    text NOT NULL,
  owner_user_id                text,
  source_kind                  text NOT NULL,
  buyer_payment_ref            text,
  accepted_work_refs_json      text NOT NULL DEFAULT '[]',
  assignment_ref               text,
  artanis_dispatch_ref         text,
  pylon_job_ref                text,
  payout_target_ref            text NOT NULL,
  payout_target_approval_ref   text NOT NULL,
  adapter_kind                 text NOT NULL,
  amount_asset                 text NOT NULL,
  amount_denomination          text NOT NULL,
  amount_minor_units           bigint NOT NULL CHECK (amount_minor_units >= 0),
  spend_cap_asset              text NOT NULL,
  spend_cap_denomination       text NOT NULL,
  spend_cap_amount_minor_units bigint NOT NULL CHECK (spend_cap_amount_minor_units >= 0),
  policy_snapshot_ref          text NOT NULL,
  status                       text NOT NULL CHECK (
    status IN ('approved', 'cancelled', 'dispatched', 'failed', 'proposed', 'rejected', 'settled')
  ),
  metadata_refs_json           text NOT NULL DEFAULT '[]',
  public_projection_json       text NOT NULL DEFAULT '{}',
  created_at                   text NOT NULL,
  updated_at                   text NOT NULL,
  archived_at                  text
);

CREATE INDEX IF NOT EXISTS idx_pg_nexus_payout_intents_status
  ON nexus_treasury_payout_intents (status, updated_at DESC)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_pg_nexus_payout_intents_assignment
  ON nexus_treasury_payout_intents (assignment_ref, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_treasury_payout_attempts (
  id                       text NOT NULL,
  payout_attempt_ref       text PRIMARY KEY,
  payout_intent_ref        text NOT NULL,
  idempotency_key_hash     text NOT NULL UNIQUE,
  adapter_kind             text NOT NULL,
  adapter_attempt_ref      text NOT NULL,
  status                   text NOT NULL CHECK (
    status IN ('confirmed', 'dispatched', 'failed', 'pending', 'rejected', 'replayed')
  ),
  redacted_payment_ref     text,
  redacted_destination_ref text NOT NULL,
  amount_asset             text NOT NULL,
  amount_denomination      text NOT NULL,
  amount_minor_units       bigint NOT NULL CHECK (amount_minor_units >= 0),
  metadata_refs_json       text NOT NULL DEFAULT '[]',
  public_projection_json   text NOT NULL DEFAULT '{}',
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  archived_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_nexus_payout_attempts_intent
  ON nexus_treasury_payout_attempts (payout_intent_ref, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_treasury_payout_reconciliation_events (
  id                     text NOT NULL,
  event_ref              text PRIMARY KEY,
  idempotency_key_hash   text NOT NULL UNIQUE,
  provider_ref           text NOT NULL,
  external_event_ref     text NOT NULL,
  adapter_kind           text NOT NULL,
  payout_intent_ref      text,
  payout_attempt_ref     text,
  status                 text NOT NULL CHECK (status IN ('matched', 'observed', 'rejected', 'replayed')),
  result_ref             text NOT NULL,
  metadata_refs_json     text NOT NULL DEFAULT '[]',
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text,
  UNIQUE (provider_ref, external_event_ref)
);

CREATE INDEX IF NOT EXISTS idx_pg_nexus_payout_reconciliation_created
  ON nexus_treasury_payout_reconciliation_events (created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_payment_authority_receipts (
  id                     text NOT NULL,
  receipt_ref            text PRIMARY KEY,
  payout_intent_ref      text NOT NULL,
  payout_attempt_ref     text,
  event_ref              text,
  receipt_kind           text NOT NULL,
  audience               text NOT NULL CHECK (audience IN ('agent', 'customer', 'operator', 'public')),
  metadata_refs_json     text NOT NULL DEFAULT '[]',
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);

CREATE INDEX IF NOT EXISTS idx_pg_nexus_payment_authority_receipts_intent
  ON nexus_payment_authority_receipts (payout_intent_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_release_gates (
  id                     text NOT NULL,
  gate_ref               text PRIMARY KEY,
  idempotency_key_hash   text NOT NULL UNIQUE,
  gate_kind              text NOT NULL,
  status                 text NOT NULL CHECK (status IN ('blocked', 'failed', 'passed', 'pending')),
  evidence_refs_json     text NOT NULL DEFAULT '[]',
  blocker_refs_json      text NOT NULL DEFAULT '[]',
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  archived_at            text
);

CREATE INDEX IF NOT EXISTS idx_pg_nexus_release_gates_kind_status
  ON nexus_release_gates (gate_kind, status, updated_at DESC)
  WHERE archived_at IS NULL;

-- ---------------------------------------------------------------------------
-- Forum money half (worker 0101/0151 + tips 0128/0131/0146/0147). Public
-- receipt/tip projections (/direct-tips evidence) list by target + recency;
-- the ForumDirectTips.archiveStaleRecoveries cron scans recovery_pending by
-- updated_at; webhook reconciliation lists by attempt + status.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS forum_money_actions (
  id                     text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  actor_ref              text NOT NULL,
  action_kind            text NOT NULL,
  target_forum_id        text,
  target_topic_id        text,
  target_post_id         text,
  amount_asset           text NOT NULL CHECK (amount_asset IN ('credits', 'sats', 'usd')),
  amount_value           bigint NOT NULL CHECK (amount_value >= 0),
  payment_event_id       text,
  receipt_id             text,
  earning_actor_ref      text,
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);

CREATE INDEX IF NOT EXISTS idx_pg_forum_money_actions_target
  ON forum_money_actions (target_topic_id, target_post_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_payment_events (
  id                     text PRIMARY KEY,
  money_action_id        text,
  provider_ref           text NOT NULL,
  external_ref           text NOT NULL,
  amount_asset           text NOT NULL CHECK (amount_asset IN ('credits', 'sats', 'usd')),
  amount_value           bigint NOT NULL CHECK (amount_value >= 0),
  redacted_evidence_ref  text NOT NULL,
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text,
  UNIQUE (provider_ref, external_ref)
);

CREATE TABLE IF NOT EXISTS forum_receipts (
  id                     text PRIMARY KEY,
  receipt_ref            text NOT NULL UNIQUE,
  action_kind            text NOT NULL,
  target_forum_id        text,
  target_topic_id        text,
  target_post_id         text,
  amount_asset           text NOT NULL CHECK (amount_asset IN ('credits', 'sats', 'usd')),
  amount_value           bigint NOT NULL CHECK (amount_value >= 0),
  recipient_actor_ref    text,
  redacted_payment_ref   text NOT NULL,
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);

CREATE INDEX IF NOT EXISTS idx_pg_forum_receipts_target
  ON forum_receipts (target_topic_id, target_post_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_l402_challenges (
  id                               text PRIMARY KEY,
  idempotency_key                  text NOT NULL UNIQUE,
  actor_ref                        text NOT NULL,
  action_kind                      text NOT NULL,
  method                           text NOT NULL,
  path                             text NOT NULL,
  route_params_json                text NOT NULL DEFAULT '{}',
  request_body_digest              text NOT NULL,
  target_forum_id                  text,
  target_topic_id                  text,
  target_post_id                   text,
  price_asset                      text NOT NULL CHECK (price_asset IN ('credits', 'sats', 'usd')),
  price_value                      bigint NOT NULL CHECK (price_value >= 0),
  spend_cap_asset                  text NOT NULL CHECK (spend_cap_asset IN ('credits', 'sats', 'usd')),
  spend_cap_value                  bigint NOT NULL CHECK (spend_cap_value >= 0),
  expires_at                       text NOT NULL,
  public_projection_json           text NOT NULL DEFAULT '{}',
  created_at                       text NOT NULL,
  archived_at                      text,
  recipient_actor_ref              text,
  recipient_readiness_ref          text,
  mdk_provider_ref                 text,
  mdk_environment                  text,
  mdk_sandbox                      smallint CHECK (mdk_sandbox IS NULL OR mdk_sandbox IN (0, 1)),
  mdk_implementation_state         text,
  mdk_checkout_ref                 text,
  mdk_checkout_url_ref             text,
  mdk_checkout_launch_path         text,
  mdk_invoice_ref                  text,
  mdk_payment_hash_ref             text,
  l402_credential_ref              text,
  l402_replay_nonce_ref            text,
  l402_endpoint_ref                text,
  l402_entitlement_scope_refs_json text,
  l402_www_authenticate            text
);

CREATE INDEX IF NOT EXISTS idx_pg_forum_l402_challenges_actor_action
  ON forum_l402_challenges (actor_ref, action_kind, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_l402_redemptions (
  id                     text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  challenge_id           text NOT NULL UNIQUE,
  actor_ref              text NOT NULL,
  proof_ref              text NOT NULL,
  entitlement_ref        text NOT NULL,
  receipt_id             text,
  replayed               smallint NOT NULL DEFAULT 0 CHECK (replayed IN (0, 1)),
  public_projection_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  archived_at            text
);

CREATE TABLE IF NOT EXISTS forum_direct_tip_attempts (
  id                    text PRIMARY KEY,
  idempotency_key       text NOT NULL UNIQUE,
  payer_actor_ref       text NOT NULL,
  recipient_actor_ref   text NOT NULL,
  target_topic_id       text NOT NULL,
  target_post_id        text NOT NULL,
  target_post_permalink text,
  amount_sats           bigint NOT NULL CHECK (amount_sats > 0),
  provider_ref          text NOT NULL,
  external_ref          text NOT NULL,
  redacted_evidence_ref text NOT NULL,
  payment_mode          text NOT NULL CHECK (payment_mode IN ('live', 'sandbox', 'signet', 'unknown')),
  payment_event_status  text NOT NULL CHECK (
    payment_event_status IN ('confirmed', 'failed', 'observed', 'refunded', 'replayed', 'reversed')
  ),
  status                text NOT NULL CHECK (status IN ('settled', 'failed', 'recovery_pending')),
  receipt_ref           text,
  payment_event_id      text,
  created_at            text NOT NULL,
  updated_at            text NOT NULL,
  archived_at           text,
  UNIQUE (provider_ref, external_ref)
);

CREATE INDEX IF NOT EXISTS idx_pg_forum_direct_tip_attempts_target
  ON forum_direct_tip_attempts (target_post_id, created_at DESC)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_pg_forum_direct_tip_attempts_status
  ON forum_direct_tip_attempts (status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_direct_tip_webhook_events (
  id                     text PRIMARY KEY,
  provider_event_ref     text NOT NULL UNIQUE,
  direct_tip_attempt_id  text NOT NULL,
  provider_ref           text NOT NULL,
  external_ref           text NOT NULL,
  amount_sats            bigint NOT NULL CHECK (amount_sats > 0),
  payment_event_status   text NOT NULL CHECK (
    payment_event_status IN ('confirmed', 'failed', 'observed', 'refunded', 'replayed', 'reversed')
  ),
  redacted_evidence_ref  text NOT NULL,
  event_body_digest_ref  text NOT NULL,
  signature_binding_ref  text NOT NULL,
  reconciliation_status  text NOT NULL CHECK (reconciliation_status IN ('settled', 'failed', 'recovery_pending')),
  reconciliation_result  text NOT NULL,
  first_seen_at          text NOT NULL,
  last_seen_at           text NOT NULL,
  delivery_count         bigint NOT NULL DEFAULT 1 CHECK (delivery_count > 0),
  archived_at            text
);

CREATE INDEX IF NOT EXISTS idx_pg_forum_direct_tip_webhook_events_attempt
  ON forum_direct_tip_webhook_events (direct_tip_attempt_id, first_seen_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_tip_recipient_wallets (
  id                         text PRIMARY KEY,
  actor_ref                  text NOT NULL UNIQUE,
  provider_class             text NOT NULL CHECK (
    provider_class IN ('mdk_agent_wallet', 'hosted_mdk', 'external_lightning')
  ),
  wallet_ref                 text NOT NULL,
  receive_capability_ref     text NOT NULL,
  payout_target_approval_ref text,
  readiness_refs_json        text NOT NULL DEFAULT '[]',
  caveat_refs_json           text NOT NULL DEFAULT '[]',
  custody_policy_refs_json   text NOT NULL DEFAULT '[]',
  claim_policy_refs_json     text NOT NULL DEFAULT '[]',
  source_ref                 text NOT NULL,
  state                      text NOT NULL CHECK (state IN ('ready', 'disabled', 'blocked')),
  public_projection_json     text NOT NULL DEFAULT '{}',
  created_at                 text NOT NULL,
  updated_at                 text NOT NULL,
  disabled_at                text,
  archived_at                text,
  bolt12_offer               text,
  lightning_address          text,
  spark_address              text
);

CREATE INDEX IF NOT EXISTS idx_pg_forum_tip_recipient_wallets_state
  ON forum_tip_recipient_wallets (state, updated_at DESC);

CREATE TABLE IF NOT EXISTS forum_tip_settlement_claims (
  id                           text PRIMARY KEY,
  idempotency_key              text NOT NULL UNIQUE,
  receipt_id                   text NOT NULL UNIQUE,
  receipt_ref                  text NOT NULL,
  recipient_actor_ref          text NOT NULL,
  settlement_ref               text NOT NULL,
  settlement_evidence_refs_json text NOT NULL DEFAULT '[]',
  source_ref                   text NOT NULL,
  public_projection_json       text NOT NULL DEFAULT '{}',
  created_at                   text NOT NULL,
  archived_at                  text
);

CREATE INDEX IF NOT EXISTS idx_pg_forum_tip_settlement_claims_receipt_ref
  ON forum_tip_settlement_claims (receipt_ref)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_pg_forum_tip_settlement_claims_recipient
  ON forum_tip_settlement_claims (recipient_actor_ref, created_at)
  WHERE archived_at IS NULL;

-- ---------------------------------------------------------------------------
-- Reward ledgers (worker 0149/0164, 0143). The
-- XClaimRewardTreasuryDispatcher.runTick cron scans state =
-- 'dispatch_requested' oldest-first and state = 'dispatched' with a
-- treasury_payment_id — both ported as the same partial indexes.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS x_claim_reward_ledger (
  id                  text PRIMARY KEY,
  challenge_id        text NOT NULL UNIQUE,
  claim_id            text NOT NULL,
  owner_user_id       text NOT NULL,
  agent_user_id       text,
  x_account_ref       text NOT NULL UNIQUE,
  amount_sats         bigint NOT NULL DEFAULT 1000,
  state               text NOT NULL CHECK (
    state IN ('eligible', 'dispatch_requested', 'dispatched', 'settled', 'failed', 'refused')
  ),
  state_reason_ref    text,
  receipt_ref         text NOT NULL UNIQUE,
  evidence_refs_json  text NOT NULL DEFAULT '[]',
  created_at          text NOT NULL,
  updated_at          text NOT NULL,
  treasury_payment_id text
);

CREATE INDEX IF NOT EXISTS idx_pg_x_claim_reward_ledger_state
  ON x_claim_reward_ledger (state, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_pg_x_claim_reward_ledger_dispatch_requested
  ON x_claim_reward_ledger (state, updated_at ASC)
  WHERE state = 'dispatch_requested';
CREATE INDEX IF NOT EXISTS idx_pg_x_claim_reward_ledger_pending_payment
  ON x_claim_reward_ledger (state, treasury_payment_id, updated_at ASC)
  WHERE state = 'dispatched' AND treasury_payment_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS agent_claim_reward_ledger (
  id                       text PRIMARY KEY,
  idempotency_key          text NOT NULL UNIQUE,
  campaign_ref             text NOT NULL,
  agent_claim_ref          text NOT NULL,
  owner_ref                text NOT NULL,
  x_account_ref            text NOT NULL,
  tweet_ref                text NOT NULL,
  state                    text NOT NULL CHECK (
    state IN ('pending', 'verified', 'approved', 'payout_intent_created', 'dispatched',
              'settled', 'rejected', 'reversed', 'expired')
  ),
  amount_sats              bigint NOT NULL CHECK (amount_sats = 1000),
  destination_kind         text NOT NULL,
  redacted_destination_ref text,
  payout_intent_ref        text,
  dispatch_attempt_ref     text,
  settlement_ref           text,
  rejection_reason         text,
  policy_refs_json         text NOT NULL DEFAULT '[]',
  caveat_refs_json         text NOT NULL DEFAULT '[]',
  created_at               text NOT NULL,
  updated_at               text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_agent_claim_reward_ledger_state
  ON agent_claim_reward_ledger (state, updated_at DESC);

-- ---------------------------------------------------------------------------
-- Agent balances (worker 0160 + 0167/0211 columns). msat-exact; the
-- TipsSweep.runTick cron scans sweep-eligible balances; the
-- TipsBuffer.backingInvariant cron SUMs balance_msat. balance_msat /
-- held_msat / usd_credit_msat reconcile to the millisat.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS agent_balances (
  actor_ref                text PRIMARY KEY,
  balance_msat             bigint NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  sweep_enabled            smallint NOT NULL DEFAULT 1,
  sweep_threshold_sat      bigint NOT NULL DEFAULT 210,
  send_credits_below_sat   bigint NOT NULL DEFAULT 10,
  receive_credits_below_sat bigint NOT NULL DEFAULT 10,
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  held_msat                bigint NOT NULL DEFAULT 0 CHECK (held_msat >= 0),
  usd_credit_msat          bigint NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0)
);

CREATE INDEX IF NOT EXISTS idx_pg_agent_balances_sweep
  ON agent_balances (sweep_enabled, balance_msat, held_msat);

-- ---------------------------------------------------------------------------
-- Labor escrows (worker 0167 → 0261 shape). One escrow per work request;
-- receipts are once-only per (escrow, transition).
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS labor_escrows (
  id                            text PRIMARY KEY,
  idempotency_key               text NOT NULL UNIQUE,
  work_request_id               text NOT NULL UNIQUE,
  requester_actor_ref           text NOT NULL,
  provider_actor_ref            text,
  amount_msat                   bigint NOT NULL CHECK (amount_msat > 0),
  state                         text NOT NULL CHECK (
    state IN ('reserved', 'released_to_provider', 'refunded', 'forfeited')
  ),
  funding_source                text NOT NULL DEFAULT 'ledger_balance' CHECK (
    funding_source IN ('ledger_balance', 'external_invoice_pending')
  ),
  job_event_id                  text NOT NULL,
  acceptance_event_ref          text,
  reserve_receipt_ref           text NOT NULL UNIQUE,
  release_receipt_ref           text UNIQUE,
  refund_receipt_ref            text UNIQUE,
  forfeit_receipt_ref           text UNIQUE,
  forfeit_destination           text CHECK (
    forfeit_destination IS NULL OR forfeit_destination IN ('counterparty', 'burn')
  ),
  forfeit_destination_actor_ref text,
  forfeit_condition_ref         text,
  public_projection_json        text NOT NULL DEFAULT '{}',
  created_at                    text NOT NULL,
  updated_at                    text NOT NULL,
  released_at                   text,
  refunded_at                   text,
  forfeited_at                  text,
  archived_at                   text
);

CREATE INDEX IF NOT EXISTS idx_pg_labor_escrows_state
  ON labor_escrows (state, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS labor_escrow_receipts (
  id                            text PRIMARY KEY,
  escrow_id                     text NOT NULL,
  idempotency_key               text NOT NULL UNIQUE,
  transition_kind               text NOT NULL CHECK (
    transition_kind IN ('reserve', 'release', 'refund', 'forfeit')
  ),
  work_request_id               text NOT NULL,
  requester_actor_ref           text NOT NULL,
  provider_actor_ref            text,
  amount_msat                   bigint NOT NULL CHECK (amount_msat > 0),
  receipt_ref                   text NOT NULL UNIQUE,
  evidence_ref                  text,
  state_after                   text NOT NULL CHECK (
    state_after IN ('reserved', 'released_to_provider', 'refunded', 'forfeited')
  ),
  forfeit_destination           text CHECK (
    forfeit_destination IS NULL OR forfeit_destination IN ('counterparty', 'burn')
  ),
  forfeit_destination_actor_ref text,
  public_projection_json        text NOT NULL,
  created_at                    text NOT NULL,
  UNIQUE (escrow_id, transition_kind)
);

CREATE INDEX IF NOT EXISTS idx_pg_labor_escrow_receipts_work_request
  ON labor_escrow_receipts (work_request_id, created_at DESC);

-- ---------------------------------------------------------------------------
-- Partner + site-referral payout ledgers (worker 0184/0214/0153).
-- Append-only reward ledgers keyed by idempotency_key with
-- period/state/payout_ref reads for feeds + receipts.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS partner_payout_ledger_entries (
  id                    text PRIMARY KEY,
  payout_ref            text NOT NULL,
  idempotency_key       text NOT NULL UNIQUE,
  partner_role          text NOT NULL CHECK (partner_role IN ('design_partner', 'referral', 'affiliate')),
  partner_user_id       text NOT NULL,
  partner_ref           text NOT NULL,
  beneficiary_user_id   text,
  asset                 text NOT NULL CHECK (asset IN ('usd', 'credits', 'sats')),
  qualifying_event_ref  text NOT NULL,
  qualifying_event_kind text NOT NULL,
  qualifying_amount     bigint NOT NULL DEFAULT 0 CHECK (qualifying_amount >= 0),
  amount                bigint NOT NULL,
  period_key            text NOT NULL,
  state                 text NOT NULL CHECK (
    state IN ('eligible', 'approved', 'dispatched', 'settled', 'failed', 'refused', 'reversed')
  ),
  state_reason_ref      text,
  previous_entry_id     text,
  reversal_of_entry_id  text,
  evidence_refs_json    text NOT NULL DEFAULT '[]',
  policy_refs_json      text NOT NULL DEFAULT '[]',
  caveat_refs_json      text NOT NULL DEFAULT '[]',
  created_at            text NOT NULL,
  archived_at           text
);

CREATE INDEX IF NOT EXISTS idx_pg_partner_payout_ledger_payout
  ON partner_payout_ledger_entries (payout_ref, created_at DESC)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_pg_partner_payout_ledger_partner_period
  ON partner_payout_ledger_entries (partner_user_id, period_key, state)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS partner_agreements (
  id              text PRIMARY KEY,
  agreement_ref   text NOT NULL UNIQUE,
  partner_ref     text NOT NULL,
  partner_user_id text NOT NULL,
  customer_user_id text NOT NULL,
  role            text NOT NULL CHECK (role IN ('design_partner', 'affiliate')),
  effective_from  text NOT NULL,
  effective_until text,
  policy_state    text NOT NULL DEFAULT 'active' CHECK (policy_state IN ('active', 'archived')),
  created_at      text NOT NULL,
  archived_at     text
);

CREATE INDEX IF NOT EXISTS idx_pg_partner_agreements_customer
  ON partner_agreements (customer_user_id, effective_from DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS site_referral_payout_ledger_entries (
  id                      text PRIMARY KEY,
  payout_ref              text NOT NULL,
  idempotency_key         text NOT NULL UNIQUE,
  referral_attribution_id text NOT NULL,
  referral_source_id      text NOT NULL,
  referral_invite_id      text,
  referrer_user_id        text NOT NULL,
  referred_user_id        text,
  qualifying_event_ref    text NOT NULL,
  qualifying_event_kind   text NOT NULL,
  qualifying_amount_sats  bigint NOT NULL DEFAULT 0 CHECK (qualifying_amount_sats >= 0),
  amount_sats             bigint NOT NULL,
  period_key              text NOT NULL,
  state                   text NOT NULL CHECK (
    state IN ('eligible', 'approved', 'dispatched', 'settled', 'failed', 'refused', 'reversed')
  ),
  state_reason_ref        text,
  previous_entry_id       text,
  reversal_of_entry_id    text,
  evidence_refs_json      text NOT NULL DEFAULT '[]',
  policy_refs_json        text NOT NULL DEFAULT '[]',
  caveat_refs_json        text NOT NULL DEFAULT '[]',
  created_at              text NOT NULL,
  archived_at             text
);

CREATE INDEX IF NOT EXISTS idx_pg_site_referral_payout_ledger_payout
  ON site_referral_payout_ledger_entries (payout_ref, created_at DESC)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_pg_site_referral_payout_ledger_referrer_period
  ON site_referral_payout_ledger_entries (referrer_user_id, period_key, state)
  WHERE archived_at IS NULL;

-- ---------------------------------------------------------------------------
-- Revenue event provenance (worker 0293). Insert-once evidence rows; reads
-- by recency and (product, demand, payment_state).
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS revenue_event_provenance (
  event_ref                 text PRIMARY KEY,
  evidence_bundle_ref       text NOT NULL UNIQUE,
  idempotency_key           text NOT NULL UNIQUE,
  product_ref               text NOT NULL CHECK (product_ref IN ('khala_code', 'qa_swarm')),
  revenue_surface_ref       text NOT NULL,
  receipt_ref               text NOT NULL,
  ledger_table              text NOT NULL,
  ledger_row_ref            text NOT NULL,
  demand_provenance         text NOT NULL CHECK (demand_provenance IN ('internal', 'external')),
  payment_state             text NOT NULL CHECK (
    payment_state IN ('requires_payment', 'payment_evidence_recorded', 'fulfilled', 'settled')
  ),
  amount_cents              bigint CHECK (amount_cents IS NULL OR amount_cents >= 0),
  amount_sats               bigint CHECK (amount_sats IS NULL OR amount_sats >= 0),
  public_evidence_refs_json text NOT NULL DEFAULT '[]',
  caveat_refs_json          text NOT NULL DEFAULT '[]',
  source_refs_json          text NOT NULL DEFAULT '[]',
  recorded_at               text NOT NULL,
  created_at                text NOT NULL,
  updated_at                text NOT NULL,
  UNIQUE (product_ref, receipt_ref),
  CHECK (amount_cents IS NOT NULL OR amount_sats IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_pg_revenue_event_provenance_product_demand
  ON revenue_event_provenance (product_ref, demand_provenance, payment_state, recorded_at);

-- ---------------------------------------------------------------------------
-- MPP replay guards (worker 0225/0224). PURE idempotency tables — the key
-- set must port EXACTLY (payment_hash / spt primary keys); point lookups
-- only, so no secondary indexes at all.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS mpp_lightning_replay (
  payment_hash text PRIMARY KEY,
  challenge_id text NOT NULL,
  consumed_at  text NOT NULL
);

CREATE TABLE IF NOT EXISTS mpp_spt_replay (
  spt               text PRIMARY KEY,
  challenge_id      text NOT NULL,
  payment_intent_id text,
  consumed_at       text NOT NULL
);
