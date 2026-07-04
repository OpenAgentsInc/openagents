// KS-8.8 (#8319): Treasury / payouts / tips settlement domain — D1 → Cloud
// SQL migration machinery, following the KS-8.6 artanis template
// (artanis-domain-store.ts) with money-domain discipline layered on top.
//
// Twenty-seven money tables, six money crons (TipsSweep.runTick,
// TipsBuffer.reconcileForwarding, TipsBuffer.backingInvariant,
// TreasuryTransactions.reconcilePending,
// XClaimRewardTreasuryDispatcher.runTick,
// ForumDirectTips.archiveStaleRecoveries). The domain's SQL lives across
// many owning modules (nexus-treasury-payout-ledger.ts,
// treasury-page-routes.ts, forum/paid-actions.ts, payments-ledger.ts,
// labor-escrow.ts, tips-sweep.ts, x-claim-reward-treasury-dispatcher.ts,
// partner/site payout ledgers, mpp replay guards, ...), so the seam is a
// DATABASE-SHAPED handle instead of a per-operation store:
//
//  1. `TreasuryDatabase = D1Database | TreasuryDomainHandle` — the money
//     module signatures take this union. A plain `D1Database` still works
//     (no mirroring, no routing — fail-safe), and `treasuryAuthorityDb(db)`
//     recovers the authoritative D1 handle either way.
//     `makeTreasuryDatabaseForEnv(env)` is the index.ts drop-in that
//     upgrades the six money crons and the domain routes to the seam.
//
//  2. `mirrorTreasuryRows(db, table, keyColumn, keys)` — the dual-write.
//     After the authoritative D1 write, the RESOLVED row(s) are read back
//     from D1 by key and converged into Postgres as full-row upserts
//     (`ON CONFLICT (natural key) DO UPDATE`) — the mirror can therefore
//     NEVER invent an amount, settlement state, idempotency key, or receipt:
//     it only ever copies what the D1 authority already holds. A Postgres
//     failure NEVER fails the request — it logs the typed
//     `khala_sync_treasury_dual_write_failed` diagnostic (the drift metric)
//     and moves on: a mirror outage must never take down a payout, a tip,
//     or a settlement cron.
//
//  3. `treasuryRead(db, op, refs, readD1, readPostgres)` — flag-routed
//     reads: d1 (default), compare (read both, SERVE D1, log mismatches),
//     postgres (bounded retry, D1 fallback + diagnostic on exhaustion).
//     MONEY RULE: every side-effect-bearing read (the payout DISPATCHER
//     scan, the sweep candidate scan, the pending-transaction reconcile
//     scan) reads exactly ONE store — D1 — until the epic-gated runbook
//     cutover; those scans do not pass a `readPostgres` twin in this lane,
//     so no flag value can make Postgres drive a dispatch decision.
//
// MONEY-DOMAIN DISCIPLINE (issue #8319 / MIGRATION_PLAN §3.5):
//   - D1 stays SOLE AUTHORITY. The Postgres twin is a best-effort mirror.
//   - Amounts, settlement states, idempotency keys, and receipt semantics
//     are copied byte-exactly, never derived, never rewritten.
//   - Wallet material and payment tokens NEVER appear in Postgres columns
//     beyond what D1 already stores (the twins are column-for-column), and
//     NEVER in diagnostics: log lines carry row KEYS only (ids/refs) — no
//     amounts, no destinations, no bolt11/offer/address strings, no
//     payment hashes.
//
// Flags (per KS-8 convention):
//   KHALA_SYNC_TREASURY_DUAL_WRITE (default ON; '0'|'off'|'false'|'disabled'|'no')
//   KHALA_SYNC_TREASURY_READS      (default 'd1'; 'd1'|'postgres'|'compare')
// With no KHALA_SYNC_DB binding everything degrades to plain D1.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Treasury settlement domain
// cutover"): dual-write on → backfill (khala-sync-server
// scripts/backfill-treasury.ts) → second sweep → --verify (exact counts +
// per-state/rail money SUMs) → compare reads → postgres reads → re-home the
// six money crons → D1 retirement only in KS-8.19 (#8330).

import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import { openAgentsDatabase } from './runtime'

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type TreasuryDomainReadsMode = 'd1' | 'postgres' | 'compare'

export type TreasuryDomainFlags = Readonly<{
  dualWrite: boolean
  reads: TreasuryDomainReadsMode
}>

