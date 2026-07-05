import {
  billingPostgresRawQueryForEnv,
  billingSyncFlagsFromEnv,
  type BillingSyncEnv,
  type MakeBillingStoreOptions,
} from './billing-store'
import { parseInferenceChargeContextRef } from './inference/inference-charge-context'
import { logWorkerRouteWarning } from './observability'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export type InferenceReceiptKind =
  | 'charge'
  | 'free_allowance'
  | 'usd_credit_grant'

export type InferenceReceiptRecord = Readonly<{
  contextRef: string | null
  createdAt: string
  payInType: string
  receiptRef: string
  state: string
  stateChangedAt: string
}>

export type PublicInferenceReceiptModelEvidence = Readonly<{
  requested_model?: string
  served_model: string
  supply_lane: string
  total_tokens: number
  worker: string
}>

export type PublicInferenceReceiptProjection = Readonly<{
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  generatedAt: string
  kind: InferenceReceiptKind
  ledgerState: 'paid' | 'free_allowance'
  modelEvidence?: PublicInferenceReceiptModelEvidence
  receiptRef: string
  schemaVersion: 'openagents.inference.receipt.v1'
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
  stateChangedAt: string
}>

export type InferenceReceiptReadStore = Readonly<{
  readInferenceReceiptByRef: (
    receiptRef: string,
  ) => Promise<InferenceReceiptRecord | null>
}>

export type InferenceReceiptStore = InferenceReceiptReadStore &
  Readonly<{
    listRecentInferenceReceipts: (
      limit: number,
    ) => Promise<ReadonlyArray<InferenceReceiptRecord>>
  }>

const unsafePublicReceiptPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer\s+|bolt11|cookie|cs_(?:live|test)_|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|idempotency|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|stripe|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

const supplyLaneForAdapterId = (adapterId: string): string => {
  switch (adapterId) {
    case 'fireworks':
      return 'fireworks'
    case 'hydralisk-vllm-glm-5p2-reap-504b':
    case 'hydralisk-vllm':
    case 'hydralisk-vllm-gpt-oss-120b':
      return 'hydralisk'
    case 'openagents-network':
      return 'openagents-network'
    case 'vertex-anthropic':
      return 'vertex-anthropic'
    case 'vertex-gemini':
      return 'vertex-gemini'
    default:
      return adapterId
  }
}

const modelEvidenceFromContextRef = (
  contextRef: string | null,
): PublicInferenceReceiptModelEvidence | undefined => {
  if (contextRef === null) {
    return undefined
  }

  const context = parseInferenceChargeContextRef(contextRef)
  if (context === undefined) {
    return undefined
  }

  return {
    ...(context.requestedModel === undefined
      ? {}
      : { requested_model: context.requestedModel }),
    served_model: context.servedModel,
    supply_lane: supplyLaneForAdapterId(context.adapterId),
    total_tokens: context.totalTokens,
    worker: context.adapterId,
  }
}

const kindForRecord = (
  record: InferenceReceiptRecord,
): InferenceReceiptKind | null => {
  if (
    record.receiptRef.startsWith('receipt.inference.charge.') &&
    record.payInType === 'adjustment'
  ) {
    return 'charge'
  }

  if (
    record.receiptRef.startsWith('receipt.inference.free.') &&
    record.payInType === 'free_allowance'
  ) {
    return 'free_allowance'
  }

  if (
    record.receiptRef.startsWith('receipt.inference.usd_credit_grant.') &&
    record.payInType === 'usd_credit_grant'
  ) {
    return 'usd_credit_grant'
  }

  return null
}

export const isPublicSafeInferenceReceiptProjection = (
  value: unknown,
): boolean => !unsafePublicReceiptPattern.test(JSON.stringify(value))

