// KS-8.9 (#8320): inference entitlements store seam — flags, fail-soft
// dual-write, hot-path non-blocking regression, and read routing.
//
// Load-bearing properties pinned here:
//   - flags: dual-write defaults ON; reads default 'd1'; unknown read
//     values fall back to 'd1' (never fail open into an unproven
//     ENFORCING read path).
//   - d1 mode (default): the routing factory returns NO gateReads, so the
//     gates keep their untouched inline D1 reads — and the Postgres SQL
//     client factory is NEVER invoked on the read path (the hot-path
//     zero-added-latency regression).
//   - the mirror is FIRE-SAFE: synchronous enqueue (the caller's write
//     path resolves before a hanging Postgres apply does), a rejected
//     apply NEVER throws into the caller — it logs the typed
//     `khala_sync_entitlements_dual_write_failed` drift diagnostic.
//   - compare mode serves the D1 decision and logs
//     `khala_sync_entitlements_read_compare_mismatch` off-path.
//   - postgres mode serves Postgres when healthy and falls back to D1
//     (with a diagnostic) on error — the gate is never broken by the
//     migration store.

import { describe, expect, test } from 'vitest'

import {
  inferenceEntitlementsFlagsFromEnv,
  makeInferenceEntitlementsMirror,
  makeInferenceEntitlementsRoutingForEnv,
  makeRoutedEntitlementsGateReads,
  mirrorOpRefs,
  type InferenceEntitlementsDiagnostic,
  type InferenceEntitlementsDiagnosticEvent,
  type InferenceEntitlementsGateReads,
  type InferenceEntitlementsMirrorOp,
} from './inference-entitlements-store'

const collectLog = () => {
  const events: Array<{
    event: InferenceEntitlementsDiagnosticEvent
    fields: InferenceEntitlementsDiagnostic
  }> = []
  return {
    events,
    log: (
      event: InferenceEntitlementsDiagnosticEvent,
      fields: InferenceEntitlementsDiagnostic,
    ) => {
      events.push({ event, fields })
    },
  }
}

const gateReadsStub = (
  overrides: Partial<InferenceEntitlementsGateReads> = {},
): InferenceEntitlementsGateReads => ({
  freeTierKeyExists: () => Promise.resolve(true),
  freeTierUsage: () => Promise.resolve({ requestsToday: 1, tokensToday: 10 }),
  freeUsageState: () =>
    Promise.resolve({ cumulativeFreeUsdMicros: 0, earnedFreeUsdMicros: 0 }),
  operatorExempt: () => Promise.resolve(false),
  premiumAllowlisted: () => Promise.resolve(false),
  privacyEntitlementExists: () => Promise.resolve(false),
  ...overrides,
})

