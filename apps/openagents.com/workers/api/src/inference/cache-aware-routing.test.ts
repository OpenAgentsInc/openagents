import { describe, expect, test } from 'vitest'

import {
  type CacheWarmthOracle,
  type LaneHealthOracle,
  decideCacheAwareRouting,
} from './cache-aware-routing'

// A warm oracle that maps one fixture affinity hash to a fixture warm lane.
const warmthFor = (hash: string, lane: string): CacheWarmthOracle => h =>
  h === hash ? lane : undefined

const PLAN = ['fireworks', 'openagents-network', 'passthrough-openai']
const HASH = 'cacheaff:fnv1a32:deadbeef'

describe('cache-aware routing — reorder to the cache-warm lane (book §5.3.3)', () => {
  test('promotes the cache-warm lane to the FRONT of the plan under a fixture', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: HASH,
      plannedLanes: PLAN,
      warmthOracle: warmthFor(HASH, 'passthrough-openai'),
    })
    expect(decision.warmLane).toBe('passthrough-openai')
    expect(decision.reason).toBe('promoted_warm_lane')
    // Warm lane first; the rest keep their original relative order (overflow tail
    // preserved).
    expect(decision.lanes).toEqual([
      'passthrough-openai',
      'fireworks',
      'openagents-network',
    ])
  })

  test('the reordered plan is a PERMUTATION of the input (never widens the plan)', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: HASH,
      plannedLanes: PLAN,
      warmthOracle: warmthFor(HASH, 'openagents-network'),
    })
    expect([...decision.lanes].sort()).toEqual([...PLAN].sort())
  })

  test('no affinity hash → plan unchanged (no reorder)', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: null,
      plannedLanes: PLAN,
      warmthOracle: warmthFor(HASH, 'passthrough-openai'),
    })
    expect(decision.lanes).toEqual(PLAN)
    expect(decision.warmLane).toBeNull()
    expect(decision.reason).toBe('no_affinity')
  })

  test('no warmth oracle wired → plan unchanged (inert by default)', () => {
    const decision = decideCacheAwareRouting({ affinityHash: HASH, plannedLanes: PLAN })
    expect(decision.lanes).toEqual(PLAN)
    expect(decision.reason).toBe('no_warm_record')
  })

  test('oracle has no warm record for this hash → plan unchanged', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: 'cacheaff:fnv1a32:00000000',
      plannedLanes: PLAN,
      warmthOracle: warmthFor(HASH, 'passthrough-openai'),
    })
    expect(decision.reason).toBe('no_warm_record')
    expect(decision.lanes).toEqual(PLAN)
  })

  test('warm lane already first → already_warm_first, plan unchanged', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: HASH,
      plannedLanes: PLAN,
      warmthOracle: warmthFor(HASH, 'fireworks'),
    })
    expect(decision.reason).toBe('already_warm_first')
    expect(decision.warmLane).toBe('fireworks')
    expect(decision.lanes).toEqual(PLAN)
  })

  test('warm lane no longer in the viable plan → not re-added (warm_not_in_plan)', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: HASH,
      plannedLanes: PLAN,
      // The previously-warm lane fell out of the plan (e.g. its secret is absent).
      warmthOracle: warmthFor(HASH, 'vertex-anthropic'),
    })
    expect(decision.reason).toBe('warm_not_in_plan')
    expect(decision.lanes).toEqual(PLAN)
    expect(decision.warmLane).toBeNull()
  })
})

describe('cache-aware routing — constraints (health / privacy / region)', () => {
  const degraded: LaneHealthOracle = lane =>
    lane === 'passthrough-openai' ? 'unhealthy' : 'healthy'

  test('a degraded/unhealthy warm lane is NOT promoted (subject to provider health)', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: HASH,
      healthOracle: degraded,
      plannedLanes: PLAN,
      warmthOracle: warmthFor(HASH, 'passthrough-openai'),
    })
    expect(decision.reason).toBe('warm_unhealthy')
    expect(decision.lanes).toEqual(PLAN)
    expect(decision.warmLane).toBeNull()
  })

  test('a healthy warm lane IS promoted even when OTHER lanes are sick', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: HASH,
      healthOracle: degraded,
      plannedLanes: PLAN,
      // openagents-network is healthy under `degraded`.
      warmthOracle: warmthFor(HASH, 'openagents-network'),
    })
    expect(decision.reason).toBe('promoted_warm_lane')
    expect(decision.lanes[0]).toBe('openagents-network')
  })

  test('a privacy/region policy can forbid pinning a warm lane (warm_pin_forbidden)', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: HASH,
      pinPolicy: lane => lane !== 'passthrough-openai',
      plannedLanes: PLAN,
      warmthOracle: warmthFor(HASH, 'passthrough-openai'),
    })
    expect(decision.reason).toBe('warm_pin_forbidden')
    expect(decision.lanes).toEqual(PLAN)
    expect(decision.warmLane).toBeNull()
  })

  test('a warm lane allowed by the pin policy is promoted', () => {
    const decision = decideCacheAwareRouting({
      affinityHash: HASH,
      pinPolicy: () => true,
      plannedLanes: PLAN,
      warmthOracle: warmthFor(HASH, 'passthrough-openai'),
    })
    expect(decision.reason).toBe('promoted_warm_lane')
    expect(decision.lanes[0]).toBe('passthrough-openai')
  })
})
