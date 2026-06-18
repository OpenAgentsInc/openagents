import { Schema as S } from 'effect'

import { type DebtReceiptKey, deriveDebtReceiptKey } from './debt-receipt-key'
import {
  type DebtReceiptSettlementInput,
  type DebtReceiptSettlementProjection,
  projectDebtReceiptSettlement,
} from './debt-receipt-policy'
import { parseJsonStringArray, parseJsonUnknown } from './json-boundary'

/**
 * Durable, payable debt-receipt store for the funded hygiene lane (EPIC #5335,
 * #5372). This is the lane settlement process step 1 made durable: the
 * requester / settlement-authority turns a discovered, merged+reviewed debt
 * into a FUNDED, PAYABLE receipt, persisted exactly once per `DebtReceiptKey`
 * (#5340).
 *
 * Design:
 *   - The canonical record is the full, public-safe `DebtReceiptSettlementInput`
 *     (the refs that produced a `payable` projection), stored as JSON so the
 *     projection re-derives faithfully on read via `projectDebtReceiptSettlement`.
 *     The policy module re-validates ref safety on every reprojection, so the
 *     store never trusts itself to keep secrets out.
 *   - The structured columns called out in #5335 (debtReceiptRef, repoBaselineRef,
 *     scopeDigest, objectiveDigest, mergedPrRef, reviewerAcceptanceRef,
 *     baseline/target metric refs, budgetCapSats, verifier-command ref,
 *     settlement-authority ref) are denormalized for query/audit only; the JSON
 *     input is the source of truth.
 *   - ONE row per `DebtReceiptKey`. A `payable` row resolves to a payable
 *     projection. A `retired` row re-projects with the key added to the retired
 *     set, so it resolves to `duplicate_replay` (a second settle is rejected as
 *     a duplicate, never re-paid).
 *
 * Fail-closed everywhere: no row -> not payable; a retired row -> duplicate
 * replay; a reprojected state that is not `payable` -> the create endpoint
 * refused it, so it can never have been stored.
 */

export const HygieneDebtReceiptState = S.Literals(['payable', 'retired'])
export type HygieneDebtReceiptState = typeof HygieneDebtReceiptState.Type

// The public-safe, durable create input. Every field is a public-safe ref or a
// bounded integer; the policy module enforces ref safety on reprojection.
export type HygieneDebtReceiptCreateInput = Readonly<{
  // The full debt-receipt policy input (refs + budget). MUST project to a
  // `payable` state, or the create endpoint refuses it.
  settlementInput: DebtReceiptSettlementInput
  // The merged-PR ref this receipt funds (denormalized for query/audit).
  mergedPrRef: string
  // The reviewer acceptance ref (denormalized for query/audit).
  reviewerAcceptanceRef: string
  nowIso: string
}>

export type HygieneDebtReceiptRecord = Readonly<{
  debtReceiptKey: DebtReceiptKey
  state: HygieneDebtReceiptState
  // Denormalized, queryable, public-safe ref columns (audit/query only).
  debtReceiptRef: string
  repoBaselineRef: string
  scopeDigest: string
  objectiveDigest: string
  mergedPrRef: string
  reviewerAcceptanceRef: string
  baselineMetricRefs: ReadonlyArray<string>
  targetMetricRefs: ReadonlyArray<string>
  verificationCommandRefs: ReadonlyArray<string>
  settlementAuthorityActorRef: string | null
  budgetCapSats: number
  payableSats: number
  // The canonical full input (JSON-serialized for D1), the projection authority.
  settlementInput: DebtReceiptSettlementInput
  createdAt: string
  updatedAt: string
  retiredAt: string | null
  // The settlement receipt ref that retired this key (when retired).
  settlementReceiptRef: string | null
}>

export type HygieneDebtReceiptCreateResult =
  | Readonly<{ kind: 'created'; record: HygieneDebtReceiptRecord }>
  | Readonly<{ kind: 'already_payable'; record: HygieneDebtReceiptRecord }>
  | Readonly<{ kind: 'retired'; record: HygieneDebtReceiptRecord }>

