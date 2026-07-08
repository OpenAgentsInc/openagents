import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ArtanisMindUsage } from './artanis-mind'
import {
  HOSTED_KHALA_LANE,
  HOSTED_KHALA_PRICING_MODEL,
  hostedKhalaChargeRequestId,
  hostedKhalaChargeUsdCents,
  hostedKhalaOwnerCreditAccountRef,
  hostedKhalaTokenUsageEventBody,
  hostedKhalaUsageEventId,
  hostedTurnUsageFromArtanisMind,
  recordHostedTurnUsageAndCharge,
  type HostedTurnUsage,
} from './khala-hosted-runtime-metering'
import {
  inferenceChargeIdempotencyKey,
  makeLedgerMeteringHook,
} from './inference/metering-hook'
import { readAgentBalance } from './payments-ledger'
import { priceRequest } from './inference/pricing'
import type { TokenUsageIngestResult, TokenUsageLedgerShape } from './token-usage-ledger'
import { usdToMsatCeil } from './inference/usd-msat-conversion'
import { makeLedgerSqliteDb } from './test/payments-ledger-sqlite'

const OWNER = 'github:14167547'
const ACCOUNT = hostedKhalaOwnerCreditAccountRef(OWNER)
const NOW = '2026-07-08T00:00:00.000Z'

// A hosted turn's exact usage: 4000 input, 900 visible output, 100 thinking.
const usage: HostedTurnUsage = {
  cacheReadTokens: 0,
  inputTokens: 4000,
  outputTokens: 1000, // 900 candidates + 100 thoughts
  reasoningTokens: 100,
  totalTokens: 5000,
}

// A larger turn whose priced charge clears one USD cent, so the cents-
// denominated credit_balance projection delta is non-zero. (A single small
// hosted turn is sub-cent — the msat ledger still decrements exactly, but the
// cents projection rounds to 0, the shared behavior of every metered lane.)
const bigUsage: HostedTurnUsage = {
  cacheReadTokens: 0,
  inputTokens: 200_000,
  outputTokens: 200_000,
  reasoningTokens: 0,
  totalTokens: 400_000,
}

const seedBalance = (msat: number) => {
  const db = makeLedgerSqliteDb()
  db.raw
    .prepare(
      `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
       VALUES (?, ?, ?, ?)`,
    )
    .run(ACCOUNT, msat, NOW, NOW)
  return db
}

// A fake token ledger that captures the ingested body and reports inserted.
const makeCapturingLedger = () => {
  const ingested: Array<unknown> = []
  const ledger = {
    ingestEvent: (body: unknown) => {
      ingested.push(body)
      return Effect.succeed({
        event: {} as never,
        inserted: true,
      } satisfies TokenUsageIngestResult)
    },
  } as unknown as TokenUsageLedgerShape
  return { ingested, ledger }
}

describe('hostedTurnUsageFromArtanisMind', () => {
  test('folds thinking tokens into output and buckets exact counts', () => {
    const raw: ArtanisMindUsage = {
      candidatesTokens: 900,
      cachedInputTokens: 0,
      promptTokens: 4000,
      thoughtsTokens: 100,
      totalTokens: 5000,
    }
    expect(hostedTurnUsageFromArtanisMind(raw)).toEqual({
      cacheReadTokens: 0,
      inputTokens: 4000,
      outputTokens: 1000,
      reasoningTokens: 100,
      totalTokens: 5000,
    })
  })

  test('null when there are no billable input/output tokens', () => {
    expect(
      hostedTurnUsageFromArtanisMind({
        candidatesTokens: 0,
        cachedInputTokens: 0,
        promptTokens: 0,
        thoughtsTokens: 0,
        totalTokens: 0,
      }),
    ).toBeNull()
  })

  test('totalTokens is floored at input+output when the provider under-reports', () => {
    const normalized = hostedTurnUsageFromArtanisMind({
      candidatesTokens: 10,
      cachedInputTokens: 0,
      promptTokens: 20,
      thoughtsTokens: 0,
      totalTokens: 1, // provider under-reported
    })
    expect(normalized?.totalTokens).toBe(30)
  })
})

describe('hostedKhalaTokenUsageEventBody', () => {
  test('is an owner-attributed exact hosted_khala row (no prompt/completion)', () => {
    const body = hostedKhalaTokenUsageEventBody({
      observedAt: NOW,
      ownerUserId: OWNER,
      threadId: 'thread.t1',
      turnId: 'turn.t1',
      usage,
    }) as Record<string, unknown>

    expect(body.schemaVersion).toBe('openagents.token_usage_event.v1')
    expect(body.usageTruth).toBe('exact')
    expect(body.model).toBe(HOSTED_KHALA_PRICING_MODEL)
    expect(body.provider).toBe('google-ai-studio')
    expect(body.sourceRoute).toBe('omega_hosted_gemini')
    expect(body.producerSystem).toBe('omega')
    expect(body.actor).toEqual({ accountRef: ACCOUNT, userId: OWNER })
    expect(body.demand).toMatchObject({
      demandChannel: 'khala_api',
      demandKind: 'external',
    })
    expect(body.tokenCounts).toEqual({
      cacheReadTokens: 0,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: 4000,
      outputTokens: 1000,
      reasoningTokens: 100,
      totalTokens: 5000,
    })
    expect((body.safeMetadata as Record<string, unknown>).lane).toBe(
      HOSTED_KHALA_LANE,
    )
    // No prompt/completion/raw material leaks into the row.
    expect(JSON.stringify(body)).not.toContain('Explain')
  })
})

