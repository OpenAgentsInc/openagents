// #8362 follow-up: pure unit tests for `makeRoutedIdentityAuthNonGateReads`
// (the bounded non-gate read router) — no local Postgres required. The
// broader D1-vs-Postgres ANSWER-parity contract test for
// `providerAccountPoolStateByUserId` lives in
// `identity-auth-domain-repository.contract.test.ts` (real local Postgres).
//
// Load-bearing properties pinned here:
//   - compare mode serves the D1 answer immediately (zero blocking
//     latency) and schedules a detached Postgres shadow read; a mismatch
//     logs `khala_sync_identity_non_gate_read_compare_mismatch`, a shadow
//     failure logs `khala_sync_identity_non_gate_postgres_read_failed` —
//     both distinct from the gate-read diagnostic event names.
//   - postgres mode serves Postgres for real when healthy; a failure falls
//     back to D1 with `khala_sync_identity_non_gate_postgres_read_fallback`
//     — this read can never break the caller.
//   - `KHALA_SYNC_IDENTITY_NON_GATE_READS` is parsed independently of
//     `KHALA_SYNC_IDENTITY_READS` (also pinned in the contract test file).

import { describe, expect, test } from 'vitest'

import {
  makeRoutedIdentityAuthNonGateReads,
  type IdentityAuthDiagnostic,
  type IdentityAuthDiagnosticEvent,
  type IdentityAuthNonGateReads,
  type ProviderAccountPoolStateRow,
} from './identity-auth-domain-store'

const collectLog = () => {
  const events: Array<{
    event: IdentityAuthDiagnosticEvent
    fields: IdentityAuthDiagnostic
  }> = []
  return {
    events,
    log: (event: IdentityAuthDiagnosticEvent, fields: IdentityAuthDiagnostic) => {
      events.push({ event, fields })
    },
  }
}

const poolRow = (
  overrides: Partial<ProviderAccountPoolStateRow> = {},
): ProviderAccountPoolStateRow => ({
  account_label: null,
  cooldown_until: null,
  health: 'healthy',
  low_credit_flag: 0,
  operator_label: null,
  provider: 'chatgpt_codex',
  provider_account_ref: 'provider-account.stub-1',
  status: 'active',
  ...overrides,
})

const nonGateReadsStub = (
  overrides: Partial<IdentityAuthNonGateReads> = {},
): IdentityAuthNonGateReads => ({
  providerAccountPoolStateByUserId: () => Promise.resolve([poolRow()]),
  ...overrides,
})

describe('makeRoutedIdentityAuthNonGateReads', () => {
  test('compare mode serves the D1 answer immediately and logs the mismatch OFF the response path, under the non-gate event names', async () => {
    const { events, log } = collectLog()
    const scheduled: Array<Promise<void>> = []
    let releasePg: (value: ReadonlyArray<ProviderAccountPoolStateRow>) => void =
      () => {}
    const pgGate = new Promise<ReadonlyArray<ProviderAccountPoolStateRow>>(
      resolve => {
        releasePg = resolve
      },
    )
    const routed = makeRoutedIdentityAuthNonGateReads({
      d1: nonGateReadsStub({
        providerAccountPoolStateByUserId: () => Promise.resolve([poolRow()]),
      }),
      flags: { dualWrite: true, nonGateReads: 'compare', reads: 'd1' },
      log,
      postgres: nonGateReadsStub({
        providerAccountPoolStateByUserId: () => pgGate,
      }),
      schedule: work => {
        scheduled.push(work)
      },
    })

    const rows = await routed.providerAccountPoolStateByUserId('user.1', 200)
    expect(rows).toEqual([poolRow()])
    expect(events).toHaveLength(0)

    releasePg([poolRow({ health: 'degraded' })])
    await Promise.all(scheduled)
    expect(events).toHaveLength(1)
    expect(events[0]?.event).toBe(
      'khala_sync_identity_non_gate_read_compare_mismatch',
    )
    expect(events[0]?.fields.op).toBe('providerAccountPoolStateByUserId')
  })

  test('compare mode: a failing shadow read logs the non-gate postgres_read_failed event, never a gate-read event name', async () => {
    const { events, log } = collectLog()
    const scheduled: Array<Promise<void>> = []
    const routed = makeRoutedIdentityAuthNonGateReads({
      d1: nonGateReadsStub(),
      flags: { dualWrite: true, nonGateReads: 'compare', reads: 'd1' },
      log,
      postgres: nonGateReadsStub({
        providerAccountPoolStateByUserId: () =>
          Promise.reject(new Error('pg exploded')),
      }),
      schedule: work => {
        scheduled.push(work)
      },
    })

    expect(
      await routed.providerAccountPoolStateByUserId('user.1', 200),
    ).toEqual([poolRow()])
    await Promise.all(scheduled)
    expect(events.map(entry => entry.event)).toEqual([
      'khala_sync_identity_non_gate_postgres_read_failed',
    ])
  })

  test('postgres mode serves Postgres for real when healthy; falls back to D1 with a diagnostic on error', async () => {
    const { events, log } = collectLog()
    const postgresRows = [poolRow({ status: 'cooldown' })]
    const routed = makeRoutedIdentityAuthNonGateReads({
      d1: nonGateReadsStub({
        providerAccountPoolStateByUserId: () => Promise.resolve([poolRow()]),
      }),
      flags: { dualWrite: true, nonGateReads: 'postgres', reads: 'd1' },
      log,
      postgres: nonGateReadsStub({
        providerAccountPoolStateByUserId: userId =>
          userId === 'user.healthy'
            ? Promise.resolve(postgresRows)
            : Promise.reject(new Error('pg timeout')),
      }),
    })

    // Healthy: the Postgres rows are served for real.
    expect(
      await routed.providerAccountPoolStateByUserId('user.healthy', 200),
    ).toEqual(postgresRows)
    // Broken: single attempt, D1 fallback + diagnostic — this read can
    // never break the caller.
    expect(
      await routed.providerAccountPoolStateByUserId('user.broken', 200),
    ).toEqual([poolRow()])
    expect(events.map(entry => entry.event)).toEqual([
      'khala_sync_identity_non_gate_postgres_read_fallback',
    ])
    expect(events[0]?.fields.op).toBe('providerAccountPoolStateByUserId')
  })
})
