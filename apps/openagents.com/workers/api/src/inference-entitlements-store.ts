// KS-8.9 (#8320): inference entitlements + quotas domain — D1 → Cloud SQL
// migration machinery. Follows the KS-8.1/KS-8.2 templates
// (`pylon-dispatch-store.ts` #8307, `token-ledger-store.ts` #8308), adapted
// for a WIDE domain (~29 tables across 12 write modules) that sits on the
// inference serving hot path.
//
// Three pieces:
//
//  1. `makePostgresInferenceEntitlementsStore` — the Postgres side over the
//     structural `SyncSql` seam via the KHALA_SYNC_DB Hyperdrive binding
//     (khala-sync-server migration 0013): a typed MIRROR-OP applier for
//     every write shape in the domain, plus the six ENFORCEMENT GATE reads
//     (free-tier key membership, free-tier daily usage, free-usage pool
//     state, premium allowlist, operator exemption, privacy entitlement)
//     that decide allow/deny on the serving path.
//
//     ENFORCEMENT COUNTERS ARE EVENT-KEYED (MIGRATION_PLAN §3.6): a lost
//     increment is a free-tier leak, a doubled one is a false denial. The
//     three tally families (`inference_free_tier_usage`,
//     `inference_free_usage_tally`, `inference_earned_allowance`) are
//     mirrored as ACCRUAL ops that insert the unique-keyed event row
//     `ON CONFLICT DO NOTHING` and increment the tally ONLY for a fresh
//     event, in one transaction — the same discipline as the D1 batches,
//     so a re-delivered mirror op can never double-count. Everything else
//     is `ON CONFLICT DO NOTHING` inserts on the SAME unique keys as D1's
//     INSERT OR IGNORE, or converge upserts matching D1's
//     ON CONFLICT DO UPDATE column set exactly.
//
//  2. `makeInferenceEntitlementsRoutingForEnv` — the flag-routed production
//     seam the gate/store modules receive:
//       - `mirror(ops)`: FIRE-SAFE. Synchronous enqueue of a best-effort
//         Postgres apply; NEVER awaited by the caller, never throws, never
//         delays a completion. A mirror failure logs the typed drift
//         diagnostic `khala_sync_entitlements_dual_write_failed` and moves
//         on — D1 stays the enforcing authority.
//       - `gateReads`: the routed enforcement reads. Modes:
//           d1        — routing returns NO gateReads; gates run their
//                       existing inline D1 reads (ZERO added latency and
//                       zero new allocations on the hot path — the d1 mode
//                       regression test pins this).
//           compare   — serve the D1 decision immediately; schedule a
//                       fire-safe Postgres shadow read + comparison off
//                       the response path and log
//                       `khala_sync_entitlements_read_compare_mismatch`
//                       (the §3.6 denial-decision shadow evidence).
//           postgres  — ONE Postgres attempt, D1 fallback + diagnostic on
//                       error. No retry backoff here: these reads gate
//                       live completions, so we fail back to the D1
//                       authority instead of stacking latency.
//
//  3. Call-site wiring: the 12 domain modules accept optional
//     `mirror` / `gateReads` deps (default absent => byte-identical D1
//     behavior) and index.ts / route factories thread this seam through
//     `makeInferenceEntitlementsRoutingForEnv(env)`. Flags:
//       KHALA_SYNC_ENTITLEMENTS_DUAL_WRITE (default ON; 'off'|'0'|'false'|…)
//       KHALA_SYNC_ENTITLEMENTS_READS      (default 'd1'; 'd1'|'postgres'|'compare')
//     With no KHALA_SYNC_DB binding everything degrades to plain D1.
//
// NOT MIRRORED BY DESIGN: `agent_search_metric_events` — the `*_metric_
// events` observability streams are Analytics Engine candidates, not
// Postgres rows (MIGRATION_PLAN §3.6). The stream keeps writing D1 only
// until the Analytics Engine sink lands with the decommission follow-up.
//
// ROW TYPING NOTE: unlike the 3-table KS-8.1/KS-8.2 lanes, this domain has
// ~29 tables, so generic `write` ops carry a snake_case `MirrorRow` checked
// against the per-table column registry at apply time (unknown columns are
// refused), while the enforcement-critical accrual/consume ops carry fully
// typed payloads. Column fidelity for the generic rows is pinned by the
// repository contract suite.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Inference entitlements
// domain"): dual-write on → backfill
// (scripts/backfill-inference-entitlements.ts) → --verify (exact counts,
// tally = SUM(events) per key, per-group tallies, newest-N hashes) →
// compare reads over a low-traffic window (zero divergence required — the
// read cutover changes which store ENFORCES) → postgres reads →
// decommission D1 tables in a follow-up issue.

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

export type InferenceEntitlementsReadsMode = 'd1' | 'postgres' | 'compare'

export type InferenceEntitlementsFlags = Readonly<{
  dualWrite: boolean
  reads: InferenceEntitlementsReadsMode
}>

export type InferenceEntitlementsFlagEnv = Readonly<{
  KHALA_SYNC_ENTITLEMENTS_DUAL_WRITE?: string | undefined
  KHALA_SYNC_ENTITLEMENTS_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.9 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding exists);
 * reads default to D1 authority until the runbook's cutover sequence flips
 * them. Unknown read values fall back to 'd1' — never fail open into an
 * unproven ENFORCING read path on a typo.
 */
