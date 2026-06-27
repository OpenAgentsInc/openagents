import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { loadArtanisNetworkStatsFromLedger } from './artanis-network-stats-d1'
import type { TokenUsageLedgerShape } from './token-usage-ledger'

const NOON_CENTRAL_UTC = '2026-06-27T17:00:00.000Z'

// A minimal fake ledger exposing only the three public read methods the loader
// touches; the rest of TokenUsageLedgerShape is unused here.
const fakeLedger = (
  over: Partial<TokenUsageLedgerShape> = {},
): TokenUsageLedgerShape =>
  ({
    readPublicTokensServed: () => Effect.succeed({ tokensServed: 478_000_000 }),
    readPublicTokensServedHistory: () =>
      Effect.succeed({
        series: [
          { day: '2026-06-26', tokensServed: 328_100_000 },
          { day: '2026-06-27', tokensServed: 62_000_000 },
        ],
      }),
    readPublicTokensServedModelMix: () =>
      Effect.succeed({
        groups: [
          { family: 'glm', label: 'GLM', pct: 100, reqs: 1, tokens: 500 },
        ],
      }),
    ...over,
  }) as unknown as TokenUsageLedgerShape

describe('loadArtanisNetworkStatsFromLedger', () => {
  test('assembles the snapshot + pace block from the live ledger', async () => {
    const stats = await loadArtanisNetworkStatsFromLedger(fakeLedger(), {
      nowIso: () => NOON_CENTRAL_UTC,
    })
    expect(stats.allTimeTokensServed).toBe(478_000_000)
    expect(stats.todayTokens).toBe(62_000_000)
    expect(stats.modelMix.map(group => group.family)).toEqual(['glm'])
    expect(stats.pace?.yesterdayTokens).toBe(328_100_000)
    // 62M at 50% of the day projects 124M, far below the 4x floor (1.3124B).
    expect(stats.pace?.paceProjection).toBe(124_000_000)
    expect(stats.pace?.behindPace).toBe(true)
  })

  test('degrades fail-soft when a ledger read fails', async () => {
    const stats = await loadArtanisNetworkStatsFromLedger(
      fakeLedger({
        readPublicTokensServedHistory: (() =>
          Effect.fail(
            'history unavailable',
          )) as unknown as TokenUsageLedgerShape['readPublicTokensServedHistory'],
      }),
      { nowIso: () => NOON_CENTRAL_UTC },
    )
    // The scalar still came through; only the history bucket degraded.
    expect(stats.allTimeTokensServed).toBe(478_000_000)
    expect(stats.history).toEqual([])
    expect(stats.pace?.todayTokens).toBe(0)
  })
})
