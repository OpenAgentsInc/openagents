import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export type InferenceReceiptKind =
  | 'charge'
  | 'usd_credit_grant'
  | 'batch_job_charge'

export type InferenceReceiptRecord = Readonly<{
  createdAt: string
  payInType: string
  receiptRef: string
  state: string
  stateChangedAt: string
}>

export type PublicInferenceReceiptProjection = Readonly<{
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  generatedAt: string
  kind: InferenceReceiptKind
  ledgerState: 'paid'
  receiptRef: string
  schemaVersion: 'openagents.inference.receipt.v1'
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
  stateChangedAt: string
}>

export type InferenceReceiptStore = Readonly<{
  readInferenceReceiptByRef: (
    receiptRef: string,
  ) => Promise<InferenceReceiptRecord | null>
}>

const unsafePublicReceiptPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer\s+|bolt11|cookie|cs_(?:live|test)_|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|idempotency|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|stripe|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

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
    ledgerState: 'paid',
    receiptRef: record.receiptRef,
    schemaVersion: 'openagents.inference.receipt.v1',
    sourceRefs: [
      `route:/api/public/inference/receipts/${record.receiptRef}`,
      `ledger.pay_ins.public_receipt_ref.${kind}`,
    ],
    staleness: liveAtReadStaleness(['pay_ins.public_receipt_ref']),
    stateChangedAt: record.stateChangedAt,
  }

  return isPublicSafeInferenceReceiptProjection(receipt) ? receipt : null
}

type InferenceReceiptRow = Readonly<{
  created_at: string
  pay_in_type: string
  public_receipt_ref: string | null
  state: string
  state_changed_at: string
}>

const rowToInferenceReceiptRecord = (
  row: InferenceReceiptRow,
): InferenceReceiptRecord | null =>
  row.public_receipt_ref === null
    ? null
    : {
        createdAt: row.created_at,
        payInType: row.pay_in_type,
        receiptRef: row.public_receipt_ref,
        state: row.state,
        stateChangedAt: row.state_changed_at,
      }

export const makeD1InferenceReceiptStore = (
  db: D1Database,
): InferenceReceiptStore => ({
  readInferenceReceiptByRef: async receiptRef => {
    const row = await db
      .prepare(
        `SELECT pay_in_type, state, public_receipt_ref, created_at, state_changed_at
           FROM pay_ins
          WHERE public_receipt_ref = ?
            AND pay_in_type IN ('adjustment', 'usd_credit_grant')
          LIMIT 1`,
      )
      .bind(receiptRef)
      .first<InferenceReceiptRow>()

    return row === null ? null : rowToInferenceReceiptRecord(row)
  },
})