describe('inferenceEntitlementsFlagsFromEnv', () => {
  test('dual-write defaults ON; reads default d1', () => {
    expect(inferenceEntitlementsFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
  })

  test('off tokens disable dual-write', () => {
    for (const value of ['0', 'off', 'false', 'disabled', 'no', ' OFF ']) {
      expect(
        inferenceEntitlementsFlagsFromEnv({
          KHALA_SYNC_ENTITLEMENTS_DUAL_WRITE: value,
        }).dualWrite,
      ).toBe(false)
    }
  })

  test('reads accept postgres/compare; unknown values fall back to d1', () => {
    expect(
      inferenceEntitlementsFlagsFromEnv({
        KHALA_SYNC_ENTITLEMENTS_READS: 'postgres',
      }).reads,
    ).toBe('postgres')
    expect(
      inferenceEntitlementsFlagsFromEnv({
        KHALA_SYNC_ENTITLEMENTS_READS: ' COMPARE ',
      }).reads,
    ).toBe('compare')
    expect(
      inferenceEntitlementsFlagsFromEnv({
        KHALA_SYNC_ENTITLEMENTS_READS: 'postgress-typo',
      }).reads,
    ).toBe('d1')
  })
})

describe('makeInferenceEntitlementsRoutingForEnv', () => {
  const fakeDb = {} as D1Database

  test('no binding => undefined (plain D1 everywhere)', () => {
    expect(
      makeInferenceEntitlementsRoutingForEnv({ OPENAGENTS_DB: fakeDb }),
    ).toBeUndefined()
  })

  test('dual-write off AND d1 reads => undefined (nothing to do)', () => {
    expect(
      makeInferenceEntitlementsRoutingForEnv({
        KHALA_SYNC_DB: { connectionString: 'postgres://example' },
        KHALA_SYNC_ENTITLEMENTS_DUAL_WRITE: 'off',
        OPENAGENTS_DB: fakeDb,
      }),
    ).toBeUndefined()
  })

  test('default flags: mirror present, gateReads ABSENT, and the SQL client factory is never invoked before a mirrored write (hot-path d1 regression)', () => {
    let clientAcquisitions = 0
    const routing = makeInferenceEntitlementsRoutingForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://example' },
        OPENAGENTS_DB: fakeDb,
      },
      {
        makeSqlClient: () => {
          clientAcquisitions += 1
          return Promise.reject(new Error('never reached in this test'))
        },
      },
    )
    expect(routing).toBeDefined()
    // Reads stay unrouted: gates keep their inline D1 reads untouched.
    expect(routing?.gateReads).toBeUndefined()
    // Constructing the seam costs no connection; a mirror with zero ops
    // costs no connection either.
    routing?.mirror([])
    expect(clientAcquisitions).toBe(0)
  })
})

describe('makeInferenceEntitlementsMirror (fire-safe dual-write)', () => {
  const writeOp: InferenceEntitlementsMirrorOp = {
    kind: 'write',
    row: { account_ref: 'agent:a', created_at: 'now', updated_at: 'now' },
    table: 'inference_free_tier_keys',
  }

  test('the caller is NEVER blocked: mirror() returns synchronously while the apply hangs', async () => {
    let applied = false
    let release: () => void = () => {}
    const gate = new Promise<void>(resolve => {
      release = resolve
    })
    const scheduled: Array<Promise<void>> = []
    const mirror = makeInferenceEntitlementsMirror({
      log: () => {},
      schedule: work => {
        scheduled.push(work)
      },
      store: {
        applyMirrorOps: async () => {
          await gate
          applied = true
        },
      },
    })

    // Synchronous enqueue: returns before the apply resolves.
    mirror([writeOp])
    expect(applied).toBe(false)
    expect(scheduled).toHaveLength(1)

    release()
    await scheduled[0]
    expect(applied).toBe(true)
  })

  test('a rejected apply NEVER throws — it logs the dual-write drift diagnostic', async () => {
    const { events, log } = collectLog()
    const scheduled: Array<Promise<void>> = []
    const mirror = makeInferenceEntitlementsMirror({
      log,
      schedule: work => {
        scheduled.push(work)
      },
      store: {
        applyMirrorOps: () => Promise.reject(new Error('pg down')),
      },
    })

    expect(() => mirror([writeOp])).not.toThrow()
    await Promise.all(scheduled)
    expect(events).toHaveLength(1)
    expect(events[0]?.event).toBe('khala_sync_entitlements_dual_write_failed')
    expect(events[0]?.fields.messageSafe).toContain('pg down')
    expect(events[0]?.fields.refs).toEqual([
      'inference_free_tier_keys:agent:a',
    ])
  })

  test('even a synchronously-throwing scheduler cannot reach the write path', () => {
    const { events, log } = collectLog()
    const mirror = makeInferenceEntitlementsMirror({
      log,
      schedule: () => {
        throw new Error('scheduler broke')
      },
      store: { applyMirrorOps: () => Promise.resolve() },
    })
    expect(() => mirror([writeOp])).not.toThrow()
    expect(events[0]?.event).toBe('khala_sync_entitlements_dual_write_failed')
  })

  test('mirrorOpRefs stays public-safe and keyed per op kind', () => {
    expect(
      mirrorOpRefs({
        event: {
          accountRef: 'agent:a',
          createdAt: 'now',
          requestId: 'req-9',
          servedModel: 'openagents/khala',
          totalTokens: 10,
          usageDay: '2026-07-04',
        },
        kind: 'accrue_free_tier_usage',
      }),
    ).toEqual(['inference_free_tier_usage_events:req-9'])
    expect(
      mirrorOpRefs({
        consumedAt: 'now',
        entitlementRef: 'ent-1',
        kind: 'consume_entitlement',
        table: 'agent_search_entitlements',
      }),
    ).toEqual(['agent_search_entitlements:ent-1'])
  })
})

