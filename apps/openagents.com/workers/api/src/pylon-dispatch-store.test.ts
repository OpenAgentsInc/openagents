// KS-8.1 (#8307): dual-write wrapper + flag routing unit tests.
//
// Fake stores, no databases: these prove the wrapper's CONTRACT —
// D1-first authority, fail-soft Postgres mirroring with typed drift
// diagnostics, per-flag read routing, bounded retry + D1 fallback in
// postgres mode, and mismatch logging (with refs) in compare mode.

import { describe, expect, test } from 'vitest'

import type {
  PylonApiAssignmentRecord,
  PylonApiEventRecord,
  PylonApiProviderJobLifecycleRecord,
  PylonApiQuarantineRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
  PylonSparkPayoutTargetRecord,
  PylonSparkPayoutTargetStore,
} from './pylon-api'
import {
  makeDualWritePylonApiStore,
  makeDualWritePylonSparkPayoutTargetStore,
  pylonDispatchFlagsFromEnv,
  type PostgresPylonDispatchStore,
  type PylonDispatchDiagnostic,
  type PylonDispatchDiagnosticEvent,
} from './pylon-dispatch-store'

// ---------------------------------------------------------------------------
// Fixtures + fakes
// ---------------------------------------------------------------------------

const assignment = (ref: string): PylonApiAssignmentRecord => ({
  acceptanceCriteriaRefs: [],
  acceptedWorkRefs: [],
  artifactRefs: [],
  assignmentRef: ref,
  closeoutRefs: [],
  codingAssignment: null,
  createdAt: '2026-07-01T01:00:00.000Z',
  id: `assignment_${ref}`,
  idempotencyKeyHash: `hash-${ref}`,
  jobKind: 'codex_agent_task',
  leaseExpiresAt: '2026-07-01T02:00:00.000Z',
  ownerAgentUserId: 'agent-user-1',
  paymentMode: 'unpaid_smoke',
  proofRefs: [],
  publicProjectionJson: '{}',
  pylonRef: 'pylon.dual.1',
  rejectionRefs: [],
  resultExpectationRefs: [],
  state: 'offered',
  taskRefs: [],
  updatedAt: '2026-07-01T01:00:00.000Z',
})

const registration = (pylonRef: string): PylonApiRegistrationRecord => ({
  capabilityRefs: [],
  clientProtocolVersion: null,
  clientVersion: null,
  createdAt: '2026-07-01T00:00:00.000Z',
  displayName: 'Dual Pylon',
  id: `registration_${pylonRef}`,
  latestCapacityRefs: [],
  latestHealthRefs: [],
  latestHeartbeatAt: null,
  latestHeartbeatStatus: null,
  latestLoadRefs: [],
  latestResourceMode: null,
  ownerAgentCredentialId: 'credential-1',
  ownerAgentTokenPrefix: 'oa_agent_x',
  ownerAgentUserId: 'agent-user-1',
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  pylonRef,
  resourceMode: 'balanced',
  status: 'active',
  updatedAt: '2026-07-01T00:00:00.000Z',
  walletReady: false,
  walletRef: null,
})

const event = (ref: string): PylonApiEventRecord => ({
  assignmentRef: null,
  createdAt: '2026-07-01T01:10:00.000Z',
  eventBody: {},
  eventKind: 'assignment_progress',
  eventRef: ref,
  id: `event_${ref}`,
  idempotencyKeyHash: `hash-${ref}`,
  ownerAgentUserId: 'agent-user-1',
  publicProjectionJson: '{}',
  pylonRef: 'pylon.dual.1',
  status: 'running',
})

const lifecycle = (ref: string): PylonApiProviderJobLifecycleRecord => ({
  acceptedWorkRefs: [],
  artifactRefs: [],
  assignmentRef: ref,
  closeoutRefs: [],
  createdAt: '2026-07-01T01:00:00.000Z',
  id: `lifecycle_${ref}`,
  jobKind: 'codex_agent_task',
  ownerAgentUserId: 'agent-user-1',
  proofRefs: [],
  publicProjectionJson: '{}',
  pylonRef: 'pylon.dual.1',
  stage: 'offered',
  taskRefs: [],
  updatedAt: '2026-07-01T01:00:00.000Z',
})