export type HygieneDebtReceiptStore = Readonly<{
  // Idempotent on `DebtReceiptKey`. Persists a payable receipt. Returns a typed
  // result so the route can distinguish created / already-payable / retired
  // (a retired key cannot be re-created — that is a duplicate replay).
  create: (
    input: HygieneDebtReceiptCreateInput,
  ) => Promise<HygieneDebtReceiptCreateResult>
  // Reads the durable record by key, or undefined when absent.
  read: (
    debtReceiptKeyRef: string,
  ) => Promise<HygieneDebtReceiptRecord | undefined>
  // Reprojects the durable record into a settlement projection (the source of
  // truth the settle route consumes). A `retired` row reprojects to
  // `duplicate_replay`. Undefined when no row exists.
  resolveProjection: (
    debtReceiptKeyRef: string,
  ) => Promise<DebtReceiptSettlementProjection | undefined>
  // Marks a payable key retired once it has settled, recording the settlement
  // receipt ref. A no-op (returns the existing row) when already retired so a
  // settle retry stays idempotent. Returns undefined when the key does not
  // exist.
  markRetired: (
    debtReceiptKeyRef: string,
    settlementReceiptRef: string,
    nowIso: string,
  ) => Promise<HygieneDebtReceiptRecord | undefined>
}>

