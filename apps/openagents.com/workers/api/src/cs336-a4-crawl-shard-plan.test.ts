import { describe, expect, it } from 'vitest'

import {
  Cs336A4CrawlShardPlanSchemaVersion,
  Cs336A4CrawlShardPlanUnsafeMaterialError,
  Cs336A4CrawlShardPlanValidationError,
  buildCs336A4CrawlShardPlan,
  type Cs336A4CrawlSnapshotDescriptor,
} from './cs336-a4-crawl-shard-plan'

const descriptor: Cs336A4CrawlSnapshotDescriptor = {
  acquisitionMode: 'public_crawl_snapshot',
  licenseRef: 'license.public.cc_main.2026_05',
  segmentCount: 10,
  snapshotRef: 'snapshot.cc_main.2026_05',
  sourceRef: 'source.cc_main',
}

describe('buildCs336A4CrawlShardPlan', () => {
  it('partitions a snapshot into the requested number of bounded shards', async () => {
    const plan = await buildCs336A4CrawlShardPlan({
      descriptor,
      targetShardCount: 4,
    })

    expect(plan.schemaVersion).toBe(Cs336A4CrawlShardPlanSchemaVersion)
    expect(plan.shards).toHaveLength(4)
    expect(plan.segmentCount).toBe(10)
    expect(plan.planRef.startsWith('plan.cs336_a4.crawl_shard.')).toBe(true)
  })

  it('tiles the snapshot with no gaps or overlaps and sums to segmentCount', async () => {
    const plan = await buildCs336A4CrawlShardPlan({
      descriptor,
      targetShardCount: 4,
    })

    let cursor = 0
    let total = 0
    for (const shard of plan.shards) {
      expect(shard.startSegment).toBe(cursor)
      expect(shard.endSegment).toBeGreaterThan(shard.startSegment)
      expect(shard.segmentCount).toBe(shard.endSegment - shard.startSegment)
      cursor = shard.endSegment
      total += shard.segmentCount
    }
    expect(cursor).toBe(descriptor.segmentCount)
    expect(total).toBe(descriptor.segmentCount)
  })

  it('front-loads the remainder so the partition is the most even possible', async () => {
    // 10 segments across 4 shards => sizes [3, 3, 2, 2].
    const plan = await buildCs336A4CrawlShardPlan({
      descriptor,
      targetShardCount: 4,
    })

    expect(plan.shards.map(shard => shard.segmentCount)).toEqual([3, 3, 2, 2])
  })

  it('is deterministic: same descriptor and shard count yield the same planRef and shardRefs', async () => {
    const first = await buildCs336A4CrawlShardPlan({
      descriptor,
      targetShardCount: 4,
    })
    const second = await buildCs336A4CrawlShardPlan({
      descriptor,
      targetShardCount: 4,
    })

    expect(second.planRef).toBe(first.planRef)
    expect(second.contentDigestRef).toBe(first.contentDigestRef)
    expect(second.shards.map(shard => shard.shardRef)).toEqual(
      first.shards.map(shard => shard.shardRef),
    )
  })

  it('changes the planRef when the target shard count changes', async () => {
    const four = await buildCs336A4CrawlShardPlan({
      descriptor,
      targetShardCount: 4,
    })
    const five = await buildCs336A4CrawlShardPlan({
      descriptor,
      targetShardCount: 5,
    })

    expect(five.planRef).not.toBe(four.planRef)
  })

  it('rejects the bounded synthetic mixture: it has no crawl segments to assign', async () => {
    await expect(
      buildCs336A4CrawlShardPlan({
        descriptor: {
          ...descriptor,
          // Not a crawl-scale acquisition mode.
          acquisitionMode: 'bounded_synthetic_corpus' as never,
        },
        targetShardCount: 2,
      }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardPlanValidationError)
  })

  it('rejects a target shard count larger than the segment count', async () => {
    await expect(
      buildCs336A4CrawlShardPlan({
        descriptor: { ...descriptor, segmentCount: 3 },
        targetShardCount: 4,
      }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardPlanValidationError)
  })

  it('rejects a non-positive or non-integer segment count', async () => {
    await expect(
      buildCs336A4CrawlShardPlan({
        descriptor: { ...descriptor, segmentCount: 0 },
        targetShardCount: 1,
      }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardPlanValidationError)

    await expect(
      buildCs336A4CrawlShardPlan({
        descriptor: { ...descriptor, segmentCount: 2.5 },
        targetShardCount: 1,
      }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardPlanValidationError)
  })

  it('rejects an empty snapshot ref', async () => {
    await expect(
      buildCs336A4CrawlShardPlan({
        descriptor: { ...descriptor, snapshotRef: '   ' },
        targetShardCount: 2,
      }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardPlanValidationError)
  })

  it('fails closed on unsafe material in the descriptor (a URL is not a snapshot id)', async () => {
    await expect(
      buildCs336A4CrawlShardPlan({
        descriptor: {
          ...descriptor,
          sourceRef: 'https://crawl.example/payload.warc.gz',
        },
        targetShardCount: 2,
      }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardPlanUnsafeMaterialError)
  })
})
