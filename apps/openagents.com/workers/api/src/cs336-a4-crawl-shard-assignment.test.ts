import { describe, expect, it } from 'vitest'

import {
  Cs336A4CrawlShardAssignmentSchemaVersion,
  Cs336A4CrawlShardAssignmentUnsafeMaterialError,
  Cs336A4CrawlShardAssignmentValidationError,
  deriveCs336A4CrawlShardAssignment,
  deriveCs336A4CrawlShardAssignments,
} from './cs336-a4-crawl-shard-assignment'
import {
  buildCs336A4CrawlShardPlan,
  type Cs336A4CrawlShardPlan,
  type Cs336A4CrawlSnapshotDescriptor,
} from './cs336-a4-crawl-shard-plan'
import { buildCs336A4ProvenanceReceipt } from './cs336-a4-provenance'

const descriptor: Cs336A4CrawlSnapshotDescriptor = {
  acquisitionMode: 'public_crawl_snapshot',
  licenseRef: 'license.public.cc_main.2026_05',
  segmentCount: 10,
  snapshotRef: 'snapshot.cc_main.2026_05',
  sourceRef: 'source.cc_main',
}

const buildPlan = (): Promise<Cs336A4CrawlShardPlan> =>
  buildCs336A4CrawlShardPlan({ descriptor, targetShardCount: 4 })

