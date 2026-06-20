import {
  assertCs336A4CrawlShardAssignmentAuthenticity,
  type Cs336A4CrawlShardAssignmentAuthenticityMismatch,
} from './cs336-a4-crawl-shard-assignment-authenticity'
import type { Cs336A4CrawlShardAssignment } from './cs336-a4-crawl-shard-assignment'
import {
  assertCs336A4CrawlShardDispatchCoverage,
  type Cs336A4CrawlShardDispatchCoverageFailure,
} from './cs336-a4-crawl-shard-dispatch-coverage'
import type { Cs336A4CrawlPlanAcquisitionMode } from './cs336-a4-crawl-shard-plan'
import type { Cs336A4CrawlShardPlan } from './cs336-a4-crawl-shard-plan'
import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'

/**
 * Crawl-shard DISPATCH MANIFEST for CS336 A4 refinery acquisition
 * (`blocker.product_promises.crawl_scale_corpus_missing`).
 *
 * Two deterministic gates already exist for the crawl-scale dispatch path but
 * nothing composes them, so an operator about to pay for a batch of crawl-shard
 * assignments has no single, auditable record that the batch is BOTH genuine
 * AND complete:
 *
 *  1. `verifyCs336A4CrawlShardDispatchCoverage`
 *     (cs336-a4-crawl-shard-dispatch-coverage.ts) proves a SET of assignments
 *     tiles the plan with no gap/overlap — but it explicitly "does NOT
 *     re-derive assignment refs": it trusts each `assignmentRef` /
 *     `contentDigestRef` BY VALUE. A set can therefore pass coverage while one
 *     assignment carries a FORGED or STALE content-addressed ref (the very
 *     identifier payment and a provenance receipt later bind to).
 *  2. `verifyCs336A4CrawlShardAssignmentAuthenticity`
 *     (cs336-a4-crawl-shard-assignment-authenticity.ts) re-derives ONE
 *     assignment from the plan and catches a forged/stale ref — but it says
 *     nothing about whether the SET an operator is paying for actually covers
 *     the snapshot. Every individual assignment can be authentic while the
 *     batch still has a gap (corpus never acquired) or an overlap (paid twice).
 *
 * This module is the fail-closed composition that closes both at once. It is
 * the single entry point a dispatch path should call before paying for a batch:
 * it asserts EVERY assignment is the genuine content-addressed unit the plan
 * derives (authenticity), asserts the batch is an exact non-overlapping cover
 * of the plan (coverage), and only then emits a deterministic,
 * content-addressed `Cs336A4CrawlShardDispatchManifest` — the auditable record
 * that "this verified batch is ready to dispatch as paid crawl-scale work".
 *
 * It is a pure composition: it accepts already-built artifacts, re-uses the two
 * existing fail-closed gates verbatim (re-raising their typed errors), and
 * never fetches, fabricates, mutates, or settles anything. It does NOT acquire
 * any segment, does NOT bind a returned provenance receipt (that is the
 * provenance-binding gate's job), and does NOT settle payment.
 */

export const Cs336A4CrawlShardDispatchManifestSchemaVersion =
  'openagents.training.data_refinery.crawl_shard_dispatch_manifest.v1' as const

export type Cs336A4CrawlShardDispatchManifest = Readonly<{
  acquisitionMode: Cs336A4CrawlPlanAcquisitionMode
  /** Ordered (by startSegment) assignment refs the batch dispatches paid work against. */
  assignmentRefs: ReadonlyArray<string>
  /** Number of assignments in the verified batch. */
  assignmentCount: number
  /** SHA-256 over the canonical manifest body (hex). */
  contentDigestRef: string
  jobKind: typeof Cs336A4DataRefineryJobKind
  licenseRef: string
  /** Content-addressed manifest ref derived from contentDigestRef. */
  manifestRef: string
  /** Content-addressed plan ref the batch was verified against. */
  planRef: string
  schemaVersion: typeof Cs336A4CrawlShardDispatchManifestSchemaVersion
  /** Total snapshot segments the batch covers (equals plan.segmentCount). */
  segmentCount: number
  snapshotRef: string
  sourceRef: string
}>

/**
 * Thrown when the batch fails the composed checks. `stage` records which gate
 * rejected, and `reason` carries that gate's own typed failure reason so a
 * dispatch audit trail can record the exact cause.
 */
export class Cs336A4CrawlShardDispatchManifestError extends Error {
  readonly _tag = 'Cs336A4CrawlShardDispatchManifestError'
  readonly stage: 'authenticity' | 'coverage'
  readonly reason:
    | Cs336A4CrawlShardAssignmentAuthenticityMismatch
    | Cs336A4CrawlShardDispatchCoverageFailure

  constructor(
    stage: 'authenticity' | 'coverage',
    reason:
      | Cs336A4CrawlShardAssignmentAuthenticityMismatch
      | Cs336A4CrawlShardDispatchCoverageFailure,
    detail: string,
  ) {
    super(detail)
    this.stage = stage
    this.reason = reason
  }
}

export class Cs336A4CrawlShardDispatchManifestUnsafeMaterialError extends Error {
  readonly _tag = 'Cs336A4CrawlShardDispatchManifestUnsafeMaterialError'
}

const unsafeManifestMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|bolt12|https?:\/\/|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(crawl|dataset|invoice|payment|payload|prompt|runner|shard|warc)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet|warc)/i

