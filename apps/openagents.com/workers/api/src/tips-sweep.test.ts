import { describe, expect, test } from 'vitest'

import {
  TIPS_SWEEP_MIN_SAT,
  runTipsSweepTick,
  selectSweepCandidates,
  sweepAmountSat,
  sweepCreateStatements,
} from './tips-sweep'

describe('sweep amount', () => {
  test('sweeps the excess above threshold, floored to sats', () => {
    expect(
      sweepAmountSat({ balanceMsat: 510_500, sweepThresholdSat: 210 }),
    ).toBe(300)
  })

  test('below the minimum sweeps nothing', () => {
    expect(
      sweepAmountSat({
        balanceMsat: (210 + TIPS_SWEEP_MIN_SAT - 1) * 1000,
        sweepThresholdSat: 210,
      }),
    ).toBe(0)
  })
})

describe('sweep statements', () => {
  test('funding leg debits the agent, payout leg is lightning with the claim ref only', () => {
    const statements = sweepCreateStatements(
      {
        actorRef: 'agent:alice',
        amountSat: 300,
        fundingLegId: 'leg_in',
        idempotencyKey: 'sweep:agent:alice:t',
        payInId: 'payin_sweep',
        payoutLegId: 'leg_out',
        walletClaimRef: 'wallet.public.alice.redacted',
      },
      '2026-06-10T21:00:00.000Z',
    )

    const flat = statements
      .map(statement => statement.sql.replace(/\s+/g, ' ').trim())
      .join('\n')
    const params = statements.flatMap(statement => statement.params)

    expect(flat).toContain('balance_msat = balance_msat - ?')
    expect(params).toContain('wallet.public.alice.redacted')
    expect(params).toContain(300_000)
    // The raw offer never appears in ledger statements.
    expect(params.some(p => typeof p === 'string' && p.startsWith('lno1'))).toBe(
      false,
    )
  })
})

describe('sweep tick', () => {
  test('unconfigured buffer is a typed skip, not an error', async () => {
    const outcome = await runTipsSweepTick(null as never, {
      makeId: () => 'id',
      nowIso: '2026-06-10T21:00:00.000Z',
      payFromBuffer: null,
    })

    expect(outcome).toEqual({
      attempted: 0,
      failed: 0,
      settled: 0,
      skippedReason: 'tips_buffer_unconfigured',
    })
  })

  test('settle and fail paths drive the ledger correctly', async () => {
    const executed: string[] = []
    const candidateRows = [
      {
        actor_ref: 'agent:alice',
        balance_msat: 510_000,
        bolt12_offer: null,
        lightning_address: 'alice@spark.money',
        sweep_threshold_sat: 210,
        wallet_ref: 'wallet.public.alice.redacted',
      },
      {
        actor_ref: 'agent:bob',
        balance_msat: 1_210_000,
        bolt12_offer: 'lno1other',
        lightning_address: null,
        sweep_threshold_sat: 210,
        wallet_ref: 'wallet.public.bob.redacted',
      },
    ]

    const fakeDb = {
      batch: async (statements: unknown[]) => {
        executed.push(`batch:${statements.length}`)
        return []
      },
      prepare: (sql: string) => ({
        bind: (..._params: unknown[]) => ({
          all: async () => ({ results: candidateRows }),
          first: async () => null,
        }),
        // direct statements inside batches
        toString: () => sql,
      }),
    } as never

    let calls = 0
    const outcome = await runTipsSweepTick(fakeDb, {
      makeId: () => `id_${++calls}_${Math.floor(calls / 100)}`,
      nowIso: '2026-06-10T21:00:00.000Z',
      payFromBuffer: async input =>
        input.destination === 'alice@spark.money'
          ? { ok: true, paymentRef: 'payment.buffer.abc' }
          : { ok: false, reason: 'invoice_fetch_timeout' },
    })

    expect(outcome.attempted).toBe(2)
    expect(outcome.settled).toBe(1)
    expect(outcome.failed).toBe(1)
    expect(outcome.skippedReason).toBeNull()
    // create+forwarding batch, then settle batch, then create+forwarding
    // batch, then fail/refund batch.
    expect(executed.length).toBe(4)
  })

  test('candidate selection excludes escrow-held balance from sweepable amount', async () => {
    let capturedSql = ''
    const fakeDb = {
      prepare: (sql: string) => {
        capturedSql = sql
        return {
          bind: (..._params: unknown[]) => ({
            all: async () => ({
              results: [
                {
                  actor_ref: 'agent:alice',
                  available_balance_msat: 510_000,
                  balance_msat: 710_000,
                  bolt12_offer: 'lno1test',
                  lightning_address: 'alice@spark.money',
                  sweep_threshold_sat: 210,
                  wallet_ref: 'wallet.public.alice.redacted',
                },
              ],
            }),
          }),
        }
      },
    } as never

    const candidates = await selectSweepCandidates(
      fakeDb,
      '2026-06-10T21:00:00.000Z',
      1,
    )

    expect(capturedSql).toContain('COALESCE(b.held_msat, 0)')
    expect(candidates).toEqual([
      {
        actorRef: 'agent:alice',
        balanceMsat: 510_000,
        payoutDestination: 'alice@spark.money',
        sweepThresholdSat: 210,
        walletClaimRef: 'wallet.public.alice.redacted',
      },
    ])
    expect(sweepAmountSat(candidates[0]!)).toBe(300)
  })
})
