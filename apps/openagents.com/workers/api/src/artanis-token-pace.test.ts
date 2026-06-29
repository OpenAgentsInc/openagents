import { describe, expect, test } from 'vitest'

import {
  computeArtanisTokenPaceBlock,
  fetchArtanisNetworkStats,
  formatArtanisTokenPaceLine,
} from './artanis-token-pace'

// 17:00 UTC in late June is 12:00 CDT (America/Chicago, UTC-5), i.e. exactly
// half of the Central day elapsed (fraction 0.5).
const NOON_CENTRAL_UTC = '2026-06-27T17:00:00.000Z'

describe('computeArtanisTokenPaceBlock', () => {
  test('behindPace is true when the midnight projection is below the 4x floor', () => {
    const pace = computeArtanisTokenPaceBlock({
      nowIso: NOON_CENTRAL_UTC,
      series: [
        { day: '2026-06-26', tokensServed: 328_100_000 },
        { day: '2026-06-27', tokensServed: 100_000_000 },
      ],
    })
    expect(pace).not.toBeNull()
    if (pace === null) return
    expect(pace.day).toBe('2026-06-27')
    expect(pace.fractionOfCentralDayElapsed).toBeCloseTo(0.5, 6)
    expect(pace.todayTokens).toBe(100_000_000)
    // 100M served at 50% of the day projects 200M by midnight.
    expect(pace.paceProjection).toBe(200_000_000)
    expect(pace.yesterdayTokens).toBe(328_100_000)
    expect(pace.target4x).toBe(4 * 328_100_000)
    expect(pace.target10x).toBe(10 * 328_100_000)
    expect(pace.gapToTarget4x).toBe(4 * 328_100_000 - 200_000_000)
    // 200M < 1.3124B floor -> URGENT.
    expect(pace.behindPace).toBe(true)
  })

  test('behindPace is false when the projection clears the 4x floor', () => {
    const pace = computeArtanisTokenPaceBlock({
      nowIso: NOON_CENTRAL_UTC,
      series: [
        { day: '2026-06-26', tokensServed: 328_100_000 },
        { day: '2026-06-27', tokensServed: 2_000_000_000 },
      ],
    })
    expect(pace).not.toBeNull()
    if (pace === null) return
    // 2B at 50% projects 4B, which clears the 1.3124B 4x floor.
    expect(pace.paceProjection).toBe(4_000_000_000)
    expect(pace.behindPace).toBe(false)
    expect(pace.gapToTarget4x).toBeLessThan(0)
  })

  test('returns null when the clock cannot be parsed', () => {
    expect(
      computeArtanisTokenPaceBlock({
        nowIso: 'not-a-timestamp',
        series: [{ day: '2026-06-27', tokensServed: 1 }],
      }),
    ).toBeNull()
  })

  test('treats a missing today row as zero served so far', () => {
    const pace = computeArtanisTokenPaceBlock({
      nowIso: NOON_CENTRAL_UTC,
      series: [{ day: '2026-06-26', tokensServed: 328_100_000 }],
    })
    expect(pace).not.toBeNull()
    if (pace === null) return
    expect(pace.todayTokens).toBe(0)
    expect(pace.paceProjection).toBe(0)
    expect(pace.yesterdayTokens).toBe(328_100_000)
    expect(pace.behindPace).toBe(true)
  })
})

describe('formatArtanisTokenPaceLine', () => {
  test('names BEHIND PACE and the targets when behind', () => {
    const pace = computeArtanisTokenPaceBlock({
      nowIso: NOON_CENTRAL_UTC,
      series: [
        { day: '2026-06-26', tokensServed: 328_100_000 },
        { day: '2026-06-27', tokensServed: 100_000_000 },
      ],
    })!
    const line = formatArtanisTokenPaceLine(pace)
    expect(line).toContain('BEHIND PACE')
    expect(line).toContain('2026-06-27')
    expect(line.toLowerCase()).toContain('by midnight')
  })
})

// A fetch stub that routes by URL to canned JSON Responses.
const makeStatsFetch = (): typeof fetch =>
  (async (input: RequestInfo | URL) => {
    const url = typeof input === 'string' ? input : input.toString()
    if (url.includes('/khala-tokens-served/history')) {
      return new Response(
        JSON.stringify({
          series: [
            { day: '2026-06-25', tokensServed: 50_000_000 },
            { day: '2026-06-26', tokensServed: 328_100_000 },
            { day: '2026-06-27', tokensServed: 100_000_000 },
          ],
        }),
        { status: 200 },
      )
    }
    if (url.includes('/khala-tokens-served/model-mix')) {
      return new Response(
        JSON.stringify({
          groups: [
            { family: 'glm', label: 'GLM', pct: 80, reqs: 10, tokens: 400 },
            { family: 'oss', label: 'gpt-oss', pct: 20, reqs: 5, tokens: 100 },
          ],
        }),
        { status: 200 },
      )
    }
    if (url.includes('/khala-tokens-served')) {
      return new Response(JSON.stringify({ tokensServed: 9_999_999_999 }), {
        status: 200,
      })
    }
    return new Response('not found', { status: 404 })
  }) as typeof fetch

describe('fetchArtanisNetworkStats', () => {
  test('fetches the three public endpoints and computes the pace block', async () => {
    const stats = await fetchArtanisNetworkStats({
      fetchImpl: makeStatsFetch(),
      nowIso: () => NOON_CENTRAL_UTC,
    })

    expect(stats.allTimeTokensServed).toBe(9_999_999_999)
    expect(stats.todayTokens).toBe(100_000_000)
    expect(stats.history.map(point => point.day)).toEqual([
      '2026-06-25',
      '2026-06-26',
      '2026-06-27',
    ])
    expect(stats.modelMix.map(group => group.family)).toEqual(['glm', 'oss'])
    expect(stats.pace).not.toBeNull()
    expect(stats.pace?.behindPace).toBe(true)
    expect(stats.pace?.paceProjection).toBe(200_000_000)
    expect(stats.pace?.yesterdayTokens).toBe(328_100_000)
  })

  test('degrades fail-soft when the endpoints are unreachable', async () => {
    const failingFetch = (async () => {
      throw new Error('network down')
    }) as typeof fetch
    const stats = await fetchArtanisNetworkStats({
      fetchImpl: failingFetch,
      nowIso: () => NOON_CENTRAL_UTC,
    })
    expect(stats.allTimeTokensServed).toBe(0)
    expect(stats.history).toEqual([])
    expect(stats.modelMix).toEqual([])
    // No series -> no today/yesterday -> pace still computes with zeros.
    expect(stats.pace?.todayTokens).toBe(0)
  })
})