const assertJsonPublicSafe = (json: string): void => {
  if (unsafeManifestMaterialPattern.test(json)) {
    throw new Cs336A4CrawlShardDispatchManifestUnsafeMaterialError(
      'CS336 A4 crawl shard dispatch manifest contains crawl payload, URL, wallet, payment, or private material.',
    )
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
 * Builds the canonical manifest body with fields in a fixed order so the
 * content digest is stable regardless of caller key/assignment ordering. The
 * assignment refs are sorted by their segment range so the same batch (in any
 * input order) yields the same manifest ref.
 */
const canonicalManifestBody = (
  input: Readonly<{
    acquisitionMode: Cs336A4CrawlPlanAcquisitionMode
    assignmentRefs: ReadonlyArray<string>
    licenseRef: string
    planRef: string
    segmentCount: number
    snapshotRef: string
    sourceRef: string
  }>,
): string =>
  JSON.stringify({
    acquisitionMode: input.acquisitionMode,
    assignmentCount: input.assignmentRefs.length,
    assignmentRefs: input.assignmentRefs,
    jobKind: Cs336A4DataRefineryJobKind,
    licenseRef: input.licenseRef,
    planRef: input.planRef,
    schemaVersion: Cs336A4CrawlShardDispatchManifestSchemaVersion,
    segmentCount: input.segmentCount,
    snapshotRef: input.snapshotRef,
    sourceRef: input.sourceRef,
  })

/**
 * Composes the per-assignment authenticity gate and the set-level coverage
 * gate into one deterministic, fail-closed dispatch manifest for a single plan.
 *
 * Order of operations:
 *  1. Re-derive and verify EVERY assignment against the trusted plan
 *     (`assertCs336A4CrawlShardAssignmentAuthenticity`), so a forged/stale
 *     content-addressed ref hard-fails before coverage trusts it by value.
 *  2. Verify the batch is an exact non-overlapping cover of the plan
 *     (`assertCs336A4CrawlShardDispatchCoverage`), so a gap or double-count
 *     hard-fails before any paid work is dispatched.
 *  3. Emit a content-addressed manifest binding the verified batch (planRef,
 *     ordered authentic assignment refs, snapshot provenance) into one
 *     auditable record. The `manifestRef` is content-addressed via SHA-256 over
 *     a canonical body, so the same plan + batch (in any order) always yields
 *     the same ref.
 *
 * Re-raises the underlying typed errors
 * (`Cs336A4CrawlShardAssignmentAuthenticityError`,
 * `Cs336A4CrawlShardDispatchCoverageError`) wrapped as a
 * `Cs336A4CrawlShardDispatchManifestError` carrying the originating `stage` and
 * `reason`. Emits refs/indices/license/source identifiers only; the
 * public-safety guard fails closed before any unsafe material is committed.
 */
export const buildCs336A4CrawlShardDispatchManifest = async (
  input: Readonly<{
    assignments: ReadonlyArray<Cs336A4CrawlShardAssignment>
    plan: Cs336A4CrawlShardPlan
  }>,
): Promise<Cs336A4CrawlShardDispatchManifest> => {
  const { assignments, plan } = input

  // Stage 1: every assignment must be the genuine content-addressed unit the
  // plan derives. Run BEFORE coverage so a forged/stale ref (which coverage
  // trusts by value) cannot slip into a "complete" batch.
  for (const assignment of assignments) {
    try {
      await assertCs336A4CrawlShardAssignmentAuthenticity({ assignment, plan })
    } catch (error) {
      const reason =
        error instanceof Error && '_tag' in error
          ? (error as { reason?: Cs336A4CrawlShardAssignmentAuthenticityMismatch })
              .reason
          : undefined
      throw new Cs336A4CrawlShardDispatchManifestError(
        'authenticity',
        reason ?? 'assignment_ref_mismatch',
        `CS336 A4 crawl-shard dispatch manifest rejected a non-authentic assignment: ${
          error instanceof Error ? error.message : String(error)
        }`,
      )
    }
  }

  // Stage 2: the verified batch must be an exact non-overlapping cover of the
  // plan (no gap, no double-count).
  try {
    assertCs336A4CrawlShardDispatchCoverage({ assignments, plan })
  } catch (error) {
    const reason =
      error instanceof Error && '_tag' in error
        ? (error as { reason?: Cs336A4CrawlShardDispatchCoverageFailure }).reason
        : undefined
    throw new Cs336A4CrawlShardDispatchManifestError(
      'coverage',
      reason ?? 'segment_gap',
      `CS336 A4 crawl-shard dispatch manifest rejected an incomplete batch: ${
        error instanceof Error ? error.message : String(error)
      }`,
    )
  }

  // Order the batch canonically by segment range so the manifest ref does not
  // depend on caller input ordering.
  const ordered = [...assignments].sort(
    (left, right) => left.startSegment - right.startSegment,
  )
  const assignmentRefs = ordered.map(assignment => assignment.assignmentRef)

  const body = canonicalManifestBody({
    acquisitionMode: plan.acquisitionMode,
    assignmentRefs,
    licenseRef: plan.licenseRef,
    planRef: plan.planRef,
    segmentCount: plan.segmentCount,
    snapshotRef: plan.snapshotRef,
    sourceRef: plan.sourceRef,
  })

  assertJsonPublicSafe(body)

  const contentDigestRef = await sha256Hex(body)
  const manifestRef = `manifest.cs336_a4.crawl_shard_dispatch.${plan.snapshotRef}.${contentDigestRef.slice(0, 16)}`

  return {
    acquisitionMode: plan.acquisitionMode,
    assignmentRefs,
    assignmentCount: assignmentRefs.length,
    contentDigestRef,
    jobKind: Cs336A4DataRefineryJobKind,
    licenseRef: plan.licenseRef,
    manifestRef,
    planRef: plan.planRef,
    schemaVersion: Cs336A4CrawlShardDispatchManifestSchemaVersion,
    segmentCount: plan.segmentCount,
    snapshotRef: plan.snapshotRef,
    sourceRef: plan.sourceRef,
  }
}
