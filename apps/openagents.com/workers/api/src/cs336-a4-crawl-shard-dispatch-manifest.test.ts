import { describe, expect, it } from 'vitest'

import {
  deriveCs336A4CrawlShardAssignments,
  type Cs336A4CrawlShardAssignment,
} from './cs336-a4-crawl-shard-assignment'
import {
  buildCs336A4CrawlShardPlan,
  type Cs336A4CrawlShardPlan,
  type Cs336A4CrawlSnapshotDescriptor,
} from './cs336-a4-crawl-shard-plan'
import {
  buildCs336A4CrawlShardDispatchManifest,
  Cs336A4CrawlShardDispatchManifestError,
  Cs336A4CrawlShardDispatchManifestSchemaVersion,
} from './cs336-a4-crawl-shard-dispatch-manifest'

const descriptor: Cs336A4CrawlSnapshotDescriptor = {
  acquisitionMode: 'public_crawl_snapshot',
  licenseRef: 'license.public.cc_main.2026_05',
  segmentCount: 10,
  snapshotRef: 'snapshot.cc_main.2026_05',
  sourceRef: 'source.cc_main',
}

const buildPlan = (): Promise<Cs336A4CrawlShardPlan> =>
  buildCs336A4CrawlShardPlan({ descriptor, targetShardCount: 4 })

describe('buildCs336A4CrawlShardDispatchManifest', () => {
  it('emits a content-addressed manifest for a complete, authentic batch', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)

    const manifest = await buildCs336A4CrawlShardDispatchManifest({
      assignments,
      plan,
    })

    expect(manifest.schemaVersion).toBe(
      Cs336A4CrawlShardDispatchManifestSchemaVersion,
    )
    expect(manifest.planRef).toBe(plan.planRef)
    expect(manifest.snapshotRef).toBe(plan.snapshotRef)
    expect(manifest.sourceRef).toBe(plan.sourceRef)
    expect(manifest.licenseRef).toBe(plan.licenseRef)
    expect(manifest.segmentCount).toBe(plan.segmentCount)
    expect(manifest.assignmentCount).toBe(assignments.length)
    expect(manifest.assignmentRefs).toEqual(
      assignments.map(a => a.assignmentRef),
    )
    expect(manifest.manifestRef).toContain(plan.snapshotRef)
    expect(manifest.contentDigestRef).toMatch(/^[0-9a-f]{64}$/)
  })

  it('is deterministic and order-independent', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)

    const first = await buildCs336A4CrawlShardDispatchManifest({
      assignments,
      plan,
    })
    const reversed = await buildCs336A4CrawlShardDispatchManifest({
      assignments: [...assignments].reverse(),
      plan,
    })

    expect(reversed.manifestRef).toBe(first.manifestRef)
    expect(reversed.contentDigestRef).toBe(first.contentDigestRef)
    expect(reversed.assignmentRefs).toEqual(first.assignmentRefs)
  })

  it('rejects a forged/stale assignment ref at the authenticity stage', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignments[0]!,
      assignmentRef: 'assignment.cs336_a4.crawl_shard.forged.0_1.deadbeefdeadbeef',
    }
    const batch = [forged, ...assignments.slice(1)]

    await expect(
      buildCs336A4CrawlShardDispatchManifest({ assignments: batch, plan }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardDispatchManifestError)

    try {
      await buildCs336A4CrawlShardDispatchManifest({ assignments: batch, plan })
      expect.unreachable('forged assignment should hard-fail')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardDispatchManifestError)
      if (error instanceof Cs336A4CrawlShardDispatchManifestError) {
        expect(error.stage).toBe('authenticity')
        expect(error.reason).toBe('assignment_ref_mismatch')
      }
    }
  })

  it('rejects an incomplete batch (gap) at the coverage stage', async () => {
    const plan = await buildPlan()
    const assignments = await deriveCs336A4CrawlShardAssignments(plan)
    // Drop the last assignment: the remaining ones are all authentic but leave
    // a gap, so the failure must be attributed to the coverage stage.
    const partial = assignments.slice(0, -1)

    try {
      await buildCs336A4CrawlShardDispatchManifest({
        assignments: partial,
        plan,
      })
      expect.unreachable('incomplete batch should hard-fail')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardDispatchManifestError)
      if (error instanceof Cs336A4CrawlShardDispatchManifestError) {
        expect(error.stage).toBe('coverage')
        expect(error.reason).toBe('segment_gap')
      }
    }
  })

  it('rejects an empty batch at the coverage stage', async () => {
    const plan = await buildPlan()

    try {
      await buildCs336A4CrawlShardDispatchManifest({ assignments: [], plan })
      expect.unreachable('empty batch should hard-fail')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardDispatchManifestError)
      if (error instanceof Cs336A4CrawlShardDispatchManifestError) {
        expect(error.stage).toBe('coverage')
        expect(error.reason).toBe('empty_assignment_set')
      }
    }
  })
})