describe('deriveCs336A4CrawlShardAssignment', () => {
  it('derives an assignment that mirrors the plan shard at the index', async () => {
    const plan = await buildPlan()
    const assignment = await deriveCs336A4CrawlShardAssignment({
      index: 1,
      plan,
    })
    const shard = plan.shards[1]

    expect(assignment.schemaVersion).toBe(
      Cs336A4CrawlShardAssignmentSchemaVersion,
    )
    expect(assignment.index).toBe(1)
    expect(assignment.planRef).toBe(plan.planRef)
    expect(assignment.inputShardRef).toBe(shard?.shardRef)
    expect(assignment.startSegment).toBe(shard?.startSegment)
    expect(assignment.endSegment).toBe(shard?.endSegment)
    expect(assignment.segmentCount).toBe(shard?.segmentCount)
    expect(assignment.acquisitionMode).toBe(plan.acquisitionMode)
  })

  it('lifts the plan source provenance verbatim for the receipt', async () => {
    const plan = await buildPlan()
    const assignment = await deriveCs336A4CrawlShardAssignment({
      index: 0,
      plan,
    })

    expect(assignment.provenanceSource).toEqual({
      acquisitionMode: plan.acquisitionMode,
      licenseRef: plan.licenseRef,
      snapshotRef: plan.snapshotRef,
      sourceRef: plan.sourceRef,
    })
  })

  it('is deterministic: same plan + index yields the same assignmentRef', async () => {
    const planA = await buildPlan()
    const planB = await buildPlan()
    const a = await deriveCs336A4CrawlShardAssignment({ index: 2, plan: planA })
    const b = await deriveCs336A4CrawlShardAssignment({ index: 2, plan: planB })

    expect(a.assignmentRef).toBe(b.assignmentRef)
    expect(a.contentDigestRef).toBe(b.contentDigestRef)
  })

  it('mints distinct assignmentRefs for distinct shards', async () => {
    const plan = await buildPlan()
    const a = await deriveCs336A4CrawlShardAssignment({ index: 0, plan })
    const b = await deriveCs336A4CrawlShardAssignment({ index: 1, plan })

    expect(a.assignmentRef).not.toBe(b.assignmentRef)
  })

  it('content-addresses the assignmentRef from the content digest', async () => {
    const plan = await buildPlan()
    const assignment = await deriveCs336A4CrawlShardAssignment({
      index: 3,
      plan,
    })

    expect(assignment.assignmentRef).toContain(
      assignment.contentDigestRef.slice(0, 16),
    )
    expect(assignment.assignmentRef).toContain(plan.snapshotRef)
  })

  it('rejects an out-of-range index', async () => {
    const plan = await buildPlan()

    await expect(
      deriveCs336A4CrawlShardAssignment({ index: plan.shards.length, plan }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardAssignmentValidationError)
    await expect(
      deriveCs336A4CrawlShardAssignment({ index: -1, plan }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardAssignmentValidationError)
  })

  it('rejects a non-integer index', async () => {
    const plan = await buildPlan()

    await expect(
      deriveCs336A4CrawlShardAssignment({ index: 1.5, plan }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardAssignmentValidationError)
  })

  it('rejects a plan shard whose declared index does not match its position', async () => {
    const plan = await buildPlan()
    const tampered: Cs336A4CrawlShardPlan = {
      ...plan,
      shards: plan.shards.map((shard, index) =>
        index === 1 ? { ...shard, index: 7 } : shard,
      ),
    }

    await expect(
      deriveCs336A4CrawlShardAssignment({ index: 1, plan: tampered }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardAssignmentValidationError)
  })

  it('rejects a plan shard whose range does not match its segmentCount', async () => {
    const plan = await buildPlan()
    const tampered: Cs336A4CrawlShardPlan = {
      ...plan,
      shards: plan.shards.map((shard, index) =>
        index === 0 ? { ...shard, segmentCount: shard.segmentCount + 1 } : shard,
      ),
    }

    await expect(
      deriveCs336A4CrawlShardAssignment({ index: 0, plan: tampered }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardAssignmentValidationError)
  })

  it('rejects a non-crawl acquisition mode', async () => {
    const plan = await buildPlan()
    const tampered = {
      ...plan,
      acquisitionMode: 'bounded_synthetic_corpus',
    } as unknown as Cs336A4CrawlShardPlan

    await expect(
      deriveCs336A4CrawlShardAssignment({ index: 0, plan: tampered }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardAssignmentValidationError)
  })

  it('fails closed when a plan ref carries unsafe material', async () => {
    const plan = await buildPlan()
    const tampered: Cs336A4CrawlShardPlan = {
      ...plan,
      planRef: 'plan.cs336_a4.crawl_shard.wallet.seed',
    }

    await expect(
      deriveCs336A4CrawlShardAssignment({ index: 0, plan: tampered }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardAssignmentUnsafeMaterialError)
  })

  it('feeds buildCs336A4ProvenanceReceipt without re-typing the source', async () => {
    const plan = await buildPlan()
    const assignment = await deriveCs336A4CrawlShardAssignment({
      index: 0,
      plan,
    })

    const sourceInputDigestRef = 'digest.source.0'
    const finalOutputDigestRef = 'digest.stage.0'
    const receipt = await buildCs336A4ProvenanceReceipt({
      assignmentRef: assignment.assignmentRef,
      finalOutputDigestRef,
      inputShardRef: assignment.inputShardRef,
      provenance: assignment.provenanceSource,
      sourceInputDigestRef,
      transformChain: [
        {
          codeVersionRef: 'refinery.v1',
          inputDigestRef: sourceInputDigestRef,
          outputDigestRef: finalOutputDigestRef,
          recomputedDigestRef: finalOutputDigestRef,
          stage: 'pii_masking',
        },
      ],
    })

    expect(receipt.assignmentRef).toBe(assignment.assignmentRef)
    expect(receipt.inputShardRef).toBe(assignment.inputShardRef)
    expect(receipt.provenance).toEqual(assignment.provenanceSource)
    expect(receipt.recomputeVerified).toBe(true)
  })
})

describe('deriveCs336A4CrawlShardAssignments', () => {
  it('derives one assignment per plan shard in order with no gaps/overlaps', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)

    expect(assignments).toHaveLength(plan.shards.length)
    assignments.forEach((assignment, index) => {
      expect(assignment.index).toBe(index)
    })

    // The assignments tile the snapshot exactly like the plan shards do.
    let cursor = 0
    let covered = 0
    for (const assignment of assignments) {
      expect(assignment.startSegment).toBe(cursor)
      cursor = assignment.endSegment
      covered += assignment.segmentCount
    }
    expect(cursor).toBe(plan.segmentCount)
    expect(covered).toBe(plan.segmentCount)
  })

  it('mints a unique assignmentRef per shard', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)
    const refs = new Set(assignments.map(a => a.assignmentRef))

    expect(refs.size).toBe(assignments.length)
  })
})
