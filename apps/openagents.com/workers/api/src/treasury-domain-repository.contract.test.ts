// KS-8.8 (#8319): Treasury domain repository CONTRACT suite.
//
// One behavioral spec, TWO real engines:
//   - D1: real SQLite (node:sqlite — the engine D1 is built on), schema
//     condensed from the live worker migrations (0101/0122/0128/0131/0143/
//     0146/0147/0149/0151/0153/0159..0167/0184/0196..0199/0203/0204/0206/
//     0211/0214/0224/0225/0261/0293) below.
//   - Postgres: a throwaway local Postgres (initdb/pg_ctl), schema from
//     khala-sync-server migration 0012. Skipped when no local Postgres
//     binaries exist.
//
// The load-bearing properties for the HIGHEST-STAKES domain:
//   1. REGISTRY FIDELITY: every one of the 27 money tables accepts a
//      registry-shaped row on BOTH engines, and `mirrorTreasuryRows`
//      converges the Postgres twin to be column-for-column equal to the
//      resolved D1 row — amounts, settlement states, idempotency keys, and
//      receipt refs port byte-exactly (the dual-write contract).
//   2. MIRROR IDEMPOTENCY: re-mirroring an unchanged row is a no-op — a
//      double-fired money cron leaves ONE identical row on both sides.
//   3. SETTLEMENT CONVERGENCE: after a D1 settlement transition
//      (treasury_transactions pending → settled with the settled amount),
//      the mirror converges the full row — no stale amount or state left
//      behind, and the mirror can never invent a settlement D1 does not
//      hold.
//   4. REPLAY-GUARD KEY EXACTNESS: the mpp_*_replay key sets port exactly
//      (a second consume collides on both engines), and their diagnostics
//      are REDACTED — payment identifiers never appear in log lines.
//   5. FAIL-SOFT: a Postgres outage never fails the D1 write path — it
//      logs `khala_sync_treasury_dual_write_failed` with row keys only.
//   6. READ EQUIVALENCE: the flag-routable public read (listRecent) decodes
//      identically from D1 and Postgres — the evidence that licenses the
//      compare/postgres read modes at the epic-gated cutover.

import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'

import { makeD1TreasuryTransactionStore } from './treasury-page-routes'
import {
  TREASURY_DOMAIN_TABLES,
  makePostgresTreasuryDomainStore,
  makeTreasuryDomainHandle,
  mirrorTreasuryRows,
  type PostgresTreasuryDomainStore,
  type TreasuryDomainDiagnostic,
  type TreasuryDomainDiagnosticEvent,
  type TreasuryDomainHandle,
  type TreasuryDomainRow,
  type TreasuryDomainTable,
} from './treasury-domain-store'
import { makeSqliteD1, type SqliteD1 } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// The D1 side of the schema (condensed from the live worker migrations,
// cross-table FOREIGN KEYs stripped: referenced content tables are outside
// this domain and integrity is verified by set-membership at reconciliation)
// ---------------------------------------------------------------------------

