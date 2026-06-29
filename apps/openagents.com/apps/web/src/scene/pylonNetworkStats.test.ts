import { describe, expect, test } from 'vitest'

import {
  PYLON_STATS_BOOT_SCRIPT_ID,
  computeActivityIntensity,
  fetchPylonStats,
  readInitialPylonStatsSnapshot,
  type PylonStatsSnapshot,
} from './pylonNetworkStats'

describe('computeActivityIntensity (#5050 homepage glow)', () => {
  test('idle/unavailable/null => 0', () => {
    expect(computeActivityIntensity(null)).toBe(0)
    expect(computeActivityIntensity({ available: false })).toBe(0)
    expect(computeActivityIntensity({ status: 'unavailable' })).toBe(0)
    expect(computeActivityIntensity({ available: true, status: 'live' })).toBe(0)
  })

  test('work raises the glow, monotonic and bounded', () => {
    const some = computeActivityIntensity({ available: true, pylonSessionsOnlineNow: 2 })
    expect(some).toBeGreaterThan(0)
    expect(some).toBeLessThan(1)
    const more = computeActivityIntensity({ available: true, pylonSessionsOnlineNow: 50 })
    expect(more).toBeGreaterThan(some)
    expect(more).toBeLessThanOrEqual(1)
  })

  test('all three signals approach full brightness; NIP-90 jobs summed', () => {
    const intensity = computeActivityIntensity({
      available: true,
      pylonSessionsOnlineNow: 40,
      trainingModelProgressContributors: 40,
      nip90MarketSettlementStats: {
        compute: { jobsSettled24h: 30 },
        data: { jobsSettled24h: 30 },
        labor: { jobsSettled24h: 30 },
      },
    })
    expect(intensity).toBeGreaterThan(0.8)
    expect(intensity).toBeLessThanOrEqual(1)
  })

  test('single saturated signal cannot exceed its 1/3 weight', () => {
    const only = computeActivityIntensity({ available: true, pylonSessionsOnlineNow: 1e6 })
    expect(only).toBeGreaterThan(0.28)
    expect(only).toBeLessThan(0.4)
  })
})

describe('fetchPylonStats (fail-soft)', () => {
  test('non-200 => null', async () => {
    const fetchFn = (async () => new Response('no', { status: 503 })) as unknown as typeof fetch
    expect(await fetchPylonStats(fetchFn, '/x')).toBeNull()
  })

  test('network error => null', async () => {
    const fetchFn = (async () => {
      throw new Error('offline')
    }) as unknown as typeof fetch
    expect(await fetchPylonStats(fetchFn, '/x')).toBeNull()
  })

  test('ok => snapshot', async () => {
    const snap: PylonStatsSnapshot = { available: true, pylonsOnlineNow: 3 }
    const fetchFn = (async () => new Response(JSON.stringify(snap), { status: 200 })) as unknown as typeof fetch
    expect((await fetchPylonStats(fetchFn, '/x'))?.pylonsOnlineNow).toBe(3)
  })
})

describe('readInitialPylonStatsSnapshot', () => {
  test('reads the Worker-embedded boot payload', () => {
    const snap: PylonStatsSnapshot = {
      available: true,
      publicRealSatsSettled24h: 150_000,
      pylonsOnlineNow: 4,
      trainingModelProgressContributors: 3,
    }
    const documentRef = {
      getElementById: (id: string) =>
        id === PYLON_STATS_BOOT_SCRIPT_ID
          ? ({ textContent: JSON.stringify(snap) } as HTMLElement)
          : null,
    } as Pick<Document, 'getElementById'>

    expect(readInitialPylonStatsSnapshot(documentRef)).toEqual(snap)
  })

  test('fails soft when the boot payload is absent or malformed', () => {
    expect(
      readInitialPylonStatsSnapshot({
        getElementById: () => null,
      } as Pick<Document, 'getElementById'>),
    ).toBeNull()
    expect(
      readInitialPylonStatsSnapshot({
        getElementById: () => ({ textContent: '{' }) as HTMLElement,
      } as Pick<Document, 'getElementById'>),
    ).toBeNull()
  })
})
