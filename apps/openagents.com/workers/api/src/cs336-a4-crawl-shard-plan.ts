import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'
import type { Cs336A4AcquisitionMode } from './cs336-a4-provenance'

/**
 * Deterministic crawl-snapshot shard plan for CS336 A4 refinery
 * acquisition (`blocker.product_promises.crawl_scale_corpus_missing`).
 *
 * The current Psion corpus is a frozen bounded synthetic mixture. Moving
 * to crawl scale means dispatching refinery work over a real crawl
 * snapshot as PAID per-shard assignments — but before any segment can be
 * acquired, two parties (the operator dispatching paid work and the
 * contributor accepting it) must agree, byte-for-byte, on HOW the
 * snapshot partitions into assignable units. Without a deterministic
 * partition there is no stable `inputShardRef` to assign, to pay for, or
 * to bind a provenance receipt to.
 *
 * This module is exactly that planning layer and nothing more. It takes a
 * snapshot DESCRIPTOR (an immutable snapshot id, a source/license id, and
 * the snapshot's total segment count) and emits a deterministic, ordered
 * list of bounded shard units, each with a content-addressed `shardRef`
 * over its segment range. Same descriptor + same target shard count
 * always yield the same plan, so the plan can be recomputed and checked
 * by either party.
 *
 * It MATERIALIZES NO PAYLOAD: it never fetches WARC records, never carries
 * URLs, contributor content, wallet, or payment material. The
 * public-safety guard fails closed before any such material can be
 * committed. The shard refs it emits are intended to feed the
 * `public_crawl_snapshot` / `licensed_public_dataset` acquisition modes of
 * `cs336-a4-provenance.ts` as `inputShardRef` values once real acquisition
 * exists — that real acquisition remains a separate planned blocker.
 */

export const Cs336A4CrawlShardPlanSchemaVersion =
  'openagents.training.data_refinery.crawl_shard_plan.v1' as const

/**
 * Acquisition modes a crawl-scale shard plan is allowed to partition. The
 * frozen bounded synthetic mixture is NOT a crawl snapshot and must not be
 * planned through this path; it has no segments to assign.
 */
export const Cs336A4CrawlPlanAcquisitionModes = [
  'licensed_public_dataset',
  'public_crawl_snapshot',
] as const satisfies ReadonlyArray<Cs336A4AcquisitionMode>

export type Cs336A4CrawlPlanAcquisitionMode =
  (typeof Cs336A4CrawlPlanAcquisitionModes)[number]

export type Cs336A4CrawlSnapshotDescriptor = Readonly<{
  acquisitionMode: Cs336A4CrawlPlanAcquisitionMode
  /** License / usage-terms identifier under which the snapshot is admitted. */
  licenseRef: string
  /** Total number of immutable segments the snapshot is divided into. */
  segmentCount: number
  /** Immutable snapshot identifier (e.g. a crawl snapshot id). */
  snapshotRef: string
  /** Stable origin identifier for the snapshot source. */
  sourceRef: string
}>

export type Cs336A4CrawlShardUnit = Readonly<{
  /** Inclusive index of this shard within the plan, 0-based. */
  index: number
  /** Number of segments this shard covers (`endSegment - startSegment`). */
  segmentCount: number
  /** Content-addressed shard ref over the snapshot + segment range. */
  shardRef: string
  /** First segment index this shard covers (inclusive). */
  startSegment: number
  /** One past the last segment index this shard covers (exclusive). */
  endSegment: number
}>

export type Cs336A4CrawlShardPlan = Readonly<{
  acquisitionMode: Cs336A4CrawlPlanAcquisitionMode
  /** SHA-256 over the canonical plan body (hex). */
  contentDigestRef: string
  jobKind: typeof Cs336A4DataRefineryJobKind
  licenseRef: string
  /** Content-addressed plan ref derived from contentDigestRef. */
  planRef: string
  schemaVersion: typeof Cs336A4CrawlShardPlanSchemaVersion
  /** Total segments partitioned; equals the sum of shard segment counts. */
  segmentCount: number
  shards: ReadonlyArray<Cs336A4CrawlShardUnit>
  snapshotRef: string
  sourceRef: string
}>

export class Cs336A4CrawlShardPlanValidationError extends Error {
  readonly _tag = 'Cs336A4CrawlShardPlanValidationError'
}

export class Cs336A4CrawlShardPlanUnsafeMaterialError extends Error {
  readonly _tag = 'Cs336A4CrawlShardPlanUnsafeMaterialError'
}

const unsafePlanMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|bolt12|https?:\/\/|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(crawl|dataset|invoice|payment|payload|prompt|runner|shard|warc)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet|warc)/i

