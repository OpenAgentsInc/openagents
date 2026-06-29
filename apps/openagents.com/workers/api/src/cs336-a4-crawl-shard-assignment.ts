import {
  Cs336A4CrawlPlanAcquisitionModes,
  type Cs336A4CrawlPlanAcquisitionMode,
  type Cs336A4CrawlShardPlan,
} from './cs336-a4-crawl-shard-plan'
import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'
import type { Cs336A4SourceProvenance } from './cs336-a4-provenance'

/**
 * Crawl-shard ASSIGNMENTS for CS336 A4 refinery acquisition
 * (`blocker.product_promises.crawl_scale_corpus_missing`).
 *
 * `buildCs336A4CrawlShardPlan` (cs336-a4-crawl-shard-plan.ts) produces a
 * deterministic partition of a crawl snapshot into bounded shard units,
 * and `buildCs336A4ProvenanceReceipt` (cs336-a4-provenance.ts) closes a
 * shard out with a source-provenance + transform-digest receipt keyed by
 * an `inputShardRef` and a `Cs336A4SourceProvenance`. Nothing connected
 * the two: a plan shard is an index + a content-addressed `shardRef`, not
 * an assignable, payable unit, and the provenance receipt's source
 * descriptor had to be re-typed by hand at the call site with no guarantee
 * it matched the plan the shard came from.
 *
 * This module is exactly that bridge. It turns ONE shard of a plan into a
 * deterministic `Cs336A4CrawlShardAssignment` carrying:
 *  - a content-addressed `assignmentRef` (stable per plan + shard), the
 *    stable identifier an operator dispatches paid work against and a
 *    settlement/provenance receipt binds to;
 *  - the `inputShardRef` (the plan shard's `shardRef`) to feed the
 *    provenance receipt; and
 *  - a ready-to-use `Cs336A4SourceProvenance` lifted verbatim from the
 *    plan, so the receipt's source descriptor cannot drift from the plan.
 *
 * Same plan + same shard index always yield the same assignment. It
 * MATERIALIZES NO PAYLOAD: it carries refs, indices, and license/source
 * identifiers only — never WARC records, URLs, contributor content,
 * wallet, or payment material. The public-safety guard fails closed
 * before any such material can be committed. Dispatching the assignment as
 * real PAID work, and acquiring its segments, remain separate planned
 * blockers.
 */

export const Cs336A4CrawlShardAssignmentSchemaVersion =
  'openagents.training.data_refinery.crawl_shard_assignment.v1' as const

export type Cs336A4CrawlShardAssignment = Readonly<{
  acquisitionMode: Cs336A4CrawlPlanAcquisitionMode
  /** Content-addressed assignment ref derived from contentDigestRef. */
  assignmentRef: string
  /** SHA-256 over the canonical assignment body (hex). */
  contentDigestRef: string
  /** One past the last segment index this shard covers (exclusive). */
  endSegment: number
  /** Index of this shard within the plan, 0-based. */
  index: number
  /** Plan shard ref; feed this as the provenance receipt `inputShardRef`. */
  inputShardRef: string
  jobKind: typeof Cs336A4DataRefineryJobKind
  /** Content-addressed plan ref this assignment was derived from. */
  planRef: string
  /** Source provenance lifted verbatim from the plan for the receipt. */
  provenanceSource: Cs336A4SourceProvenance
  schemaVersion: typeof Cs336A4CrawlShardAssignmentSchemaVersion
  /** Number of segments this shard covers. */
  segmentCount: number
  /** First segment index this shard covers (inclusive). */
  startSegment: number
}>

export class Cs336A4CrawlShardAssignmentValidationError extends Error {
  readonly _tag = 'Cs336A4CrawlShardAssignmentValidationError'
}

export class Cs336A4CrawlShardAssignmentUnsafeMaterialError extends Error {
  readonly _tag = 'Cs336A4CrawlShardAssignmentUnsafeMaterialError'
}

const unsafeAssignmentMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|bolt12|https?:\/\/|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(crawl|dataset|invoice|payment|payload|prompt|runner|shard|warc)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet|warc)/i