const quarantine = (ref: string): PylonApiQuarantineRecord => ({
  actionRefs: ['action.public.pylon_quarantine'],
  createdAt: '2026-07-01T01:00:00.000Z',
  expiresAt: null,
  id: `quarantine_${ref}`,
  ownerAgentUserId: 'agent-user-1',
  publicProjectionJson: '{}',
  pylonRef: 'pylon.dual.1',
  quarantineRef: ref,
  reasonRefs: ['reason.public.pylon_quarantine'],
  releasedAt: null,
  sourceRefs: ['source.public.pylon_quarantine'],
  state: 'active',
  updatedAt: '2026-07-01T01:00:00.000Z',
})

const sparkTarget = (pylonRef: string): PylonSparkPayoutTargetRecord => ({
  createdAt: '2026-07-01T01:00:00.000Z',
  ownerAgentUserId: 'agent-user-1',
  payoutTargetRef: 'payout.spark.12345678',
  pylonRef,
  rawSparkAddress: 'spark1abcdefghijklmnopqrstuv',
  updatedAt: '2026-07-01T01:00:00.000Z',
})

type Call = { method: string; args: ReadonlyArray<unknown> }

const makeFakeD1 = (overrides: Partial<PylonApiStore> = {}) => {
  const calls: Array<Call> = []
  const track =
    <A extends ReadonlyArray<unknown>, R>(method: string, result: R) =>
    (...args: A): Promise<R> => {
      calls.push({ args, method })
      return Promise.resolve(result)
    }
  const store: PylonApiStore = {
    createAssignment: track('createAssignment', {
      idempotent: false,
      record: assignment('a1'),
    }),
    createEvent: track('createEvent', {
      idempotent: false,
      record: event('e1'),
    }),
    listAssignmentsForPylon: track('listAssignmentsForPylon', [
      assignment('a1'),
    ]),
    listAssignmentsForPylons: track('listAssignmentsForPylons', [
      assignment('a1'),
    ]),
    listEventsForAssignment: track('listEventsForAssignment', []),
    listEventsForPylon: track('listEventsForPylon', []),
    listProviderJobLifecycleForPylons: track(
      'listProviderJobLifecycleForPylons',
      [],
    ),
    listRegistrations: track('listRegistrations', [
      registration('pylon.dual.1'),
    ]),
    listRegistrationsForOwnerAgentUserIds: track(
      'listRegistrationsForOwnerAgentUserIds',
      [registration('pylon.dual.1')],
    ),
    readAssignment: track('readAssignment', assignment('a1')),
    readAssignmentByIdempotencyKeyHash: track(
      'readAssignmentByIdempotencyKeyHash',
      undefined,
    ),
    readEventByIdempotencyKeyHash: track(
      'readEventByIdempotencyKeyHash',
      undefined,
    ),
    readActiveQuarantineForPylon: track(
      'readActiveQuarantineForPylon',
      quarantine('quarantine.public.pylon.dual.1'),
    ),
    readRegistration: track('readRegistration', registration('pylon.dual.1')),
    sweepStaleAssignmentLeases: track('sweepStaleAssignmentLeases', ['a9']),
    updateAssignment: track('updateAssignment', assignment('a1')),
    updateAssignmentIfState: track('updateAssignmentIfState', assignment('a1')),
    upsertProviderJobLifecycle: track(
      'upsertProviderJobLifecycle',
      lifecycle('a1'),
    ),
    upsertQuarantine: track(
      'upsertQuarantine',
      quarantine('quarantine.public.pylon.dual.1'),
    ),
    upsertRegistration: track(
      'upsertRegistration',
      registration('pylon.dual.1'),
    ),
    ...overrides,
  }
  return { calls, store }
}

