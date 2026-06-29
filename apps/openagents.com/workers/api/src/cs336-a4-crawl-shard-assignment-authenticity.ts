import {
  deriveCs336A4CrawlShardAssignment,
  type Cs336A4CrawlShardAssignment,
} from './cs336-a4-crawl-shard-assignment'
import type { Cs336A4CrawlShardPlan } from './cs336-a4-crawl-shard-plan'

/**
 * Re-derivation AUTHENTICITY gate for a single crawl-shard assignment
 * (`blocker.product_promises.crawl_scale_corpus_missing`).
 *
 * `deriveCs336A4CrawlShardAssignment` (cs336-a4-crawl-shard-assignment.ts)
 * mints a deterministic, content-addressed `assignmentRef` (and its backing
 * `contentDigestRef`) for one shard of a `buildCs336A4CrawlShardPlan`
 * partition. Everything downstream binds to that ref by VALUE: the
 * dispatch-coverage gate (cs336-a4-crawl-shard-dispatch-coverage.ts) checks a
 * set tiles the plan but states it "does NOT re-derive assignment refs", the
 * provenance-binding gate (cs336-a4-crawl-shard-provenance-binding.ts) matches
 * a receipt's `assignmentRef` against the assignment's, and the eval-delta
 * settlement receipt pays against that same ref. None of them recompute the
 * ref from the plan — they trust the value the assignment carries.
 *
 * That leaves one unguarded forgery: an assignment handed back over the wire
 * can carry the right `planRef`, source provenance, and segment range while
 * carrying a FORGED or STALE `assignmentRef` / `contentDigestRef` — the very
 * content-addressed identifier payment, provenance, and settlement bind to. A
 * contributor (or a buggy serializer) could then close out, and be paid
 * against, a ref that is not the canonical unit the plan deterministically
 * derives, defeating the point of content-addressing.
 *
 * This module is that gate. `verifyCs336A4CrawlShardAssignmentAuthenticity`
 * re-derives the expected assignment from `(plan, assignment.index)` and
 * compares the handed-back assignment against it field by field, so a forged
 * or stale ref fails closed BEFORE the assignment is dispatched as paid work
 * or accepted as the target of a provenance/settlement receipt.
 *
 * It is a pure comparison: it accepts an already-built assignment plus the
 * trusted plan, recomputes the canonical assignment, decides
 * `authentic` / not, and never fetches, fabricates, or mutates anything. It
 * does NOT verify a SET tiles the plan (that is the dispatch-coverage gate's
 * job), does NOT bind a returned provenance receipt (that is the
 * provenance-binding gate's job), and does NOT settle payment; it answers
 * exactly one question: is this assignment the genuine content-addressed unit
 * this plan derives at this index?
 */

export const Cs336A4CrawlShardAssignmentAuthenticityMismatches = [
  'index_out_of_range',
  'plan_ref_mismatch',
  'acquisition_mode_mismatch',
  'input_shard_ref_mismatch',
  'segment_range_mismatch',
  'provenance_source_mismatch',
  'schema_version_mismatch',
  'content_digest_ref_mismatch',
  'assignment_ref_mismatch',
] as const
export type Cs336A4CrawlShardAssignmentAuthenticityMismatch =
  (typeof Cs336A4CrawlShardAssignmentAuthenticityMismatches)[number]

export type Cs336A4CrawlShardAssignmentAuthenticityResult =
  | Readonly<{
      assignmentRef: string
      authentic: true
      contentDigestRef: string
      index: number
      planRef: string
    }>
  | Readonly<{
      authentic: false
      detail: string
      reason: Cs336A4CrawlShardAssignmentAuthenticityMismatch
    }>

export class Cs336A4CrawlShardAssignmentAuthenticityError extends Error {
  readonly _tag = 'Cs336A4CrawlShardAssignmentAuthenticityError'
  readonly reason: Cs336A4CrawlShardAssignmentAuthenticityMismatch

  constructor(
    reason: Cs336A4CrawlShardAssignmentAuthenticityMismatch,
    detail: string,
  ) {
    super(detail)
    this.reason = reason
  }
}

/**
 * Decides whether `assignment` is the genuine content-addressed unit `plan`
 * derives at `assignment.index`. Returns a discriminated result so callers can
 * branch on `authentic` and record the precise mismatch reason in a dispatch
 * audit trail.
 *
 * The handed-back `assignment.index` is bounds-checked first
 * (`index_out_of_range`); then the expected assignment is re-derived from the
 * trusted plan via `deriveCs336A4CrawlShardAssignment` and the handed-back
 * assignment is compared against it: structural fields first (plan ref,
 * acquisition mode, input shard ref, segment range, source provenance, schema
 * version), then the recomputed `contentDigestRef`, then the `assignmentRef`.
 * An `authentic: true` result is returned only when every checked field
 * matches the re-derivation byte for byte.
 */