const TREASURY_DOMAIN_D1_SCHEMA = `
CREATE TABLE "treasury_transactions" (
  id TEXT PRIMARY KEY,
  direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
  amount_sat INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL CHECK (state IN ('pending', 'settled', 'expired', 'failed')),
  bolt11 TEXT,
  payment_ref TEXT,
  created_at TEXT NOT NULL,
  settled_at TEXT,
  expires_at TEXT
, failure_reason_ref TEXT, recipient_ref TEXT, redacted_destination_ref TEXT, owed_ref TEXT, owed_sat INTEGER, recipient_confirmation_state TEXT NOT NULL DEFAULT 'unconfirmed'
    CHECK (recipient_confirmation_state IN ('unconfirmed', 'confirmed_received')), recipient_confirmation_ref TEXT, recipient_confirmed_at TEXT);

CREATE TABLE nexus_payout_target_approvals (
  id TEXT PRIMARY KEY NOT NULL,
  approval_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  payout_target_ref TEXT NOT NULL,
  redacted_destination_ref TEXT NOT NULL,
  owner_user_id TEXT,
  agent_ref TEXT,
  pylon_ref TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('active', 'expired', 'rejected', 'revoked')
  ),
  approved_by_ref TEXT NOT NULL,
  approval_policy_ref TEXT NOT NULL,
  scope_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT,
  archived_at TEXT
);

CREATE TABLE nexus_treasury_payout_intents (
  id TEXT PRIMARY KEY NOT NULL,
  payout_intent_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT,
  source_kind TEXT NOT NULL CHECK (
    source_kind IN (
      'accepted_work',
      'forum_reward',
      'operator_test',
      'pylon_marketplace_assignment'
    )
  ),
  buyer_payment_ref TEXT,
  accepted_work_refs_json TEXT NOT NULL DEFAULT '[]',
  assignment_ref TEXT,
  artanis_dispatch_ref TEXT,
  pylon_job_ref TEXT,
  payout_target_ref TEXT NOT NULL,
  payout_target_approval_ref TEXT NOT NULL,
  adapter_kind TEXT NOT NULL CHECK (
    adapter_kind IN (
      'hosted_mdk',
      'legacy_nexus_import',
      'mdk_agent_wallet',
      'simulation',
      'spark_treasury'
    )
  ),
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination TEXT NOT NULL CHECK (
    amount_denomination IN ('bitcoin_millisatoshi', 'credit', 'usd_cent')
  ),
  amount_minor_units INTEGER NOT NULL CHECK (amount_minor_units >= 0),
  spend_cap_asset TEXT NOT NULL CHECK (spend_cap_asset IN ('bitcoin', 'credits', 'usd')),
  spend_cap_denomination TEXT NOT NULL CHECK (
    spend_cap_denomination IN ('bitcoin_millisatoshi', 'credit', 'usd_cent')
  ),
  spend_cap_amount_minor_units INTEGER NOT NULL CHECK (
    spend_cap_amount_minor_units >= 0
  ),
  policy_snapshot_ref TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN (
      'approved',
      'cancelled',
      'dispatched',
      'failed',
      'proposed',
      'rejected',
      'settled'
    )
  ),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT);

CREATE TABLE nexus_treasury_payout_attempts (
  id TEXT PRIMARY KEY NOT NULL,
  payout_attempt_ref TEXT NOT NULL UNIQUE,
  payout_intent_ref TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  adapter_kind TEXT NOT NULL CHECK (
    adapter_kind IN (
      'hosted_mdk',
      'legacy_nexus_import',
      'mdk_agent_wallet',
      'simulation',
      'spark_treasury'
    )
  ),
  adapter_attempt_ref TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN (
      'confirmed',
      'dispatched',
      'failed',
      'pending',
      'rejected',
      'replayed'
    )
  ),
  redacted_payment_ref TEXT,
  redacted_destination_ref TEXT NOT NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination TEXT NOT NULL CHECK (
    amount_denomination IN ('bitcoin_millisatoshi', 'credit', 'usd_cent')
  ),
  amount_minor_units INTEGER NOT NULL CHECK (amount_minor_units >= 0),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT);

CREATE TABLE nexus_treasury_payout_reconciliation_events (
  id TEXT PRIMARY KEY NOT NULL,
  event_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  provider_ref TEXT NOT NULL,
  external_event_ref TEXT NOT NULL,
  adapter_kind TEXT NOT NULL CHECK (
    adapter_kind IN (
      'hosted_mdk',
      'legacy_nexus_import',
      'mdk_agent_wallet',
      'simulation',
      'spark_treasury'
    )
  ),
  payout_intent_ref TEXT,
  payout_attempt_ref TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('matched', 'observed', 'rejected', 'replayed')
  ),
  result_ref TEXT NOT NULL,
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (provider_ref, external_event_ref));

CREATE TABLE nexus_payment_authority_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  payout_intent_ref TEXT NOT NULL,
  payout_attempt_ref TEXT,
  event_ref TEXT,
  receipt_kind TEXT NOT NULL CHECK (
    receipt_kind IN (
      'attempt_recorded',
      'confirmation_recorded',
      'dispatch_recorded',
      'intent_created',
      'pause_recorded',
      'policy_rejected',
      'settlement_recorded',
      'verification_recorded'
    )
  ),
  audience TEXT NOT NULL CHECK (
    audience IN ('agent', 'customer', 'operator', 'public')
  ),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT);

CREATE TABLE nexus_release_gates (
  id TEXT PRIMARY KEY NOT NULL,
  gate_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  gate_kind TEXT NOT NULL CHECK (
    gate_kind IN (
      'artanis_real_assignment',
      'artanis_simulated_assignment',
      'mdk_adapter',
      'operator_dashboard',
      'public_receipt',
      'pylon_api',
      'pylon_v02_release',
      'simulation_adapter'
    )
  ),
  status TEXT NOT NULL CHECK (
    status IN ('blocked', 'failed', 'passed', 'pending')
  ),
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE "forum_money_actions" (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  action_kind TEXT NOT NULL CHECK (
    action_kind IN (
      'topic_create_fee',
      'post_reply_fee',
      'post_reward',
      'post_boost',
      'topic_boost',
      'topic_fund',
      'post_down_signal',
      'report_fee',
      'orange_check'
    )
  ),
  target_forum_id TEXT,
  target_topic_id TEXT,
  target_post_id TEXT,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('credits', 'sats', 'usd')),
  amount_value INTEGER NOT NULL CHECK (amount_value >= 0),
  payment_event_id TEXT,
  receipt_id TEXT,
  earning_actor_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_payment_events (
  id TEXT PRIMARY KEY NOT NULL,
  money_action_id TEXT,
  provider_ref TEXT NOT NULL,
  external_ref TEXT NOT NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('credits', 'sats', 'usd')),
  amount_value INTEGER NOT NULL CHECK (amount_value >= 0),
  redacted_evidence_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (provider_ref, external_ref)
);

CREATE TABLE forum_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  action_kind TEXT NOT NULL,
  target_forum_id TEXT,
  target_topic_id TEXT,
  target_post_id TEXT,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('credits', 'sats', 'usd')),
  amount_value INTEGER NOT NULL CHECK (amount_value >= 0),
  recipient_actor_ref TEXT,
  redacted_payment_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_l402_challenges (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  action_kind TEXT NOT NULL,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  route_params_json TEXT NOT NULL DEFAULT '{}',
  request_body_digest TEXT NOT NULL,
  target_forum_id TEXT,
  target_topic_id TEXT,
  target_post_id TEXT,
  price_asset TEXT NOT NULL CHECK (price_asset IN ('credits', 'sats', 'usd')),
  price_value INTEGER NOT NULL CHECK (price_value >= 0),
  spend_cap_asset TEXT NOT NULL CHECK (spend_cap_asset IN ('credits', 'sats', 'usd')),
  spend_cap_value INTEGER NOT NULL CHECK (spend_cap_value >= 0),
  expires_at TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
, recipient_actor_ref TEXT, recipient_readiness_ref TEXT, mdk_provider_ref TEXT, mdk_environment TEXT CHECK (
    mdk_environment IS NULL OR mdk_environment IN ('production', 'sandbox')
  ), mdk_sandbox INTEGER CHECK (
    mdk_sandbox IS NULL OR mdk_sandbox IN (0, 1)
  ), mdk_implementation_state TEXT CHECK (
    mdk_implementation_state IS NULL OR mdk_implementation_state IN (
      'fake_provider_contract',
      'live_provider_configured',
      'missing_configuration'
    )
  ), mdk_checkout_ref TEXT, mdk_checkout_url_ref TEXT, mdk_checkout_launch_path TEXT, mdk_invoice_ref TEXT, mdk_payment_hash_ref TEXT, l402_credential_ref TEXT, l402_replay_nonce_ref TEXT, l402_endpoint_ref TEXT, l402_entitlement_scope_refs_json TEXT, l402_www_authenticate TEXT);

CREATE TABLE forum_l402_redemptions (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  challenge_id TEXT NOT NULL,
  actor_ref TEXT NOT NULL,
  proof_ref TEXT NOT NULL,
  entitlement_ref TEXT NOT NULL,
  receipt_id TEXT,
  replayed INTEGER NOT NULL DEFAULT 0 CHECK (replayed IN (0, 1)),
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (challenge_id)
);

CREATE TABLE forum_direct_tip_attempts (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  payer_actor_ref TEXT NOT NULL,
  recipient_actor_ref TEXT NOT NULL,
  target_topic_id TEXT NOT NULL,
  target_post_id TEXT NOT NULL,
  target_post_permalink TEXT,
  amount_sats INTEGER NOT NULL CHECK (amount_sats > 0),
  provider_ref TEXT NOT NULL,
  external_ref TEXT NOT NULL,
  redacted_evidence_ref TEXT NOT NULL,
  payment_mode TEXT NOT NULL CHECK (payment_mode IN ('live', 'sandbox', 'signet', 'unknown')),
  payment_event_status TEXT NOT NULL CHECK (
    payment_event_status IN (
      'confirmed',
      'failed',
      'observed',
      'refunded',
      'replayed',
      'reversed'
    )
  ),
  status TEXT NOT NULL CHECK (status IN ('settled', 'failed', 'recovery_pending')),
  receipt_ref TEXT,
  payment_event_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (provider_ref, external_ref)
);

CREATE TABLE forum_direct_tip_webhook_events (
  id TEXT PRIMARY KEY,
  provider_event_ref TEXT NOT NULL UNIQUE,
  direct_tip_attempt_id TEXT NOT NULL,
  provider_ref TEXT NOT NULL,
  external_ref TEXT NOT NULL,
  amount_sats INTEGER NOT NULL CHECK (amount_sats > 0),
  payment_event_status TEXT NOT NULL CHECK (
    payment_event_status IN ('confirmed', 'failed', 'observed', 'refunded', 'replayed', 'reversed')
  ),
  redacted_evidence_ref TEXT NOT NULL,
  event_body_digest_ref TEXT NOT NULL,
  signature_binding_ref TEXT NOT NULL,
  reconciliation_status TEXT NOT NULL CHECK (
    reconciliation_status IN ('settled', 'failed', 'recovery_pending')
  ),
  reconciliation_result TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  delivery_count INTEGER NOT NULL DEFAULT 1 CHECK (delivery_count > 0),
  archived_at TEXT
);

CREATE TABLE forum_tip_recipient_wallets (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL UNIQUE,
  provider_class TEXT NOT NULL CHECK (
    provider_class IN ('mdk_agent_wallet', 'hosted_mdk', 'external_lightning')
  ),
  wallet_ref TEXT NOT NULL,
  receive_capability_ref TEXT NOT NULL,
  payout_target_approval_ref TEXT,
  readiness_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  custody_policy_refs_json TEXT NOT NULL DEFAULT '[]',
  claim_policy_refs_json TEXT NOT NULL DEFAULT '[]',
  source_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('ready', 'disabled', 'blocked')),
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  disabled_at TEXT,
  archived_at TEXT
, bolt12_offer TEXT, lightning_address TEXT, spark_address TEXT);

CREATE TABLE forum_tip_settlement_claims (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  receipt_id TEXT NOT NULL,
  receipt_ref TEXT NOT NULL,
  recipient_actor_ref TEXT NOT NULL,
  settlement_ref TEXT NOT NULL,
  settlement_evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  source_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (receipt_id)
);

CREATE TABLE x_claim_reward_ledger (
  id TEXT PRIMARY KEY,
  challenge_id TEXT NOT NULL UNIQUE,
  claim_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  agent_user_id TEXT,
  x_account_ref TEXT NOT NULL UNIQUE,
  amount_sats INTEGER NOT NULL DEFAULT 1000,
  state TEXT NOT NULL CHECK (
    state IN (
      'eligible',
      'dispatch_requested',
      'dispatched',
      'settled',
      'failed',
      'refused'
    )
  ),
  state_reason_ref TEXT,
  receipt_ref TEXT NOT NULL UNIQUE,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
, treasury_payment_id TEXT);

CREATE TABLE agent_claim_reward_ledger (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_ref TEXT NOT NULL,
  agent_claim_ref TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  x_account_ref TEXT NOT NULL,
  tweet_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'pending',
      'verified',
      'approved',
      'payout_intent_created',
      'dispatched',
      'settled',
      'rejected',
      'reversed',
      'expired'
    )
  ),
  amount_sats INTEGER NOT NULL CHECK (amount_sats = 1000),
  destination_kind TEXT NOT NULL CHECK (
    destination_kind IN (
      'lightning_address',
      'lnurl',
      'bolt12',
      'bolt11_invoice',
      'unknown'
    )
  ),
  redacted_destination_ref TEXT,
  payout_intent_ref TEXT,
  dispatch_attempt_ref TEXT,
  settlement_ref TEXT,
  rejection_reason TEXT,
  policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
, held_msat INTEGER NOT NULL DEFAULT 0 CHECK (held_msat >= 0), usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0));

CREATE TABLE "labor_escrows" (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  work_request_id TEXT NOT NULL UNIQUE,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  state TEXT NOT NULL CHECK (
    state IN ('reserved', 'released_to_provider', 'refunded', 'forfeited')
  ),
  funding_source TEXT NOT NULL DEFAULT 'ledger_balance' CHECK (
    funding_source IN ('ledger_balance', 'external_invoice_pending')
  ),
  job_event_id TEXT NOT NULL,
  acceptance_event_ref TEXT,
  reserve_receipt_ref TEXT NOT NULL UNIQUE,
  release_receipt_ref TEXT UNIQUE,
  refund_receipt_ref TEXT UNIQUE,
  forfeit_receipt_ref TEXT UNIQUE,
  forfeit_destination TEXT CHECK (
    forfeit_destination IS NULL OR forfeit_destination IN ('counterparty', 'burn')
  ),
  forfeit_destination_actor_ref TEXT,
  forfeit_condition_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  released_at TEXT,
  refunded_at TEXT,
  forfeited_at TEXT,
  archived_at TEXT
);

CREATE TABLE "labor_escrow_receipts" (
  id TEXT PRIMARY KEY NOT NULL,
  escrow_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  transition_kind TEXT NOT NULL CHECK (
    transition_kind IN ('reserve', 'release', 'refund', 'forfeit')
  ),
  work_request_id TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  receipt_ref TEXT NOT NULL UNIQUE,
  evidence_ref TEXT,
  state_after TEXT NOT NULL CHECK (
    state_after IN ('reserved', 'released_to_provider', 'refunded', 'forfeited')
  ),
  forfeit_destination TEXT CHECK (
    forfeit_destination IS NULL OR forfeit_destination IN ('counterparty', 'burn')
  ),
  forfeit_destination_actor_ref TEXT,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE partner_payout_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  payout_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  partner_role TEXT NOT NULL CHECK (
    partner_role IN (
      'design_partner',
      'referral',
      'affiliate'
    )
  ),
  partner_user_id TEXT NOT NULL,
  partner_ref TEXT NOT NULL,
  beneficiary_user_id TEXT,
  asset TEXT NOT NULL CHECK (
    asset IN (
      'usd',
      'credits',
      'sats'
    )
  ),
  qualifying_event_ref TEXT NOT NULL,
  qualifying_event_kind TEXT NOT NULL,
  qualifying_amount INTEGER NOT NULL DEFAULT 0 CHECK (qualifying_amount >= 0),
  amount INTEGER NOT NULL,
  period_key TEXT NOT NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'eligible',
      'approved',
      'dispatched',
      'settled',
      'failed',
      'refused',
      'reversed'
    )
  ),
  state_reason_ref TEXT,
  previous_entry_id TEXT,
  reversal_of_entry_id TEXT,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE partner_agreements (
  id TEXT PRIMARY KEY NOT NULL,
  agreement_ref TEXT NOT NULL UNIQUE,
  partner_ref TEXT NOT NULL,
  partner_user_id TEXT NOT NULL,
  customer_user_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (
    role IN (
      'design_partner',
      'affiliate'
    )
  ),
  effective_from TEXT NOT NULL,
  effective_until TEXT,
  policy_state TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN (
      'active',
      'archived'
    )
  ),
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE site_referral_payout_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  payout_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  referral_attribution_id TEXT NOT NULL,
  referral_source_id TEXT NOT NULL,
  referral_invite_id TEXT,
  referrer_user_id TEXT NOT NULL,
  referred_user_id TEXT,
  qualifying_event_ref TEXT NOT NULL,
  qualifying_event_kind TEXT NOT NULL,
  qualifying_amount_sats INTEGER NOT NULL DEFAULT 0 CHECK (qualifying_amount_sats >= 0),
  amount_sats INTEGER NOT NULL,
  period_key TEXT NOT NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'eligible',
      'approved',
      'dispatched',
      'settled',
      'failed',
      'refused',
      'reversed'
    )
  ),
  state_reason_ref TEXT,
  previous_entry_id TEXT,
  reversal_of_entry_id TEXT,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE revenue_event_provenance (
  event_ref TEXT PRIMARY KEY,
  evidence_bundle_ref TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  product_ref TEXT NOT NULL CHECK (product_ref IN ('khala_code', 'qa_swarm')),
  revenue_surface_ref TEXT NOT NULL,
  receipt_ref TEXT NOT NULL,
  ledger_table TEXT NOT NULL CHECK (
    ledger_table IN (
      'khala_code_paid_plan_payment_intents',
      'qa_swarm_first_engagements'
    )
  ),
  ledger_row_ref TEXT NOT NULL,
  demand_provenance TEXT NOT NULL CHECK (demand_provenance IN ('internal', 'external')),
  payment_state TEXT NOT NULL CHECK (
    payment_state IN (
      'requires_payment',
      'payment_evidence_recorded',
      'fulfilled',
      'settled'
    )
  ),
  amount_cents INTEGER CHECK (amount_cents IS NULL OR amount_cents >= 0),
  amount_sats INTEGER CHECK (amount_sats IS NULL OR amount_sats >= 0),
  public_evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  recorded_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (amount_cents IS NOT NULL OR amount_sats IS NOT NULL)
);

CREATE TABLE mpp_lightning_replay (
  -- The BOLT11 payment hash (sha256 of the preimage), lowercase hex. PRIMARY KEY
  -- so a second use of the same paid invoice collides and is refused.
  payment_hash TEXT PRIMARY KEY,
  -- The challenge id the payment was consumed under (binds the proof to the
  -- exact quote, for audit).
  challenge_id TEXT NOT NULL,
  consumed_at TEXT NOT NULL
);

CREATE TABLE mpp_spt_replay (
  -- The Shared Payment Token id (starts with \`spt_\`).
  spt TEXT PRIMARY KEY,
  -- The challenge id the SPT was consumed under (binds the proof to the quote).
  challenge_id TEXT NOT NULL,
  -- The resulting Stripe PaymentIntent id, for dereference.
  payment_intent_id TEXT,
  consumed_at TEXT NOT NULL
);
`

