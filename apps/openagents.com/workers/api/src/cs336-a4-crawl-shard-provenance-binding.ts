import type { Cs336A4CrawlShardAssignment } from './cs336-a4-crawl-shard-assignment'
import type { Cs336A4ProvenanceReceipt } from './cs336-a4-provenance'

/**
 * Binds a returned corpus provenance receipt back to the crawl-shard
 * ASSIGNMENT it claims to close out
 * (`blocker.product_promises.corpus_provenance_receipts_missing`).
 *
 * `deriveCs336A4CrawlShardAssignment` (cs336-a4-crawl-shard-assignment.ts)
 * mints a deterministic, payable `assignmentRef` carrying the
 * `inputShardRef` and a `Cs336A4SourceProvenance` lifted verbatim from the
 * plan, and `buildCs336A4ProvenanceReceipt` (cs336-a4-provenance.ts) emits a
 * receipt that an operator gets back when a contributor closes a shard out.
 * The assignment is "ready to feed" the receipt builder, but nothing on the
 * RETURN path checked that a receipt handed back actually closes out THE
 * dispatched assignment: a contributor could return a perfectly valid,
 * internally-consistent provenance receipt for a DIFFERENT shard, source, or
 * snapshot, and the operator would have no deterministic gate to catch it
 * before admitting or paying for it.
 *
 * This module is that gate. `verifyCs336A4CrawlShardProvenanceBinding`
 * checks, field by field, that a provenance receipt is bound to a specific
 * crawl-shard assignment:
 *  - the same `assignmentRef` (the receipt closes out THIS assignment);
 *  - the same `inputShardRef` (the receipt's input is the assigned shard);
 *  - the same source provenance — acquisition mode, source, snapshot, and
 *    license — so a receipt cannot silently re-attribute the corpus to a
 *    different origin or license than the operator dispatched.
 *
 * It is a pure comparison: it accepts already-built artifacts, decides
 * `bound` / not, and never fetches, fabricates, or mutates anything. It does
 * NOT re-validate the receipt's internal transform chain (that is
 * `buildCs336A4ProvenanceReceipt`'s job) and does NOT settle payment (that is
 * the eval-delta settlement receipt's job); it answers exactly one question:
 * does this receipt close out this assignment?
 */

export const Cs336A4CrawlShardProvenanceBindingMismatches = [
  'assignment_ref_mismatch',
  'input_shard_ref_mismatch',
  'acquisition_mode_mismatch',
  'source_ref_mismatch',
  'snapshot_ref_mismatch',
  'license_ref_mismatch',
] as const
export type Cs336A4CrawlShardProvenanceBindingMismatch =
  (typeof Cs336A4CrawlShardProvenanceBindingMismatches)[number]

export type Cs336A4CrawlShardProvenanceBindingResult =
  | Readonly<{
      assignmentRef: string
      bound: true
      inputShardRef: string
      provenanceReceiptRef: string
    }>
  | Readonly<{
      bound: false
      detail: string
      reason: Cs336A4CrawlShardProvenanceBindingMismatch
    }>

export class Cs336A4CrawlShardProvenanceBindingError extends Error {
  readonly _tag = 'Cs336A4CrawlShardProvenanceBindingError'
  readonly reason: Cs336A4CrawlShardProvenanceBindingMismatch

  constructor(reason: Cs336A4CrawlShardProvenanceBindingMismatch, detail: string) {
    super(detail)
    this.reason = reason
  }
}

/**
 * Decides whether `receipt` closes out `assignment`. Returns a discriminated
 * result so callers can branch on `bound` and record the precise mismatch
 * reason in an admission/settlement audit trail. The first mismatch
 * (assignment ref, then input shard ref, then each source-provenance field)
 * is reported; a `bound: true` result is returned only when every checked
 * field matches.
 */
export const verifyCs336A4CrawlShardProvenanceBinding = (
  input: Readonly<{
    assignment: Cs336A4CrawlShardAssignment
    receipt: Cs336A4ProvenanceReceipt
  }>,
): Cs336A4CrawlShardProvenanceBindingResult => {
  const { assignment, receipt } = input

  if (receipt.assignmentRef !== assignment.assignmentRef) {
    return {
      bound: false,
      detail: `provenance receipt assignmentRef (${receipt.assignmentRef}) does not match the assignment (${assignment.assignmentRef}).`,
      reason: 'assignment_ref_mismatch',
    }
  }

  if (receipt.inputShardRef !== assignment.inputShardRef) {
    return {
      bound: false,
      detail: `provenance receipt inputShardRef (${receipt.inputShardRef}) does not match the assignment inputShardRef (${assignment.inputShardRef}).`,
      reason: 'input_shard_ref_mismatch',
    }
  }

  const source = assignment.provenanceSource
  const { provenance } = receipt

  if (provenance.acquisitionMode !== source.acquisitionMode) {
    return {
      bound: false,
      detail: `provenance receipt acquisitionMode (${provenance.acquisitionMode}) does not match the assignment (${source.acquisitionMode}).`,
      reason: 'acquisition_mode_mismatch',
    }
  }

  if (provenance.sourceRef !== source.sourceRef) {
    return {
      bound: false,
      detail: `provenance receipt sourceRef (${provenance.sourceRef}) does not match the assignment (${source.sourceRef}).`,
      reason: 'source_ref_mismatch',
    }
  }

  if (provenance.snapshotRef !== source.snapshotRef) {
    return {
      bound: false,
      detail: `provenance receipt snapshotRef (${provenance.snapshotRef}) does not match the assignment (${source.snapshotRef}).`,
      reason: 'snapshot_ref_mismatch',
    }
  }

  if (provenance.licenseRef !== source.licenseRef) {
    return {
      bound: false,
      detail: `provenance receipt licenseRef (${provenance.licenseRef}) does not match the assignment (${source.licenseRef}).`,
      reason: 'license_ref_mismatch',
    }
  }

  return {
    assignmentRef: assignment.assignmentRef,
    bound: true,
    inputShardRef: assignment.inputShardRef,
    provenanceReceiptRef: receipt.receiptRef,
  }
}

/**
 * Fail-closed wrapper around `verifyCs336A4CrawlShardProvenanceBinding`:
 * throws `Cs336A4CrawlShardProvenanceBindingError` (carrying the mismatch
 * reason) when the receipt does not close out the assignment, and returns
 * the receipt's content-addressed `receiptRef` when it does. Use this on an
 * admission/closeout path where an unbound receipt must hard-fail rather
 * than be branched on.
 */
export const assertCs336A4CrawlShardProvenanceBinding = (
  input: Readonly<{
    assignment: Cs336A4CrawlShardAssignment
    receipt: Cs336A4ProvenanceReceipt
  }>,
): string => {
  const result = verifyCs336A4CrawlShardProvenanceBinding(input)

  if (!result.bound) {
    throw new Cs336A4CrawlShardProvenanceBindingError(
      result.reason,
      `CS336 A4 provenance receipt does not bind its crawl-shard assignment: ${result.detail}`,
    )
  }

  return result.provenanceReceiptRef
}