describe('makeRoutedEntitlementsGateReads', () => {
  test('compare mode serves the D1 decision immediately and logs the mismatch OFF the response path', async () => {
    const { events, log } = collectLog()
    const scheduled: Array<Promise<void>> = []
    let releasePg: (value: boolean) => void = () => {}
    const pgGate = new Promise<boolean>(resolve => {
      releasePg = resolve
    })
    const routed = makeRoutedEntitlementsGateReads({
      d1: gateReadsStub({ freeTierKeyExists: () => Promise.resolve(true) }),
      flags: { dualWrite: true, reads: 'compare' },
      log,
      postgres: gateReadsStub({ freeTierKeyExists: () => pgGate }),
      schedule: work => {
        scheduled.push(work)
      },
    })

    // The caller's promise resolves on the D1 read alone — the Postgres
    // shadow has not even resolved yet.
    const decision = await routed.freeTierKeyExists('agent:a')
    expect(decision).toBe(true)
    expect(events).toHaveLength(0)

    releasePg(false)
    await Promise.all(scheduled)
    expect(events).toHaveLength(1)
    expect(events[0]?.event).toBe(
      'khala_sync_entitlements_read_compare_mismatch',
    )
    expect(events[0]?.fields.op).toBe('freeTierKeyExists')
  })

  test('compare mode: matching decisions log nothing; a failing shadow read logs read_failed', async () => {
    const { events, log } = collectLog()
    const scheduled: Array<Promise<void>> = []
    const routed = makeRoutedEntitlementsGateReads({
      d1: gateReadsStub(),
      flags: { dualWrite: true, reads: 'compare' },
      log,
      postgres: gateReadsStub({
        freeTierUsage: () => Promise.reject(new Error('pg exploded')),
      }),
      schedule: work => {
        scheduled.push(work)
      },
    })

    expect(await routed.freeTierKeyExists('agent:a')).toBe(true)
    expect(await routed.freeTierUsage('agent:a', '2026-07-04')).toEqual({
      requestsToday: 1,
      tokensToday: 10,
    })
    await Promise.all(scheduled)
    expect(events.map(entry => entry.event)).toEqual([
      'khala_sync_entitlements_postgres_read_failed',
    ])
  })

  test('postgres mode serves Postgres when healthy; falls back to D1 with a diagnostic on error', async () => {
    const { events, log } = collectLog()
    const routed = makeRoutedEntitlementsGateReads({
      d1: gateReadsStub({
        freeUsageState: () =>
          Promise.resolve({
            cumulativeFreeUsdMicros: 111,
            earnedFreeUsdMicros: 0,
          }),
        premiumAllowlisted: () => Promise.resolve(true),
      }),
      flags: { dualWrite: true, reads: 'postgres' },
      log,
      postgres: gateReadsStub({
        freeUsageState: () =>
          Promise.resolve({
            cumulativeFreeUsdMicros: 222,
            earnedFreeUsdMicros: 5,
          }),
        premiumAllowlisted: () => Promise.reject(new Error('pg timeout')),
      }),
    })

    // Healthy: the Postgres value is served.
    expect(await routed.freeUsageState('owner:u1')).toEqual({
      cumulativeFreeUsdMicros: 222,
      earnedFreeUsdMicros: 5,
    })
    // Broken: single attempt, D1 fallback + diagnostic — the gate decision
    // is never broken by the migration store.
    expect(await routed.premiumAllowlisted('owner:u1')).toBe(true)
    expect(events.map(entry => entry.event)).toEqual([
      'khala_sync_entitlements_postgres_read_fallback',
    ])
    expect(events[0]?.fields.op).toBe('premiumAllowlisted')
  })
})