// ---------------------------------------------------------------------------
// Fixtures — one registry-shaped sample row per table (CHECK-satisfying)
// ---------------------------------------------------------------------------

const ISO = '2026-07-04T12:00:00.000Z'
const ALL_TABLES = Object.keys(
  TREASURY_DOMAIN_TABLES,
) as ReadonlyArray<TreasuryDomainTable>

const sampleRow = (table: TreasuryDomainTable): TreasuryDomainRow => {
  switch (table) {
    case 'treasury_transactions':
      return {
        amount_sat: 21_000,
        bolt11: null,
        created_at: ISO,
        direction: 'out',
        expires_at: null,
        failure_reason_ref: null,
        id: 'treasury-tx-contract-1',
        owed_ref: null,
        owed_sat: null,
        payment_ref: 'payment-contract-1',
        recipient_confirmation_ref: null,
        recipient_confirmation_state: 'unconfirmed',
        recipient_confirmed_at: null,
        recipient_ref: 'actor.forum.recipient',
        redacted_destination_ref: null,
        settled_at: null,
        state: 'pending',
      }
    case 'nexus_payout_target_approvals':
      return {
        agent_ref: null,
        approval_policy_ref: 'policy.payout_target.v1',
        approval_ref: 'approval-contract-1',
        approved_by_ref: 'operator.owner',
        archived_at: null,
        created_at: ISO,
        expires_at: null,
        id: 'approval-id-contract-1',
        idempotency_key_hash: 'approval-idem-contract-1',
        owner_user_id: null,
        payout_target_ref: 'target-contract-1',
        public_projection_json: '{}',
        pylon_ref: null,
        redacted_destination_ref: 'redacted:target-contract-1',
        scope_refs_json: '[]',
        status: 'active',
        updated_at: ISO,
      }
    case 'nexus_treasury_payout_intents':
      return {
        accepted_work_refs_json: '[]',
        actor_ref: 'actor.pylon.worker',
        adapter_kind: 'spark_treasury',
        amount_asset: 'bitcoin',
        amount_denomination: 'bitcoin_millisatoshi',
        amount_minor_units: 21_000_000,
        archived_at: null,
        artanis_dispatch_ref: null,
        assignment_ref: 'assignment-contract-1',
        buyer_payment_ref: null,
        created_at: ISO,
        id: 'intent-id-contract-1',
        idempotency_key_hash: 'intent-idem-contract-1',
        metadata_refs_json: '[]',
        owner_user_id: null,
        payout_intent_ref: 'intent-contract-1',
        payout_target_approval_ref: 'approval-contract-1',
        payout_target_ref: 'target-contract-1',
        policy_snapshot_ref: 'policy.snapshot.v1',
        public_projection_json: '{}',
        pylon_job_ref: null,
        source_kind: 'accepted_work',
        spend_cap_amount_minor_units: 42_000_000,
        spend_cap_asset: 'bitcoin',
        spend_cap_denomination: 'bitcoin_millisatoshi',
        status: 'approved',
        updated_at: ISO,
      }
    case 'nexus_treasury_payout_attempts':
      return {
        adapter_attempt_ref: 'adapter-attempt-contract-1',
        adapter_kind: 'spark_treasury',
        amount_asset: 'bitcoin',
        amount_denomination: 'bitcoin_millisatoshi',
        amount_minor_units: 21_000_000,
        archived_at: null,
        created_at: ISO,
        id: 'attempt-id-contract-1',
        idempotency_key_hash: 'attempt-idem-contract-1',
        metadata_refs_json: '[]',
        payout_attempt_ref: 'attempt-contract-1',
        payout_intent_ref: 'intent-contract-1',
        public_projection_json: '{}',
        redacted_destination_ref: 'redacted:target-contract-1',
        redacted_payment_ref: null,
        status: 'pending',
        updated_at: ISO,
      }
    case 'nexus_treasury_payout_reconciliation_events':
      return {
        adapter_kind: 'spark_treasury',
        archived_at: null,
        created_at: ISO,
        event_ref: 'recon-contract-1',
        external_event_ref: 'external-contract-1',
        id: 'recon-id-contract-1',
        idempotency_key_hash: 'recon-idem-contract-1',
        metadata_refs_json: '[]',
        payout_attempt_ref: 'attempt-contract-1',
        payout_intent_ref: 'intent-contract-1',
        provider_ref: 'provider.spark',
        public_projection_json: '{}',
        result_ref: 'result-contract-1',
        status: 'matched',
      }
    case 'nexus_payment_authority_receipts':
      return {
        archived_at: null,
        audience: 'public',
        created_at: ISO,
        event_ref: null,
        id: 'receipt-id-contract-1',
        metadata_refs_json: '[]',
        payout_attempt_ref: null,
        payout_intent_ref: 'intent-contract-1',
        public_projection_json: '{}',
        receipt_kind: 'intent_created',
        receipt_ref: 'nexus-receipt-contract-1',
      }
    case 'nexus_release_gates':
      return {
        archived_at: null,
        blocker_refs_json: '[]',
        created_at: ISO,
        evidence_refs_json: '[]',
        gate_kind: 'public_receipt',
        gate_ref: 'gate-contract-1',
        id: 'gate-id-contract-1',
        idempotency_key_hash: 'gate-idem-contract-1',
        public_projection_json: '{}',
        status: 'passed',
        updated_at: ISO,
      }
    case 'forum_money_actions':
      return {
        action_kind: 'post_reward',
        actor_ref: 'actor.forum.payer',
        amount_asset: 'sats',
        amount_value: 210,
        archived_at: null,
        created_at: ISO,
        earning_actor_ref: 'actor.forum.recipient',
        id: 'money-action-contract-1',
        idempotency_key: 'money-action-idem-contract-1',
        payment_event_id: 'payment-event-contract-1',
        public_projection_json: '{}',
        receipt_id: 'forum-receipt-contract-1',
        target_forum_id: null,
        target_post_id: 'post-contract-1',
        target_topic_id: 'topic-contract-1',
      }
    case 'forum_payment_events':
      return {
        amount_asset: 'sats',
        amount_value: 210,
        archived_at: null,
        created_at: ISO,
        external_ref: 'external-contract-1',
        id: 'payment-event-contract-1',
        money_action_id: 'money-action-contract-1',
        provider_ref: 'provider.spark',
        public_projection_json: '{}',
        redacted_evidence_ref: 'redacted:evidence-contract-1',
      }
    case 'forum_receipts':
      return {
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: 210,
        archived_at: null,
        created_at: ISO,
        id: 'forum-receipt-contract-1',
        public_projection_json: '{}',
        receipt_ref: 'receipt.forum.contract.1',
        recipient_actor_ref: 'actor.forum.recipient',
        redacted_payment_ref: 'redacted:payment-contract-1',
        target_forum_id: null,
        target_post_id: 'post-contract-1',
        target_topic_id: 'topic-contract-1',
      }
    case 'forum_l402_challenges':
      return {
        action_kind: 'post_reward',
        actor_ref: 'actor.forum.payer',
        archived_at: null,
        created_at: ISO,
        expires_at: '2026-07-04T13:00:00.000Z',
        id: 'challenge-contract-1',
        idempotency_key: 'challenge-idem-contract-1',
        l402_credential_ref: null,
        l402_endpoint_ref: null,
        l402_entitlement_scope_refs_json: null,
        l402_replay_nonce_ref: null,
        l402_www_authenticate: null,
        mdk_checkout_launch_path: null,
        mdk_checkout_ref: null,
        mdk_checkout_url_ref: null,
        mdk_environment: null,
        mdk_implementation_state: null,
        mdk_invoice_ref: null,
        mdk_payment_hash_ref: null,
        mdk_provider_ref: null,
        mdk_sandbox: null,
        method: 'POST',
        path: '/api/forum/posts/post-contract-1/reward',
        price_asset: 'sats',
        price_value: 210,
        public_projection_json: '{}',
        recipient_actor_ref: null,
        recipient_readiness_ref: null,
        request_body_digest: 'digest-contract-1',
        route_params_json: '{}',
        spend_cap_asset: 'sats',
        spend_cap_value: 1000,
        target_forum_id: null,
        target_post_id: 'post-contract-1',
        target_topic_id: 'topic-contract-1',
      }
    case 'forum_l402_redemptions':
      return {
        actor_ref: 'actor.forum.payer',
        archived_at: null,
        challenge_id: 'challenge-contract-1',
        created_at: ISO,
        entitlement_ref: 'entitlement-contract-1',
        id: 'redemption-contract-1',
        idempotency_key: 'redemption-idem-contract-1',
        proof_ref: 'proof-contract-1',
        public_projection_json: '{}',
        receipt_id: 'forum-receipt-contract-1',
        replayed: 0,
      }
    case 'forum_direct_tip_attempts':
      return {
        amount_sats: 21,
        archived_at: null,
        created_at: ISO,
        external_ref: 'tip-external-contract-1',
        id: 'tip-attempt-contract-1',
        idempotency_key: 'tip-attempt-idem-contract-1',
        payer_actor_ref: 'actor.forum.payer',
        payment_event_id: null,
        payment_event_status: 'confirmed',
        payment_mode: 'live',
        provider_ref: 'provider.spark',
        receipt_ref: 'receipt.forum.contract.1',
        recipient_actor_ref: 'actor.forum.recipient',
        redacted_evidence_ref: 'redacted:tip-contract-1',
        status: 'settled',
        target_post_id: 'post-contract-1',
        target_post_permalink: null,
        target_topic_id: 'topic-contract-1',
        updated_at: ISO,
      }
    case 'forum_direct_tip_webhook_events':
      return {
        amount_sats: 21,
        archived_at: null,
        delivery_count: 1,
        direct_tip_attempt_id: 'tip-attempt-contract-1',
        event_body_digest_ref: 'digest-contract-1',
        external_ref: 'tip-external-contract-1',
        first_seen_at: ISO,
        id: 'webhook-event-contract-1',
        last_seen_at: ISO,
        payment_event_status: 'confirmed',
        provider_event_ref: 'provider-event-contract-1',
        provider_ref: 'provider.spark',
        reconciliation_result: 'settled_existing_attempt',
        reconciliation_status: 'settled',
        redacted_evidence_ref: 'redacted:webhook-contract-1',
        signature_binding_ref: 'signature-contract-1',
      }
    case 'forum_tip_recipient_wallets':
      return {
        actor_ref: 'actor.forum.wallet-owner-contract',
        archived_at: null,
        bolt12_offer: 'lno1contractsample',
        caveat_refs_json: '[]',
        claim_policy_refs_json: '[]',
        created_at: ISO,
        custody_policy_refs_json: '[]',
        disabled_at: null,
        id: 'wallet-contract-1',
        lightning_address: null,
        payout_target_approval_ref: null,
        provider_class: 'hosted_mdk',
        public_projection_json: '{}',
        readiness_refs_json: '[]',
        receive_capability_ref: 'capability-contract-1',
        source_ref: 'source.wallet_registration',
        spark_address: null,
        state: 'ready',
        updated_at: ISO,
        wallet_ref: 'wallet-ref-contract-1',
      }
    case 'forum_tip_settlement_claims':
      return {
        archived_at: null,
        created_at: ISO,
        id: 'claim-contract-1',
        idempotency_key: 'claim-idem-contract-1',
        public_projection_json: '{}',
        receipt_id: 'forum-receipt-contract-1',
        receipt_ref: 'receipt.forum.contract.1',
        recipient_actor_ref: 'actor.forum.recipient',
        settlement_evidence_refs_json: '[]',
        settlement_ref: 'settlement-contract-1',
        source_ref: 'source.tip_claim',
      }
    case 'x_claim_reward_ledger':
      return {
        agent_user_id: null,
        amount_sats: 1000,
        challenge_id: 'x-challenge-contract-1',
        claim_id: 'x-claim-contract-1',
        created_at: ISO,
        evidence_refs_json: '[]',
        id: 'x-reward-contract-1',
        owner_user_id: 'owner-contract-1',
        receipt_ref: 'receipt.x_claim.contract.1',
        state: 'dispatch_requested',
        state_reason_ref: null,
        treasury_payment_id: null,
        updated_at: ISO,
        x_account_ref: 'x.account.contract.1',
      }
    case 'agent_claim_reward_ledger':
      return {
        agent_claim_ref: 'agent-claim-contract-1',
        amount_sats: 1000,
        campaign_ref: 'campaign.agent_claim.v1',
        caveat_refs_json: '[]',
        created_at: ISO,
        destination_kind: 'lightning_address',
        dispatch_attempt_ref: null,
        id: 'agent-reward-contract-1',
        idempotency_key: 'agent-reward-idem-contract-1',
        owner_ref: 'owner-contract-1',
        payout_intent_ref: null,
        policy_refs_json: '[]',
        redacted_destination_ref: null,
        rejection_reason: null,
        settlement_ref: null,
        state: 'pending',
        tweet_ref: 'tweet-contract-1',
        updated_at: ISO,
        x_account_ref: 'x.account.contract.1',
      }
    case 'agent_balances':
      return {
        actor_ref: 'actor.balance-contract-1',
        balance_msat: 21_000_000,
        created_at: ISO,
        held_msat: 1_000_000,
        receive_credits_below_sat: 10,
        send_credits_below_sat: 10,
        sweep_enabled: 1,
        sweep_threshold_sat: 210,
        updated_at: ISO,
        usd_credit_msat: 0,
      }
    case 'labor_escrows':
      return {
        acceptance_event_ref: null,
        amount_msat: 5_000_000,
        archived_at: null,
        created_at: ISO,
        forfeit_condition_ref: null,
        forfeit_destination: null,
        forfeit_destination_actor_ref: null,
        forfeit_receipt_ref: null,
        forfeited_at: null,
        funding_source: 'ledger_balance',
        id: 'escrow-contract-1',
        idempotency_key: 'escrow-idem-contract-1',
        job_event_id: 'job-event-contract-1',
        provider_actor_ref: null,
        public_projection_json: '{}',
        refund_receipt_ref: null,
        refunded_at: null,
        release_receipt_ref: null,
        released_at: null,
        requester_actor_ref: 'actor.forum.requester',
        reserve_receipt_ref: 'receipt.escrow.reserve.contract.1',
        state: 'reserved',
        updated_at: ISO,
        work_request_id: 'work-request-contract-1',
      }
    case 'labor_escrow_receipts':
      return {
        amount_msat: 5_000_000,
        created_at: ISO,
        escrow_id: 'escrow-contract-1',
        evidence_ref: null,
        forfeit_destination: null,
        forfeit_destination_actor_ref: null,
        id: 'escrow-receipt-contract-1',
        idempotency_key: 'escrow-receipt-idem-contract-1',
        provider_actor_ref: null,
        public_projection_json: '{}',
        receipt_ref: 'receipt.escrow.reserve.contract.1',
        requester_actor_ref: 'actor.forum.requester',
        state_after: 'reserved',
        transition_kind: 'reserve',
        work_request_id: 'work-request-contract-1',
      }
    case 'partner_payout_ledger_entries':
      return {
        amount: 2500,
        archived_at: null,
        asset: 'usd',
        beneficiary_user_id: null,
        caveat_refs_json: '[]',
        created_at: ISO,
        evidence_refs_json: '[]',
        id: 'partner-entry-contract-1',
        idempotency_key: 'partner-entry-idem-contract-1',
        partner_ref: 'partner.design.contract',
        partner_role: 'design_partner',
        partner_user_id: 'partner-user-contract',
        payout_ref: 'partner-payout-contract-1',
        period_key: '2026-07',
        policy_refs_json: '[]',
        previous_entry_id: null,
        qualifying_amount: 10_000,
        qualifying_event_kind: 'subscription_payment',
        qualifying_event_ref: 'qualifying-contract-1',
        reversal_of_entry_id: null,
        state: 'eligible',
        state_reason_ref: null,
      }
    case 'partner_agreements':
      return {
        agreement_ref: 'agreement-contract-1',
        archived_at: null,
        created_at: ISO,
        customer_user_id: 'customer-contract-1',
        effective_from: ISO,
        effective_until: null,
        id: 'agreement-id-contract-1',
        partner_ref: 'partner.design.contract',
        partner_user_id: 'partner-user-contract',
        policy_state: 'active',
        role: 'design_partner',
      }
    case 'site_referral_payout_ledger_entries':
      return {
        amount_sats: 210,
        archived_at: null,
        caveat_refs_json: '[]',
        created_at: ISO,
        evidence_refs_json: '[]',
        id: 'referral-entry-contract-1',
        idempotency_key: 'referral-entry-idem-contract-1',
        payout_ref: 'referral-payout-contract-1',
        period_key: '2026-07',
        policy_refs_json: '[]',
        previous_entry_id: null,
        qualifying_amount_sats: 1000,
        qualifying_event_kind: 'site_payment',
        qualifying_event_ref: 'qualifying-contract-1',
        referral_attribution_id: 'attribution-contract-1',
        referral_invite_id: null,
        referral_source_id: 'source-contract-1',
        referred_user_id: null,
        referrer_user_id: 'referrer-contract-1',
        reversal_of_entry_id: null,
        state: 'eligible',
        state_reason_ref: null,
      }
    case 'revenue_event_provenance':
      return {
        amount_cents: 2000,
        amount_sats: null,
        caveat_refs_json: '[]',
        created_at: ISO,
        demand_provenance: 'external',
        event_ref: 'revenue-event-contract-1',
        evidence_bundle_ref: 'evidence-bundle-contract-1',
        idempotency_key: 'revenue-idem-contract-1',
        ledger_row_ref: 'ledger-row-contract-1',
        ledger_table: 'khala_code_paid_plan_payment_intents',
        payment_state: 'settled',
        product_ref: 'khala_code',
        public_evidence_refs_json: '[]',
        receipt_ref: 'revenue-receipt-contract-1',
        recorded_at: ISO,
        revenue_surface_ref: 'khala_code.paid_plan',
        source_refs_json: '[]',
        updated_at: ISO,
      }
    case 'mpp_lightning_replay':
      return {
        challenge_id: 'mpp-challenge-contract-1',
        consumed_at: ISO,
        payment_hash: `${'ab'.repeat(32)}`,
      }
    case 'mpp_spt_replay':
      return {
        challenge_id: 'mpp-challenge-contract-1',
        consumed_at: ISO,
        payment_intent_id: null,
        spt: 'spt_contract_sample_1',
      }
  }
}