const makeFakePostgres = (
  overrides: Partial<PostgresPylonDispatchStore> = {},
) => {
  const calls: Array<Call> = []
  const track =
    <A extends ReadonlyArray<unknown>, R>(method: string, result: R) =>
    (...args: A): Promise<R> => {
      calls.push({ args, method })
      return Promise.resolve(result)
    }
  const store: PostgresPylonDispatchStore = {
    createAssignment: track('createAssignment', {
      idempotent: false,
      record: assignment('a1'),
    }),
    createEvent: track('createEvent', {
      idempotent: false,
      record: event('e1'),
    }),
    listAssignmentsForPylon: track('listAssignmentsForPylon', [
      assignment('a1'),
    ]),
    listAssignmentsForPylons: track('listAssignmentsForPylons', [
      assignment('a1'),
    ]),
    listEventsForAssignment: track('listEventsForAssignment', []),
    listEventsForPylon: track('listEventsForPylon', []),
    listRegistrations: track('listRegistrations', [
      registration('pylon.dual.1'),
    ]),
    listRegistrationsForOwnerAgentUserIds: track(
      'listRegistrationsForOwnerAgentUserIds',
      [registration('pylon.dual.1')],
    ),
    listProviderJobLifecycleForPylons: track(
      'listProviderJobLifecycleForPylons',
      [],
    ),
    mirrorAssignment: track('mirrorAssignment', undefined),
    mirrorEvent: track('mirrorEvent', undefined),
    mirrorProviderJobLifecycle: track('mirrorProviderJobLifecycle', undefined),
    mirrorQuarantine: track('mirrorQuarantine', undefined),
    mirrorRegistration: track('mirrorRegistration', undefined),
    mirrorSparkPayoutTarget: track('mirrorSparkPayoutTarget', undefined),
    upsertSparkPayoutTarget: track('upsertSparkPayoutTarget', sparkTarget('pylon.dual.1')),
    readSparkPayoutTarget: track('readSparkPayoutTarget', sparkTarget('pylon.dual.1')),
    readSparkPayoutTargetByOwner: track(
      'readSparkPayoutTargetByOwner',
      sparkTarget('pylon.dual.1'),
    ),
    mirrorStaleSweep: track('mirrorStaleSweep', undefined),
    readAssignment: track('readAssignment', assignment('a1')),
    readAssignmentByIdempotencyKeyHash: track(
      'readAssignmentByIdempotencyKeyHash',
      undefined,
    ),
    readEventByIdempotencyKeyHash: track(
      'readEventByIdempotencyKeyHash',
      undefined,
    ),
    readRegistration: track('readRegistration', registration('pylon.dual.1')),
    sweepStaleAssignmentLeases: track('sweepStaleAssignmentLeases', []),
    updateAssignment: track('updateAssignment', assignment('a1')),
    updateAssignmentIfState: track('updateAssignmentIfState', assignment('a1')),
    upsertProviderJobLifecycle: track(
      'upsertProviderJobLifecycle',
      lifecycle('a1'),
    ),
    upsertQuarantine: track(
      'upsertQuarantine',
      quarantine('quarantine.public.pylon.dual.1'),
    ),
    upsertRegistration: track(
      'upsertRegistration',
      registration('pylon.dual.1'),
    ),
    ...overrides,
  }
  return { calls, store }
}

const makeLogSink = () => {
  const events: Array<{
    event: PylonDispatchDiagnosticEvent
    fields: PylonDispatchDiagnostic
  }> = []
  return {
    events,
    log: (
      event: PylonDispatchDiagnosticEvent,
      fields: PylonDispatchDiagnostic,
    ) => {
      events.push({ event, fields })
    },
  }
}

const noWait = () => Promise.resolve()

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