export const inferenceEntitlementsFlagsFromEnv = (
  env: InferenceEntitlementsFlagEnv,
): InferenceEntitlementsFlags => {
  const dualWriteRaw =
    env.KHALA_SYNC_ENTITLEMENTS_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_ENTITLEMENTS_READS?.trim().toLowerCase()

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

export type InferenceEntitlementsDiagnosticEvent =
  | 'khala_sync_entitlements_dual_write_failed'
  | 'khala_sync_entitlements_read_compare_mismatch'
  | 'khala_sync_entitlements_postgres_read_failed'
  | 'khala_sync_entitlements_postgres_read_fallback'

export type InferenceEntitlementsDiagnostic = Readonly<{
  /** The store operation, e.g. 'accrue_free_tier_usage'. */
  op: string
  /** Public-safe refs identifying the affected rows (never payloads). */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type InferenceEntitlementsLog = (
  event: InferenceEntitlementsDiagnosticEvent,
  fields: InferenceEntitlementsDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

const defaultLog: InferenceEntitlementsLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

// ---------------------------------------------------------------------------
// Mirror ops (the typed write seam)
// ---------------------------------------------------------------------------

export type MirrorValue = string | number | null
export type MirrorRow = Readonly<Record<string, MirrorValue>>

type WriteTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /**
   * undefined => plain `ON CONFLICT DO NOTHING` (D1 INSERT OR IGNORE
   * parity across ALL unique surfaces). Otherwise a converge upsert whose
   * SET list matches the D1 twin's ON CONFLICT DO UPDATE column set
   * exactly (created_at is preserved on conflict, like D1's excluded-set).
   */
  conflict?: Readonly<{
    target: ReadonlyArray<string>
    update: ReadonlyArray<string>
  }>
}>

/**
 * Every generic-write table in the domain with its exact column set and
 * D1-matching conflict behavior. Tables written through the SPECIAL ops
 * below (tallies, consumes, batch updates, cache) are absent on purpose.
 */
const ENTITLEMENTS_WRITE_TABLES = {
  agent_rate_limit_challenges: {
    columns: [
      'id', 'idempotency_key_hash', 'actor_ref', 'owner_user_id',
      'route_key', 'method', 'path', 'submission_idempotency_key_hash',
      'client_fingerprint_hash', 'request_body_digest', 'price_asset',
      'price_denomination', 'price_value', 'spend_cap_asset',
      'spend_cap_denomination', 'spend_cap_value', 'entitlement_kind',
      'expires_at', 'public_projection_json', 'created_at', 'archived_at',
    ],
  },
  agent_rate_limit_entitlements: {
    columns: [
      'id', 'entitlement_ref', 'challenge_id', 'receipt_ref', 'actor_ref',
      'owner_user_id', 'route_key', 'method', 'path',
      'submission_idempotency_key_hash', 'client_fingerprint_hash',
      'request_body_digest', 'entitlement_kind', 'status', 'expires_at',
      'created_at', 'consumed_at', 'archived_at',
    ],
  },
  agent_rate_limit_receipts: {
    columns: [
      'id', 'receipt_ref', 'challenge_id', 'actor_ref', 'owner_user_id',
      'route_key', 'amount_asset', 'amount_denomination', 'amount_value',
      'entitlement_ref', 'redacted_payment_ref', 'public_projection_json',
      'created_at', 'archived_at',
    ],
  },
  agent_rate_limit_redemptions: {
    columns: [
      'id', 'idempotency_key_hash', 'challenge_id', 'actor_ref',
      'proof_ref', 'entitlement_ref', 'receipt_ref', 'replayed',
      'public_projection_json', 'created_at', 'archived_at',
    ],
  },
  agent_search_entitlements: {
    columns: [
      'id', 'entitlement_ref', 'challenge_id', 'receipt_ref', 'actor_ref',
      'agent_user_id', 'credential_id', 'product_id', 'scope_ref',
      'method', 'path', 'mode', 'request_body_digest', 'status',
      'expires_at', 'created_at', 'consumed_at', 'archived_at',
    ],
  },
  agent_search_payment_challenges: {
    columns: [
      'id', 'idempotency_key_hash', 'actor_ref', 'agent_user_id',
      'credential_id', 'token_prefix', 'method', 'path', 'mode',
      'request_body_digest', 'product_id', 'price_asset',
      'price_denomination', 'price_value', 'spend_cap_asset',
      'spend_cap_denomination', 'spend_cap_value', 'expires_at',
      'public_projection_json', 'created_at', 'archived_at',
    ],
  },
  agent_search_payment_receipts: {
    columns: [
      'id', 'receipt_ref', 'challenge_id', 'actor_ref', 'agent_user_id',
      'credential_id', 'product_id', 'amount_asset', 'amount_denomination',
      'amount_value', 'entitlement_ref', 'redacted_payment_ref',
      'public_projection_json', 'created_at', 'archived_at',
    ],
  },
  agent_search_payment_redemptions: {
    columns: [
      'id', 'idempotency_key_hash', 'challenge_id', 'actor_ref',
      'credential_id', 'proof_ref', 'entitlement_ref', 'receipt_ref',
      'public_projection_json', 'created_at', 'archived_at',
    ],
  },
  agent_search_quota_events: {
    columns: [
      'id', 'actor_ref', 'credential_id', 'event_kind', 'mode', 'units',
      'product_id', 'entitlement_ref', 'created_at',
    ],
  },
  agent_search_requests: {
    columns: [
      'id', 'receipt_ref', 'actor_ref', 'agent_user_id', 'credential_id',
      'token_prefix', 'idempotency_key_hash', 'request_body_digest',
      'query_hash', 'query_text', 'mode', 'provider',
      'provider_request_id', 'status', 'cache_status', 'charge_state',
      'product_id', 'entitlement_ref', 'provider_cost_dollars',
      'public_projection_json', 'created_at', 'completed_at', 'archived_at',
    ],
  },
  agent_search_sources: {
    columns: [
      'id', 'search_request_id', 'source_ref', 'title', 'url', 'domain',
      'published_date', 'score', 'highlight_text', 'selected_text_hash',
      'public_safe', 'created_at',
    ],
  },
  builtin_compute_agent_quota_events: {
    columns: [
      'id', 'actor_user_id', 'grant_ref', 'provider', 'budget_class',
      'session_units', 'session_budget_seconds', 'token_ceiling',
      'created_at',
    ],
  },
  inference_batch_jobs: {
    columns: [
      'job_id', 'account_ref', 'status', 'charge_receipt_ref',
      'dataset_size', 'processed_items', 'failed_items', 'results_r2_key',
      'created_at', 'updated_at', 'enqueued_at', 'started_at',
    ],
  },
  inference_confidential_compute_execution_receipts: {
    columns: [
      'receipt_ref', 'execution_ref', 'account_ref', 'request_ref',
      'idempotency_key', 'capture_excluded', 'reason_ref', 'created_at',
      'updated_at',
    ],
  },
  inference_free_tier_keys: {
    columns: [
      'account_ref', 'scope', 'mint_source', 'note', 'created_at',
      'updated_at',
    ],
    conflict: {
      target: ['account_ref'],
      update: ['scope', 'mint_source', 'note', 'updated_at'],
    },
  },
  inference_operator_exemption: {
    columns: [
      'owner_key', 'scope', 'granted_by', 'note', 'created_at',
      'updated_at',
    ],
    conflict: {
      target: ['owner_key'],
      update: ['scope', 'granted_by', 'note', 'updated_at'],
    },
  },
  inference_premium_allowlist: {
    columns: [
      'owner_key', 'scope', 'granted_by', 'note', 'created_at',
      'updated_at',
    ],
    conflict: {
      target: ['owner_key'],
      update: ['scope', 'granted_by', 'note', 'updated_at'],
    },
  },
  inference_privacy_entitlement_receipts: {
    columns: [
      'receipt_ref', 'entitlement_ref', 'account_ref', 'purchase_ref',
      'idempotency_key', 'privacy_tier', 'capture_excluded', 'reason_ref',
      'created_at', 'updated_at',
    ],
  },
  inference_privacy_entitlements: {
    columns: [
      'account_ref', 'privacy_tier', 'note', 'created_at', 'updated_at',
    ],
    conflict: {
      target: ['account_ref'],
      update: ['privacy_tier', 'note', 'updated_at'],
    },
  },
  inference_referral_margin_splits: {
    columns: [
      'id', 'request_id', 'account_ref', 'referred_user_id',
      'referrer_user_id', 'referral_attribution_id', 'referral_source_id',
      'referral_invite_id', 'payout_ref', 'qualifying_event_ref',
      'charge_receipt_ref', 'funding_kind', 'adapter_id',
      'requested_model', 'served_model', 'served_by_contributor',
      'serving_node_count', 'charge_usd', 'cost_usd', 'margin_usd',
      'margin_sats', 'openagents_usd', 'openagents_sats',
      'serving_node_usd', 'serving_node_sats', 'referrer_usd',
      'referrer_sats', 'created_at', 'archived_at',
    ],
  },
  orange_check_entitlements: {
    columns: [
      'id', 'agent_user_id', 'actor_ref', 'state', 'receipt_ref',
      'action_ref', 'paid_amount_cents', 'created_at', 'updated_at',
    ],
  },
} as const satisfies Record<string, WriteTableSpec>

export type InferenceEntitlementsWriteTable =
  keyof typeof ENTITLEMENTS_WRITE_TABLES

export type InferenceEntitlementsMirrorOp =
  | Readonly<{
      kind: 'write'
      table: InferenceEntitlementsWriteTable
      row: MirrorRow
    }>
  | Readonly<{
      kind: 'delete_owner_grant'
      table: 'inference_operator_exemption' | 'inference_premium_allowlist'
      ownerKey: string
    }>
  | Readonly<{
      kind: 'accrue_free_tier_usage'
      event: Readonly<{
        requestId: string
        accountRef: string
        usageDay: string
        servedModel: string
        totalTokens: number
        createdAt: string
      }>
    }>
  | Readonly<{
      kind: 'accrue_free_usage'
      identityKind: string
      event: Readonly<{
        requestId: string
        ownerKey: string
        accountRef: string
        servedModel: string
        freeUsdMicros: number
        createdAt: string
      }>
    }>
  | Readonly<{
      kind: 'accrue_earned_allowance'
      event: Readonly<{
        accrualEventRef: string
        ownerKey: string
        accrualKind: string
        earnedUsdMicros: number
        createdAt: string
      }>
    }>
  | Readonly<{
      kind: 'increment_free_key_mint'
      ipHash: string
      mintDay: string
      nowIso: string
    }>
  | Readonly<{
      kind: 'consume_entitlement'
      table: 'agent_rate_limit_entitlements' | 'agent_search_entitlements'
      entitlementRef: string
      consumedAt: string
    }>
  | Readonly<{
      kind: 'update_batch_job'
      jobId: string
      status: string
      updatedAt: string
      processedItems?: number | undefined
      failedItems?: number | undefined
      resultsR2Key?: string | undefined
      startedAt?: string | undefined
    }>
  | Readonly<{
      kind: 'store_agent_search_cache'
      /** The previous active entry is archived at this timestamp. */
      archivedAt: string
      cacheKey: string
      row: MirrorRow
    }>

/** Public-safe ref(s) for a mirror op (diagnostics only, never payloads). */
export const mirrorOpRefs = (
  op: InferenceEntitlementsMirrorOp,
): ReadonlyArray<string> => {
  switch (op.kind) {
    case 'write': {
      const key =
        op.row['id'] ??
        op.row['request_id'] ??
        op.row['receipt_ref'] ??
        op.row['owner_key'] ??
        op.row['account_ref'] ??
        op.row['job_id'] ??
        null
      return [`${op.table}:${String(key ?? 'unknown')}`]
    }
    case 'delete_owner_grant':
      return [`${op.table}:${op.ownerKey}`]
    case 'accrue_free_tier_usage':
      return [`inference_free_tier_usage_events:${op.event.requestId}`]
    case 'accrue_free_usage':
      return [`inference_free_usage_events:${op.event.requestId}`]
    case 'accrue_earned_allowance':
      return [
        `inference_earned_allowance_events:${op.event.accrualEventRef}`,
      ]
    case 'increment_free_key_mint':
      return [`inference_free_key_mints:${op.ipHash}:${op.mintDay}`]
    case 'consume_entitlement':
      return [`${op.table}:${op.entitlementRef}`]
    case 'update_batch_job':
      return [`inference_batch_jobs:${op.jobId}`]
    case 'store_agent_search_cache':
      return [`agent_search_cache_entries:${op.cacheKey}`]
  }
}

/**
 * The fire-safe mirror seam call sites receive: enqueue mirror op(s) for a
 * D1 write that JUST succeeded. Synchronous, never throws, never awaited —
 * a completion is never delayed and never failed by the mirror.
 */
export type InferenceEntitlementsMirror = (
  ops: ReadonlyArray<InferenceEntitlementsMirrorOp>,
) => void

// ---------------------------------------------------------------------------
// Enforcement gate reads (the routed decision surface)
// ---------------------------------------------------------------------------

export type FreeTierUsageRead = Readonly<{
  requestsToday: number
  tokensToday: number
}>

export type FreeUsageStateRead = Readonly<{
  cumulativeFreeUsdMicros: number
  earnedFreeUsdMicros: number
}>

/**
 * The six enforcement reads that decide allow/deny on the inference
 * serving path. Both stores implement this interface; the routing wrapper
 * below serves it per KHALA_SYNC_ENTITLEMENTS_READS.
 */
export type InferenceEntitlementsGateReads = Readonly<{
  freeTierKeyExists: (accountRef: string) => Promise<boolean>
  freeTierUsage: (
    accountRef: string,
    usageDay: string,
  ) => Promise<FreeTierUsageRead>
  freeUsageState: (ownerKey: string) => Promise<FreeUsageStateRead>
  premiumAllowlisted: (ownerKey: string) => Promise<boolean>
  operatorExempt: (ownerKey: string) => Promise<boolean>
  privacyEntitlementExists: (accountRef: string) => Promise<boolean>
}>

const toCount = (value: unknown): number =>
  Math.max(0, Math.trunc(Number(value ?? 0)))

/**
 * The D1 implementations of the gate reads — the SAME statements the gate
 * modules run inline today (single-row primary-key lookups; see
 * inference-free-tier-key.ts / inference-free-allowance.ts /
 * inference-premium-allowlist.ts / inference-operator-exemption.ts /
 * inference-privacy-entitlement.ts). Kept here so the compare/postgres
 * router has a D1 side without a runtime import cycle into those modules.
 */
export const makeD1InferenceEntitlementsGateReads = (
  db: D1Database,
): InferenceEntitlementsGateReads => ({
  freeTierKeyExists: async accountRef => {
    const row = await db
      .prepare(
        `SELECT account_ref FROM inference_free_tier_keys WHERE account_ref = ? LIMIT 1`,
      )
      .bind(accountRef)
      .first<{ account_ref: string }>()
    return row !== null
  },
  freeTierUsage: async (accountRef, usageDay) => {
    const row = await db
      .prepare(
        `SELECT free_request_count, free_total_tokens
           FROM inference_free_tier_usage
          WHERE account_ref = ? AND usage_day = ?
          LIMIT 1`,
      )
      .bind(accountRef, usageDay)
      .first<{ free_request_count: number; free_total_tokens: number }>()
    return {
      requestsToday:
        typeof row?.free_request_count === 'number'
          ? row.free_request_count
          : 0,
      tokensToday:
        typeof row?.free_total_tokens === 'number'
          ? row.free_total_tokens
          : 0,
    }
  },
  freeUsageState: async ownerKey => {
    const tally = await db
      .prepare(
        `SELECT cumulative_free_usd_micros
           FROM inference_free_usage_tally
          WHERE owner_key = ?
          LIMIT 1`,
      )
      .bind(ownerKey)
      .first<{ cumulative_free_usd_micros: number }>()
    const earned = await db
      .prepare(
        `SELECT earned_free_usd_micros
           FROM inference_earned_allowance
          WHERE owner_key = ?
          LIMIT 1`,
      )
      .bind(ownerKey)
      .first<{ earned_free_usd_micros: number }>()
    return {
      cumulativeFreeUsdMicros:
        typeof tally?.cumulative_free_usd_micros === 'number'
          ? tally.cumulative_free_usd_micros
          : 0,
      earnedFreeUsdMicros:
        typeof earned?.earned_free_usd_micros === 'number'
          ? earned.earned_free_usd_micros
          : 0,
    }
  },
  operatorExempt: async ownerKey => {
    const row = await db
      .prepare(
        `SELECT owner_key FROM inference_operator_exemption WHERE owner_key = ? LIMIT 1`,
      )
      .bind(ownerKey)
      .first<{ owner_key: string }>()
    return row !== null
  },
  premiumAllowlisted: async ownerKey => {
    const row = await db
      .prepare(
        `SELECT owner_key FROM inference_premium_allowlist WHERE owner_key = ? LIMIT 1`,
      )
      .bind(ownerKey)
      .first<{ owner_key: string }>()
    return row !== null
  },
  privacyEntitlementExists: async accountRef => {
    const row = await db
      .prepare(
        `SELECT account_ref FROM inference_privacy_entitlements WHERE account_ref = ? LIMIT 1`,
      )
      .bind(accountRef)
      .first<{ account_ref: string }>()
    return row !== null
  },
})

// ---------------------------------------------------------------------------
// Postgres store
// ---------------------------------------------------------------------------

export type PostgresInferenceEntitlementsStore = Readonly<{
  /**
   * Apply mirror op(s), each idempotently (event-gated accruals, DO
   * NOTHING inserts, converge upserts, terminal-state consumes). Ops apply
   * sequentially on one connection; multi-statement ops run in one
   * transaction. Throws on failure — the fire-safe wrapper owns catching.
   */
  applyMirrorOps: (
    ops: ReadonlyArray<InferenceEntitlementsMirrorOp>,
  ) => Promise<void>
  /** The Postgres side of the enforcement gate reads. */
  gateReads: InferenceEntitlementsGateReads
}>

export type MakePostgresInferenceEntitlementsStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the KS-8.1/8.2 stores.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

/**
 * postgres.js / Bun SQL both expose `unsafe(text, params)`; the structural
 * `SyncSql` seam deliberately does not, so this module widens it locally
 * for the registry-driven generic writes (same note as the backfill core).
 */
type UnsafeQuery = (
  text: string,
  params?: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== 'function') {
    throw new Error(
      'inference entitlements mirror requires a driver exposing unsafe(text, params)',
    )
  }
  return unsafe
}

const genericWriteStatement = (
  table: InferenceEntitlementsWriteTable,
  row: MirrorRow,
): Readonly<{ text: string; params: Array<unknown> }> => {
  const spec: WriteTableSpec = ENTITLEMENTS_WRITE_TABLES[table]
  const unknownColumns = Object.keys(row).filter(
    column => !spec.columns.includes(column),
  )
  if (unknownColumns.length > 0) {
    throw new Error(
      `mirror row for ${table} carries unknown column(s): ${unknownColumns.join(', ')}`,
    )
  }
  const params: Array<unknown> = []
  const placeholders = spec.columns.map(column => {
    params.push(row[column] ?? null)
    return `$${params.length}`
  })
  const conflict =
    spec.conflict === undefined
      ? 'ON CONFLICT DO NOTHING'
      : `ON CONFLICT (${spec.conflict.target.join(', ')}) DO UPDATE SET ${spec.conflict.update
          .map(column => `${column} = EXCLUDED.${column}`)
          .join(', ')}`
  return {
    params,
    text: `INSERT INTO ${table} (${spec.columns.join(', ')}) VALUES (${placeholders.join(', ')}) ${conflict}`,
  }
}

export const makePostgresInferenceEntitlementsStore = (
  deps: MakePostgresInferenceEntitlementsStoreDependencies,
): PostgresInferenceEntitlementsStore => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }

  const applyOp = async (
    sql: SyncSql,
    op: InferenceEntitlementsMirrorOp,
  ): Promise<void> => {
    switch (op.kind) {
      case 'write': {
        const statement = genericWriteStatement(op.table, op.row)
        await requireUnsafe(sql)(statement.text, statement.params)
        return
      }
      case 'delete_owner_grant': {
        if (op.table === 'inference_premium_allowlist') {
          await sql`DELETE FROM inference_premium_allowlist WHERE owner_key = ${op.ownerKey}`
        } else {
          await sql`DELETE FROM inference_operator_exemption WHERE owner_key = ${op.ownerKey}`
        }
        return
      }
      case 'accrue_free_tier_usage': {
        // EVENT-GATED tally increment (increment-idempotent): the tally
        // moves ONLY when the unique-keyed event row is FRESH, in one
        // transaction — the same discipline as the D1 batch.
        await sql.begin(async tx => {
          const inserted: Array<{ request_id: string }> = await tx`
            INSERT INTO inference_free_tier_usage_events
              (request_id, account_ref, usage_day, served_model, total_tokens, created_at)
            VALUES
              (${op.event.requestId}, ${op.event.accountRef},
               ${op.event.usageDay}, ${op.event.servedModel},
               ${op.event.totalTokens}, ${op.event.createdAt})
            ON CONFLICT DO NOTHING
            RETURNING request_id`
          if (inserted.length === 0) {
            return
          }
          await tx`
            INSERT INTO inference_free_tier_usage
              (account_ref, usage_day, free_request_count, free_total_tokens, created_at, updated_at)
            VALUES
              (${op.event.accountRef}, ${op.event.usageDay}, 1,
               ${op.event.totalTokens}, ${op.event.createdAt}, ${op.event.createdAt})
            ON CONFLICT (account_ref, usage_day) DO UPDATE SET
              free_request_count = inference_free_tier_usage.free_request_count + 1,
              free_total_tokens = inference_free_tier_usage.free_total_tokens
                + EXCLUDED.free_total_tokens,
              updated_at = EXCLUDED.updated_at`
        })
        return
      }
      case 'accrue_free_usage': {
        await sql.begin(async tx => {
          const inserted: Array<{ request_id: string }> = await tx`
            INSERT INTO inference_free_usage_events
              (request_id, owner_key, account_ref, served_model, free_usd_micros, created_at)
            VALUES
              (${op.event.requestId}, ${op.event.ownerKey},
               ${op.event.accountRef}, ${op.event.servedModel},
               ${op.event.freeUsdMicros}, ${op.event.createdAt})
            ON CONFLICT DO NOTHING
            RETURNING request_id`
          if (inserted.length === 0) {
            return
          }
          await tx`
            INSERT INTO inference_free_usage_tally
              (owner_key, identity_kind, cumulative_free_usd_micros, free_request_count, created_at, updated_at)
            VALUES
              (${op.event.ownerKey}, ${op.identityKind},
               ${op.event.freeUsdMicros}, 1, ${op.event.createdAt},
               ${op.event.createdAt})
            ON CONFLICT (owner_key) DO UPDATE SET
              cumulative_free_usd_micros = inference_free_usage_tally.cumulative_free_usd_micros
                + EXCLUDED.cumulative_free_usd_micros,
              free_request_count = inference_free_usage_tally.free_request_count + 1,
              identity_kind = EXCLUDED.identity_kind,
              updated_at = EXCLUDED.updated_at`
        })
        return
      }
      case 'accrue_earned_allowance': {
        await sql.begin(async tx => {
          const inserted: Array<{ accrual_event_ref: string }> = await tx`
            INSERT INTO inference_earned_allowance_events
              (accrual_event_ref, owner_key, accrual_kind, earned_usd_micros, created_at)
            VALUES
              (${op.event.accrualEventRef}, ${op.event.ownerKey},
               ${op.event.accrualKind}, ${op.event.earnedUsdMicros},
               ${op.event.createdAt})
            ON CONFLICT DO NOTHING
            RETURNING accrual_event_ref`
          if (inserted.length === 0) {
            return
          }
          await tx`
            INSERT INTO inference_earned_allowance
              (owner_key, earned_free_usd_micros, accrual_count, created_at, updated_at)
            VALUES
              (${op.event.ownerKey}, ${op.event.earnedUsdMicros}, 1,
               ${op.event.createdAt}, ${op.event.createdAt})
            ON CONFLICT (owner_key) DO UPDATE SET
              earned_free_usd_micros = inference_earned_allowance.earned_free_usd_micros
                + EXCLUDED.earned_free_usd_micros,
              accrual_count = inference_earned_allowance.accrual_count + 1,
              updated_at = EXCLUDED.updated_at`
        })
        return
      }
      case 'increment_free_key_mint': {
        // The one counter WITHOUT an event key upstream (bounded per-IP
        // abuse guard, not billing enforcement). Mirrored as the same +1
        // upsert D1 runs; a dropped mirror under-counts Postgres only and
        // is converged by the backfill sweep (--verify flags drift).
        await sql`
          INSERT INTO inference_free_key_mints
            (ip_hash, mint_day, mint_count, created_at, updated_at)
          VALUES
            (${op.ipHash}, ${op.mintDay}, 1, ${op.nowIso}, ${op.nowIso})
          ON CONFLICT (ip_hash, mint_day) DO UPDATE SET
            mint_count = inference_free_key_mints.mint_count + 1,
            updated_at = EXCLUDED.updated_at`
        return
      }
      case 'consume_entitlement': {
        // Terminal-state converge: the D1 authority already applied the
        // guarded predicate (active + unexpired + digest match); the
        // mirror records the SAME terminal state keyed by the unique
        // entitlement_ref. Re-delivery writes identical values
        // (idempotent); a missing pre-backfill row no-ops and the
        // backfill converges it.
        if (op.table === 'agent_rate_limit_entitlements') {
          await sql`
            UPDATE agent_rate_limit_entitlements
               SET status = 'consumed', consumed_at = ${op.consumedAt}
             WHERE entitlement_ref = ${op.entitlementRef}`
        } else {
          await sql`
            UPDATE agent_search_entitlements
               SET status = 'consumed', consumed_at = ${op.consumedAt}
             WHERE entitlement_ref = ${op.entitlementRef}`
        }
        return
      }
      case 'update_batch_job': {
        // Same partial update as the D1 store (a missing pre-backfill row
        // no-ops; the backfill converges the full row).
        await sql`
          UPDATE inference_batch_jobs
             SET status = ${op.status},
                 updated_at = ${op.updatedAt},
                 processed_items = COALESCE(${op.processedItems ?? null}, processed_items),
                 failed_items = COALESCE(${op.failedItems ?? null}, failed_items),
                 results_r2_key = COALESCE(${op.resultsR2Key ?? null}, results_r2_key),
                 started_at = COALESCE(${op.startedAt ?? null}, started_at)
           WHERE job_id = ${op.jobId}`
        return
      }
      case 'store_agent_search_cache': {
        // Archive-then-insert in one transaction — the D1 batch parity
        // that keeps the one-active-entry-per-key partial unique index
        // satisfied. The archive matches D1's predicate; the insert is
        // conflict-guarded on the id PK for re-delivery safety.
        const statement = genericWriteStatement_cache(op.row)
        await sql.begin(async tx => {
          await tx`
            UPDATE agent_search_cache_entries
               SET archived_at = ${op.archivedAt}
             WHERE cache_key = ${op.cacheKey}
               AND archived_at IS NULL
               AND id <> ${String(op.row['id'] ?? '')}`
          await requireUnsafe(tx as SyncSql)(statement.text, statement.params)
        })
        return
      }
    }
  }

  return {
    applyMirrorOps: ops =>
      withSql(async sql => {
        for (const op of ops) {
          await applyOp(sql, op)
        }
      }),
    gateReads: {
      freeTierKeyExists: accountRef =>
        withSql(async sql => {
          const rows: Array<{ account_ref: string }> = await sql`
            SELECT account_ref FROM inference_free_tier_keys
             WHERE account_ref = ${accountRef} LIMIT 1`
          return rows.length > 0
        }),
      freeTierUsage: (accountRef, usageDay) =>
        withSql(async sql => {
          const rows: Array<{
            free_request_count: unknown
            free_total_tokens: unknown
          }> = await sql`
            SELECT free_request_count, free_total_tokens
              FROM inference_free_tier_usage
             WHERE account_ref = ${accountRef} AND usage_day = ${usageDay}
             LIMIT 1`
          return {
            requestsToday: toCount(rows[0]?.free_request_count),
            tokensToday: toCount(rows[0]?.free_total_tokens),
          }
        }),
      freeUsageState: ownerKey =>
        withSql(async sql => {
          const tally: Array<{ cumulative_free_usd_micros: unknown }> =
            await sql`
              SELECT cumulative_free_usd_micros
                FROM inference_free_usage_tally
               WHERE owner_key = ${ownerKey} LIMIT 1`
          const earned: Array<{ earned_free_usd_micros: unknown }> =
            await sql`
              SELECT earned_free_usd_micros
                FROM inference_earned_allowance
               WHERE owner_key = ${ownerKey} LIMIT 1`
          return {
            cumulativeFreeUsdMicros: toCount(
              tally[0]?.cumulative_free_usd_micros,
            ),
            earnedFreeUsdMicros: toCount(earned[0]?.earned_free_usd_micros),
          }
        }),
      operatorExempt: ownerKey =>
        withSql(async sql => {
          const rows: Array<{ owner_key: string }> = await sql`
            SELECT owner_key FROM inference_operator_exemption
             WHERE owner_key = ${ownerKey} LIMIT 1`
          return rows.length > 0
        }),
      premiumAllowlisted: ownerKey =>
        withSql(async sql => {
          const rows: Array<{ owner_key: string }> = await sql`
            SELECT owner_key FROM inference_premium_allowlist
             WHERE owner_key = ${ownerKey} LIMIT 1`
          return rows.length > 0
        }),
      privacyEntitlementExists: accountRef =>
        withSql(async sql => {
          const rows: Array<{ account_ref: string }> = await sql`
            SELECT account_ref FROM inference_privacy_entitlements
             WHERE account_ref = ${accountRef} LIMIT 1`
          return rows.length > 0
        }),
    },
  }
}

