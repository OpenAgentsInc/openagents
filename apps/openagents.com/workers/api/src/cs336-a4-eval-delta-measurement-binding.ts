import type { Cs336A4EvalDeltaMeasurement } from './cs336-a4-eval-delta-payment'
import type { Cs336A4ProvenanceReceipt } from './cs336-a4-provenance'

/**
 * Binds a held-constant-trainer eval-delta MEASUREMENT back to the corpus
 * provenance receipt of the shard whose bonus it would price
 * (`blocker.product_promises.eval_delta_payment_missing`).
 *
 * `settleCs336A4EvalDeltaPayment` (cs336-a4-eval-delta-payment.ts) prices a
 * bonus from a `Cs336A4EvalDeltaMeasurement` (a filtered-vs-baseline
 * downstream eval score measured on a `sourceRef`), and
 * `buildCs336A4EvalDeltaSettlementReceipt`
 * (cs336-a4-eval-delta-settlement-receipt.ts) binds that settlement decision
 * to the shard's provenance receipt by `assignmentRef` and refuses to record
 * a payable bonus unless the receipt's deterministic recompute verified.
 *
 * There is still a gap between those two checks: the settlement decision
 * drops the measurement's `sourceRef`, so the settlement receipt can confirm
 * that a payment points at the right ASSIGNMENT but cannot confirm that the
 * eval delta was actually measured on that shard's real corpus SOURCE. A
 * contributor could measure a genuine positive delta on an easy/unrelated
 * source and attach it to a shard whose admitted corpus is a different
 * (harder) source; every assignment-ref check would still pass, and the bonus
 * would be recorded against a delta that was never measured on the shard it
 * pays for.
 *
 * This module is that missing precondition. It is a pure comparison over two
 * already-built artifacts — it fetches, fabricates, and mutates nothing — and
 * answers exactly one question: was this eval delta measured on the source the
 * provenance receipt admits for this shard? It does NOT re-price the bonus
 * (that is the settlement's job), does NOT re-validate the transform chain
 * (that is the provenance builder's job), and does NOT settle payment (that is
 * the settlement receipt's job).
 */

export const Cs336A4EvalDeltaMeasurementBindingMismatches = [
  'source_ref_mismatch',
] as const
export type Cs336A4EvalDeltaMeasurementBindingMismatch =
  (typeof Cs336A4EvalDeltaMeasurementBindingMismatches)[number]

export type Cs336A4EvalDeltaMeasurementBindingResult =
  | Readonly<{
      bound: true
      provenanceReceiptRef: string
      sourceRef: string
    }>
  | Readonly<{
      bound: false
      detail: string
      reason: Cs336A4EvalDeltaMeasurementBindingMismatch
    }>

export class Cs336A4EvalDeltaMeasurementBindingValidationError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaMeasurementBindingValidationError'
}

export class Cs336A4EvalDeltaMeasurementBindingError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaMeasurementBindingError'
  readonly reason: Cs336A4EvalDeltaMeasurementBindingMismatch

  constructor(reason: Cs336A4EvalDeltaMeasurementBindingMismatch, detail: string) {
    super(detail)
    this.reason = reason
  }
}

const requireNonEmptyRef = (label: string, value: string): string => {
  const trimmed = value.trim()

  if (trimmed === '') {
    throw new Cs336A4EvalDeltaMeasurementBindingValidationError(
      `CS336 A4 eval-delta measurement binding requires a non-empty ${label}.`,
    )
  }

  return trimmed
}

/**
 * Decides whether `measurement` was taken on the source that
 * `provenanceReceipt` admits for its shard. Returns a discriminated result so
 * callers can branch on `bound` and record the precise mismatch reason in a
 * settlement/admission audit trail. A `bound: true` result is returned only
 * when the measurement's `sourceRef` equals the receipt's
 * `provenance.sourceRef` (after trimming). Empty refs on either side fail
 * closed with a validation error rather than silently comparing as equal.
 */
export const verifyCs336A4EvalDeltaMeasurementBinding = (
  input: Readonly<{
    measurement: Cs336A4EvalDeltaMeasurement
    provenanceReceipt: Cs336A4ProvenanceReceipt
  }>,
): Cs336A4EvalDeltaMeasurementBindingResult => {
  const { measurement, provenanceReceipt } = input

  const measurementSourceRef = requireNonEmptyRef(
    'measurement.sourceRef',
    measurement.sourceRef,
  )
  const provenanceSourceRef = requireNonEmptyRef(
    'provenanceReceipt.provenance.sourceRef',
    provenanceReceipt.provenance.sourceRef,
  )

  if (measurementSourceRef !== provenanceSourceRef) {
    return {
      bound: false,
      detail: `eval-delta measurement sourceRef (${measurementSourceRef}) does not match the provenance receipt source (${provenanceSourceRef}); the delta was not measured on this shard's corpus.`,
      reason: 'source_ref_mismatch',
    }
  }

  return {
    bound: true,
    provenanceReceiptRef: provenanceReceipt.receiptRef,
    sourceRef: provenanceSourceRef,
  }
}

/**
 * Fail-closed wrapper around `verifyCs336A4EvalDeltaMeasurementBinding`:
 * throws `Cs336A4EvalDeltaMeasurementBindingError` (carrying the mismatch
 * reason) when the eval delta was not measured on the shard's admitted source,
 * and returns the provenance receipt's content-addressed `receiptRef` when it
 * was. Use this on a settlement/closeout path where a measurement bound to the
 * wrong source must hard-fail before a bonus is priced or recorded.
 */
export const assertCs336A4EvalDeltaMeasurementBinding = (
  input: Readonly<{
    measurement: Cs336A4EvalDeltaMeasurement
    provenanceReceipt: Cs336A4ProvenanceReceipt
  }>,
): string => {
  const result = verifyCs336A4EvalDeltaMeasurementBinding(input)

  if (!result.bound) {
    throw new Cs336A4EvalDeltaMeasurementBindingError(
      result.reason,
      `CS336 A4 eval-delta measurement does not bind its shard's provenance: ${result.detail}`,
    )
  }

  return result.provenanceReceiptRef
}
