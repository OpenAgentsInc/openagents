import { describe, expect, test } from 'vitest'

import { awaitPostgresReady } from './postgres-readiness'

const proxyNotReady = (): Error =>
  Object.assign(
    new Error(
      'write CONNECT_TIMEOUT /cloudsql/openagentsgemini:us-central1:khala-sync-pg/.s.PGSQL.5432',
    ),
    { code: 'CONNECT_TIMEOUT' },
  )

/** A deterministic clock so a 90-second budget costs a test no wall time. */
const fakeClock = (intervalMs: number) => {
  let nowMs = 0
  return {
    now: () => nowMs,
    sleep: async (ms: number) => {
      nowMs += ms
    },
    advanceOnFailure: intervalMs,
  }
}

describe('Cloud Run Postgres readiness gate', () => {
  test('opens the port as soon as the first probe answers', async () => {
    const clock = fakeClock(1_000)
    let probes = 0
    const outcome = await awaitPostgresReady(
      async () => {
        probes += 1
      },
      { now: clock.now, sleep: clock.sleep },
    )
    expect(outcome).toEqual({ attempts: 1, elapsedMs: 0, ready: true })
    expect(probes).toBe(1)
  })

  test('waits out the Cloud SQL proxy cold start instead of serving 503s', async () => {
    const clock = fakeClock(1_000)
    let probes = 0
    const outcome = await awaitPostgresReady(
      async () => {
        probes += 1
        // 10-70s of proxy warmup was the measured 2026-07-31 range; 40 x 1s
        // sits inside it.
        if (probes <= 40) throw proxyNotReady()
      },
      { intervalMs: 1_000, now: clock.now, sleep: clock.sleep },
    )
    expect(outcome.ready).toBe(true)
    expect(outcome.attempts).toBe(41)
    expect(outcome.elapsedMs).toBe(40_000)
  })

  test('is fail-open: a genuinely down database still starts the server', async () => {
    const clock = fakeClock(1_000)
    const failures: Array<number> = []
    const outcome = await awaitPostgresReady(
      async () => {
        throw proxyNotReady()
      },
      {
        budgetMs: 5_000,
        intervalMs: 1_000,
        now: clock.now,
        onAttemptFailed: info => failures.push(info.attempt),
        sleep: clock.sleep,
      },
    )
    expect(outcome.ready).toBe(false)
    // Bounded: it must not wait forever, and it must not exceed the budget.
    expect(outcome.elapsedMs).toBeLessThanOrEqual(5_000)
    expect(failures.length).toBe(outcome.attempts)
  })

  test('reports every failed attempt so a slow start is diagnosable', async () => {
    const clock = fakeClock(1_000)
    let probes = 0
    const seen: Array<string> = []
    await awaitPostgresReady(
      async () => {
        probes += 1
        if (probes <= 2) throw proxyNotReady()
      },
      {
        intervalMs: 1_000,
        now: clock.now,
        onAttemptFailed: info =>
          seen.push(
            `${info.attempt}@${info.elapsedMs}:${
              info.error instanceof Error ? info.error.message.slice(0, 20) : ''
            }`,
          ),
        sleep: clock.sleep,
      },
    )
    expect(seen).toEqual([
      '1@0:write CONNECT_TIMEOU',
      '2@1000:write CONNECT_TIMEOU',
    ])
  })

  test('never throws, whatever the probe does', async () => {
    const clock = fakeClock(1_000)
    const outcome = await awaitPostgresReady(
      async () => {
        throw 'not an Error'
      },
      {
        budgetMs: 2_000,
        intervalMs: 1_000,
        now: clock.now,
        sleep: clock.sleep,
      },
    )
    expect(outcome.ready).toBe(false)
  })
})