const genericWriteStatement_cache = (
  row: MirrorRow,
): Readonly<{ text: string; params: Array<unknown> }> => {
  const columns = [
    'id', 'cache_key', 'mode', 'provider', 'results_json', 'result_count',
    'cost_dollars', 'created_at', 'expires_at', 'archived_at',
  ]
  const unknownColumns = Object.keys(row).filter(
    column => !columns.includes(column),
  )
  if (unknownColumns.length > 0) {
    throw new Error(
      `mirror row for agent_search_cache_entries carries unknown column(s): ${unknownColumns.join(', ')}`,
    )
  }
  const params: Array<unknown> = []
  const placeholders = columns.map(column => {
    params.push(row[column] ?? null)
    return `$${params.length}`
  })
  return {
    params,
    text: `INSERT INTO agent_search_cache_entries (${columns.join(', ')}) VALUES (${placeholders.join(', ')}) ON CONFLICT DO NOTHING`,
  }
}

// ---------------------------------------------------------------------------
// Read routing (d1 | compare | postgres-with-D1-fallback)
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

export type MakeRoutedEntitlementsGateReadsDependencies = Readonly<{
  d1: InferenceEntitlementsGateReads
  postgres: InferenceEntitlementsGateReads
  flags: InferenceEntitlementsFlags
  log?: InferenceEntitlementsLog | undefined
  /**
   * Fire-safe scheduler for compare-mode shadow reads (production: leave
   * default — the shadow promise runs detached; tests inject a collector).
   */
  schedule?: ((work: Promise<void>) => void) | undefined
}>

