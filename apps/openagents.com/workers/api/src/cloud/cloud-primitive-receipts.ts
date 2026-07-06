// Dereferenceable PAID receipt resolver for sellable OpenAgents Cloud primitives
// (EPIC #5510: sandbox compute #5517, fine-tuning #5516).
//
// THE GAP this closes: the shared cloud-metering seam (`cloud-metering.ts`)
// already decrements credits receipt-first for a finished sandbox rental / a
// finished fine-tune job and writes a real `pay_ins` row whose
// `public_receipt_ref` is `receipt.cloud.<primitive>.charge.<chargeId>`. The
// surfaces ADVERTISE that ref, but NOTHING read it back: there was no public
// route that turns a cloud-primitive charge ref into a dereferenceable receipt,
// so the metering produced a ledger fact no one could verify. This module is the
// READ seam (mirrors `inference-receipts.ts`): given a charge receipt ref, it
// reads the settled `pay_ins` row and projects a public-safe receipt proving the
// metered debit landed (`state = 'paid'`). It adds NO ledger writes and moves no
// money — it derefs a debit the metering seam already made.
//
// HONEST SCOPE: a dereferenceable PAID charge receipt proves rent -> metered ->
// charge for ONE rental/job. It is the missing receipt artifact for
// `cloud.sandbox_compute_service.v1` / `cloud.fine_tuning_service.v1`; the final
// green flip of those promises still requires real demand provenance +
// owner sign-off per `proof.claim_upgrade_receipts.v1` and
// `proof.demand_provenance.v1`. This resolver never asserts a promise is green.

import type { PaymentsLedgerDb } from '../payments-ledger-db'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from '../public-projection-staleness'

// The cloud primitives that expose a dereferenceable charge receipt. Each maps
// 1:1 to a `cloudChargeReceiptRef(primitive, chargeId)` prefix
// (`receipt.<primitive>.charge.`).
export type CloudPrimitiveReceiptKind =
  | 'sandbox_compute_rental'
  | 'fine_tuning_job'

// Public, stable prefix per kind. Kept in lockstep with the primitive tags the
// scaffolds pass to the metering seam (`SANDBOX_COMPUTE_PRIMITIVE`,
// `FINE_TUNING_PRIMITIVE`) so a ref a surface advertises always resolves here.
const RECEIPT_PREFIX_BY_KIND: ReadonlyArray<
  Readonly<{ kind: CloudPrimitiveReceiptKind; prefix: string }>
> = [
  {
    kind: 'sandbox_compute_rental',
    prefix: 'receipt.cloud.sandbox_compute.rental.charge.',
  },
  {
    kind: 'fine_tuning_job',
    prefix: 'receipt.cloud.fine_tuning.job.charge.',
  },
]

export type CloudPrimitiveReceiptRecord = Readonly<{
  contextRef: string | null
  createdAt: string
  payInType: string
  receiptRef: string
  state: string
  stateChangedAt: string
}>

export type PublicCloudPrimitiveReceiptProjection = Readonly<{
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  generatedAt: string
  kind: CloudPrimitiveReceiptKind
  ledgerState: 'paid'
  receiptRef: string
  schemaVersion: 'openagents.cloud.primitive.receipt.v1'
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
  stateChangedAt: string
}>

export type CloudPrimitiveReceiptReadStore = Readonly<{
  readCloudPrimitiveReceiptByRef: (
    receiptRef: string,
  ) => Promise<CloudPrimitiveReceiptRecord | null>
}>

// Same redaction guard the inference-receipt projection uses: refuse to publish
// anything that looks like raw payment material, secrets, or filesystem paths.
const unsafePublicReceiptPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer\s+|bolt11|cookie|cs_(?:live|test)_|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|idempotency|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|stripe|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

export const isPublicSafeCloudPrimitiveReceiptProjection = (
  value: unknown,
): boolean => !unsafePublicReceiptPattern.test(JSON.stringify(value))

const kindForRecord = (
  record: CloudPrimitiveReceiptRecord,
): CloudPrimitiveReceiptKind | null => {
  if (record.payInType !== 'adjustment') {
    return null
  }
  for (const { kind, prefix } of RECEIPT_PREFIX_BY_KIND) {
    if (record.receiptRef.startsWith(prefix)) {
      return kind
    }
  }
  return null
}

export const publicCloudPrimitiveReceiptFromRecord = (
  record: CloudPrimitiveReceiptRecord,
  generatedAt: string,
): PublicCloudPrimitiveReceiptProjection | null => {
  const kind = kindForRecord(record)

  // Only a settled (`paid`) cloud-primitive charge is a dereferenceable receipt.
  // A `pending`/`failed` row is NOT projected (it is not a proven metered debit).
  if (kind === null || record.state !== 'paid') {
    return null
  }

  const receipt: PublicCloudPrimitiveReceiptProjection = {
    authorityBoundary:
      'Public proof only. This receipt read grants no spend, refund, payout, checkout, settlement, provisioning, or registry authority, and asserts no product-promise is green.',
    caveatRefs: [
      'caveat.public.no_private_payment_material',
      'caveat.public.no_account_or_amount_projection',
      'caveat.public.cloud_primitive_ledger_receipt_exists_only',
      'caveat.public.cloud_primitive_demand_provenance_and_owner_signoff_pending',
    ],
    generatedAt,
    kind,
    ledgerState: 'paid',
    receiptRef: record.receiptRef,
    schemaVersion: 'openagents.cloud.primitive.receipt.v1',
    sourceRefs: [
      `route:/api/public/cloud/receipts/${record.receiptRef}`,
      `ledger.pay_ins.public_receipt_ref.${kind}`,
    ],
    staleness: liveAtReadStaleness(['pay_ins.public_receipt_ref']),
    stateChangedAt: record.stateChangedAt,
  }

  return isPublicSafeCloudPrimitiveReceiptProjection(receipt) ? receipt : null
}

type CloudPrimitiveReceiptRow = Readonly<{
  context_ref: string | null
  created_at: string
  pay_in_type: string
  public_receipt_ref: string | null
  state: string
  state_changed_at: string
}>

const rowToCloudPrimitiveReceiptRecord = (
  row: CloudPrimitiveReceiptRow,
): CloudPrimitiveReceiptRecord | null =>
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

// CFG-4 (#8519): `pay_ins` is Postgres-authoritative — the receipt read goes
// through the credits-domain `PaymentsLedgerDb` (formerly
// `makeD1CloudPrimitiveReceiptStore` over D1, which is gone for this table).
export const makeLedgerCloudPrimitiveReceiptStore = (
  db: PaymentsLedgerDb,
): CloudPrimitiveReceiptReadStore => ({
  readCloudPrimitiveReceiptByRef: async receiptRef => {
    const rows = await db.query(
      `SELECT pay_in_type, state, public_receipt_ref, context_ref, created_at, state_changed_at
         FROM pay_ins
        WHERE public_receipt_ref = ?
          AND pay_in_type = 'adjustment'
        LIMIT 1`,
      [receiptRef],
    )

    const row = rows[0]
    return row === undefined
      ? null
      : rowToCloudPrimitiveReceiptRecord(row as unknown as CloudPrimitiveReceiptRow)
  },
})