describe('pylonDispatchFlagsFromEnv', () => {
  test('dual-write defaults ON; reads default d1; writes default postgres (#8515)', () => {
    expect(pylonDispatchFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
      writes: 'postgres',
    })
  })

  test('writes: explicit d1 restores D1 authority; everything else is postgres', () => {
    expect(
      pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_WRITES: 'd1' }).writes,
    ).toBe('d1')
    expect(
      pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_WRITES: 'D1' }).writes,
    ).toBe('d1')
    expect(
      pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_WRITES: 'postgres' }).writes,
    ).toBe('postgres')
    // A typo must NOT silently route writes back to the 401-dead D1 bridge.
    expect(
      pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_WRITES: 'd11' }).writes,
    ).toBe('postgres')
    expect(
      pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_WRITES: '' }).writes,
    ).toBe('postgres')
  })

  test('dual-write off values', () => {
    for (const value of ['off', '0', 'false', 'DISABLED', 'no']) {
      expect(
        pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_DUAL_WRITE: value })
          .dualWrite,
      ).toBe(false)
    }
    expect(
      pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_DUAL_WRITE: 'on' })
        .dualWrite,
    ).toBe(true)
  })

  test('reads accepts postgres/compare; typos fall back to d1', () => {
    expect(
      pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_READS: 'postgres' }).reads,
    ).toBe('postgres')
    expect(
      pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_READS: 'Compare' }).reads,
    ).toBe('compare')
    expect(
      pylonDispatchFlagsFromEnv({ KHALA_SYNC_PYLON_READS: 'psotgres' }).reads,
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Dual-write
// ---------------------------------------------------------------------------

describe('dual-write mirroring', () => {
  test('writes go D1 first, then mirror the RESOLVED record to Postgres', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'd1', writes: 'd1' },
      postgres: pg.store,
      wait: noWait,
    })

    const created = await store.createAssignment(assignment('a1'))
    expect(created.idempotent).toBe(false)
    await store.createEvent(event('e1'))
    await store.updateAssignment(assignment('a1'))
    await store.updateAssignmentIfState(assignment('a1'), 'offered')
    await store.upsertRegistration(registration('pylon.dual.1'))
    const swept = await store.sweepStaleAssignmentLeases!(
      'pylon.dual.1',
      '2026-07-01T05:00:00.000Z',
      '2026-07-01T04:00:00.000Z',
    )
    expect(swept).toEqual(['a9'])

    expect(pg.calls.map(call => call.method)).toEqual([
      'mirrorAssignment',
      'mirrorEvent',
      'mirrorAssignment',
      'mirrorAssignment',
      'mirrorRegistration',
      'mirrorStaleSweep',
    ])
    // The sweep mirrors exactly what D1 swept.
    expect(pg.calls[5]?.args).toEqual([['a9'], '2026-07-01T05:00:00.000Z'])
  })

  test('a Postgres mirror failure NEVER fails the request; it logs a typed diagnostic', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres({
      mirrorAssignment: () => Promise.reject(new Error('pg down')),
    })
    const sink = makeLogSink()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'd1', writes: 'd1' },
      log: sink.log,
      postgres: pg.store,
      wait: noWait,
    })

    const result = await store.createAssignment(assignment('a1'))
    expect(result.record.assignmentRef).toBe('a1')
    expect(sink.events).toHaveLength(1)
    expect(sink.events[0]?.event).toBe('khala_sync_pylon_dual_write_failed')
    expect(sink.events[0]?.fields.op).toBe('createAssignment')
    expect(sink.events[0]?.fields.refs).toEqual(['a1'])
    expect(sink.events[0]?.fields.messageSafe).toContain('pg down')
  })

  test('dual-write OFF: nothing reaches Postgres', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: false, reads: 'd1', writes: 'd1' },
      postgres: pg.store,
      wait: noWait,
    })
    await store.createAssignment(assignment('a1'))
    await store.upsertRegistration(registration('pylon.dual.1'))
    expect(pg.calls).toEqual([])
  })

  test('updateAssignmentIfState skips the mirror when the CAS missed', async () => {
    const d1 = makeFakeD1({
      updateAssignmentIfState: () => Promise.resolve(undefined),
    })
    const pg = makeFakePostgres()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'd1', writes: 'd1' },
      postgres: pg.store,
      wait: noWait,
    })
    expect(
      await store.updateAssignmentIfState(assignment('a1'), 'accepted'),
    ).toBeUndefined()
    expect(pg.calls).toEqual([])
  })
})

// ---------------------------------------------------------------------------
// Write cutover (#8515): Postgres is the SOLE write authority; D1 is dead
// ---------------------------------------------------------------------------