/**
 * Route the enforcement gate reads per KHALA_SYNC_ENTITLEMENTS_READS.
 * NEVER constructed in 'd1' mode (the routing factory returns no
 * gateReads then, so gates run their untouched inline D1 reads).
 *
 * compare — serve D1 immediately; schedule a detached Postgres shadow
 * read + comparison (the §3.6 denial-decision shadow evidence). ZERO
 * blocking latency: the caller's promise resolves on the D1 read alone.
 *
 * postgres — ONE Postgres attempt, then D1 fallback + diagnostic. No
 * retry backoff: these reads gate live completions.
 */
export const makeRoutedEntitlementsGateReads = (
  deps: MakeRoutedEntitlementsGateReadsDependencies,
): InferenceEntitlementsGateReads => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? defaultLog
  const schedule =
    deps.schedule ??
    ((work: Promise<void>) => {
      void work
    })

  const route = async <A>(
    op: string,
    d1Read: () => Promise<A>,
    postgresRead: () => Promise<A>,
  ): Promise<A> => {
    if (flags.reads === 'postgres') {
      try {
        return await postgresRead()
      } catch (error) {
        log('khala_sync_entitlements_postgres_read_fallback', {
          messageSafe: safeMessage(error),
          op,
          refs: [],
        })
        return d1Read()
      }
    }

    // compare: serve D1; shadow-compare off the response path.
    const d1Result = await d1Read()
    schedule(
      postgresRead()
        .then(postgresResult => {
          if (stableStringify(d1Result) !== stableStringify(postgresResult)) {
            log('khala_sync_entitlements_read_compare_mismatch', {
              messageSafe: 'postgres gate read differs from d1 authority',
              op,
              refs: [],
            })
          }
        })
        .catch((error: unknown) => {
          log('khala_sync_entitlements_postgres_read_failed', {
            messageSafe: safeMessage(error),
            op,
            refs: [],
          })
        }),
    )
    return d1Result
  }

  return {
    freeTierKeyExists: accountRef =>
      route(
        'freeTierKeyExists',
        () => d1.freeTierKeyExists(accountRef),
        () => postgres.freeTierKeyExists(accountRef),
      ),
    freeTierUsage: (accountRef, usageDay) =>
      route(
        'freeTierUsage',
        () => d1.freeTierUsage(accountRef, usageDay),
        () => postgres.freeTierUsage(accountRef, usageDay),
      ),
    freeUsageState: ownerKey =>
      route(
        'freeUsageState',
        () => d1.freeUsageState(ownerKey),
        () => postgres.freeUsageState(ownerKey),
      ),
    operatorExempt: ownerKey =>
      route(
        'operatorExempt',
        () => d1.operatorExempt(ownerKey),
        () => postgres.operatorExempt(ownerKey),
      ),
    premiumAllowlisted: ownerKey =>
      route(
        'premiumAllowlisted',
        () => d1.premiumAllowlisted(ownerKey),
        () => postgres.premiumAllowlisted(ownerKey),
      ),
    privacyEntitlementExists: accountRef =>
      route(
        'privacyEntitlementExists',
        () => d1.privacyEntitlementExists(accountRef),
        () => postgres.privacyEntitlementExists(accountRef),
      ),
  }
}