export const verifyCs336A4CrawlShardAssignmentAuthenticity = async (
  input: Readonly<{
    assignment: Cs336A4CrawlShardAssignment
    plan: Cs336A4CrawlShardPlan
  }>,
): Promise<Cs336A4CrawlShardAssignmentAuthenticityResult> => {
  const { assignment, plan } = input

  if (
    !Number.isInteger(assignment.index) ||
    assignment.index < 0 ||
    assignment.index >= plan.shards.length
  ) {
    return {
      authentic: false,
      detail: `assignment index (${String(assignment.index)}) is out of range for a plan with ${plan.shards.length} shard(s); it cannot name a unit this plan derives.`,
      reason: 'index_out_of_range',
    }
  }

  // Re-derive the canonical assignment the plan produces at this index. For a
  // valid in-range index against a well-formed crawl plan this never throws;
  // an internally malformed plan is a precondition violation and is allowed to
  // propagate (the plan is the trusted reference here, the assignment is not).
  const expected = await deriveCs336A4CrawlShardAssignment({
    index: assignment.index,
    plan,
  })

  if (assignment.planRef !== expected.planRef) {
    return {
      authentic: false,
      detail: `assignment planRef (${assignment.planRef}) does not match the plan (${expected.planRef}).`,
      reason: 'plan_ref_mismatch',
    }
  }

  if (assignment.acquisitionMode !== expected.acquisitionMode) {
    return {
      authentic: false,
      detail: `assignment acquisitionMode (${assignment.acquisitionMode}) does not match the plan-derived value (${expected.acquisitionMode}).`,
      reason: 'acquisition_mode_mismatch',
    }
  }

  if (assignment.inputShardRef !== expected.inputShardRef) {
    return {
      authentic: false,
      detail: `assignment inputShardRef (${assignment.inputShardRef}) does not match the plan-derived value (${expected.inputShardRef}).`,
      reason: 'input_shard_ref_mismatch',
    }
  }

  if (
    assignment.startSegment !== expected.startSegment ||
    assignment.endSegment !== expected.endSegment ||
    assignment.segmentCount !== expected.segmentCount
  ) {
    return {
      authentic: false,
      detail: `assignment segment range [${String(assignment.startSegment)}, ${String(assignment.endSegment)}) / count ${String(assignment.segmentCount)} does not match the plan-derived [${expected.startSegment}, ${expected.endSegment}) / count ${expected.segmentCount}.`,
      reason: 'segment_range_mismatch',
    }
  }

  const source = assignment.provenanceSource
  const expectedSource = expected.provenanceSource

  if (
    source.acquisitionMode !== expectedSource.acquisitionMode ||
    source.sourceRef !== expectedSource.sourceRef ||
    source.snapshotRef !== expectedSource.snapshotRef ||
    source.licenseRef !== expectedSource.licenseRef
  ) {
    return {
      authentic: false,
      detail: `assignment provenance source (${source.acquisitionMode}/${source.sourceRef}/${source.snapshotRef}/${source.licenseRef}) does not match the plan-derived value (${expectedSource.acquisitionMode}/${expectedSource.sourceRef}/${expectedSource.snapshotRef}/${expectedSource.licenseRef}).`,
      reason: 'provenance_source_mismatch',
    }
  }

  if (assignment.schemaVersion !== expected.schemaVersion) {
    return {
      authentic: false,
      detail: `assignment schemaVersion (${assignment.schemaVersion}) does not match the plan-derived value (${expected.schemaVersion}).`,
      reason: 'schema_version_mismatch',
    }
  }

  if (assignment.contentDigestRef !== expected.contentDigestRef) {
    return {
      authentic: false,
      detail: `assignment contentDigestRef (${assignment.contentDigestRef}) does not match the recomputed digest (${expected.contentDigestRef}); the assignment body is not the canonical one this plan derives.`,
      reason: 'content_digest_ref_mismatch',
    }
  }

  if (assignment.assignmentRef !== expected.assignmentRef) {
    return {
      authentic: false,
      detail: `assignment assignmentRef (${assignment.assignmentRef}) does not match the recomputed ref (${expected.assignmentRef}).`,
      reason: 'assignment_ref_mismatch',
    }
  }

  return {
    assignmentRef: expected.assignmentRef,
    authentic: true,
    contentDigestRef: expected.contentDigestRef,
    index: expected.index,
    planRef: expected.planRef,
  }
}

/**
 * Fail-closed wrapper around `verifyCs336A4CrawlShardAssignmentAuthenticity`:
 * throws `Cs336A4CrawlShardAssignmentAuthenticityError` (carrying the mismatch
 * reason) when the assignment is not the genuine content-addressed unit the
 * plan derives, and returns the recomputed `assignmentRef` when it is. Use
 * this on a dispatch/closeout path where a forged or stale assignment ref must
 * hard-fail before any paid work is dispatched or any receipt is bound to it.
 */
export const assertCs336A4CrawlShardAssignmentAuthenticity = async (
  input: Readonly<{
    assignment: Cs336A4CrawlShardAssignment
    plan: Cs336A4CrawlShardPlan
  }>,
): Promise<string> => {
  const result = await verifyCs336A4CrawlShardAssignmentAuthenticity(input)

  if (!result.authentic) {
    throw new Cs336A4CrawlShardAssignmentAuthenticityError(
      result.reason,
      `CS336 A4 crawl-shard assignment is not the canonical unit its plan derives: ${result.detail}`,
    )
  }

  return result.assignmentRef
}