export const publicInferenceReceiptFromRecord = (
  record: InferenceReceiptRecord,
  generatedAt: string,
): PublicInferenceReceiptProjection | null => {
  const kind = kindForRecord(record)

  if (kind === null || record.state !== 'paid') {
    return null
  }

  const modelEvidence =
    kind === 'charge' ? modelEvidenceFromContextRef(record.contextRef) : undefined

  const receipt: PublicInferenceReceiptProjection = {
    authorityBoundary:
      'Public proof only. This receipt read grants no spend, refund, payout, checkout, settlement, provider, or registry authority.',
    caveatRefs: [
      'caveat.public.no_private_payment_material',
      'caveat.public.no_account_or_amount_projection',
      'caveat.public.inference_ledger_receipt_exists_only',
    ],
    generatedAt,
    kind,
    ledgerState: kind === 'free_allowance' ? 'free_allowance' : 'paid',
    ...(modelEvidence === undefined ? {} : { modelEvidence }),
    receiptRef: record.receiptRef,
    schemaVersion: 'openagents.inference.receipt.v1',
    sourceRefs: [
      `route:/api/public/inference/receipts/${record.receiptRef}`,
      kind === 'free_allowance'
        ? 'ledger.inference_free_usage_events.request_id'
        : `ledger.pay_ins.public_receipt_ref.${kind}`,
    ],
    staleness: liveAtReadStaleness(['pay_ins.public_receipt_ref']),
    stateChangedAt: record.stateChangedAt,
  }

  return isPublicSafeInferenceReceiptProjection(receipt) ? receipt : null
}

type InferenceReceiptRow = Readonly<{
  context_ref: string | null
  created_at: string
  pay_in_type: string
  public_receipt_ref: string | null
  state: string
  state_changed_at: string
}>

type FreeInferenceReceiptRow = Readonly<{
  created_at: string
  request_id: string
}>

const rowToInferenceReceiptRecord = (
  row: InferenceReceiptRow,
): InferenceReceiptRecord | null =>
  row.public_receipt_ref === null
    ? null
    : {
        contextRef: row.context_ref,
        createdAt: row.created_at,
        payInType: row.pay_in_type,
        receiptRef: row.public_receipt_ref,
        state: row.state,
        stateChangedAt: row.state_changed_at,
      }

const freeRowToInferenceReceiptRecord = (
  row: FreeInferenceReceiptRow,
): InferenceReceiptRecord => ({
  contextRef: null,
  createdAt: row.created_at,
  payInType: 'free_allowance',
  receiptRef: `receipt.inference.free.${row.request_id}`,
  state: 'paid',
  stateChangedAt: row.created_at,
})

const freeRequestIdFromReceiptRef = (receiptRef: string): string | null => {
  const prefix = 'receipt.inference.free.'
  return receiptRef.startsWith(prefix) && receiptRef.length > prefix.length
    ? receiptRef.slice(prefix.length)
    : null
}

const INFERENCE_RECEIPT_LISTING_LIMIT_MAX = 200