// ---------------------------------------------------------------------------
// Fire-safe mirror
// ---------------------------------------------------------------------------

export type MakeInferenceEntitlementsMirrorDependencies = Readonly<{
  store: Pick<PostgresInferenceEntitlementsStore, 'applyMirrorOps'>
  log?: InferenceEntitlementsLog | undefined
  /** Fire-safe scheduler (production default: detached; tests collect). */
  schedule?: ((work: Promise<void>) => void) | undefined
}>

/**
 * Wrap the Postgres apply as the fire-safe `InferenceEntitlementsMirror`:
 * synchronous enqueue, detached execution, all failures swallowed into
 * the `khala_sync_entitlements_dual_write_failed` drift diagnostic. The
 * caller's write path can NEVER be failed or delayed by the mirror.
 */
export const makeInferenceEntitlementsMirror = (
  deps: MakeInferenceEntitlementsMirrorDependencies,
): InferenceEntitlementsMirror => {
  const log = deps.log ?? defaultLog
  const schedule =
    deps.schedule ??
    ((work: Promise<void>) => {
      void work
    })
  return ops => {
    if (ops.length === 0) {
      return
    }
    try {
      schedule(
        deps.store.applyMirrorOps(ops).catch((error: unknown) => {
          log('khala_sync_entitlements_dual_write_failed', {
            messageSafe: safeMessage(error),
            op: ops.map(op => op.kind).join(','),
            refs: ops.flatMap(mirrorOpRefs),
          })
        }),
      )
    } catch (error) {
      // Even a synchronous scheduler fault must not reach the write path.
      log('khala_sync_entitlements_dual_write_failed', {
        messageSafe: safeMessage(error),
        op: ops.map(op => op.kind).join(','),
        refs: ops.flatMap(mirrorOpRefs),
      })
    }
  }
}

