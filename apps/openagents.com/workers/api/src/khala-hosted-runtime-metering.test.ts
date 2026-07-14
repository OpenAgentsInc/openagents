import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ArtanisMindUsage } from './artanis-mind'
import {
  hostedKhalaTokenUsageEventBody,
  hostedTurnUsageFromArtanisMind,
  recordHostedTurnUsage,
  type HostedTurnUsage,
} from './khala-hosted-runtime-metering'
import type { TokenUsageIngestResult, TokenUsageLedgerShape } from './token-usage-ledger'

const usage: HostedTurnUsage = {
  cacheReadTokens: 0,
  inputTokens: 4_000,
  outputTokens: 1_000,
  reasoningTokens: 100,
  totalTokens: 5_000,
}

const input = {
  observedAt: '2026-07-08T00:00:00.000Z',
  ownerUserId: 'github:14167547',
  threadId: 'thread-1',
  turnId: 'turn-1',
  usage,
} as const

describe('hosted Khala exact usage recording', () => {
  test('normalizes provider counters without inventing usage', () => {
    const raw: ArtanisMindUsage = {
      cachedInputTokens: 0,
      candidatesTokens: 900,
      promptTokens: 4_000,
      thoughtsTokens: 100,
      totalTokens: 5_000,
    }
    expect(hostedTurnUsageFromArtanisMind(raw)).toEqual(usage)
    expect(hostedTurnUsageFromArtanisMind({
      cachedInputTokens: 0,
      candidatesTokens: 0,
      promptTokens: 0,
      thoughtsTokens: 0,
      totalTokens: 0,
    })).toBeNull()
  })

  test('records an exact owner-attributed row with no charge or settlement fields', async () => {
    const rows: Array<unknown> = []
    const ledger = {
      ingestEvent: (body: unknown) => {
        rows.push(body)
        return Effect.succeed({ event: {} as never, inserted: true } satisfies TokenUsageIngestResult)
      },
    } as unknown as TokenUsageLedgerShape

    const outcome = await recordHostedTurnUsage({ ledger }, input)

    expect(rows).toEqual([hostedKhalaTokenUsageEventBody(input)])
    expect(outcome).toEqual({
      insertedTokenUsage: true,
      tokenUsageEventRef: 'event.inference.served-tokens.khala-hosted.turn-1',
      tokensServed: 5_000,
      usageRef: 'usage.khala-hosted.turn-1',
    })
    expect(outcome).not.toHaveProperty('chargeReceiptRef')
    expect(outcome).not.toHaveProperty('chargeUsdCents')
    expect(outcome).not.toHaveProperty('metered')
  })
})
