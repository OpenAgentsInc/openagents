import { describe, expect, test } from 'vitest'

import type { AuthKvStore } from '../auth/auth-kv'
import {
  DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS,
  hostedComputeReservationKey,
  hostedComputeReservationPrefix,
  hostedComputeReservedTokens,
  reserveHostedComputeTokens,
  reservedTokensByOthers,
} from './hosted-compute-token-reservation'
import {
  decideHostedComputeDailyCeiling,
  makeHostedComputeDailyCeilingGate,
} from './hosted-compute-daily-ceiling'

const makeMemoryKv = (): Readonly<{
  store: AuthKvStore
  entries: Map<string, string>
}> => {
  const entries = new Map<string, string>()
  const store: AuthKvStore = {
    delete: async key => {
      entries.delete(key)
    },
    get: (async (key: string) => entries.get(key) ?? null) as AuthKvStore['get'],
    listPrefix: async prefix =>
      [...entries.entries()]
        .filter(([key]) => key.startsWith(prefix))
        .map(([key, value]) => ({ key, value }))
        .sort((left, right) => left.key.localeCompare(right.key)),
    put: async (key, value) => {
      entries.set(key, value)
    },
    putIfAbsent: async (key, value) => {
      if (entries.has(key)) {
        return false
      }
      entries.set(key, value)

      return true
    },
  }

  return { entries, store }
}

const DIGEST = 'a'.repeat(32)
const OTHER_DIGEST = 'b'.repeat(32)

const reserve = (store: AuthKvStore, attemptId: string, actorDigest = DIGEST) =>
  reserveHostedComputeTokens({
    actorDigest,
    attemptId,
    reservedTokens: DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS,
    store,
  })

describe('hosted compute token reservation', () => {
  test('a serial caller sees no reservation from others (allowance is unchanged)', async () => {
    const { store } = makeMemoryKv()
    const first = await reserve(store, 'attempt-1')

    expect(first.reservedByOthers).toBe(0)

    await first.release()

    const second = await reserve(store, 'attempt-2')

    expect(second.reservedByOthers).toBe(0)
  })

  test('a concurrent burst sees every already-admitted execution', async () => {
    const { store } = makeMemoryKv()
    const held = await Promise.all([
      reserve(store, 'attempt-1'),
      reserve(store, 'attempt-2'),
      reserve(store, 'attempt-3'),
    ])

    // Each reserves BEFORE reading, so the LAST reader necessarily sees the
    // other two. Sequential admission is what the bound relies on.
    const fourth = await reserve(store, 'attempt-4')

    expect(fourth.reservedByOthers).toBe(
      3 * DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS,
    )
    expect(held).toHaveLength(3)
  })

  test('one actor never counts another actor whose key prefix overlaps', async () => {
    const { store } = makeMemoryKv()

    await reserve(store, 'attempt-1', OTHER_DIGEST)

    const mine = await reserve(store, 'attempt-2', DIGEST)

    expect(mine.reservedByOthers).toBe(0)
    expect(hostedComputeReservationPrefix(DIGEST)).not.toBe(
      hostedComputeReservationPrefix(OTHER_DIGEST),
    )
  })

  test('release drops the hold so it cannot outlive the execution', async () => {
    const { entries, store } = makeMemoryKv()
    const held = await reserve(store, 'attempt-1')

    expect(entries.has(hostedComputeReservationKey(DIGEST, 'attempt-1'))).toBe(
      true,
    )

    await held.release()

    expect(entries.has(hostedComputeReservationKey(DIGEST, 'attempt-1'))).toBe(
      false,
    )
  })

  test('a failing listPrefix drops its own marker and propagates (fail closed, no phantom hold)', async () => {
    const { entries, store } = makeMemoryKv()
    const failing: AuthKvStore = {
      ...store,
      listPrefix: () => Promise.reject(new Error('kv_down')),
    }

    await expect(
      reserveHostedComputeTokens({
        actorDigest: DIGEST,
        attemptId: 'attempt-1',
        reservedTokens: DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS,
        store: failing,
      }),
    ).rejects.toThrow('kv_down')

    expect(entries.size).toBe(0)
  })

  test('an unreadable marker still counts as a real in-flight execution', () => {
    expect(
      reservedTokensByOthers(
        [
          { key: 'p:other', value: 'not-a-number' },
          { key: 'p:mine', value: '5' },
        ],
        'p:mine',
        320_000,
      ),
    ).toBe(320_000)
  })

  test('the reserved-token size is owner-tunable and never silently disabled', () => {
    expect(hostedComputeReservedTokens('50000')).toBe(50_000)
    expect(hostedComputeReservedTokens(undefined)).toBe(
      DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS,
    )
    // Zero or negative would disable the bound, so both fall back.
    expect(hostedComputeReservedTokens('0')).toBe(
      DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS,
    )
    expect(hostedComputeReservedTokens('-1')).toBe(
      DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS,
    )
    expect(hostedComputeReservedTokens('nonsense')).toBe(
      DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS,
    )
  })
})