const assertJsonPublicSafe = (json: string): void => {
  if (unsafePlanMaterialPattern.test(json)) {
    throw new Cs336A4CrawlShardPlanUnsafeMaterialError(
      'CS336 A4 crawl shard plan contains crawl payload, URL, wallet, payment, or private material.',
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

const requireNonEmptyRef = (label: string, value: string): string => {
  const trimmed = value.trim()

  if (trimmed === '') {
    throw new Cs336A4CrawlShardPlanValidationError(
      `CS336 A4 crawl shard plan requires a non-empty ${label}.`,
    )
  }

  return trimmed
}

const requirePositiveInteger = (label: string, value: number): number => {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Cs336A4CrawlShardPlanValidationError(
      `CS336 A4 crawl shard plan requires a positive integer ${label}; received ${String(value)}.`,
    )
  }

  return value
}

const canonicalPlanBody = (
  input: Readonly<{
    acquisitionMode: Cs336A4CrawlPlanAcquisitionMode
    licenseRef: string
    segmentCount: number
    shards: ReadonlyArray<Cs336A4CrawlShardUnit>
    snapshotRef: string
    sourceRef: string
  }>,
): string =>
  JSON.stringify({
    acquisitionMode: input.acquisitionMode,
    jobKind: Cs336A4DataRefineryJobKind,
    licenseRef: input.licenseRef,
    schemaVersion: Cs336A4CrawlShardPlanSchemaVersion,
    segmentCount: input.segmentCount,
    shards: input.shards.map(shard => ({
      endSegment: shard.endSegment,
      index: shard.index,
      segmentCount: shard.segmentCount,
      shardRef: shard.shardRef,
      startSegment: shard.startSegment,
    })),
    snapshotRef: input.snapshotRef,
    sourceRef: input.sourceRef,
  })

/**
 * Partitions `segmentCount` segments across `shardCount` shards as evenly
 * as possible, deterministically. The first `segmentCount % shardCount`
 * shards each take one extra segment (the remainder is front-loaded), so
 * the partition is a pure function of the two integers — no ordering
 * ambiguity, no floating point.
 */
const partitionSegments = (
  segmentCount: number,
  shardCount: number,
): ReadonlyArray<Readonly<{ endSegment: number; startSegment: number }>> => {
  const base = Math.floor(segmentCount / shardCount)
  const remainder = segmentCount % shardCount
  const ranges: Array<{ endSegment: number; startSegment: number }> = []
  let cursor = 0

  for (let index = 0; index < shardCount; index += 1) {
    const size = base + (index < remainder ? 1 : 0)
    ranges.push({ endSegment: cursor + size, startSegment: cursor })
    cursor += size
  }

  return ranges
}

/**
 * Builds a deterministic, public-safe crawl-snapshot shard plan. Fails
 * closed when:
 *  - the acquisition mode is not a crawl-scale mode (the bounded synthetic
 *    mixture has no segments to assign),
 *  - any ref is empty,
 *  - `segmentCount` or `targetShardCount` is not a positive integer,
 *  - `targetShardCount` exceeds `segmentCount` (a shard would be empty;
 *    an empty shard cannot be assigned or paid for), or
 *  - the descriptor would carry crawl payload, URL, wallet, or private
 *    material.
 *
 * The returned `planRef` is content-addressed: it is derived from a
 * SHA-256 over the canonical plan body, so the same descriptor and target
 * shard count always yield the same plan ref. The sum of every shard's
 * `segmentCount` always equals the snapshot `segmentCount`, and the shard
 * ranges tile the snapshot with no gaps and no overlaps.
 */
export const buildCs336A4CrawlShardPlan = async (
  input: Readonly<{
    descriptor: Cs336A4CrawlSnapshotDescriptor
    targetShardCount: number
  }>,
): Promise<Cs336A4CrawlShardPlan> => {
  const { descriptor } = input

  if (
    !(Cs336A4CrawlPlanAcquisitionModes as ReadonlyArray<string>).includes(
      descriptor.acquisitionMode,
    )
  ) {
    throw new Cs336A4CrawlShardPlanValidationError(
      `CS336 A4 crawl shard plan only partitions crawl-scale acquisition modes; received: ${descriptor.acquisitionMode}.`,
    )
  }

  const snapshotRef = requireNonEmptyRef('snapshotRef', descriptor.snapshotRef)
  const sourceRef = requireNonEmptyRef('sourceRef', descriptor.sourceRef)
  const licenseRef = requireNonEmptyRef('licenseRef', descriptor.licenseRef)
  const segmentCount = requirePositiveInteger(
    'segmentCount',
    descriptor.segmentCount,
  )
  const targetShardCount = requirePositiveInteger(
    'targetShardCount',
    input.targetShardCount,
  )

  if (targetShardCount > segmentCount) {
    throw new Cs336A4CrawlShardPlanValidationError(
      `CS336 A4 crawl shard plan targetShardCount (${targetShardCount}) exceeds segmentCount (${segmentCount}); a shard with no segments cannot be assigned.`,
    )
  }

  const ranges = partitionSegments(segmentCount, targetShardCount)

  const shards = await Promise.all(
    ranges.map(async (range, index): Promise<Cs336A4CrawlShardUnit> => {
      const shardDigest = await sha256Hex(
        JSON.stringify({
          acquisitionMode: descriptor.acquisitionMode,
          endSegment: range.endSegment,
          schemaVersion: Cs336A4CrawlShardPlanSchemaVersion,
          snapshotRef,
          startSegment: range.startSegment,
        }),
      )

      return {
        endSegment: range.endSegment,
        index,
        segmentCount: range.endSegment - range.startSegment,
        shardRef: `shard.cs336_a4.crawl.${snapshotRef}.${range.startSegment}_${range.endSegment}.${shardDigest.slice(0, 16)}`,
        startSegment: range.startSegment,
      }
    }),
  )

  const body = canonicalPlanBody({
    acquisitionMode: descriptor.acquisitionMode,
    licenseRef,
    segmentCount,
    shards,
    snapshotRef,
    sourceRef,
  })

  assertJsonPublicSafe(body)

  const contentDigestRef = await sha256Hex(body)
  const planRef = `plan.cs336_a4.crawl_shard.${snapshotRef}.${contentDigestRef.slice(0, 16)}`

  return {
    acquisitionMode: descriptor.acquisitionMode,
    contentDigestRef,
    jobKind: Cs336A4DataRefineryJobKind,
    licenseRef,
    planRef,
    schemaVersion: Cs336A4CrawlShardPlanSchemaVersion,
    segmentCount,
    shards,
    snapshotRef,
    sourceRef,
  }
}