export class HygieneDebtReceiptStoreError extends S.TaggedErrorClass<HygieneDebtReceiptStoreError>()(
  'HygieneDebtReceiptStoreError',
  {
    kind: S.Literals([
      'not_payable',
      'conflict',
      'storage_error',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

/**
 * Build the durable record from a create input, AFTER confirming the input
 * projects to a `payable` state. The reprojection both validates ref safety and
 * proves payability; a non-payable input is refused (never stored). Pure.
 */
export const buildHygieneDebtReceiptRecord = (
  input: HygieneDebtReceiptCreateInput,
): HygieneDebtReceiptRecord => {
  const { settlementInput } = input

  if (settlementInput.debtReceiptKeyInput === undefined) {
    throw new HygieneDebtReceiptStoreError({
      kind: 'validation_error',
      reason: 'hygiene_debt_receipt_create:debt_receipt_key_input_missing',
    })
  }

  // Reproject: this validates every ref is public-safe (policy throws
  // `DebtReceiptPolicyUnsafe` otherwise) AND computes the typed key.
  const projection = projectDebtReceiptSettlement(settlementInput)

  if (projection.debtReceiptKey === null) {
    throw new HygieneDebtReceiptStoreError({
      kind: 'validation_error',
      reason: 'hygiene_debt_receipt_create:debt_receipt_key_unresolved',
    })
  }

  // A funded, payable receipt is the unit. Anything that does not reach the
  // `payable` state (blocked / fundable / funded / verified / credit_class /
  // retired / duplicate_replay / quarantined) is NOT a payable receipt and is
  // refused.
  if (projection.state !== 'payable') {
    throw new HygieneDebtReceiptStoreError({
      kind: 'not_payable',
      reason: `hygiene_debt_receipt_create:not_payable:${projection.state}`,
    })
  }

  const settlementAuthorityActorRef =
    settlementInput.settlementAuthorityActorRef === undefined ||
    settlementInput.settlementAuthorityActorRef === null
      ? null
      : settlementInput.settlementAuthorityActorRef

  return {
    baselineMetricRefs: projection.baselineMetricRefs,
    budgetCapSats: projection.budgetCapSats,
    createdAt: input.nowIso,
    debtReceiptKey: deriveDebtReceiptKey(settlementInput.debtReceiptKeyInput),
    debtReceiptRef: settlementInput.debtReceiptKeyInput.debtReceiptRef,
    mergedPrRef: input.mergedPrRef,
    objectiveDigest: settlementInput.debtReceiptKeyInput.objectiveDigest,
    payableSats: projection.payableSats,
    repoBaselineRef: settlementInput.debtReceiptKeyInput.repoBaselineRef,
    retiredAt: null,
    reviewerAcceptanceRef: input.reviewerAcceptanceRef,
    scopeDigest: settlementInput.debtReceiptKeyInput.scopeDigest,
    settlementAuthorityActorRef,
    settlementInput,
    settlementReceiptRef: null,
    state: 'payable',
    targetMetricRefs: projection.targetMetricRefs,
    updatedAt: input.nowIso,
    verificationCommandRefs: projection.verificationCommandRefs,
  }
}

/**
 * Reproject a durable record into the settlement projection the settle route
 * consumes. A `retired` row re-projects with its OWN key added to the retired
 * set, so the policy yields `duplicate_replay` (state + duplicateReplay:true) —
 * exactly what makes a second settle a duplicate replay, never re-paid.
 */
export const reprojectHygieneDebtReceipt = (
  record: HygieneDebtReceiptRecord,
): DebtReceiptSettlementProjection =>
  projectDebtReceiptSettlement(
    record.state === 'retired'
      ? {
          ...record.settlementInput,
          retiredDebtReceiptKeys: [
            ...(record.settlementInput.retiredDebtReceiptKeys ?? []),
            record.debtReceiptKey,
          ],
        }
      : record.settlementInput,
  )

// ---------------------------------------------------------------------------
// In-memory store (tests / fixtures). Same contract as the D1 store.
// ---------------------------------------------------------------------------

export const makeInMemoryHygieneDebtReceiptStore = (): HygieneDebtReceiptStore & {
  readonly rows: Map<string, HygieneDebtReceiptRecord>
} => {
  const rows = new Map<string, HygieneDebtReceiptRecord>()

  return {
    create: async input => {
      const record = buildHygieneDebtReceiptRecord(input)
      const existing = rows.get(record.debtReceiptKey)

      if (existing !== undefined) {
        return existing.state === 'retired'
          ? { kind: 'retired', record: existing }
          : { kind: 'already_payable', record: existing }
      }

      rows.set(record.debtReceiptKey, record)

      return { kind: 'created', record }
    },
    markRetired: async (debtReceiptKeyRef, settlementReceiptRef, nowIso) => {
      const existing = rows.get(debtReceiptKeyRef)

      if (existing === undefined) {
        return undefined
      }

      if (existing.state === 'retired') {
        return existing
      }

      const retired: HygieneDebtReceiptRecord = {
        ...existing,
        retiredAt: nowIso,
        settlementReceiptRef,
        state: 'retired',
        updatedAt: nowIso,
      }
      rows.set(debtReceiptKeyRef, retired)

      return retired
    },
    read: async debtReceiptKeyRef => rows.get(debtReceiptKeyRef),
    resolveProjection: async debtReceiptKeyRef => {
      const record = rows.get(debtReceiptKeyRef)

      return record === undefined
        ? undefined
        : reprojectHygieneDebtReceipt(record)
    },
    rows,
  }
}

// ---------------------------------------------------------------------------
// D1 store. Persists the full input as JSON + denormalized query columns.
// ---------------------------------------------------------------------------

type D1HygieneDebtReceiptRow = {
  debt_receipt_key: string
  state: string
  debt_receipt_ref: string
  repo_baseline_ref: string
  scope_digest: string
  objective_digest: string
  merged_pr_ref: string
  reviewer_acceptance_ref: string
  baseline_metric_refs_json: string
  target_metric_refs_json: string
  verification_command_refs_json: string
  settlement_authority_actor_ref: string | null
  budget_cap_sats: number
  payable_sats: number
  settlement_input_json: string
  created_at: string
  updated_at: string
  retired_at: string | null
  settlement_receipt_ref: string | null
}

const rowToRecord = (
  row: D1HygieneDebtReceiptRow,
): HygieneDebtReceiptRecord => ({
  baselineMetricRefs: parseJsonStringArray(row.baseline_metric_refs_json),
  budgetCapSats: row.budget_cap_sats,
  createdAt: row.created_at,
  debtReceiptKey: row.debt_receipt_key as DebtReceiptKey,
  debtReceiptRef: row.debt_receipt_ref,
  mergedPrRef: row.merged_pr_ref,
  objectiveDigest: row.objective_digest,
  payableSats: row.payable_sats,
  repoBaselineRef: row.repo_baseline_ref,
  retiredAt: row.retired_at,
  reviewerAcceptanceRef: row.reviewer_acceptance_ref,
  scopeDigest: row.scope_digest,
  settlementAuthorityActorRef: row.settlement_authority_actor_ref,
  // We serialized this object ourselves from a policy-validated input; the
  // reprojection on read re-validates every ref via the debt-receipt policy.
  settlementInput: parseJsonUnknown(
    row.settlement_input_json,
  ) as DebtReceiptSettlementInput,
  settlementReceiptRef: row.settlement_receipt_ref,
  state: row.state === 'retired' ? 'retired' : 'payable',
  targetMetricRefs: parseJsonStringArray(row.target_metric_refs_json),
  updatedAt: row.updated_at,
  verificationCommandRefs: parseJsonStringArray(
    row.verification_command_refs_json,
  ),
})

const readRow = async (
  db: D1Database,
  debtReceiptKeyRef: string,
): Promise<HygieneDebtReceiptRecord | undefined> => {
  const row = await db
    .prepare(
      `SELECT debt_receipt_key, state, debt_receipt_ref, repo_baseline_ref,
              scope_digest, objective_digest, merged_pr_ref,
              reviewer_acceptance_ref, baseline_metric_refs_json,
              target_metric_refs_json, verification_command_refs_json,
              settlement_authority_actor_ref, budget_cap_sats, payable_sats,
              settlement_input_json, created_at, updated_at, retired_at,
              settlement_receipt_ref
         FROM hygiene_debt_receipts
        WHERE debt_receipt_key = ?
        LIMIT 1`,
    )
    .bind(debtReceiptKeyRef)
    .first<D1HygieneDebtReceiptRow>()

  return row === null ? undefined : rowToRecord(row)
}

export const makeD1HygieneDebtReceiptStore = (
  db: D1Database,
): HygieneDebtReceiptStore => ({
  create: async input => {
    const record = buildHygieneDebtReceiptRecord(input)
    const existing = await readRow(db, record.debtReceiptKey)

    if (existing !== undefined) {
      return existing.state === 'retired'
        ? { kind: 'retired', record: existing }
        : { kind: 'already_payable', record: existing }
    }

    // INSERT OR IGNORE guards against a racing concurrent create on the same
    // key: the second insert is ignored and we re-read the winning row.
    await db
      .prepare(
        `INSERT OR IGNORE INTO hygiene_debt_receipts
           (debt_receipt_key, state, debt_receipt_ref, repo_baseline_ref,
            scope_digest, objective_digest, merged_pr_ref,
            reviewer_acceptance_ref, baseline_metric_refs_json,
            target_metric_refs_json, verification_command_refs_json,
            settlement_authority_actor_ref, budget_cap_sats, payable_sats,
            settlement_input_json, created_at, updated_at, retired_at,
            settlement_receipt_ref)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL)`,
      )
      .bind(
        record.debtReceiptKey,
        record.state,
        record.debtReceiptRef,
        record.repoBaselineRef,
        record.scopeDigest,
        record.objectiveDigest,
        record.mergedPrRef,
        record.reviewerAcceptanceRef,
        JSON.stringify(record.baselineMetricRefs),
        JSON.stringify(record.targetMetricRefs),
        JSON.stringify(record.verificationCommandRefs),
        record.settlementAuthorityActorRef,
        record.budgetCapSats,
        record.payableSats,
        JSON.stringify(record.settlementInput),
        record.createdAt,
        record.updatedAt,
      )
      .run()

    const stored = await readRow(db, record.debtReceiptKey)

    if (stored === undefined) {
      throw new HygieneDebtReceiptStoreError({
        kind: 'storage_error',
        reason: 'hygiene_debt_receipt_create:insert_then_read_missing',
      })
    }

    // If the row already existed (race / prior create), report it honestly.
    return stored.state === 'retired'
      ? { kind: 'retired', record: stored }
      : stored.createdAt === record.createdAt
        ? { kind: 'created', record: stored }
        : { kind: 'already_payable', record: stored }
  },
  markRetired: async (debtReceiptKeyRef, settlementReceiptRef, nowIso) => {
    const existing = await readRow(db, debtReceiptKeyRef)

    if (existing === undefined) {
      return undefined
    }

    if (existing.state === 'retired') {
      return existing
    }

    // Only a payable row transitions to retired. The WHERE clause keeps this
    // idempotent under concurrent settle retries.
    await db
      .prepare(
        `UPDATE hygiene_debt_receipts
            SET state = 'retired',
                retired_at = ?,
                settlement_receipt_ref = ?,
                updated_at = ?
          WHERE debt_receipt_key = ?
            AND state = 'payable'`,
      )
      .bind(nowIso, settlementReceiptRef, nowIso, debtReceiptKeyRef)
      .run()

    return readRow(db, debtReceiptKeyRef)
  },
  read: async debtReceiptKeyRef => readRow(db, debtReceiptKeyRef),
  resolveProjection: async debtReceiptKeyRef => {
    const record = await readRow(db, debtReceiptKeyRef)

    return record === undefined
      ? undefined
      : reprojectHygieneDebtReceipt(record)
  },
})