const assertJsonPublicSafe = (json: string): void => {
  if (unsafeAssignmentMaterialPattern.test(json)) {
    throw new Cs336A4CrawlShardAssignmentUnsafeMaterialError(
      'CS336 A4 crawl shard assignment contains crawl payload, URL, wallet, payment, or private material.',
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
 * Builds the canonical assignment body with fields in a fixed order so the
 * content digest is stable regardless of caller key ordering.
 */
const canonicalAssignmentBody = (
  input: Readonly<{
    acquisitionMode: Cs336A4CrawlPlanAcquisitionMode
    endSegment: number
    index: number
    inputShardRef: string
    planRef: string
    provenanceSource: Cs336A4SourceProvenance
    segmentCount: number
    startSegment: number
  }>,
): string =>
  JSON.stringify({
    acquisitionMode: input.acquisitionMode,
    endSegment: input.endSegment,
    index: input.index,
    inputShardRef: input.inputShardRef,
    jobKind: Cs336A4DataRefineryJobKind,
    planRef: input.planRef,
    provenanceSource: {
      acquisitionMode: input.provenanceSource.acquisitionMode,
      licenseRef: input.provenanceSource.licenseRef,
      snapshotRef: input.provenanceSource.snapshotRef,
      sourceRef: input.provenanceSource.sourceRef,
    },
    schemaVersion: Cs336A4CrawlShardAssignmentSchemaVersion,
    segmentCount: input.segmentCount,
    startSegment: input.startSegment,
  })

/**
 * Derives a deterministic, public-safe assignment for a SINGLE shard of a
 * crawl shard plan. Fails closed when:
 *  - the plan's acquisition mode is not a crawl-scale mode,
 *  - `index` is not an in-range integer for the plan's shard list,
 *  - the plan shard at `index` is internally inconsistent (its declared
 *    index does not match its position, or its segment range does not
 *    match its `segmentCount`), or
 *  - the assignment would carry crawl payload, URL, wallet, or private
 *    material.
 *
 * The returned `assignmentRef` is content-addressed: it is derived from a
 * SHA-256 over the canonical assignment body, so the same plan and shard
 * index always yield the same assignment ref. The `inputShardRef` and
 * `provenanceSource` are ready to feed `buildCs336A4ProvenanceReceipt`.
 */
export const deriveCs336A4CrawlShardAssignment = async (
  input: Readonly<{
    index: number
    plan: Cs336A4CrawlShardPlan
  }>,
): Promise<Cs336A4CrawlShardAssignment> => {
  const { index, plan } = input

  if (
    !(Cs336A4CrawlPlanAcquisitionModes as ReadonlyArray<string>).includes(
      plan.acquisitionMode,
    )
  ) {
    throw new Cs336A4CrawlShardAssignmentValidationError(
      `CS336 A4 crawl shard assignment only derives from crawl-scale acquisition modes; received: ${plan.acquisitionMode}.`,
    )
  }

  if (!Number.isInteger(index) || index < 0 || index >= plan.shards.length) {
    throw new Cs336A4CrawlShardAssignmentValidationError(
      `CS336 A4 crawl shard assignment index (${String(index)}) is out of range for a plan with ${plan.shards.length} shard(s).`,
    )
  }

  const shard = plan.shards[index]

  if (shard === undefined) {
    throw new Cs336A4CrawlShardAssignmentValidationError(
      `CS336 A4 crawl shard assignment found no shard at index ${index}.`,
    )
  }

  // Defensive integrity checks: a plan handed in from elsewhere must be
  // self-consistent before we mint an assignment (and a payable ref) for it.
  if (shard.index !== index) {
    throw new Cs336A4CrawlShardAssignmentValidationError(
      `CS336 A4 crawl shard assignment shard at position ${index} declares index ${shard.index}.`,
    )
  }

  if (shard.endSegment - shard.startSegment !== shard.segmentCount) {
    throw new Cs336A4CrawlShardAssignmentValidationError(
      `CS336 A4 crawl shard assignment shard ${index} segment range does not match its segmentCount.`,
    )
  }

  const provenanceSource: Cs336A4SourceProvenance = {
    acquisitionMode: plan.acquisitionMode,
    licenseRef: plan.licenseRef,
    snapshotRef: plan.snapshotRef,
    sourceRef: plan.sourceRef,
  }

  const body = canonicalAssignmentBody({
    acquisitionMode: plan.acquisitionMode,
    endSegment: shard.endSegment,
    index,
    inputShardRef: shard.shardRef,
    planRef: plan.planRef,
    provenanceSource,
    segmentCount: shard.segmentCount,
    startSegment: shard.startSegment,
  })

  assertJsonPublicSafe(body)

  const contentDigestRef = await sha256Hex(body)
  const assignmentRef = `assignment.cs336_a4.crawl_shard.${plan.snapshotRef}.${shard.startSegment}_${shard.endSegment}.${contentDigestRef.slice(0, 16)}`

  return {
    acquisitionMode: plan.acquisitionMode,
    assignmentRef,
    contentDigestRef,
    endSegment: shard.endSegment,
    index,
    inputShardRef: shard.shardRef,
    jobKind: Cs336A4DataRefineryJobKind,
    planRef: plan.planRef,
    provenanceSource,
    schemaVersion: Cs336A4CrawlShardAssignmentSchemaVersion,
    segmentCount: shard.segmentCount,
    startSegment: shard.startSegment,
  }
}

/**
 * Derives the deterministic assignment for EVERY shard in a plan, in plan
 * order. Each assignment is content-addressed and self-contained; the same
 * plan always yields the same ordered list of assignments. Fails closed on
 * the first shard that does not pass `deriveCs336A4CrawlShardAssignment`.
 */
export const deriveCs336A4CrawlShardAssignments = async (
  plan: Cs336A4CrawlShardPlan,
): Promise<ReadonlyArray<Cs336A4CrawlShardAssignment>> =>
  Promise.all(
    plan.shards.map((_, index) =>
      deriveCs336A4CrawlShardAssignment({ index, plan }),
    ),
  )
