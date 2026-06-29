import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { parseInferenceChargeContextRef } from './inference/inference-charge-context'

export type InferenceReceiptKind =
  | 'charge'
  | 'free_allowance'
  | 'usd_credit_grant'
  | 'batch_job_charge'

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
    record.receiptRef.startsWith('receipt.inference.batch_job_charge.') &&
    record.payInType === 'adjustment'
  ) {
    return 'batch_job_charge'
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
    kind === 'charge' || kind === 'batch_job_charge'
      ? modelEvidenceFromContextRef(record.contextRef)
      : undefined

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

export const makeD1InferenceReceiptStore = (
  db: D1Database,
): InferenceReceiptStore => ({
  listRecentInferenceReceipts: async limit => {
    const rowLimit = Math.max(1, Math.min(200, Math.trunc(limit)))
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