export type TreasuryDomainFlagEnv = Readonly<{
  KHALA_SYNC_TREASURY_DUAL_WRITE?: string | undefined
  KHALA_SYNC_TREASURY_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.8 migration flags from Worker vars. Dual-write defaults ON
 * (this lane lands with the mirror active wherever the binding exists);
 * reads default to D1 authority — read flips are EPIC-GATED ops decisions
 * (#8282), never a code default. Unknown read values fall back to 'd1' —
 * never fail open into an unproven read path on a typo.
 */
export const treasuryDomainFlagsFromEnv = (
  env: TreasuryDomainFlagEnv,
): TreasuryDomainFlags => {
  const dualWriteRaw = env.KHALA_SYNC_TREASURY_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_TREASURY_READS?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads:
      readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type TreasuryDomainDiagnosticEvent =
  | 'khala_sync_treasury_dual_write_failed'
  | 'khala_sync_treasury_read_compare_mismatch'
  | 'khala_sync_treasury_postgres_read_failed'
  | 'khala_sync_treasury_postgres_read_fallback'

export type TreasuryDomainDiagnostic = Readonly<{
  /** The mirrored table or read operation, e.g. 'treasury_transactions'. */
  op: string
  /**
   * Public-safe refs identifying the affected rows — row KEYS only
   * (ids/refs). NEVER amounts, destinations, bolt11/offer/address strings,
   * payment hashes, or any wallet/payment-token material.
   */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type TreasuryDomainLog = (
  event: TreasuryDomainDiagnosticEvent,
  fields: TreasuryDomainDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

// ---------------------------------------------------------------------------
// Table registry
// ---------------------------------------------------------------------------
//
// Column lists mirror khala-sync-server migration 0016_treasury_domain.sql
// (which mirrors the live D1 schema: worker migrations 0101/0122/0128/0131/
// 0143/0146/0147/0149/0151/0153/0159..0167/0184/0196..0199/0203/0204/0206/
// 0211/0214/0224/0225/0261/0293) and the registry in
// packages/khala-sync-server/src/treasury-backfill.ts. The contract test
// proves the registry against BOTH engines' real SQL.

export type TreasuryDomainTable =
  | 'treasury_transactions'
  | 'nexus_payout_target_approvals'
  | 'nexus_treasury_payout_intents'
  | 'nexus_treasury_payout_attempts'
  | 'nexus_treasury_payout_reconciliation_events'
  | 'nexus_payment_authority_receipts'
  | 'nexus_release_gates'
  | 'forum_money_actions'
  | 'forum_payment_events'
  | 'forum_receipts'
  | 'forum_l402_challenges'
  | 'forum_l402_redemptions'
  | 'forum_direct_tip_attempts'
  | 'forum_direct_tip_webhook_events'
  | 'forum_tip_recipient_wallets'
  | 'forum_tip_settlement_claims'
  | 'x_claim_reward_ledger'
  | 'agent_claim_reward_ledger'
  | 'agent_balances'
  | 'labor_escrows'
  | 'labor_escrow_receipts'
  | 'partner_payout_ledger_entries'
  | 'partner_agreements'
  | 'site_referral_payout_ledger_entries'
  | 'revenue_event_provenance'
  | 'mpp_lightning_replay'
  | 'mpp_spt_replay'

type TreasuryDomainTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /** Conflict target for the converge upsert (the table's natural key). */
  conflictKey: string
  /** Columns modules may key mirrors/reads by (validated, never dynamic). */
  keyColumns: ReadonlyArray<string>
  /** Column latest-N reads order by (text ISO timestamps sort correctly). */
  orderColumn: string
  /**
   * True for tables whose row keys ARE payment identifiers (the MPP replay
   * guards: payment_hash / spt). Their keys are still used for the D1
   * read-back, but diagnostics show only a redacted count — payment tokens
   * never appear in log lines.
   */
  redactKeysInDiagnostics?: true
}>

export const TREASURY_DOMAIN_TABLES: Readonly<
  Record<TreasuryDomainTable, TreasuryDomainTableSpec>
> = {
  agent_balances: {
    columns: [
      'actor_ref', 'balance_msat', 'sweep_enabled', 'sweep_threshold_sat',
      'send_credits_below_sat', 'receive_credits_below_sat', 'created_at',
      'updated_at', 'held_msat', 'usd_credit_msat',
    ],
    conflictKey: 'actor_ref',
    keyColumns: ['actor_ref'],
    orderColumn: 'updated_at',
  },
  agent_claim_reward_ledger: {
    columns: [
      'id', 'idempotency_key', 'campaign_ref', 'agent_claim_ref', 'owner_ref',
      'x_account_ref', 'tweet_ref', 'state', 'amount_sats',
      'destination_kind', 'redacted_destination_ref', 'payout_intent_ref',
      'dispatch_attempt_ref', 'settlement_ref', 'rejection_reason',
      'policy_refs_json', 'caveat_refs_json', 'created_at', 'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key'],
    orderColumn: 'updated_at',
  },
  forum_direct_tip_attempts: {
    columns: [
      'id', 'idempotency_key', 'payer_actor_ref', 'recipient_actor_ref',
      'target_topic_id', 'target_post_id', 'target_post_permalink',
      'amount_sats', 'provider_ref', 'external_ref', 'redacted_evidence_ref',
      'payment_mode', 'payment_event_status', 'status', 'receipt_ref',
      'payment_event_id', 'created_at', 'updated_at', 'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key'],
    orderColumn: 'updated_at',
  },
  forum_direct_tip_webhook_events: {
    columns: [
      'id', 'provider_event_ref', 'direct_tip_attempt_id', 'provider_ref',
      'external_ref', 'amount_sats', 'payment_event_status',
      'redacted_evidence_ref', 'event_body_digest_ref',
      'signature_binding_ref', 'reconciliation_status',
      'reconciliation_result', 'first_seen_at', 'last_seen_at',
      'delivery_count', 'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'provider_event_ref', 'direct_tip_attempt_id'],
    orderColumn: 'last_seen_at',
  },
  forum_l402_challenges: {
    columns: [
      'id', 'idempotency_key', 'actor_ref', 'action_kind', 'method', 'path',
      'route_params_json', 'request_body_digest', 'target_forum_id',
      'target_topic_id', 'target_post_id', 'price_asset', 'price_value',
      'spend_cap_asset', 'spend_cap_value', 'expires_at',
      'public_projection_json', 'created_at', 'archived_at',
      'recipient_actor_ref', 'recipient_readiness_ref', 'mdk_provider_ref',
      'mdk_environment', 'mdk_sandbox', 'mdk_implementation_state',
      'mdk_checkout_ref', 'mdk_checkout_url_ref', 'mdk_checkout_launch_path',
      'mdk_invoice_ref', 'mdk_payment_hash_ref', 'l402_credential_ref',
      'l402_replay_nonce_ref', 'l402_endpoint_ref',
      'l402_entitlement_scope_refs_json', 'l402_www_authenticate',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key'],
    orderColumn: 'created_at',
  },
  forum_l402_redemptions: {
    columns: [
      'id', 'idempotency_key', 'challenge_id', 'actor_ref', 'proof_ref',
      'entitlement_ref', 'receipt_id', 'replayed', 'public_projection_json',
      'created_at', 'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'challenge_id', 'idempotency_key'],
    orderColumn: 'created_at',
  },
  forum_money_actions: {
    columns: [
      'id', 'idempotency_key', 'actor_ref', 'action_kind', 'target_forum_id',
      'target_topic_id', 'target_post_id', 'amount_asset', 'amount_value',
      'payment_event_id', 'receipt_id', 'earning_actor_ref',
      'public_projection_json', 'created_at', 'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key', 'payment_event_id'],
    orderColumn: 'created_at',
  },
  forum_payment_events: {
    columns: [
      'id', 'money_action_id', 'provider_ref', 'external_ref',
      'amount_asset', 'amount_value', 'redacted_evidence_ref',
      'public_projection_json', 'created_at', 'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'money_action_id'],
    orderColumn: 'created_at',
  },
  forum_receipts: {
    columns: [
      'id', 'receipt_ref', 'action_kind', 'target_forum_id',
      'target_topic_id', 'target_post_id', 'amount_asset', 'amount_value',
      'recipient_actor_ref', 'redacted_payment_ref', 'public_projection_json',
      'created_at', 'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'receipt_ref'],
    orderColumn: 'created_at',
  },
  forum_tip_recipient_wallets: {
    columns: [
      'id', 'actor_ref', 'provider_class', 'wallet_ref',
      'receive_capability_ref', 'payout_target_approval_ref',
      'readiness_refs_json', 'caveat_refs_json', 'custody_policy_refs_json',
      'claim_policy_refs_json', 'source_ref', 'state',
      'public_projection_json', 'created_at', 'updated_at', 'disabled_at',
      'archived_at', 'bolt12_offer', 'lightning_address', 'spark_address',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'actor_ref'],
    orderColumn: 'updated_at',
  },
  forum_tip_settlement_claims: {
    columns: [
      'id', 'idempotency_key', 'receipt_id', 'receipt_ref',
      'recipient_actor_ref', 'settlement_ref',
      'settlement_evidence_refs_json', 'source_ref',
      'public_projection_json', 'created_at', 'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'receipt_id', 'idempotency_key'],
    orderColumn: 'created_at',
  },
  labor_escrow_receipts: {
    columns: [
      'id', 'escrow_id', 'idempotency_key', 'transition_kind',
      'work_request_id', 'requester_actor_ref', 'provider_actor_ref',
      'amount_msat', 'receipt_ref', 'evidence_ref', 'state_after',
      'forfeit_destination', 'forfeit_destination_actor_ref',
      'public_projection_json', 'created_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'escrow_id', 'receipt_ref', 'idempotency_key'],
    orderColumn: 'created_at',
  },
  labor_escrows: {
    columns: [
      'id', 'idempotency_key', 'work_request_id', 'requester_actor_ref',
      'provider_actor_ref', 'amount_msat', 'state', 'funding_source',
      'job_event_id', 'acceptance_event_ref', 'reserve_receipt_ref',
      'release_receipt_ref', 'refund_receipt_ref', 'forfeit_receipt_ref',
      'forfeit_destination', 'forfeit_destination_actor_ref',
      'forfeit_condition_ref', 'public_projection_json', 'created_at',
      'updated_at', 'released_at', 'refunded_at', 'forfeited_at',
      'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'work_request_id', 'idempotency_key'],
    orderColumn: 'updated_at',
  },
  mpp_lightning_replay: {
    columns: ['payment_hash', 'challenge_id', 'consumed_at'],
    conflictKey: 'payment_hash',
    keyColumns: ['payment_hash'],
    orderColumn: 'consumed_at',
    redactKeysInDiagnostics: true,
  },
  mpp_spt_replay: {
    columns: ['spt', 'challenge_id', 'payment_intent_id', 'consumed_at'],
    conflictKey: 'spt',
    keyColumns: ['spt'],
    orderColumn: 'consumed_at',
    redactKeysInDiagnostics: true,
  },
  nexus_payment_authority_receipts: {
    columns: [
      'id', 'receipt_ref', 'payout_intent_ref', 'payout_attempt_ref',
      'event_ref', 'receipt_kind', 'audience', 'metadata_refs_json',
      'public_projection_json', 'created_at', 'archived_at',
    ],
    conflictKey: 'receipt_ref',
    keyColumns: ['receipt_ref', 'id', 'payout_intent_ref'],
    orderColumn: 'created_at',
  },
  nexus_payout_target_approvals: {
    columns: [
      'id', 'approval_ref', 'idempotency_key_hash', 'payout_target_ref',
      'redacted_destination_ref', 'owner_user_id', 'agent_ref', 'pylon_ref',
      'status', 'approved_by_ref', 'approval_policy_ref', 'scope_refs_json',
      'public_projection_json', 'created_at', 'updated_at', 'expires_at',
      'archived_at',
    ],
    conflictKey: 'approval_ref',
    keyColumns: ['approval_ref', 'id', 'idempotency_key_hash'],
    orderColumn: 'updated_at',
  },
  nexus_release_gates: {
    columns: [
      'id', 'gate_ref', 'idempotency_key_hash', 'gate_kind', 'status',
      'evidence_refs_json', 'blocker_refs_json', 'public_projection_json',
      'created_at', 'updated_at', 'archived_at',
    ],
    conflictKey: 'gate_ref',
    keyColumns: ['gate_ref', 'id', 'idempotency_key_hash'],
    orderColumn: 'updated_at',
  },
  nexus_treasury_payout_attempts: {
    columns: [
      'id', 'payout_attempt_ref', 'payout_intent_ref',
      'idempotency_key_hash', 'adapter_kind', 'adapter_attempt_ref',
      'status', 'redacted_payment_ref', 'redacted_destination_ref',
      'amount_asset', 'amount_denomination', 'amount_minor_units',
      'metadata_refs_json', 'public_projection_json', 'created_at',
      'updated_at', 'archived_at',
    ],
    conflictKey: 'payout_attempt_ref',
    keyColumns: [
      'payout_attempt_ref', 'id', 'idempotency_key_hash', 'payout_intent_ref',
    ],
    orderColumn: 'updated_at',
  },
  nexus_treasury_payout_intents: {
    columns: [
      'id', 'payout_intent_ref', 'idempotency_key_hash', 'actor_ref',
      'owner_user_id', 'source_kind', 'buyer_payment_ref',
      'accepted_work_refs_json', 'assignment_ref', 'artanis_dispatch_ref',
      'pylon_job_ref', 'payout_target_ref', 'payout_target_approval_ref',
      'adapter_kind', 'amount_asset', 'amount_denomination',
      'amount_minor_units', 'spend_cap_asset', 'spend_cap_denomination',
      'spend_cap_amount_minor_units', 'policy_snapshot_ref', 'status',
      'metadata_refs_json', 'public_projection_json', 'created_at',
      'updated_at', 'archived_at',
    ],
    conflictKey: 'payout_intent_ref',
    keyColumns: ['payout_intent_ref', 'id', 'idempotency_key_hash'],
    orderColumn: 'updated_at',
  },
  nexus_treasury_payout_reconciliation_events: {
    columns: [
      'id', 'event_ref', 'idempotency_key_hash', 'provider_ref',
      'external_event_ref', 'adapter_kind', 'payout_intent_ref',
      'payout_attempt_ref', 'status', 'result_ref', 'metadata_refs_json',
      'public_projection_json', 'created_at', 'archived_at',
    ],
    conflictKey: 'event_ref',
    keyColumns: ['event_ref', 'id', 'idempotency_key_hash'],
    orderColumn: 'created_at',
  },
  partner_agreements: {
    columns: [
      'id', 'agreement_ref', 'partner_ref', 'partner_user_id',
      'customer_user_id', 'role', 'effective_from', 'effective_until',
      'policy_state', 'created_at', 'archived_at',
    ],
    conflictKey: 'agreement_ref',
    keyColumns: ['agreement_ref', 'id'],
    orderColumn: 'created_at',
  },
  partner_payout_ledger_entries: {
    columns: [
      'id', 'payout_ref', 'idempotency_key', 'partner_role',
      'partner_user_id', 'partner_ref', 'beneficiary_user_id', 'asset',
      'qualifying_event_ref', 'qualifying_event_kind', 'qualifying_amount',
      'amount', 'period_key', 'state', 'state_reason_ref',
      'previous_entry_id', 'reversal_of_entry_id', 'evidence_refs_json',
      'policy_refs_json', 'caveat_refs_json', 'created_at', 'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key', 'payout_ref'],
    orderColumn: 'created_at',
  },
  revenue_event_provenance: {
    columns: [
      'event_ref', 'evidence_bundle_ref', 'idempotency_key', 'product_ref',
      'revenue_surface_ref', 'receipt_ref', 'ledger_table', 'ledger_row_ref',
      'demand_provenance', 'payment_state', 'amount_cents', 'amount_sats',
      'public_evidence_refs_json', 'caveat_refs_json', 'source_refs_json',
      'recorded_at', 'created_at', 'updated_at',
    ],
    conflictKey: 'event_ref',
    keyColumns: ['event_ref', 'idempotency_key'],
    orderColumn: 'recorded_at',
  },
  site_referral_payout_ledger_entries: {
    columns: [
      'id', 'payout_ref', 'idempotency_key', 'referral_attribution_id',
      'referral_source_id', 'referral_invite_id', 'referrer_user_id',
      'referred_user_id', 'qualifying_event_ref', 'qualifying_event_kind',
      'qualifying_amount_sats', 'amount_sats', 'period_key', 'state',
      'state_reason_ref', 'previous_entry_id', 'reversal_of_entry_id',
      'evidence_refs_json', 'policy_refs_json', 'caveat_refs_json',
      'created_at', 'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key', 'payout_ref'],
    orderColumn: 'created_at',
  },
  treasury_transactions: {
    columns: [
      'id', 'direction', 'amount_sat', 'state', 'bolt11', 'payment_ref',
      'created_at', 'settled_at', 'expires_at', 'failure_reason_ref',
      'recipient_ref', 'redacted_destination_ref', 'owed_ref', 'owed_sat',
      'recipient_confirmation_state', 'recipient_confirmation_ref',
      'recipient_confirmed_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'created_at',
  },
  x_claim_reward_ledger: {
    columns: [
      'id', 'challenge_id', 'claim_id', 'owner_user_id', 'agent_user_id',
      'x_account_ref', 'amount_sats', 'state', 'state_reason_ref',
      'receipt_ref', 'evidence_refs_json', 'created_at', 'updated_at',
      'treasury_payment_id',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'challenge_id'],
    orderColumn: 'updated_at',
  },
}

export type TreasuryDomainRow = Readonly<Record<string, unknown>>

export class TreasuryDomainKeyColumnError extends Error {
  override readonly name = 'TreasuryDomainKeyColumnError'
}

export class TreasuryDomainDriverError extends Error {
  override readonly name = 'TreasuryDomainDriverError'
}

const requireKeyColumn = (
  table: TreasuryDomainTable,
  keyColumn: string,
): string => {
  if (!TREASURY_DOMAIN_TABLES[table].keyColumns.includes(keyColumn)) {
    throw new TreasuryDomainKeyColumnError(
      `treasury domain store: ${keyColumn} is not a registered key column of ${table}`,
    )
  }
  return keyColumn
}

// ---------------------------------------------------------------------------
// Postgres store (registry-driven, single parameterized statements)
// ---------------------------------------------------------------------------

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally (the same
 * discipline as the khala-sync-server backfill cores). Every statement
 * built here is ONE parameterized statement whose dynamic text comes only
 * from the compile-time table registry — no session state, so it stays
 * Hyperdrive transaction-mode safe.
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== 'function') {
    throw new TreasuryDomainDriverError(
      'treasury domain store requires a driver exposing unsafe(text, params)',
    )
  }
  return unsafe
}

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === 'number') return value
  if (typeof value === 'boolean') return value ? 1 : 0
  return String(value)
}

export type PostgresTreasuryDomainStore = Readonly<{
  /**
   * Converge Postgres to the RESOLVED rows the authoritative D1 write
   * produced — full-row `ON CONFLICT (natural key) DO UPDATE` upserts, so
   * a row touched by dual-write self-heals even before the backfill
   * reaches it, and re-mirroring the same row is a no-op. Amounts,
   * settlement states, and idempotency keys are copied byte-exactly.
   */
  upsertRows: (
    table: TreasuryDomainTable,
    rows: ReadonlyArray<TreasuryDomainRow>,
  ) => Promise<void>
  /** Registry-validated key lookup (read cutover + compare mode). */
  selectRowsByKey: (
    table: TreasuryDomainTable,
    keyColumn: string,
    keys: ReadonlyArray<string | number>,
  ) => Promise<Array<TreasuryDomainRow>>
  /** Latest-N by the table's order column (read cutover + compare mode). */
  selectLatestRows: (
    table: TreasuryDomainTable,
    limit: number,
  ) => Promise<Array<TreasuryDomainRow>>
  /**
   * The TipsBuffer.backingInvariant twin: exact msat SUM over
   * agent_balances, returned as a decimal string (bigint-safe). Used by
   * compare mode as a continuously-running money reconciliation probe.
   */
  sumAgentBalancesMsat: () => Promise<string>
}>

export type MakePostgresTreasuryDomainStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the push route.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresTreasuryDomainStore = (
  deps: MakePostgresTreasuryDomainStoreDependencies,
): PostgresTreasuryDomainStore => {
  const withSql = async <A>(
    fn: (unsafe: UnsafeQuery) => Promise<A>,
  ): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(requireUnsafe(client.sql))
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }

  return {
    selectLatestRows: (table, limit) =>
      withSql(unsafe => {
        const spec = TREASURY_DOMAIN_TABLES[table]
        return unsafe(
          `SELECT ${spec.columns.join(', ')} FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.conflictKey} DESC LIMIT $1`,
          [Math.max(1, Math.min(200, Math.trunc(limit)))],
        )
      }),

    selectRowsByKey: (table, keyColumn, keys) =>
      keys.length === 0
        ? Promise.resolve([])
        : withSql(unsafe => {
            const spec = TREASURY_DOMAIN_TABLES[table]
            const column = requireKeyColumn(table, keyColumn)
            const placeholders = keys
              .map((_, index) => `$${index + 1}`)
              .join(', ')
            return unsafe(
              `SELECT ${spec.columns.join(', ')} FROM ${table} WHERE ${column} IN (${placeholders})`,
              [...keys],
            )
          }),

    sumAgentBalancesMsat: () =>
      withSql(async unsafe => {
        const rows = await unsafe(
          `SELECT COALESCE(SUM(balance_msat), 0)::text AS total FROM agent_balances`,
          [],
        )
        return String(rows[0]?.['total'] ?? '0')
      }),

    upsertRows: (table, rows) =>
      rows.length === 0
        ? Promise.resolve()
        : withSql(async unsafe => {
            const spec = TREASURY_DOMAIN_TABLES[table]
            const columnsSql = spec.columns.join(', ')
            const updates = spec.columns
              .filter(column => column !== spec.conflictKey)
              .map(column => `${column} = EXCLUDED.${column}`)
              .join(', ')
            for (const row of rows) {
              const values = spec.columns.map(column =>
                normalizeValue(row[column]),
              )
              const placeholders = values
                .map((_, index) => `$${index + 1}`)
                .join(', ')
              await unsafe(
                `INSERT INTO ${table} (${columnsSql}) VALUES (${placeholders}) ON CONFLICT (${spec.conflictKey}) DO UPDATE SET ${updates}`,
                values as Array<unknown>,
              )
            }
          }),
  }
}

// ---------------------------------------------------------------------------
// The seam handle
// ---------------------------------------------------------------------------

export type TreasuryDomainHandle = Readonly<{
  /** Brand — discriminates the handle from a bare D1Database. */
  treasuryDomainSeam: true
  /** The authoritative D1 database (ALL writes and default reads). */
  d1: D1Database
  flags: TreasuryDomainFlags
  log: TreasuryDomainLog
  /** Undefined when no KHALA_SYNC_DB binding: plain-D1 degradation. */
  postgres: PostgresTreasuryDomainStore | undefined
  /** Bounded-retry backoff hook (tests inject a no-op). */
  wait: (ms: number) => Promise<void>
}>

/**
 * What the money-domain module signatures take. A plain `D1Database` keeps
 * working (no mirroring, no routing), so non-domain call sites and tests
 * need no ceremony; `makeTreasuryDatabaseForEnv` upgrades the money call
 * sites (including the six crons) to the dual-write seam.
 */
export type TreasuryDatabase = D1Database | TreasuryDomainHandle

export const isTreasuryDomainHandle = (
  db: TreasuryDatabase,
): db is TreasuryDomainHandle =>
  // Null-safe on purpose: some refusal-path tests hand a null database
  // double through the union; it must behave exactly like a bare D1 handle
  // (pass-through, no seam) rather than throwing at the type guard.
  (db as { treasuryDomainSeam?: unknown } | null)?.treasuryDomainSeam === true

/** The authoritative D1 handle, whichever side of the union arrived. */
export const treasuryAuthorityDb = (db: TreasuryDatabase): D1Database =>
  isTreasuryDomainHandle(db) ? db.d1 : db

const READ_RETRY_DELAYS_MS: ReadonlyArray<number> = [50, 150]

export type MakeTreasuryDomainHandleDependencies = Readonly<{
  d1: D1Database
  flags: TreasuryDomainFlags
  log?: TreasuryDomainLog | undefined
  postgres: PostgresTreasuryDomainStore | undefined
  wait?: ((ms: number) => Promise<void>) | undefined
}>

export const makeTreasuryDomainHandle = (
  deps: MakeTreasuryDomainHandleDependencies,
): TreasuryDomainHandle => ({
  d1: deps.d1,
  flags: deps.flags,
  log: deps.log ?? (() => {}),
  postgres: deps.postgres,
  treasuryDomainSeam: true,
  wait:
    deps.wait ??
    ((ms: number) => new Promise(resolve => setTimeout(resolve, ms))),
})

// ---------------------------------------------------------------------------
// Dual-write mirror
// ---------------------------------------------------------------------------

/**
 * Best-effort Postgres mirror after an authoritative D1 write: reads the
 * RESOLVED row(s) back from D1 by `keyColumn` and converges the Postgres
 * twins. The mirror therefore copies exactly what D1 holds — it can never
 * alter an amount, a settlement state, an idempotency key, or a receipt.
 * NEVER throws — any failure (including the D1 read-back) logs the
 * `khala_sync_treasury_dual_write_failed` diagnostic (row keys only) and
 * returns: mirror degradation must never take down a payout, a tip, or a
 * settlement cron. On a plain D1Database, a missing binding, or dual-write
 * off it is a no-op.
 */
export const mirrorTreasuryRows = async (
  db: TreasuryDatabase,
  table: TreasuryDomainTable,
  keyColumn: string,
  keys: ReadonlyArray<string | number>,
): Promise<void> => {
  if (!isTreasuryDomainHandle(db)) return
  const { d1, flags, log, postgres } = db
  if (postgres === undefined || !flags.dualWrite || keys.length === 0) return

  const refs =
    TREASURY_DOMAIN_TABLES[table].redactKeysInDiagnostics === true
      ? [`<redacted:${keys.length}>`]
      : keys.map(String)
  try {
    const spec = TREASURY_DOMAIN_TABLES[table]
    const column = requireKeyColumn(table, keyColumn)
    const placeholders = keys.map(() => '?').join(', ')
    const result = await d1
      .prepare(
        `SELECT ${spec.columns.join(', ')} FROM ${table} WHERE ${column} IN (${placeholders})`,
      )
      .bind(...keys)
      .all<TreasuryDomainRow>()
    const rows = result.results ?? []
    if (rows.length === 0) return
    await postgres.upsertRows(table, rows)
  } catch (error) {
    log('khala_sync_treasury_dual_write_failed', {
      messageSafe: safeMessage(error),
      op: table,
      refs,
    })
  }
}

// ---------------------------------------------------------------------------
// Flag-routed reads
// ---------------------------------------------------------------------------

const stableStringify = (value: unknown): string =>
  JSON.stringify(value, (_key, val: unknown) =>
    val !== null && typeof val === 'object' && !Array.isArray(val)
      ? Object.fromEntries(
          Object.entries(val as Record<string, unknown>).sort(([a], [b]) =>
            a < b ? -1 : a > b ? 1 : 0,
          ),
        )
      : val,
  )

/**
 * Flag-routed read: d1 (default) | postgres (bounded retry + D1 fallback)
 * | compare (read both, SERVE D1, log mismatches). Reads with no Postgres
 * twin — and EVERY side-effect-bearing money scan (payout dispatch, sweep
 * candidates, pending-transaction reconcile) — pass no `readPostgres` and
 * stay on D1 regardless of the flag: the dispatcher reads exactly one
 * store until the epic-gated cutover.
 */
export const treasuryRead = async <A>(
  db: TreasuryDatabase,
  op: string,
  refs: ReadonlyArray<string>,
  readD1: () => Promise<A>,
  readPostgres?: (postgres: PostgresTreasuryDomainStore) => Promise<A>,
): Promise<A> => {
  if (!isTreasuryDomainHandle(db)) return readD1()
  const { flags, log, postgres, wait } = db
  if (
    postgres === undefined ||
    readPostgres === undefined ||
    flags.reads === 'd1'
  ) {
    return readD1()
  }

  if (flags.reads === 'postgres') {
    for (let attempt = 0; ; attempt++) {
      try {
        return await readPostgres(postgres)
      } catch (error) {
        const delay = READ_RETRY_DELAYS_MS[attempt]
        if (delay === undefined) {
          log('khala_sync_treasury_postgres_read_fallback', {
            messageSafe: safeMessage(error),
            op,
            refs,
          })
          return readD1()
        }
        log('khala_sync_treasury_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op,
          refs,
        })
        await wait(delay)
      }
    }
  }

  // compare
  const d1Result = await readD1()
  try {
    const postgresResult = await readPostgres(postgres)
    if (stableStringify(d1Result) !== stableStringify(postgresResult)) {
      log('khala_sync_treasury_read_compare_mismatch', {
        messageSafe: 'postgres read differs from d1 authority',
        op,
        refs,
      })
    }
  } catch (error) {
    log('khala_sync_treasury_postgres_read_failed', {
      messageSafe: safeMessage(error),
      op,
      refs,
    })
  }
  return d1Result
}

// ---------------------------------------------------------------------------
// Env factory (the index.ts drop-in)
// ---------------------------------------------------------------------------

export type TreasuryDomainStoreEnv = TreasuryDomainFlagEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeTreasuryDatabaseForEnvOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: TreasuryDomainLog | undefined
}>

const defaultLog: TreasuryDomainLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

/**
 * The production `TreasuryDatabase` factory: D1 authority + flag-gated
 * Postgres dual-write/reads. Replaces bare `openAgentsDatabase(env)` at
 * the money-domain Worker call sites, including the six money crons. With
 * no KHALA_SYNC_DB binding (or everything flagged off) it returns the
 * plain D1Database — behavior-identical to before this lane.
 */
export const makeTreasuryDatabaseForEnv = (
  env: TreasuryDomainStoreEnv,
  options: MakeTreasuryDatabaseForEnvOptions = {},
): TreasuryDatabase => {
  const d1 = openAgentsDatabase(env)
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  const flags = treasuryDomainFlagsFromEnv(env)

  if (
    connectionString === undefined ||
    connectionString.length === 0 ||
    (!flags.dualWrite && flags.reads === 'd1')
  ) {
    return d1
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres = makePostgresTreasuryDomainStore({
    acquireSql: () => makeSqlClient(connectionString),
  })

  return makeTreasuryDomainHandle({
    d1,
    flags,
    log: options.log ?? defaultLog,
    postgres,
  })
}