/** Normalize a row to the registry column set for cross-engine equality. */
const canonical = (
  table: TreasuryDomainTable,
  row: TreasuryDomainRow,
): Record<string, string | null> =>
  Object.fromEntries(
    TREASURY_DOMAIN_TABLES[table].columns.map(column => {
      const value = row[column]
      return [
        column,
        value === null || value === undefined ? null : String(value),
      ]
    }),
  )

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

type PgClient = {
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    text: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}

const MIGRATION_0012 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0016_treasury_domain.sql',
)

type LoggedDiagnostic = readonly [
  TreasuryDomainDiagnosticEvent,
  TreasuryDomainDiagnostic,
]

describe.skipIf(!hasLocalPostgres())(
  'treasury domain repository contract — D1 authority + Postgres mirror',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: SqliteD1
    let postgresStore: PostgresTreasuryDomainStore
    let handle: TreasuryDomainHandle
    let diagnostics: Array<LoggedDiagnostic>

    const pgRow = async (
      table: TreasuryDomainTable,
      keyColumn: string,
      key: string | number,
    ): Promise<TreasuryDomainRow | undefined> => {
      const rows = await client!.unsafe(
        `SELECT * FROM ${table} WHERE ${keyColumn} = $1`,
        [key],
      )
      return rows[0]
    }

    const d1Row = async (
      table: TreasuryDomainTable,
      keyColumn: string,
      key: string | number,
    ): Promise<TreasuryDomainRow | undefined> => {
      const row = await sqlite.db
        .prepare(`SELECT * FROM ${table} WHERE ${keyColumn} = ?`)
        .bind(key)
        .first<TreasuryDomainRow>()
      return row ?? undefined
    }

    const expectConverged = async (
      table: TreasuryDomainTable,
      keyColumn: string,
      key: string | number,
    ): Promise<void> => {
      const d1 = await d1Row(table, keyColumn, key)
      const postgres = await pgRow(table, keyColumn, key)
      expect(d1, `${table} D1 row ${String(key)}`).toBeDefined()
      expect(postgres, `${table} Postgres row ${String(key)}`).toBeDefined()
      expect(canonical(table, postgres!)).toEqual(canonical(table, d1!))
    }

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE treasury_domain_contract')
      await admin.end({ timeout: 5 })

      const raw = postgres(pg.urlFor('treasury_domain_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await client.unsafe(readFileSync(MIGRATION_0012, 'utf8'))

      sqlite = makeSqliteD1()
      sqlite.exec(TREASURY_DOMAIN_D1_SCHEMA)

      postgresStore = makePostgresTreasuryDomainStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: client as never,
          }),
      })
      diagnostics = []
      handle = makeTreasuryDomainHandle({
        d1: sqlite.db,
        flags: { dualWrite: true, reads: 'd1' },
        log: (event, fields) => diagnostics.push([event, fields]),
        postgres: postgresStore,
        wait: () => Promise.resolve(),
      })
    }, 120_000)

    afterAll(async () => {
      sqlite?.close()
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    test('registry fidelity: all 27 money tables mirror a D1 row into an equal Postgres twin', async () => {
      for (const table of ALL_TABLES) {
        const spec = TREASURY_DOMAIN_TABLES[table]
        const row = sampleRow(table)
        const columns = spec.columns
        await sqlite.db
          .prepare(
            `INSERT INTO ${table} (${columns.join(', ')}) VALUES (${columns
              .map(() => '?')
              .join(', ')})`,
          )
          .bind(...columns.map(column => row[column] ?? null))
          .run()

        const key = row[spec.conflictKey] as string | number
        await mirrorTreasuryRows(handle, table, spec.conflictKey, [key])
        await expectConverged(table, spec.conflictKey, key)
      }
      expect(diagnostics).toEqual([])
    })

    test('mirror idempotency: a double-fired mirror is a no-op on both engines', async () => {
      const spec = TREASURY_DOMAIN_TABLES.forum_receipts
      const key = 'forum-receipt-contract-1'
      const before = await pgRow('forum_receipts', spec.conflictKey, key)
      await mirrorTreasuryRows(handle, 'forum_receipts', 'id', [key])
      await mirrorTreasuryRows(handle, 'forum_receipts', 'id', [key])
      const after = await pgRow('forum_receipts', spec.conflictKey, key)
      expect(after).toEqual(before)
      const count = await client!.unsafe(
        `SELECT count(*)::int AS n FROM forum_receipts WHERE id = $1`,
        [key],
      )
      expect(count[0]?.n).toBe(1)
      expect(diagnostics).toEqual([])
    })

    test('settlement convergence: pending → settled ports state AND amount exactly', async () => {
      await sqlite.db
        .prepare(
          `UPDATE treasury_transactions
           SET state = 'settled', amount_sat = 21001, settled_at = ?
           WHERE id = ?`,
        )
        .bind('2026-07-04T12:30:00.000Z', 'treasury-tx-contract-1')
        .run()

      await mirrorTreasuryRows(handle, 'treasury_transactions', 'id', [
        'treasury-tx-contract-1',
      ])
      await expectConverged(
        'treasury_transactions',
        'id',
        'treasury-tx-contract-1',
      )
      const pgTx = await pgRow(
        'treasury_transactions',
        'id',
        'treasury-tx-contract-1',
      )
      expect(String(pgTx?.state)).toBe('settled')
      expect(Number(pgTx?.amount_sat)).toBe(21_001)
      expect(diagnostics).toEqual([])
    })

    test('replay-guard key exactness: a second consume collides on Postgres too', async () => {
      // The mirror converges by key; a DIFFERENT challenge under the same
      // payment_hash can only exist if D1 held it — Postgres backfill-style
      // inserts under the same key collide.
      const guard = sampleRow('mpp_lightning_replay')
      const inserted = await client!.unsafe(
        `INSERT INTO mpp_lightning_replay (payment_hash, challenge_id, consumed_at)
         VALUES ($1, $2, $3) ON CONFLICT (payment_hash) DO NOTHING RETURNING payment_hash`,
        [guard['payment_hash'], 'a-DIFFERENT-challenge', ISO],
      )
      expect(inserted.length).toBe(0)
      const row = await pgRow(
        'mpp_lightning_replay',
        'payment_hash',
        String(guard['payment_hash']),
      )
      expect(String(row?.challenge_id)).toBe('mpp-challenge-contract-1')
    })

    test('replay-guard diagnostics are redacted: payment identifiers never hit logs', async () => {
      const failing = makeTreasuryDomainHandle({
        d1: sqlite.db,
        flags: { dualWrite: true, reads: 'd1' },
        log: (event, fields) => diagnostics.push([event, fields]),
        postgres: {
          ...postgresStore,
          upsertRows: () => Promise.reject(new Error('postgres down')),
        },
        wait: () => Promise.resolve(),
      })
      await mirrorTreasuryRows(failing, 'mpp_spt_replay', 'spt', [
        'spt_contract_sample_1',
      ])
      const logged = diagnostics.at(-1)
      expect(logged?.[0]).toBe('khala_sync_treasury_dual_write_failed')
      expect(logged?.[1].refs).toEqual(['<redacted:1>'])
      expect(JSON.stringify(logged?.[1])).not.toContain('spt_contract')
      diagnostics = []
    })

    test('fail-soft: a Postgres outage never fails the write path (keys-only diagnostic)', async () => {
      const failing = makeTreasuryDomainHandle({
        d1: sqlite.db,
        flags: { dualWrite: true, reads: 'd1' },
        log: (event, fields) => diagnostics.push([event, fields]),
        postgres: {
          ...postgresStore,
          upsertRows: () => Promise.reject(new Error('postgres down')),
        },
        wait: () => Promise.resolve(),
      })
      await expect(
        mirrorTreasuryRows(failing, 'forum_receipts', 'id', [
          'forum-receipt-contract-1',
        ]),
      ).resolves.toBeUndefined()
      const logged = diagnostics.at(-1)
      expect(logged?.[0]).toBe('khala_sync_treasury_dual_write_failed')
      expect(logged?.[1].op).toBe('forum_receipts')
      expect(logged?.[1].refs).toEqual(['forum-receipt-contract-1'])
      diagnostics = []
    })

    test('read equivalence: listRecent decodes identically from D1 and Postgres', async () => {
      const d1Store = makeD1TreasuryTransactionStore(sqlite.db)
      const compared: Array<LoggedDiagnostic> = []
      const compareStore = makeD1TreasuryTransactionStore(
        makeTreasuryDomainHandle({
          d1: sqlite.db,
          flags: { dualWrite: true, reads: 'compare' },
          log: (event, fields) => compared.push([event, fields]),
          postgres: postgresStore,
          wait: () => Promise.resolve(),
        }),
      )
      const fromD1 = await d1Store.listRecent(10)
      const fromCompare = await compareStore.listRecent(10)
      expect(fromCompare).toEqual(fromD1)
      expect(
        compared.filter(
          ([event]) => event === 'khala_sync_treasury_read_compare_mismatch',
        ),
      ).toEqual([])

      const postgresStoreReads = makeD1TreasuryTransactionStore(
        makeTreasuryDomainHandle({
          d1: sqlite.db,
          flags: { dualWrite: true, reads: 'postgres' },
          log: () => {},
          postgres: postgresStore,
          wait: () => Promise.resolve(),
        }),
      )
      expect(await postgresStoreReads.listRecent(10)).toEqual(fromD1)
    })
  },
)