// ---------------------------------------------------------------------------
// Env factories (the index.ts / route-module drop-ins)
// ---------------------------------------------------------------------------

export type InferenceEntitlementsStoreEnv = InferenceEntitlementsFlagEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeInferenceEntitlementsRoutingOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: InferenceEntitlementsLog | undefined
  schedule?: ((work: Promise<void>) => void) | undefined
}>

export type InferenceEntitlementsRouting = Readonly<{
  flags: InferenceEntitlementsFlags
  /** Fire-safe dual-write mirror (no-op function when dual-write is off). */
  mirror: InferenceEntitlementsMirror
  /**
   * Routed enforcement reads — present ONLY when reads != 'd1'. In the
   * default 'd1' mode this is undefined and the gates run their untouched
   * inline D1 reads (zero added hot-path latency).
   */
  gateReads: InferenceEntitlementsGateReads | undefined
}>

const postgresStoreForEnv = (
  env: InferenceEntitlementsStoreEnv,
  options: MakeInferenceEntitlementsRoutingOptions,
): PostgresInferenceEntitlementsStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresInferenceEntitlementsStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

/**
 * The production KS-8.9 seam factory. Returns undefined when the domain
 * runs plainly on D1 (no KHALA_SYNC_DB binding, or dual-write off AND
 * reads on 'd1') — call sites then pass nothing and every module keeps its
 * byte-identical D1 behavior.
 */
