import type { Cs336A4CrawlShardAssignment } from './cs336-a4-crawl-shard-assignment'
import type { Cs336A4CrawlShardPlan } from './cs336-a4-crawl-shard-plan'

/**
 * Dispatch-readiness COVERAGE gate for a set of crawl-shard assignments
 * (`blocker.product_promises.crawl_scale_corpus_missing`).
 *
 * `buildCs336A4CrawlShardPlan` (cs336-a4-crawl-shard-plan.ts) partitions a
 * crawl snapshot into bounded shard units, and
 * `deriveCs336A4CrawlShardAssignment(s)` (cs336-a4-crawl-shard-assignment.ts)
 * turns those shards into payable assignment units. Before an operator
 * dispatches a SET of assignments as paid crawl-scale work, it needs one
 * deterministic, fail-closed answer: does the set of assignments I am about
 * to pay for completely and uniquely cover the snapshot this plan describes?
 *
 * Without that check there is no guard against the two ways a paid dispatch
 * can be silently wrong:
 *  - a GAP — some snapshot segment is not assigned at all, so part of the
 *    corpus is never acquired (and the operator believes the snapshot is
 *    fully covered when it is not); or
 *  - an OVERLAP / duplicate — the same segment is assigned twice, so the
 *    operator pays twice for the same bytes.
 *
 * The happy-path output of `deriveCs336A4CrawlShardAssignments` does tile a
 * plan, but assignments handed to an operator are not always that pristine
 * list: they may arrive over the wire, be a hand-curated subset, be
 * re-ordered, or be mixed across plans. This module verifies coverage for an
 * ARBITRARY set of assignments against a plan.
 *
 * It is a pure comparison: it accepts already-built artifacts, decides
 * `complete` / not, and never fetches, fabricates, or mutates anything. It
 * does NOT re-derive assignment refs (that is the assignment builder's job),
 * does NOT bind a returned provenance receipt (that is the provenance-binding
 * gate's job), and does NOT settle payment; it answers exactly one question:
 * is this set of assignments a complete, non-overlapping cover of this plan?
 */

export const Cs336A4CrawlShardDispatchCoverageFailures = [
  'empty_assignment_set',
  'plan_ref_mismatch',
  'acquisition_mode_mismatch',
  'source_ref_mismatch',
  'snapshot_ref_mismatch',
  'license_ref_mismatch',
  'duplicate_assignment_ref',
  'segment_range_invalid',
  'segment_out_of_bounds',
  'duplicate_segment_coverage',
  'segment_gap',
] as const
export type Cs336A4CrawlShardDispatchCoverageFailure =
  (typeof Cs336A4CrawlShardDispatchCoverageFailures)[number]

export type Cs336A4CrawlShardDispatchCoverageResult =
  | Readonly<{
      assignmentCount: number
      assignmentRefs: ReadonlyArray<string>
      complete: true
      planRef: string
      segmentCount: number
      snapshotRef: string
    }>
  | Readonly<{
      complete: false
      detail: string
      reason: Cs336A4CrawlShardDispatchCoverageFailure
    }>

export class Cs336A4CrawlShardDispatchCoverageError extends Error {
  readonly _tag = 'Cs336A4CrawlShardDispatchCoverageError'
  readonly reason: Cs336A4CrawlShardDispatchCoverageFailure

  constructor(
    reason: Cs336A4CrawlShardDispatchCoverageFailure,
    detail: string,
  ) {
    super(detail)
    this.reason = reason
  }
}

/**
 * Decides whether `assignments` completely and uniquely cover `plan`. Returns
 * a discriminated result so callers can branch on `complete` and record the
 * precise failure reason in a dispatch audit trail.
 *
 * Every assignment is first checked to belong to this plan (matching
 * `planRef`, acquisition mode, and source/snapshot/license provenance) and to
 * be internally consistent (in-bounds, integral segment range). The segment
 * intervals `[startSegment, endSegment)` are then swept once across
 * `[0, plan.segmentCount)`: a segment covered twice is an overlap, a segment
 * covered zero times is a gap. A `complete: true` result is returned only when
 * the assignments form an exact, non-overlapping tiling of the snapshot.
 */
