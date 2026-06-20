import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'
import type { Cs336A4EvalDeltaSettlement } from './cs336-a4-eval-delta-payment'
import type { Cs336A4ProvenanceReceipt } from './cs336-a4-provenance'

/**
 * Eval-delta settlement receipts for CS336 A4 refinery shards.
 *
 * `settleCs336A4EvalDeltaPayment` (cs336-a4-eval-delta-payment.ts) turns a
 * held-constant-trainer downstream eval measurement into a `payable` or
 * `blocked` settlement DECISION, and `buildCs336A4ProvenanceReceipt`
 * (cs336-a4-provenance.ts) proves WHERE a shard's corpus came from and
 * that its transform chain is internally consistent. Neither, on its own,
 * is an auditable bonus payment record: a settlement decision floating
 * free of the shard it pays for cannot be checked, and a provenance
 * receipt says nothing about payment.
 *
 * This module binds the two into a single deterministic, content-addressed,
 * public-safe receipt. It fails closed so a bonus can never be recorded
 * against a shard whose provenance does not check out:
 *
 *  - the settlement and the bound provenance receipt must name the SAME
 *    `assignmentRef` (a payment must point at the shard it pays for);
 *  - a `payable` settlement REQUIRES the bound provenance receipt to be
 *    `recomputeVerified` (you cannot pay an eval-delta bonus for a shard
 *    whose deterministic recompute did not verify);
 *  - the receipt carries the provenance receipt's content-addressed ref
 *    and final output digest, so an auditor can re-derive both halves.
 *
 * Like its inputs, this module emits refs, digests, and a sats amount
 * only. It never accepts or emits wallet, invoice, preimage, raw payload,
 * or private material; the public-safety guard fails closed first.
 */

export const Cs336A4EvalDeltaSettlementReceiptSchemaVersion =
  'openagents.training.data_refinery.eval_delta_settlement_receipt.v1' as const

export type Cs336A4EvalDeltaSettlementReceipt = Readonly<{
  assignmentRef: string
  /** SHA-256 over the canonical receipt body (hex). */
  contentDigestRef: string
  /** Final output digest of the shard the bonus is settled against. */
  finalOutputDigestRef: string
  jobKind: typeof Cs336A4DataRefineryJobKind
  /** True when the bound settlement decision is payable. */
  payable: boolean
  /** Content-addressed ref of the bound corpus provenance receipt. */
  provenanceReceiptRef: string
  /** Content-addressed receipt ref derived from contentDigestRef. */
  receiptRef: string
  schemaVersion: typeof Cs336A4EvalDeltaSettlementReceiptSchemaVersion
  settlement: Cs336A4EvalDeltaSettlement
  /** Bonus in sats recorded by this receipt; 0 for a blocked settlement. */
  settledBonusSats: number
}>

export class Cs336A4EvalDeltaSettlementReceiptError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaSettlementReceiptError'
}

export class Cs336A4EvalDeltaSettlementReceiptUnsafeMaterialError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaSettlementReceiptUnsafeMaterialError'
}

const unsafeMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|bolt12|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(crawl|dataset|invoice|payment|payload|prompt|runner|shard|warc)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)/i

/**
 * Guards the externally-derived ref strings that flow into this receipt.
 * The embedded `Cs336A4EvalDeltaSettlement` carries only trusted constant
 * policy/boundary refs defined in this codebase (some legitimately
 * containing words like "private" inside a documented boundary id), so
 * the guard is applied to the caller-derived surface — the refs that bind
 * the receipt to a shard — rather than over the whole serialized object.
 */
const assertRefsPublicSafe = (refs: ReadonlyArray<string>): void => {
  for (const ref of refs) {
    if (unsafeMaterialPattern.test(ref)) {
      throw new Cs336A4EvalDeltaSettlementReceiptUnsafeMaterialError(
        'CS336 A4 eval-delta settlement receipt contains wallet, payment, raw payload, or private material.',
      )
    }
  }
}

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

/**
 * Builds the canonical receipt body with fields in a fixed order so the
 * content digest is stable regardless of caller key ordering.
 */
const canonicalReceiptBody = (
  input: Readonly<{
    assignmentRef: string
    finalOutputDigestRef: string
    payable: boolean
    provenanceReceiptRef: string
    settledBonusSats: number
    settlement: Cs336A4EvalDeltaSettlement
  }>,
): string =>
  JSON.stringify({
    assignmentRef: input.assignmentRef,
    finalOutputDigestRef: input.finalOutputDigestRef,
    jobKind: Cs336A4DataRefineryJobKind,
    payable: input.payable,
    provenanceReceiptRef: input.provenanceReceiptRef,
    schemaVersion: Cs336A4EvalDeltaSettlementReceiptSchemaVersion,
    settledBonusSats: input.settledBonusSats,
    settlement: input.settlement,
  })

/**
 * Binds an eval-delta settlement decision to the corpus provenance
 * receipt of the shard it settles, producing a deterministic,
 * content-addressed, public-safe bonus receipt. Fails closed when the
 * settlement and provenance receipt name different assignments, or when a
 * payable settlement is bound to a provenance receipt that did not pass
 * deterministic recompute.
 *
 * The returned `receiptRef` is content-addressed: it is derived from a
 * SHA-256 over the canonical receipt body, so the same settlement and
 * provenance receipt always yield the same ref.
 */
export const buildCs336A4EvalDeltaSettlementReceipt = async (
  input: Readonly<{
    settlement: Cs336A4EvalDeltaSettlement
    provenanceReceipt: Cs336A4ProvenanceReceipt
  }>,
): Promise<Cs336A4EvalDeltaSettlementReceipt> => {
  const { provenanceReceipt, settlement } = input

  if (settlement.assignmentRef !== provenanceReceipt.assignmentRef) {
    throw new Cs336A4EvalDeltaSettlementReceiptError(
      'CS336 A4 eval-delta settlement receipt requires the settlement and provenance receipt to name the same assignmentRef.',
    )
  }

  const payable = settlement.payable
  const settledBonusSats = payable ? settlement.settledBonusSats : 0

  // A bonus can only be recorded against a shard whose deterministic
  // recompute verified; otherwise the bonus has no trustworthy basis.
  if (payable && !provenanceReceipt.recomputeVerified) {
    throw new Cs336A4EvalDeltaSettlementReceiptError(
      'CS336 A4 eval-delta settlement receipt cannot record a payable bonus against a provenance receipt that did not pass deterministic recompute.',
    )
  }

  const assignmentRef = settlement.assignmentRef
  const finalOutputDigestRef = provenanceReceipt.finalOutputDigestRef
  const provenanceReceiptRef = provenanceReceipt.receiptRef

  assertRefsPublicSafe([
    assignmentRef,
    finalOutputDigestRef,
    provenanceReceiptRef,
  ])

  const body = canonicalReceiptBody({
    assignmentRef,
    finalOutputDigestRef,
    payable,
    provenanceReceiptRef,
    settledBonusSats,
    settlement,
  })

  const contentDigestRef = await sha256Hex(body)
  const receiptRef = `receipt.cs336_a4.eval_delta_settlement.${assignmentRef}.${contentDigestRef.slice(0, 16)}`

  const receipt: Cs336A4EvalDeltaSettlementReceipt = {
    assignmentRef,
    contentDigestRef,
    finalOutputDigestRef,
    jobKind: Cs336A4DataRefineryJobKind,
    payable,
    provenanceReceiptRef,
    receiptRef,
    schemaVersion: Cs336A4EvalDeltaSettlementReceiptSchemaVersion,
    settlement,
    settledBonusSats,
  }

  return receipt
}