describe('hosted compute ceiling overshoot bound', () => {
  // THE BOUND. A request is admitted only while `served + (k-1)*R < ceiling`,
  // so at most floor(H/R)+1 executions are ever concurrently admitted. If no
  // single execution draws more than R, the total drawn is at most
  // `ceiling + R` -- ONE reservation of overshoot, where before it was
  // unbounded.
  //
  // The simulation below is the executable form of that argument: it admits
  // requests exactly the way the gate does, then draws the WORST observed
  // per-execution figure for every admitted request.
  const RESERVED = DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS
  // Largest single `omega_provider_broker` execution ever recorded (2,418 rows).
  const WORST_OBSERVED_DRAW = 301_155

  const simulateBurst = (
    input: Readonly<{
      burst: number
      ceiling: number
      servedBefore: number
      drawPerExecution: number
      reserved: number
    }>,
  ): Readonly<{ admitted: number; totalDrawn: number }> => {
    let admitted = 0
    for (let index = 0; index < input.burst; index += 1) {
      // Worst case: every earlier request of the burst is still in flight, so
      // it holds a reservation and nothing has settled into the exact sum yet.
      const refusal = decideHostedComputeDailyCeiling({
        reservedByOthers: admitted * input.reserved,
        servedToday: input.servedBefore,
        tokensPerDay: input.ceiling,
      })
      if (refusal === undefined) {
        admitted += 1
      }
    }

    return {
      admitted,
      totalDrawn: input.servedBefore + admitted * input.drawPerExecution,
    }
  }

  test('a burst of 1000 cannot exceed the ceiling by more than one reservation', () => {
    const ceiling = 1_000_000
    const result = simulateBurst({
      burst: 1000,
      ceiling,
      drawPerExecution: WORST_OBSERVED_DRAW,
      reserved: RESERVED,
      servedBefore: 0,
    })

    expect(result.admitted).toBe(Math.floor(ceiling / RESERVED) + 1)
    expect(result.totalDrawn).toBeLessThanOrEqual(ceiling + RESERVED)
  })

  test('the bound holds from any starting headroom', () => {
    const ceiling = 1_000_000
    for (const servedBefore of [0, 1, 250_000, 640_000, 900_000, 999_999]) {
      const result = simulateBurst({
        burst: 500,
        ceiling,
        drawPerExecution: WORST_OBSERVED_DRAW,
        reserved: RESERVED,
        servedBefore,
      })

      expect(result.totalDrawn).toBeLessThanOrEqual(ceiling + RESERVED)
    }
  })

  test('the reservation is sized above every draw observed on this route', () => {
    expect(RESERVED).toBeGreaterThan(WORST_OBSERVED_DRAW)
  })

  // THE COUNTERFACTUAL: without reservations, the same burst is unbounded.
  // This is what production did before, and it is why 4 concurrent executions
  // could already blow a whole day's ceiling.
  test('WITHOUT reservations the same burst is unbounded (the defect being fixed)', () => {
    const ceiling = 1_000_000
    const result = simulateBurst({
      burst: 1000,
      ceiling,
      drawPerExecution: WORST_OBSERVED_DRAW,
      reserved: 0,
      servedBefore: 0,
    })

    expect(result.admitted).toBe(1000)
    expect(result.totalDrawn).toBeGreaterThan(300 * ceiling)
  })

  test('a serial caller keeps exactly the allowance it had before reservations', () => {
    const ceiling = 1_000_000

    // Right at the boundary, with nothing else in flight, behaviour is
    // identical to the pre-reservation gate.
    expect(
      decideHostedComputeDailyCeiling({
        reservedByOthers: 0,
        servedToday: 999_999,
        tokensPerDay: ceiling,
      }),
    ).toBeUndefined()
    expect(
      decideHostedComputeDailyCeiling({
        reservedByOthers: 0,
        servedToday: ceiling,
        tokensPerDay: ceiling,
      }),
    ).toMatchObject({ tokensServedToday: ceiling })
  })

  test('a refusal reports EXACT settled tokens, never the provisional hold', async () => {
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: new Set<string>(),
      reservedTokensByOthers: () => Promise.resolve(900_000),
      servedTokensToday: () => Promise.resolve(150_000),
      tokensPerDay: () => 1_000_000,
    })

    await expect(gate('agent:test')).resolves.toMatchObject({
      dailyTokenCeiling: 1_000_000,
      tokensServedToday: 150_000,
    })
  })

  test('a reservation read that throws refuses (fail closed)', async () => {
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: new Set<string>(),
      reservedTokensByOthers: () => Promise.reject(new Error('kv_down')),
      servedTokensToday: () => Promise.resolve(0),
      tokensPerDay: () => 1_000_000,
    })

    await expect(gate('agent:test')).resolves.toMatchObject({
      error: 'free_tier_daily_token_ceiling_reached',
    })
  })

  test('an admitted actor takes no reservation and reads no ledger', async () => {
    let touched = false
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: new Set(['agent:owner']),
      reservedTokensByOthers: () => {
        touched = true

        return Promise.resolve(0)
      },
      servedTokensToday: () => {
        touched = true

        return Promise.resolve(0)
      },
      tokensPerDay: () => 0,
    })

    await expect(gate('owner')).resolves.toBeUndefined()
    expect(touched).toBe(false)
  })
})
