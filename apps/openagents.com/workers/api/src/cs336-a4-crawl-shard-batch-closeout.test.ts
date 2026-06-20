import { describe, expect, it } from 'vitest'

import {
  deriveCs336A4CrawlShardAssignments,
  type Cs336A4CrawlShardAssignment,
} from './cs336-a4-crawl-shard-assignment'
import {
  buildCs336A4CrawlShardBatchCloseoutReceipt,
  Cs336A4CrawlShardBatchCloseoutError,
  Cs336A4CrawlShardBatchCloseoutSchemaVersion,
} from './cs336-a4-crawl-shard-batch-closeout'
import {
  buildCs336A4CrawlShardDispatchManifest,
  type Cs336A4CrawlShardDispatchManifest,
} from './cs336-a4-crawl-shard-dispatch-manifest'
import {
  buildCs336A4CrawlShardPlan,
  type Cs336A4CrawlShardPlan,
  type Cs336A4CrawlSnapshotDescriptor,
} from './cs336-a4-crawl-shard-plan'
import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'
import {
  Cs336A4ProvenanceSchemaVersion,
  type Cs336A4ProvenanceReceipt,
} from './cs336-a4-provenance'

const descriptor: Cs336A4CrawlSnapshotDescriptor = {
  acquisitionMode: 'public_crawl_snapshot',
  licenseRef: 'license.public.cc_main.2026_05',
  segmentCount: 10,
  snapshotRef: 'snapshot.cc_main.2026_05',
  sourceRef: 'source.cc_main',
}

const buildPlan = (): Promise<Cs336A4CrawlShardPlan> =>
  buildCs336A4CrawlShardPlan({ descriptor, targetShardCount: 4 })

/**
 * A receipt that correctly closes out an assignment. The closeout gate reads
 * only the assignment-binding fields (assignmentRef, inputShardRef,
 * provenance) plus the content-addressed receiptRef; the transform-chain
 * validity is the receipt builder's responsibility, so the fixture carries a
 * minimal recompute-verified chain.
 */
const receiptFor = (
  assignment: Cs336A4CrawlShardAssignment,
): Cs336A4ProvenanceReceipt => ({
  assignmentRef: assignment.assignmentRef,
  contentDigestRef: `digest.${assignment.index}`,
  finalOutputDigestRef: `final.${assignment.index}`,
  inputShardRef: assignment.inputShardRef,
  jobKind: Cs336A4DataRefineryJobKind,
  provenance: assignment.provenanceSource,
  receiptRef: `receipt.cs336_a4.provenance.${assignment.assignmentRef}`,
  recomputeVerified: true,
  schemaVersion: Cs336A4ProvenanceSchemaVersion,
  sourceInputDigestRef: `source-input.${assignment.index}`,
  transformChain: [],
})

const setup = async (): Promise<{
  assignments: ReadonlyArray<Cs336A4CrawlShardAssignment>
  manifest: Cs336A4CrawlShardDispatchManifest
  receipts: Cs336A4ProvenanceReceipt[]
}> => {
  const plan = await buildPlan()
  const assignments = await deriveCs336A4CrawlShardAssignments(plan)
  const manifest = await buildCs336A4CrawlShardDispatchManifest({
    assignments,
    plan,
  })
  const receipts = assignments.map(receiptFor)
  return { assignments, manifest, receipts }
}