export const makeD1InferenceReceiptStore = (
  db: D1Database,
): InferenceReceiptStore => ({
  listRecentInferenceReceipts: async limit => {
    const rowLimit = Math.max(
      1,
      Math.min(INFERENCE_RECEIPT_LISTING_LIMIT_MAX, Math.trunc(limit)),
    )
    const rows = await db
      .prepare(
        `SELECT pay_in_type, state, public_receipt_ref, context_ref, created_at, state_changed_at
           FROM pay_ins
          WHERE public_receipt_ref LIKE 'receipt.inference.charge.%'
            AND pay_in_type = 'adjustment'
            AND state = 'paid'
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(rowLimit)
      .all<InferenceReceiptRow>()

    return (rows.results ?? [])
      .map(rowToInferenceReceiptRecord)
      .filter((record): record is InferenceReceiptRecord => record !== null)
  },
  readInferenceReceiptByRef: async receiptRef => {
    const row = await db
      .prepare(
        `SELECT pay_in_type, state, public_receipt_ref, context_ref, created_at, state_changed_at
           FROM pay_ins
          WHERE public_receipt_ref = ?
            AND pay_in_type IN ('adjustment', 'usd_credit_grant')
          LIMIT 1`,
      )
      .bind(receiptRef)
      .first<InferenceReceiptRow>()

    const payInRecord = row === null ? null : rowToInferenceReceiptRecord(row)
    if (payInRecord !== null) {
      return payInRecord
    }

    const requestId = freeRequestIdFromReceiptRef(receiptRef)
    if (requestId === null) {
      return null
    }

    const freeRow = await db
      .prepare(
        `SELECT request_id, created_at
           FROM inference_free_usage_events
          WHERE request_id = ?
          LIMIT 1`,
      )
      .bind(requestId)
      .first<FreeInferenceReceiptRow>()

    return freeRow === null ? null : freeRowToInferenceReceiptRecord(freeRow)
  },
})

/**
 * #8337: this receipt is scoped to `pay_in_type IN ('adjustment',
 * 'usd_credit_grant')` and an immutable `public_receipt_ref` — an
 * already-settled, non-decision-critical read — so `pay_ins` is eligible
 * for the bounded Postgres-served read allowlist
 * (`BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES`, billing-store.ts). The
 * free-allowance branch (`inference_free_usage_events`) is a DIFFERENT
 * domain's table with no live Postgres mirror in this lane, so a
 * free-shaped ref is intentionally NOT servable here — see
 * `InferenceReceiptPostgresNotServableError` below; the router always
 * falls back to D1 for that shape.
 */
export class InferenceReceiptPostgresNotServableError extends Error {}

export const makePostgresInferenceReceiptStore = (
  query: (
    text: string,
    params: ReadonlyArray<unknown>,
  ) => Promise<ReadonlyArray<Readonly<Record<string, unknown>>>>,
): InferenceReceiptStore => {
  const rowFromPostgres = (
    row: Readonly<Record<string, unknown>>,
  ): InferenceReceiptRow => ({
    context_ref: row['context_ref'] === null ? null : String(row['context_ref']),
    created_at: String(row['created_at']),
    pay_in_type: String(row['pay_in_type']),
    public_receipt_ref:
      row['public_receipt_ref'] === null
        ? null
        : String(row['public_receipt_ref']),
    state: String(row['state']),
    state_changed_at: String(row['state_changed_at']),
  })

  return {
    listRecentInferenceReceipts: async limit => {
      const rowLimit = Math.max(
        1,
        Math.min(INFERENCE_RECEIPT_LISTING_LIMIT_MAX, Math.trunc(limit)),
      )
      const rows = await query(
        `SELECT pay_in_type, state, public_receipt_ref, context_ref, created_at, state_changed_at
           FROM pay_ins
          WHERE public_receipt_ref LIKE 'receipt.inference.charge.%'
            AND pay_in_type = 'adjustment'
            AND state = 'paid'
          ORDER BY created_at DESC
          LIMIT $1`,
        [rowLimit],
      )
      return rows
        .map(row => rowToInferenceReceiptRecord(rowFromPostgres(row)))
        .filter((record): record is InferenceReceiptRecord => record !== null)
    },
    readInferenceReceiptByRef: async receiptRef => {
      if (freeRequestIdFromReceiptRef(receiptRef) !== null) {
        throw new InferenceReceiptPostgresNotServableError(
          'free-allowance receipt refs read a different domain\'s table (inference_free_usage_events) with no live Postgres mirror in this lane',
        )
      }
      const rows = await query(
        `SELECT pay_in_type, state, public_receipt_ref, context_ref, created_at, state_changed_at
           FROM pay_ins
          WHERE public_receipt_ref = $1
            AND pay_in_type IN ('adjustment', 'usd_credit_grant')
          LIMIT 1`,
        [receiptRef],
      )
      const row = rows[0]
      return row === undefined
        ? null
        : rowToInferenceReceiptRecord(rowFromPostgres(row))
    },
  }
}

/**
 * The #8337 KHALA_SYNC_BILLING_READS router for this receipt store, the
 * same compare/postgres semantics as
 * `makeReadsRoutedStripeCheckoutReceiptStore` (stripe-checkout-receipts.ts):
 *   'compare'  — read both, SERVE D1, log any divergence
 *   'postgres' — serve Postgres (single attempt), D1 fallback on failure
 *                (including the free-allowance-ref
 *                `InferenceReceiptPostgresNotServableError` case)
 */
export const makeReadsRoutedInferenceReceiptStore = (input: {
  d1: InferenceReceiptStore
  postgres: InferenceReceiptStore
  reads: 'compare' | 'postgres'
  log: (
    event:
      | 'khala_sync_billing_read_compare_mismatch'
      | 'khala_sync_billing_postgres_read_failed'
      | 'khala_sync_billing_postgres_read_fallback',
    fields: Readonly<{ messageSafe: string; op: string; refs: ReadonlyArray<string> }>,
  ) => void
}): InferenceReceiptStore => ({
  listRecentInferenceReceipts: async limit => {
    if (input.reads === 'postgres') {
      try {
        return await input.postgres.listRecentInferenceReceipts(limit)
      } catch (error) {
        input.log('khala_sync_billing_postgres_read_fallback', {
          messageSafe: error instanceof Error ? error.message : String(error),
          op: 'listRecentInferenceReceipts',
          refs: [],
        })
      }
    }

    const d1Result = await input.d1.listRecentInferenceReceipts(limit)

    if (input.reads === 'compare') {
      try {
        const postgresResult =
          await input.postgres.listRecentInferenceReceipts(limit)
        if (JSON.stringify(postgresResult) !== JSON.stringify(d1Result)) {
          input.log('khala_sync_billing_read_compare_mismatch', {
            messageSafe: `recent inference receipts differ: d1=${d1Result.length} postgres=${postgresResult.length} rows`,
            op: 'listRecentInferenceReceipts',
            refs: [],
          })
        }
      } catch (error) {
        input.log('khala_sync_billing_postgres_read_failed', {
          messageSafe: error instanceof Error ? error.message : String(error),
          op: 'listRecentInferenceReceipts',
          refs: [],
        })
      }
    }

    return d1Result
  },
  readInferenceReceiptByRef: async receiptRef => {
    if (input.reads === 'postgres') {
      try {
        return await input.postgres.readInferenceReceiptByRef(receiptRef)
      } catch (error) {
        input.log('khala_sync_billing_postgres_read_fallback', {
          messageSafe: error instanceof Error ? error.message : String(error),
          op: 'readInferenceReceiptByRef',
          refs: [receiptRef],
        })
      }
    }

    const d1Result = await input.d1.readInferenceReceiptByRef(receiptRef)

    if (input.reads === 'compare') {
      try {
        const postgresResult =
          await input.postgres.readInferenceReceiptByRef(receiptRef)
        if (JSON.stringify(postgresResult) !== JSON.stringify(d1Result)) {
          input.log('khala_sync_billing_read_compare_mismatch', {
            messageSafe: 'inference receipt differs between d1 and postgres',
            op: 'readInferenceReceiptByRef',
            refs: [receiptRef],
          })
        }
      } catch (error) {
        input.log('khala_sync_billing_postgres_read_failed', {
          messageSafe: error instanceof Error ? error.message : String(error),
          op: 'readInferenceReceiptByRef',
          refs: [receiptRef],
        })
      }
    }

    return d1Result
  },
})

/**
 * The env-wiring drop-in for the three call sites that build
 * `makeD1InferenceReceiptStore` (index.ts: the public inference receipt
 * route, the hosted Gemini promise readiness route, and the public
 * activity timeline): builds the D1 store exactly as before, then — only
 * when `KHALA_SYNC_DB` is bound and `KHALA_SYNC_BILLING_READS !== 'd1'` —
 * wraps it with the #8337 compare/postgres router. Degrades to the plain
 * D1 store whenever the binding is absent or the flag is 'd1'.
 */
export const inferenceReceiptStoreForEnv = (
  env: BillingSyncEnv,
  d1Store: InferenceReceiptStore,
  options: MakeBillingStoreOptions = {},
): InferenceReceiptStore => {
  const flags = billingSyncFlagsFromEnv(env)
  if (flags.reads === 'd1') {
    return d1Store
  }
  const query = billingPostgresRawQueryForEnv(env, options)
  if (query === undefined) {
    return d1Store
  }
  const log =
    options.log ??
    ((event, fields) =>
      logWorkerRouteWarning(event, {
        messageSafe: fields.messageSafe,
        op: fields.op,
        refs: fields.refs.slice(0, 10).join(','),
      }))
  return makeReadsRoutedInferenceReceiptStore({
    d1: d1Store,
    log,
    postgres: makePostgresInferenceReceiptStore(query),
    reads: flags.reads,
  })
}