export const verifyCs336A4CrawlShardDispatchCoverage = (
  input: Readonly<{
    assignments: ReadonlyArray<Cs336A4CrawlShardAssignment>
    plan: Cs336A4CrawlShardPlan
  }>,
): Cs336A4CrawlShardDispatchCoverageResult => {
  const { assignments, plan } = input

  if (assignments.length === 0) {
    return {
      complete: false,
      detail: 'no assignments were supplied to cover the plan.',
      reason: 'empty_assignment_set',
    }
  }

  const seenAssignmentRefs = new Set<string>()
  // Per-segment coverage count over [0, segmentCount); swept once after the
  // per-assignment checks so a gap/overlap is reported with an exact segment.
  const segmentCoverage = new Array<number>(plan.segmentCount).fill(0)

  for (const assignment of assignments) {
    if (assignment.planRef !== plan.planRef) {
      return {
        complete: false,
        detail: `assignment ${assignment.assignmentRef} planRef (${assignment.planRef}) does not match the plan (${plan.planRef}).`,
        reason: 'plan_ref_mismatch',
      }
    }

    if (assignment.acquisitionMode !== plan.acquisitionMode) {
      return {
        complete: false,
        detail: `assignment ${assignment.assignmentRef} acquisitionMode (${assignment.acquisitionMode}) does not match the plan (${plan.acquisitionMode}).`,
        reason: 'acquisition_mode_mismatch',
      }
    }

    const source = assignment.provenanceSource

    if (source.sourceRef !== plan.sourceRef) {
      return {
        complete: false,
        detail: `assignment ${assignment.assignmentRef} sourceRef (${source.sourceRef}) does not match the plan (${plan.sourceRef}).`,
        reason: 'source_ref_mismatch',
      }
    }

    if (source.snapshotRef !== plan.snapshotRef) {
      return {
        complete: false,
        detail: `assignment ${assignment.assignmentRef} snapshotRef (${source.snapshotRef}) does not match the plan (${plan.snapshotRef}).`,
        reason: 'snapshot_ref_mismatch',
      }
    }

    if (source.licenseRef !== plan.licenseRef) {
      return {
        complete: false,
        detail: `assignment ${assignment.assignmentRef} licenseRef (${source.licenseRef}) does not match the plan (${plan.licenseRef}).`,
        reason: 'license_ref_mismatch',
      }
    }

    if (seenAssignmentRefs.has(assignment.assignmentRef)) {
      return {
        complete: false,
        detail: `assignmentRef ${assignment.assignmentRef} appears more than once in the dispatch set.`,
        reason: 'duplicate_assignment_ref',
      }
    }
    seenAssignmentRefs.add(assignment.assignmentRef)

    if (
      !Number.isInteger(assignment.startSegment) ||
      !Number.isInteger(assignment.endSegment) ||
      assignment.endSegment <= assignment.startSegment ||
      assignment.endSegment - assignment.startSegment !==
        assignment.segmentCount
    ) {
      return {
        complete: false,
        detail: `assignment ${assignment.assignmentRef} has an invalid segment range [${String(assignment.startSegment)}, ${String(assignment.endSegment)}) for segmentCount ${String(assignment.segmentCount)}.`,
        reason: 'segment_range_invalid',
      }
    }

    if (
      assignment.startSegment < 0 ||
      assignment.endSegment > plan.segmentCount
    ) {
      return {
        complete: false,
        detail: `assignment ${assignment.assignmentRef} segment range [${assignment.startSegment}, ${assignment.endSegment}) falls outside the snapshot [0, ${plan.segmentCount}).`,
        reason: 'segment_out_of_bounds',
      }
    }

    for (
      let segment = assignment.startSegment;
      segment < assignment.endSegment;
      segment += 1
    ) {
      const previous = segmentCoverage[segment] ?? 0
      if (previous > 0) {
        return {
          complete: false,
          detail: `segment ${segment} is covered by more than one assignment.`,
          reason: 'duplicate_segment_coverage',
        }
      }
      segmentCoverage[segment] = previous + 1
    }
  }

  for (let segment = 0; segment < plan.segmentCount; segment += 1) {
    if ((segmentCoverage[segment] ?? 0) === 0) {
      return {
        complete: false,
        detail: `segment ${segment} is not covered by any assignment.`,
        reason: 'segment_gap',
      }
    }
  }

  return {
    assignmentCount: assignments.length,
    assignmentRefs: assignments.map(assignment => assignment.assignmentRef),
    complete: true,
    planRef: plan.planRef,
    segmentCount: plan.segmentCount,
    snapshotRef: plan.snapshotRef,
  }
}

/**
 * Fail-closed wrapper around `verifyCs336A4CrawlShardDispatchCoverage`: throws
 * `Cs336A4CrawlShardDispatchCoverageError` (carrying the failure reason) when
 * the assignment set is not an exact, non-overlapping cover of the plan, and
 * returns the plan's content-addressed `planRef` when it is. Use this on a
 * dispatch path where an incomplete or double-counted assignment set must
 * hard-fail before any paid work is dispatched.
 */
export const assertCs336A4CrawlShardDispatchCoverage = (
  input: Readonly<{
    assignments: ReadonlyArray<Cs336A4CrawlShardAssignment>
    plan: Cs336A4CrawlShardPlan
  }>,
): string => {
  const result = verifyCs336A4CrawlShardDispatchCoverage(input)

  if (!result.complete) {
    throw new Cs336A4CrawlShardDispatchCoverageError(
      result.reason,
      `CS336 A4 crawl-shard dispatch set does not cover its plan: ${result.detail}`,
    )
  }

  return result.planRef
}