describe('recordHostedTurnUsageAndCharge (real ledger charge)', () => {
  test('records the exact usage row AND debits the balance by the priced msat', async () => {
    const FUNDED = 1_000_000_000
    const db = seedBalance(FUNDED)
    const { ingested, ledger } = makeCapturingLedger()

    // Capture the credit-balance projection delta the metering hook forwards
    // after a fresh charge (this is what drives scope.user.<userId>).
    const projected: Array<{
      accountRef: string
      deltaUsdCents: number
      idempotencyKey: string
    }> = []
    const meteringHook = makeLedgerMeteringHook({
      ledgerDb: db,
      nowIso: () => NOW,
      recordCreditBalanceProjection: async event => {
        projected.push({
          accountRef: event.accountRef,
          deltaUsdCents: event.deltaUsdCents,
          idempotencyKey: event.idempotencyKey,
        })
      },
    })

    const outcome = await recordHostedTurnUsageAndCharge(
      { ledger, meteringHook },
      {
        observedAt: NOW,
        ownerUserId: OWNER,
        threadId: 'thread.t1',
        turnId: 'turn.t1',
        usage: bigUsage,
      },
    )

    // 1. Exact usage row was ingested.
    expect(outcome.insertedTokenUsage).toBe(true)
    expect(outcome.tokenUsageEventRef).toBe(hostedKhalaUsageEventId('turn.t1'))
    expect(ingested).toHaveLength(1)
    expect((ingested[0] as Record<string, unknown>).usageTruth).toBe('exact')

    // 2. The balance decreased by EXACTLY the priced charge (no invented rate).
    const expectedUsd = priceRequest({
      fundingKind: 'card',
      model: HOSTED_KHALA_PRICING_MODEL,
      usage: {
        completionTokens: bigUsage.outputTokens,
        promptTokens: bigUsage.inputTokens,
        totalTokens: bigUsage.totalTokens,
      },
    }).chargeUsd
    const expectedMsat = usdToMsatCeil(expectedUsd)
    expect(expectedMsat).toBeGreaterThan(0)
    expect(outcome.metered).toBe(true)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(FUNDED - expectedMsat)

    // 3. The credit_balance projection delta was written (negative charge, same
    // idempotency key the ledger charge used) — this is what fans out to
    // scope.user.<userId>.
    expect(projected).toHaveLength(1)
    expect(projected[0]!.accountRef).toBe(ACCOUNT)
    expect(projected[0]!.idempotencyKey).toBe(
      inferenceChargeIdempotencyKey(hostedKhalaChargeRequestId('turn.t1')),
    )
    expect(projected[0]!.deltaUsdCents).toBeLessThan(0)
  })

  test('refuses to double-charge: a replayed turn keeps the balance flat', async () => {
    const FUNDED = 1_000_000_000
    const db = seedBalance(FUNDED)
    const { ledger } = makeCapturingLedger()
    const meteringHook = makeLedgerMeteringHook({ ledgerDb: db, nowIso: () => NOW })
    const input = {
      observedAt: NOW,
      ownerUserId: OWNER,
      threadId: 'thread.t1',
      turnId: 'turn.t1',
      usage,
    }

    await recordHostedTurnUsageAndCharge({ ledger, meteringHook }, input)
    const afterFirst = await readAgentBalance(db, ACCOUNT)
    await recordHostedTurnUsageAndCharge({ ledger, meteringHook }, input)
    const afterSecond = await readAgentBalance(db, ACCOUNT)

    expect(afterSecond?.balanceMsat).toBe(afterFirst?.balanceMsat)
    expect(afterFirst?.balanceMsat).toBeLessThan(FUNDED)
  })

  test('zero balance => insufficient_credit, balance never goes negative', async () => {
    const db = seedBalance(1) // 1 msat, far less than any real charge
    const { ledger } = makeCapturingLedger()
    const meteringHook = makeLedgerMeteringHook({ ledgerDb: db, nowIso: () => NOW })

    const outcome = await recordHostedTurnUsageAndCharge(
      { ledger, meteringHook },
      {
        observedAt: NOW,
        ownerUserId: OWNER,
        threadId: 'thread.t1',
        turnId: 'turn.t1',
        usage,
      },
    )

    expect(outcome.metered).toBe(false)
    expect(outcome.failureReason).toBe('insufficient_credit')
    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(1) // unchanged, never negative
  })

  test('chargeUsdCents matches the pricing engine for the hosted Gemini row', () => {
    const cents = hostedKhalaChargeUsdCents(usage)
    const expected = Math.round(
      priceRequest({
        fundingKind: 'card',
        model: HOSTED_KHALA_PRICING_MODEL,
        usage: {
          completionTokens: usage.outputTokens,
          promptTokens: usage.inputTokens,
          totalTokens: usage.totalTokens,
        },
      }).chargeUsd * 100,
    )
    expect(cents).toBe(expected)
  })
})