describe('writes: postgres authority', () => {
  test('every assignment/heartbeat/dispatch write hits Postgres, D1 untouched', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'postgres', writes: 'postgres' },
      postgres: pg.store,
      wait: noWait,
    })

    await store.createAssignment(assignment('a1'))
    await store.createEvent(event('e1'))
    await store.updateAssignment(assignment('a1'))
    await store.updateAssignmentIfState(assignment('a1'), 'offered')
    await store.upsertRegistration(registration('pylon.dual.1'))
    await store.upsertProviderJobLifecycle(lifecycle('a1'))
    await store.upsertQuarantine?.(quarantine('quarantine.public.pylon.dual.1'))
    await store.sweepStaleAssignmentLeases!(
      'pylon.dual.1',
      '2026-07-01T05:00:00.000Z',
      '2026-07-01T04:00:00.000Z',
    )

    // The dead D1 bridge is never touched on the write path.
    expect(d1.calls).toEqual([])
    // Authoritative Postgres write methods are used — NOT the mirror* methods.
    expect(pg.calls.map(c => c.method)).toEqual([
      'createAssignment',
      'createEvent',
      'updateAssignment',
      'updateAssignmentIfState',
      'upsertRegistration',
      'upsertProviderJobLifecycle',
      'upsertQuarantine',
      'sweepStaleAssignmentLeases',
    ])
    expect(pg.calls.some(c => c.method.startsWith('mirror'))).toBe(false)
  })

  test('a Postgres write error propagates (fail loud — no silent D1 fallback)', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres({
      createAssignment: () => Promise.reject(new Error('pg write down')),
    })
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'postgres', writes: 'postgres' },
      postgres: pg.store,
      wait: noWait,
    })

    await expect(store.createAssignment(assignment('a1'))).rejects.toThrow(
      'pg write down',
    )
    // D1 was NOT written as a fallback — a Postgres outage fails loud.
    expect(d1.calls).toEqual([])
  })

  test('createAssignment returns the Postgres store result verbatim', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres({
      createAssignment: () =>
        Promise.resolve({ idempotent: true, record: assignment('a-pg') }),
    })
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'postgres', writes: 'postgres' },
      postgres: pg.store,
      wait: noWait,
    })
    const result = await store.createAssignment(assignment('a1'))
    expect(result.idempotent).toBe(true)
    expect(result.record.assignmentRef).toBe('a-pg')
    expect(d1.calls).toEqual([])
  })
})

// ---------------------------------------------------------------------------
// Read routing
// ---------------------------------------------------------------------------