export const makeInferenceEntitlementsRoutingForEnv = (
  env: InferenceEntitlementsStoreEnv,
  options: MakeInferenceEntitlementsRoutingOptions = {},
): InferenceEntitlementsRouting | undefined => {
  const flags = inferenceEntitlementsFlagsFromEnv(env)
  if (!flags.dualWrite && flags.reads === 'd1') {
    return undefined
  }
  const postgres = postgresStoreForEnv(env, options)
  if (postgres === undefined) {
    return undefined
  }
  const log = options.log ?? defaultLog

  return {
    flags,
    gateReads:
      flags.reads === 'd1'
        ? undefined
        : makeRoutedEntitlementsGateReads({
            d1: makeD1InferenceEntitlementsGateReads(openAgentsDatabase(env)),
            flags,
            log,
            postgres: postgres.gateReads,
            schedule: options.schedule,
          }),
    mirror: flags.dualWrite
      ? makeInferenceEntitlementsMirror({
          log,
          schedule: options.schedule,
          store: postgres,
        })
      : () => {},
  }
}

/**
 * Convenience for call sites that only mirror writes (store factories in
 * route modules): the fire-safe mirror, or undefined when dual-write is
 * off / unbound (callers pass it straight into the optional store dep).
 */
export const inferenceEntitlementsMirrorForEnv = (
  env: InferenceEntitlementsStoreEnv,
  options: MakeInferenceEntitlementsRoutingOptions = {},
): InferenceEntitlementsMirror | undefined => {
  const flags = inferenceEntitlementsFlagsFromEnv(env)
  if (!flags.dualWrite) {
    return undefined
  }
  const postgres = postgresStoreForEnv(env, options)
  if (postgres === undefined) {
    return undefined
  }
  return makeInferenceEntitlementsMirror({
    log: options.log,
    schedule: options.schedule,
    store: postgres,
  })
}
