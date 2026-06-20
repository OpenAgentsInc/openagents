import { describe, expect, it } from 'vitest'

import {
  deriveCs336A4CrawlShardAssignment,
  deriveCs336A4CrawlShardAssignments,
  type Cs336A4CrawlShardAssignment,
} from './cs336-a4-crawl-shard-assignment'
import {
  buildCs336A4CrawlShardPlan,
  type Cs336A4CrawlShardPlan,
  type Cs336A4CrawlSnapshotDescriptor,
} from './cs336-a4-crawl-shard-plan'
import {
  assertCs336A4CrawlShardDispatchCoverage,
  Cs336A4CrawlShardDispatchCoverageError,
  verifyCs336A4CrawlShardDispatchCoverage,
} from './cs336-a4-crawl-shard-dispatch-coverage'

const descriptor: Cs336A4CrawlSnapshotDescriptor = {
  acquisitionMode: 'public_crawl_snapshot',
  licenseRef: 'license.public.cc_main.2026_05',
  segmentCount: 10,
  snapshotRef: 'snapshot.cc_main.2026_05',
  sourceRef: 'source.cc_main',
}

const buildPlan = (): Promise<Cs336A4CrawlShardPlan> =>
  buildCs336A4CrawlShardPlan({ descriptor, targetShardCount: 4 })

describe('verifyCs336A4CrawlShardDispatchCoverage', () => {
  it('accepts the full assignment set derived from the plan', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)

    const result = verifyCs336A4CrawlShardDispatchCoverage({
      assignments,
      plan,
    })

    expect(result.complete).toBe(true)
    if (result.complete) {
      expect(result.planRef).toBe(plan.planRef)
      expect(result.snapshotRef).toBe(plan.snapshotRef)
      expect(result.segmentCount).toBe(plan.segmentCount)
      expect(result.assignmentCount).toBe(plan.shards.length)
      expect(result.assignmentRefs).toHaveLength(plan.shards.length)
    }
  })

  it('accepts the set regardless of assignment order', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)
    const shuffled = [...assignments].reverse()

    expect(
      verifyCs336A4CrawlShardDispatchCoverage({ assignments: shuffled, plan })
        .complete,
    ).toBe(true)
  })

  it('rejects an empty assignment set', async () => {
    const plan = await buildPlan()

    const result = verifyCs336A4CrawlShardDispatchCoverage({
      assignments: [],
      plan,
    })

    expect(result.complete).toBe(false)
    if (!result.complete) {
      expect(result.reason).toBe('empty_assignment_set')
    }
  })

  it('reports a gap when a shard is missing from the set', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)

    const result = verifyCs336A4CrawlShardDispatchCoverage({
      assignments: assignments.slice(0, assignments.length - 1),
      plan,
    })

    expect(result.complete).toBe(false)
    if (!result.complete) {
      expect(result.reason).toBe('segment_gap')
    }
  })

  it('reports an overlap when two assignments cover the same segment', async () => {
    const plan = await buildPlan()
    const first = await deriveCs336A4CrawlShardAssignment({ index: 0, plan })
    const rest = await deriveCs336A4CrawlShardAssignments(plan)
    // A second copy of shard 0 (with a distinct ref) overlaps its segments
    // and leaves another shard's segments double-covered against the full set.
    const overlapping: Cs336A4CrawlShardAssignment = {
      ...first,
      assignmentRef: `${first.assignmentRef}.dup`,
    }

    const result = verifyCs336A4CrawlShardDispatchCoverage({
      assignments: [...rest, overlapping],
      plan,
    })

    expect(result.complete).toBe(false)
    if (!result.complete) {
      expect(result.reason).toBe('duplicate_segment_coverage')
    }
  })

  it('reports a duplicate assignmentRef when the same assignment appears twice', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)
    const first = assignments[0]
    if (first === undefined) {
      throw new Error('expected at least one assignment')
    }

    const result = verifyCs336A4CrawlShardDispatchCoverage({
      assignments: [...assignments, first],
      plan,
    })

    expect(result.complete).toBe(false)
    if (!result.complete) {
      expect(result.reason).toBe('duplicate_assignment_ref')
    }
  })

  it('rejects an assignment that belongs to a different plan', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)
    const tampered = assignments.map((assignment, index) =>
      index === 0
        ? { ...assignment, planRef: 'plan.cs336_a4.crawl_shard.other' }
        : assignment,
    )

    const result = verifyCs336A4CrawlShardDispatchCoverage({
      assignments: tampered,
      plan,
    })

    expect(result.complete).toBe(false)
    if (!result.complete) {
      expect(result.reason).toBe('plan_ref_mismatch')
    }
  })

  it('rejects an assignment that re-attributes the snapshot source', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)
    const tampered = assignments.map((assignment, index) =>
      index === 0
        ? {
            ...assignment,
            provenanceSource: {
              ...assignment.provenanceSource,
              sourceRef: 'source.unrelated',
            },
          }
        : assignment,
    )

    const result = verifyCs336A4CrawlShardDispatchCoverage({
      assignments: tampered,
      plan,
    })

    expect(result.complete).toBe(false)
    if (!result.complete) {
      expect(result.reason).toBe('source_ref_mismatch')
    }
  })

  it('reports an out-of-bounds assignment segment range', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)
    const tampered = assignments.map((assignment, index) =>
      index === assignments.length - 1
        ? {
            ...assignment,
            endSegment: plan.segmentCount + 1,
            segmentCount: plan.segmentCount + 1 - assignment.startSegment,
          }
        : assignment,
    )

    const result = verifyCs336A4CrawlShardDispatchCoverage({
      assignments: tampered,
      plan,
    })

    expect(result.complete).toBe(false)
    if (!result.complete) {
      expect(result.reason).toBe('segment_out_of_bounds')
    }
  })

  it('reports an internally inconsistent segment range', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)
    const tampered = assignments.map((assignment, index) =>
      index === 0
        ? { ...assignment, segmentCount: assignment.segmentCount + 1 }
        : assignment,
    )

    const result = verifyCs336A4CrawlShardDispatchCoverage({
      assignments: tampered,
      plan,
    })

    expect(result.complete).toBe(false)
    if (!result.complete) {
      expect(result.reason).toBe('segment_range_invalid')
    }
  })
})

describe('assertCs336A4CrawlShardDispatchCoverage', () => {
  it('returns the planRef for a complete cover', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)

    expect(
      assertCs336A4CrawlShardDispatchCoverage({ assignments, plan }),
    ).toBe(plan.planRef)
  })

  it('throws a tagged error carrying the failure reason for a gap', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)

    try {
      assertCs336A4CrawlShardDispatchCoverage({
        assignments: assignments.slice(1),
        plan,
      })
      throw new Error('expected dispatch coverage assertion to throw')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardDispatchCoverageError)
      if (error instanceof Cs336A4CrawlShardDispatchCoverageError) {
        expect(error.reason).toBe('segment_gap')
      }
    }
  })
})