describe('read routing', () => {
  test('d1 mode: reads never touch Postgres', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'd1', writes: 'd1' },
      postgres: pg.store,
      wait: noWait,
    })
    await store.readRegistration('pylon.dual.1')
    await store.listAssignmentsForPylons!(['pylon.dual.1'], 10)
    expect(pg.calls).toEqual([])
    expect(d1.calls.map(c => c.method)).toEqual([
      'readRegistration',
      'listAssignmentsForPylons',
    ])
  })

  test('postgres mode: gate reads are served from Postgres, D1 untouched', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'postgres', writes: 'd1' },
      postgres: pg.store,
      wait: noWait,
    })
    await store.readRegistration('pylon.dual.1')
    await store.listRegistrationsForOwnerAgentUserIds!(['agent-user-1'], 100)
    await store.listAssignmentsForPylons!(['pylon.dual.1'], 10)
    expect(d1.calls).toEqual([])
    expect(pg.calls.map(c => c.method)).toEqual([
      'readRegistration',
      'listRegistrationsForOwnerAgentUserIds',
      'listAssignmentsForPylons',
    ])
  })

  test('postgres mode: bounded retry (3 attempts), then D1 fallback + diagnostics', async () => {
    let attempts = 0
    const d1 = makeFakeD1()
    const pg = makeFakePostgres({
      readRegistration: () => {
        attempts += 1
        return Promise.reject(new Error(`pg transient ${attempts}`))
      },
    })
    const sink = makeLogSink()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'postgres', writes: 'd1' },
      log: sink.log,
      postgres: pg.store,
      wait: noWait,
    })

    const result = await store.readRegistration('pylon.dual.1')
    expect(result?.pylonRef).toBe('pylon.dual.1')
    expect(attempts).toBe(3)
    expect(d1.calls.map(c => c.method)).toEqual(['readRegistration'])
    expect(sink.events.map(e => e.event)).toEqual([
      'khala_sync_pylon_postgres_read_failed',
      'khala_sync_pylon_postgres_read_failed',
      'khala_sync_pylon_postgres_read_fallback',
    ])
    expect(sink.events[2]?.fields.refs).toEqual(['pylon.dual.1'])
  })

  test('postgres mode: a transient failure recovers within the retry budget', async () => {
    let attempts = 0
    const pg = makeFakePostgres({
      readRegistration: () => {
        attempts += 1
        return attempts < 2
          ? Promise.reject(new Error('blip'))
          : Promise.resolve(registration('pylon.dual.1'))
      },
    })
    const d1 = makeFakeD1()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'postgres', writes: 'd1' },
      postgres: pg.store,
      wait: noWait,
    })
    const result = await store.readRegistration('pylon.dual.1')
    expect(result?.pylonRef).toBe('pylon.dual.1')
    expect(attempts).toBe(2)
    expect(d1.calls).toEqual([])
  })

  test('compare mode: serves D1, logs mismatches with refs', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres({
      readRegistration: () =>
        Promise.resolve({
          ...registration('pylon.dual.1'),
          displayName: 'Diverged Pylon',
        }),
    })
    const sink = makeLogSink()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'compare', writes: 'd1' },
      log: sink.log,
      postgres: pg.store,
      wait: noWait,
    })

    const result = await store.readRegistration('pylon.dual.1')
    expect(result?.displayName).toBe('Dual Pylon') // D1 authority served
    expect(sink.events).toHaveLength(1)
    expect(sink.events[0]?.event).toBe(
      'khala_sync_pylon_read_compare_mismatch',
    )
    expect(sink.events[0]?.fields.op).toBe('readRegistration')
    expect(sink.events[0]?.fields.refs).toEqual(['pylon.dual.1'])
  })

  test('compare mode: matching reads log nothing; Postgres errors log read_failed but never fail', async () => {
    const d1 = makeFakeD1()
    const matching = makeFakePostgres()
    const sink = makeLogSink()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'compare', writes: 'd1' },
      log: sink.log,
      postgres: matching.store,
      wait: noWait,
    })
    await store.readRegistration('pylon.dual.1')
    expect(sink.events).toEqual([])

    const broken = makeFakePostgres({
      readRegistration: () => Promise.reject(new Error('pg down')),
    })
    const sink2 = makeLogSink()
    const store2 = makeDualWritePylonApiStore({
      d1: makeFakeD1().store,
      flags: { dualWrite: true, reads: 'compare', writes: 'd1' },
      log: sink2.log,
      postgres: broken.store,
      wait: noWait,
    })
    const served = await store2.readRegistration('pylon.dual.1')
    expect(served?.pylonRef).toBe('pylon.dual.1')
    expect(sink2.events.map(e => e.event)).toEqual([
      'khala_sync_pylon_postgres_read_failed',
    ])
  })

  test('KS-8.4 reads stay D1-authoritative while writes mirror', async () => {
    const d1 = makeFakeD1()
    const pg = makeFakePostgres()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'postgres', writes: 'd1' },
      postgres: pg.store,
      wait: noWait,
    })
    await store.listProviderJobLifecycleForPylons(['pylon.dual.1'], 10)
    await store.upsertProviderJobLifecycle(lifecycle('a1'))
    await store.upsertQuarantine?.(quarantine('quarantine.public.pylon.dual.1'))
    expect(d1.calls.map(c => c.method)).toEqual([
      'listProviderJobLifecycleForPylons',
      'upsertProviderJobLifecycle',
      'upsertQuarantine',
    ])
    expect(pg.calls.map(c => c.method)).toEqual([
      'mirrorProviderJobLifecycle',
      'mirrorQuarantine',
    ])
  })

  test('no Postgres store (missing binding): the wrapper IS the D1 store', () => {
    const d1 = makeFakeD1()
    const store = makeDualWritePylonApiStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'postgres', writes: 'd1' },
      postgres: undefined,
      wait: noWait,
    })
    expect(store).toBe(d1.store)
  })
})