describe('buildCs336A4CrawlShardBatchCloseoutReceipt', () => {
  it('emits a content-addressed closeout for a fully closed-out batch', async () => {
    const { assignments, manifest, receipts } = await setup()

    const closeout = await buildCs336A4CrawlShardBatchCloseoutReceipt({
      assignments,
      manifest,
      receipts,
    })

    expect(closeout.schemaVersion).toBe(
      Cs336A4CrawlShardBatchCloseoutSchemaVersion,
    )
    expect(closeout.manifestRef).toBe(manifest.manifestRef)
    expect(closeout.planRef).toBe(manifest.planRef)
    expect(closeout.snapshotRef).toBe(manifest.snapshotRef)
    expect(closeout.assignmentCount).toBe(manifest.assignmentCount)
    expect(closeout.closures).toHaveLength(assignments.length)
    expect(closeout.closeoutRef).toContain(manifest.snapshotRef)
    expect(closeout.contentDigestRef).toMatch(/^[0-9a-f]{64}$/)
    // closures cover exactly the dispatched assignment set
    expect(new Set(closeout.closures.map(c => c.assignmentRef))).toEqual(
      new Set(manifest.assignmentRefs),
    )
  })

  it('is deterministic and receipt-order independent', async () => {
    const { assignments, manifest, receipts } = await setup()

    const first = await buildCs336A4CrawlShardBatchCloseoutReceipt({
      assignments,
      manifest,
      receipts,
    })
    const reversed = await buildCs336A4CrawlShardBatchCloseoutReceipt({
      assignments: [...assignments].reverse(),
      manifest,
      receipts: [...receipts].reverse(),
    })

    expect(reversed.closeoutRef).toBe(first.closeoutRef)
    expect(reversed.contentDigestRef).toBe(first.contentDigestRef)
    expect(reversed.closures).toEqual(first.closures)
  })

  it('rejects an empty manifest', async () => {
    const { assignments, manifest, receipts } = await setup()
    const empty: Cs336A4CrawlShardDispatchManifest = {
      ...manifest,
      assignmentRefs: [],
      assignmentCount: 0,
    }

    await expect(
      buildCs336A4CrawlShardBatchCloseoutReceipt({
        assignments,
        manifest: empty,
        receipts,
      }),
    ).rejects.toMatchObject({ reason: 'empty_manifest' })
  })

  it('rejects an unclosed dispatched assignment (gap)', async () => {
    const { assignments, manifest, receipts } = await setup()
    // Drop one receipt: the dispatched assignment it would have closed is left
    // with no provenance.
    const partial = receipts.slice(0, -1)

    try {
      await buildCs336A4CrawlShardBatchCloseoutReceipt({
        assignments,
        manifest,
        receipts: partial,
      })
      expect.unreachable('unclosed assignment should hard-fail')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardBatchCloseoutError)
      if (error instanceof Cs336A4CrawlShardBatchCloseoutError) {
        expect(error.reason).toBe('unclosed_assignment')
      }
    }
  })

  it('rejects a receipt for an assignment not in the batch', async () => {
    const { assignments, manifest, receipts } = await setup()
    const stray: Cs336A4ProvenanceReceipt = {
      ...receipts[0]!,
      assignmentRef: 'assignment.cs336_a4.crawl_shard.other.0_1.deadbeefdeadbeef',
    }

    try {
      await buildCs336A4CrawlShardBatchCloseoutReceipt({
        assignments,
        manifest,
        receipts: [...receipts, stray],
      })
      expect.unreachable('undispatched receipt should hard-fail')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardBatchCloseoutError)
      if (error instanceof Cs336A4CrawlShardBatchCloseoutError) {
        expect(error.reason).toBe('receipt_for_undispatched_assignment')
      }
    }
  })

  it('rejects two receipts that close the same assignment', async () => {
    const { assignments, manifest, receipts } = await setup()

    try {
      await buildCs336A4CrawlShardBatchCloseoutReceipt({
        assignments,
        manifest,
        receipts: [...receipts, receiptFor(assignments[0]!)],
      })
      expect.unreachable('duplicate receipt should hard-fail')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardBatchCloseoutError)
      if (error instanceof Cs336A4CrawlShardBatchCloseoutError) {
        expect(error.reason).toBe('duplicate_receipt')
      }
    }
  })

  it('rejects a receipt that re-attributes the corpus source (binding failure)', async () => {
    const { assignments, manifest, receipts } = await setup()
    const tampered: Cs336A4ProvenanceReceipt = {
      ...receipts[0]!,
      provenance: {
        ...receipts[0]!.provenance,
        sourceRef: 'source.unrelated_corpus',
      },
    }

    try {
      await buildCs336A4CrawlShardBatchCloseoutReceipt({
        assignments,
        manifest,
        receipts: [tampered, ...receipts.slice(1)],
      })
      expect.unreachable('re-attributed source should hard-fail')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardBatchCloseoutError)
      if (error instanceof Cs336A4CrawlShardBatchCloseoutError) {
        expect(error.reason).toBe('provenance_binding_failed')
        expect(error.bindingReason).toBe('source_ref_mismatch')
      }
    }
  })

  it('rejects an assignment set that does not match the manifest', async () => {
    const { assignments, manifest, receipts } = await setup()

    try {
      await buildCs336A4CrawlShardBatchCloseoutReceipt({
        assignments: assignments.slice(0, -1),
        manifest,
        receipts,
      })
      expect.unreachable('mismatched assignment set should hard-fail')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardBatchCloseoutError)
      if (error instanceof Cs336A4CrawlShardBatchCloseoutError) {
        expect(error.reason).toBe('assignment_set_mismatch')
      }
    }
  })
})