describe('Spark payout target dual-write', () => {
  test('upsert mirrors raw Spark targets without using Postgres for reads', async () => {
    const calls: Array<Call> = []
    const target = sparkTarget('pylon.dual.1')
    const d1: PylonSparkPayoutTargetStore = {
      read: async pylonRef => {
        calls.push({ args: [pylonRef], method: 'd1.read' })
        return target
      },
      readByOwner: async ownerAgentUserId => {
        calls.push({ args: [ownerAgentUserId], method: 'd1.readByOwner' })
        return target
      },
      upsert: async record => {
        calls.push({ args: [record], method: 'd1.upsert' })
        return record
      },
    }
    const pgCalls: Array<Call> = []
    const postgres: Pick<
      PostgresPylonDispatchStore,
      | 'mirrorSparkPayoutTarget'
      | 'upsertSparkPayoutTarget'
      | 'readSparkPayoutTarget'
      | 'readSparkPayoutTargetByOwner'
    > = {
      mirrorSparkPayoutTarget: async record => {
        pgCalls.push({ args: [record], method: 'mirrorSparkPayoutTarget' })
      },
      upsertSparkPayoutTarget: async record => {
        pgCalls.push({ args: [record], method: 'upsertSparkPayoutTarget' })
        return record
      },
      readSparkPayoutTarget: async () => undefined,
      readSparkPayoutTargetByOwner: async () => undefined,
    }

    const store = makeDualWritePylonSparkPayoutTargetStore({
      d1,
      flags: { dualWrite: true, writes: 'd1' },
      postgres,
    })

    await store.upsert(target)
    await store.read(target.pylonRef)
    await store.readByOwner(target.ownerAgentUserId)

    expect(calls.map(c => c.method)).toEqual([
      'd1.upsert',
      'd1.read',
      'd1.readByOwner',
    ])
    expect(pgCalls.map(c => c.method)).toEqual(['mirrorSparkPayoutTarget'])
  })

  test('Spark target mirror failure is fail-soft and logs no raw address ref', async () => {
    const target = sparkTarget('pylon.dual.1')
    const d1: PylonSparkPayoutTargetStore = {
      read: async () => target,
      readByOwner: async () => target,
      upsert: async record => record,
    }
    const sink = makeLogSink()
    const store = makeDualWritePylonSparkPayoutTargetStore({
      d1,
      flags: { dualWrite: true, writes: 'd1' },
      log: sink.log,
      postgres: {
        mirrorSparkPayoutTarget: () => Promise.reject(new Error('pg down')),
        upsertSparkPayoutTarget: async record => record,
        readSparkPayoutTarget: async () => undefined,
        readSparkPayoutTargetByOwner: async () => undefined,
      },
    })

    await expect(store.upsert(target)).resolves.toEqual(target)
    expect(sink.events).toHaveLength(1)
    expect(sink.events[0]?.fields.op).toBe('upsertSparkPayoutTarget')
    expect(sink.events[0]?.fields.refs).toEqual([
      target.pylonRef,
      target.payoutTargetRef,
    ])
    expect(sink.events[0]?.fields.refs).not.toContain(target.rawSparkAddress)
  })

  test('writes=postgres: Spark targets are Postgres-authoritative (no D1 touch)', async () => {
    // #8515 WRITE cutover: with the D1 bridge 401-dead, the Spark payout
    // target store must upsert AND read from Postgres only — a single D1 call
    // would hit the dead bridge.
    const target = sparkTarget('pylon.pg.1')
    const d1Calls: Array<Call> = []
    const d1: PylonSparkPayoutTargetStore = {
      read: async pylonRef => {
        d1Calls.push({ args: [pylonRef], method: 'd1.read' })
        return undefined
      },
      readByOwner: async ownerAgentUserId => {
        d1Calls.push({ args: [ownerAgentUserId], method: 'd1.readByOwner' })
        return undefined
      },
      upsert: async record => {
        d1Calls.push({ args: [record], method: 'd1.upsert' })
        return record
      },
    }
    const pgCalls: Array<Call> = []
    const postgres: Pick<
      PostgresPylonDispatchStore,
      | 'mirrorSparkPayoutTarget'
      | 'upsertSparkPayoutTarget'
      | 'readSparkPayoutTarget'
      | 'readSparkPayoutTargetByOwner'
    > = {
      mirrorSparkPayoutTarget: async () => {
        pgCalls.push({ args: [], method: 'mirrorSparkPayoutTarget' })
      },
      upsertSparkPayoutTarget: async record => {
        pgCalls.push({ args: [record], method: 'upsertSparkPayoutTarget' })
        return record
      },
      readSparkPayoutTarget: async pylonRef => {
        pgCalls.push({ args: [pylonRef], method: 'readSparkPayoutTarget' })
        return target
      },
      readSparkPayoutTargetByOwner: async ownerAgentUserId => {
        pgCalls.push({
          args: [ownerAgentUserId],
          method: 'readSparkPayoutTargetByOwner',
        })
        return target
      },
    }

    const store = makeDualWritePylonSparkPayoutTargetStore({
      d1,
      flags: { dualWrite: true, writes: 'postgres' },
      postgres,
    })

    await expect(store.upsert(target)).resolves.toEqual(target)
    await expect(store.read(target.pylonRef)).resolves.toEqual(target)
    await expect(
      store.readByOwner(target.ownerAgentUserId),
    ).resolves.toEqual(target)

    expect(d1Calls).toEqual([])
    expect(pgCalls.map(c => c.method)).toEqual([
      'upsertSparkPayoutTarget',
      'readSparkPayoutTarget',
      'readSparkPayoutTargetByOwner',
    ])
  })
})
