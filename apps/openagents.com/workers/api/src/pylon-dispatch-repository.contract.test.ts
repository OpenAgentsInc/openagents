// KS-8.1 (#8307): pylon dispatch repository CONTRACT suite.
//
// One behavioral spec, TWO implementations:
//   - D1: `makeD1PylonApiStore` over real SQLite (node:sqlite — the engine
//     D1 is built on), schema from the worker migrations.
//   - Postgres: `makePostgresPylonDispatchStore` over a throwaway local
//     Postgres (initdb/pg_ctl), schema from khala-sync-server migration
//     0005. Skipped when no local Postgres binaries exist
//     (`hasLocalPostgres()` — same gating as the khala-sync-server suites).
//
// Every case runs identically against both stores: proving behavioral
// equivalence on the covered operations is what licenses the read cutover.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  makeD1PylonApiStore,
  PylonApiStoreError,
  type PylonApiAssignmentRecord,
  type PylonApiAssignmentState,
  type PylonApiEventRecord,
  type PylonApiRegistrationRecord,
  type PylonApiStore,
} from './pylon-api'
import { makePostgresPylonDispatchStore } from './pylon-dispatch-store'
import { makeSqliteD1, PYLON_DISPATCH_D1_SCHEMA } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

let refCounter = 0
const nextRef = (prefix: string) => `${prefix}.contract.${++refCounter}`

const registrationRecord = (
  overrides: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord => {
  const pylonRef = overrides.pylonRef ?? nextRef('pylon')
  return {
    capabilityRefs: ['capability.codex_worker.v1'],
    clientProtocolVersion: '1',
    clientVersion: '0.2.0',
    createdAt: '2026-07-01T00:00:00.000Z',
    displayName: 'Contract Pylon',
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
    ...overrides,
  }
}

const assignmentRecord = (
  pylonRef: string,
  overrides: Partial<PylonApiAssignmentRecord> = {},
): PylonApiAssignmentRecord => {
  const assignmentRef = overrides.assignmentRef ?? nextRef('assignment')
  return {
    acceptanceCriteriaRefs: ['criteria.contract'],
    acceptedWorkRefs: [],
    artifactRefs: [],
    assignmentRef,
    closeoutRefs: [],
    codingAssignment: null,
    createdAt: '2026-07-01T01:00:00.000Z',
    id: `assignment_${assignmentRef}`,
    idempotencyKeyHash: `hash-${assignmentRef}`,
    jobKind: 'codex_agent_task',
    leaseExpiresAt: '2026-07-01T02:00:00.000Z',
    ownerAgentUserId: 'agent-user-1',
    paymentMode: 'unpaid_smoke',
    proofRefs: [],
    publicProjectionJson: '{}',
    pylonRef,
    rejectionRefs: [],
    resultExpectationRefs: [],
    state: 'offered',
    taskRefs: ['task.contract'],
    updatedAt: '2026-07-01T01:00:00.000Z',
    ...overrides,
  }
}

const eventRecord = (
  pylonRef: string,
  overrides: Partial<PylonApiEventRecord> = {},
): PylonApiEventRecord => {
  const eventRef = overrides.eventRef ?? nextRef('event')
  return {
    assignmentRef: null,
    createdAt: '2026-07-01T01:10:00.000Z',
    eventBody: { note: 'contract' },
    eventKind: 'assignment_progress',
    eventRef,
    id: `event_${eventRef}`,
    idempotencyKeyHash: `hash-${eventRef}`,
    ownerAgentUserId: 'agent-user-1',
    publicProjectionJson: '{}',
    pylonRef,
    status: 'running',
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// The shared behavioral spec
// ---------------------------------------------------------------------------

type ContractStore = Pick<
  PylonApiStore,
  | 'createAssignment'
  | 'createEvent'
  | 'listAssignmentsForPylon'
  | 'listEventsForPylon'
  | 'listEventsForAssignment'
  | 'listProviderJobLifecycleForPylons'
  | 'listRegistrations'
  | 'readAssignment'
  | 'readAssignmentByIdempotencyKeyHash'
  | 'readEventByIdempotencyKeyHash'
  | 'readRegistration'
  | 'updateAssignment'
  | 'updateAssignmentIfState'
  | 'upsertRegistration'
> &
  Readonly<{
    listAssignmentsForPylons: NonNullable<
      PylonApiStore['listAssignmentsForPylons']
    >
    listRegistrationsForOwnerAgentUserIds: NonNullable<
      PylonApiStore['listRegistrationsForOwnerAgentUserIds']
    >
    sweepStaleAssignmentLeases: NonNullable<
      PylonApiStore['sweepStaleAssignmentLeases']
    >
  }>

const specContractSuite = (getStore: () => ContractStore) => {
  const withRegistration = async () => {
    const store = getStore()
    const registration = registrationRecord()
    await store.upsertRegistration(registration)
    return { registration, store }
  }

  test('createAssignment inserts once; the idempotency key replays the FIRST record', async () => {
    const { registration, store } = await withRegistration()
    const record = assignmentRecord(registration.pylonRef)

    const first = await store.createAssignment(record)
    expect(first.idempotent).toBe(false)
    expect(first.record.assignmentRef).toBe(record.assignmentRef)

    // Same idempotency key, different payload — the stored record wins.
    const replay = await store.createAssignment(
      assignmentRecord(registration.pylonRef, {
        idempotencyKeyHash: record.idempotencyKeyHash,
        state: 'running',
      }),
    )
    expect(replay.idempotent).toBe(true)
    expect(replay.record.assignmentRef).toBe(record.assignmentRef)
    expect(replay.record.state).toBe('offered')

    const readBack = await store.readAssignment(record.assignmentRef)
    expect(readBack).toEqual(record)
    expect(
      await store.readAssignmentByIdempotencyKeyHash(record.idempotencyKeyHash),
    ).toEqual(record)
  })

  test('createEvent is idempotent on the key hash and returns the stored event', async () => {
    const { registration, store } = await withRegistration()
    const record = eventRecord(registration.pylonRef)

    const first = await store.createEvent(record)
    expect(first.idempotent).toBe(false)
    expect(first.record).toEqual(record)

    const replay = await store.createEvent(
      eventRecord(registration.pylonRef, {
        idempotencyKeyHash: record.idempotencyKeyHash,
        status: 'different',
      }),
    )
    expect(replay.idempotent).toBe(true)
    expect(replay.record).toEqual(record)

    expect(
      await store.readEventByIdempotencyKeyHash(record.idempotencyKeyHash),
    ).toEqual(record)
  })

  test('updateAssignment persists lifecycle columns and recomputes the projection', async () => {
    const { registration, store } = await withRegistration()
    const record = assignmentRecord(registration.pylonRef)
    await store.createAssignment(record)

    const next = await store.updateAssignment({
      ...record,
      artifactRefs: ['artifact.contract.1'],
      state: 'running',
      updatedAt: '2026-07-01T01:30:00.000Z',
    })
    expect(next.state).toBe('running')
    expect(next.publicProjectionJson).not.toBe(record.publicProjectionJson)

    const readBack = await store.readAssignment(record.assignmentRef)
    expect(readBack).toEqual(next)
  })

  test('assignment writes keep the provider job lifecycle twin current', async () => {
    const { registration, store } = await withRegistration()
    const record = assignmentRecord(registration.pylonRef)
    await store.createAssignment(record)

    const offered = await store.listProviderJobLifecycleForPylons(
      [registration.pylonRef],
      10,
    )
    expect(offered).toHaveLength(1)
    expect(offered[0]?.assignmentRef).toBe(record.assignmentRef)
    expect(offered[0]?.stage).toBe('offered')

    await store.updateAssignment({
      ...record,
      artifactRefs: ['artifact.contract.1'],
      state: 'running',
      updatedAt: '2026-07-01T01:30:00.000Z',
    })

    const updated = await store.listProviderJobLifecycleForPylons(
      [registration.pylonRef],
      10,
    )
    expect(updated).toHaveLength(1)
    expect(updated[0]?.assignmentRef).toBe(record.assignmentRef)
    expect(updated[0]?.artifactRefs).toEqual(['artifact.contract.1'])
    expect(updated[0]?.stage).toBe('artifact_submitted')
  })

  test('updateAssignmentIfState is a compare-and-set on state', async () => {
    const { registration, store } = await withRegistration()
    const record = assignmentRecord(registration.pylonRef)
    await store.createAssignment(record)

    const miss = await store.updateAssignmentIfState(
      { ...record, state: 'running', updatedAt: '2026-07-01T01:31:00.000Z' },
      'accepted',
    )
    expect(miss).toBeUndefined()
    expect((await store.readAssignment(record.assignmentRef))?.state).toBe(
      'offered',
    )

    const hit = await store.updateAssignmentIfState(
      { ...record, state: 'accepted', updatedAt: '2026-07-01T01:32:00.000Z' },
      'offered',
    )
    expect(hit?.state).toBe('accepted')
    expect(await store.readAssignment(record.assignmentRef)).toEqual(hit)
  })

  test('upsertRegistration: insert, update preserving id/createdAt, owner conflict, owner transfer', async () => {
    const store = getStore()
    const registration = registrationRecord()

    const inserted = await store.upsertRegistration(registration)
    expect(inserted).toEqual(registration)

    const updated = await store.upsertRegistration({
      ...registration,
      displayName: 'Renamed Pylon',
      id: 'ignored-new-id',
      updatedAt: '2026-07-01T00:10:00.000Z',
      walletReady: true,
    })
    expect(updated.id).toBe(registration.id)
    expect(updated.createdAt).toBe(registration.createdAt)
    expect(updated.displayName).toBe('Renamed Pylon')
    expect(updated.walletReady).toBe(true)
    expect(await store.readRegistration(registration.pylonRef)).toEqual(updated)

    // Foreign owner without transfer approval: typed conflict.
    await expect(
      store.upsertRegistration({
        ...registration,
        ownerAgentUserId: 'agent-user-intruder',
      }),
    ).rejects.toMatchObject({ kind: 'conflict' })
    await expect(
      store.upsertRegistration({
        ...registration,
        ownerAgentUserId: 'agent-user-intruder',
      }),
    ).rejects.toBeInstanceOf(PylonApiStoreError)

    // Sanctioned transfer.
    const transferred = await store.upsertRegistration(
      {
        ...registration,
        ownerAgentUserId: 'agent-user-2',
        updatedAt: '2026-07-01T00:20:00.000Z',
      },
      { allowOwnerTransferFrom: registration.ownerAgentUserId },
    )
    expect(transferred.ownerAgentUserId).toBe('agent-user-2')
    expect(
      (await store.readRegistration(registration.pylonRef))?.ownerAgentUserId,
    ).toBe('agent-user-2')
  })

  test('listAssignmentsForPylon(s): active-lease states only, newest updated first, limited', async () => {
    const { registration, store } = await withRegistration()
    const otherRegistration = registrationRecord()
    await store.upsertRegistration(otherRegistration)

    const running = assignmentRecord(registration.pylonRef, {
      state: 'running',
      updatedAt: '2026-07-01T01:03:00.000Z',
    })
    const offered = assignmentRecord(registration.pylonRef, {
      state: 'offered',
      updatedAt: '2026-07-01T01:02:00.000Z',
    })
    const terminal = assignmentRecord(registration.pylonRef, {
      state: 'accepted_work',
      updatedAt: '2026-07-01T01:04:00.000Z',
    })
    const foreign = assignmentRecord(otherRegistration.pylonRef, {
      state: 'accepted',
      updatedAt: '2026-07-01T01:05:00.000Z',
    })
    for (const record of [running, offered, terminal, foreign]) {
      await store.createAssignment(record)
    }

    const single = await store.listAssignmentsForPylon(
      registration.pylonRef,
      10,
    )
    expect(single.map(a => a.assignmentRef)).toEqual([
      running.assignmentRef,
      offered.assignmentRef,
    ])

    const limited = await store.listAssignmentsForPylon(
      registration.pylonRef,
      1,
    )
    expect(limited.map(a => a.assignmentRef)).toEqual([running.assignmentRef])

    const merged = await store.listAssignmentsForPylons(
      [registration.pylonRef, otherRegistration.pylonRef],
      10,
    )
    expect(merged.map(a => a.assignmentRef)).toEqual([
      foreign.assignmentRef,
      running.assignmentRef,
      offered.assignmentRef,
    ])

    expect(await store.listAssignmentsForPylons([], 10)).toEqual([])
  })

  test('listEventsForPylon / listEventsForAssignment: newest first, filtered', async () => {
    const { registration, store } = await withRegistration()
    const assignment = assignmentRecord(registration.pylonRef)
    await store.createAssignment(assignment)

    const early = eventRecord(registration.pylonRef, {
      assignmentRef: assignment.assignmentRef,
      createdAt: '2026-07-01T01:11:00.000Z',
    })
    const late = eventRecord(registration.pylonRef, {
      assignmentRef: assignment.assignmentRef,
      createdAt: '2026-07-01T01:12:00.000Z',
    })
    const unrelated = eventRecord(registration.pylonRef, {
      createdAt: '2026-07-01T01:13:00.000Z',
    })
    for (const record of [early, late, unrelated]) {
      await store.createEvent(record)
    }

    expect(
      (await store.listEventsForPylon(registration.pylonRef, 10)).map(
        e => e.eventRef,
      ),
    ).toEqual([unrelated.eventRef, late.eventRef, early.eventRef])

    expect(
      (
        await store.listEventsForAssignment(assignment.assignmentRef, 10)
      ).map(e => e.eventRef),
    ).toEqual([late.eventRef, early.eventRef])
  })

  test('listRegistrations + listRegistrationsForOwnerAgentUserIds (the dispatch-gate owner read)', async () => {
    const store = getStore()
    const mine = registrationRecord({
      ownerAgentUserId: 'gate-owner-a',
      updatedAt: '2026-07-01T00:02:00.000Z',
    })
    const alsoMine = registrationRecord({
      ownerAgentUserId: 'gate-owner-b',
      updatedAt: '2026-07-01T00:03:00.000Z',
    })
    const foreign = registrationRecord({
      ownerAgentUserId: 'gate-owner-foreign',
      updatedAt: '2026-07-01T00:04:00.000Z',
    })
    for (const record of [mine, alsoMine, foreign]) {
      await store.upsertRegistration(record)
    }

    const all = await store.listRegistrations(100)
    const refs = all.map(r => r.pylonRef)
    expect(refs).toContain(mine.pylonRef)
    expect(refs).toContain(foreign.pylonRef)

    const owned = await store.listRegistrationsForOwnerAgentUserIds(
      ['gate-owner-a', 'gate-owner-b'],
      100,
    )
    expect(owned.map(r => r.pylonRef)).toEqual([
      alsoMine.pylonRef,
      mine.pylonRef,
    ])

    expect(
      await store.listRegistrationsForOwnerAgentUserIds([], 100),
    ).toEqual([])
  })

  test('sweepStaleAssignmentLeases flips stuck active leases to stale and reports refs', async () => {
    const { registration, store } = await withRegistration()
    const nowIso = '2026-07-01T05:00:00.000Z'
    const staleBeforeIso = '2026-07-01T04:00:00.000Z'

    const stuck = assignmentRecord(registration.pylonRef, {
      leaseExpiresAt: '2026-07-01T06:00:00.000Z',
      state: 'running',
      updatedAt: '2026-07-01T03:00:00.000Z',
    })
    const fresh = assignmentRecord(registration.pylonRef, {
      leaseExpiresAt: '2026-07-01T06:00:00.000Z',
      state: 'running',
      updatedAt: '2026-07-01T04:30:00.000Z',
    })
    const expired = assignmentRecord(registration.pylonRef, {
      leaseExpiresAt: '2026-07-01T04:15:00.000Z',
      state: 'running',
      updatedAt: '2026-07-01T03:00:00.000Z',
    })
    const terminal = assignmentRecord(registration.pylonRef, {
      leaseExpiresAt: '2026-07-01T06:00:00.000Z',
      state: 'accepted_work',
      updatedAt: '2026-07-01T03:00:00.000Z',
    })
    for (const record of [stuck, fresh, expired, terminal]) {
      await store.createAssignment(record)
    }

    const swept = await store.sweepStaleAssignmentLeases(
      registration.pylonRef,
      nowIso,
      staleBeforeIso,
    )
    expect(swept).toEqual([stuck.assignmentRef])

    const states = new Map<string, PylonApiAssignmentState>()
    for (const record of [stuck, fresh, expired, terminal]) {
      const read = await store.readAssignment(record.assignmentRef)
      states.set(record.assignmentRef, read!.state)
    }
    expect(states.get(stuck.assignmentRef)).toBe('stale')
    expect(states.get(fresh.assignmentRef)).toBe('running')
    expect(states.get(expired.assignmentRef)).toBe('running')
    expect(states.get(terminal.assignmentRef)).toBe('accepted_work')

    const sweptRecord = await store.readAssignment(stuck.assignmentRef)
    expect(sweptRecord?.leaseExpiresAt).toBe(nowIso)
    expect(sweptRecord?.updatedAt).toBe(nowIso)
  })
}

// ---------------------------------------------------------------------------
// D1 implementation (real SQLite)
// ---------------------------------------------------------------------------

describe('pylon dispatch repository contract — D1 (SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1>
  let store: PylonApiStore

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(PYLON_DISPATCH_D1_SCHEMA)
    store = makeD1PylonApiStore(sqlite.db)
  })

  afterAll(() => {
    sqlite.close()
  })

  specContractSuite(() => store as unknown as ContractStore)
})

// ---------------------------------------------------------------------------
// Postgres implementation (throwaway local instance)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  'pylon dispatch repository contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let rawSql:
      | { end: (options?: { timeout?: number }) => Promise<void> }
      | undefined
    let store: ContractStore

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE pylon_dispatch_contract')
      await admin.end({ timeout: 5 })

      const url = pg.urlFor('pylon_dispatch_contract')
      const client = postgres(url, { max: 4, prepare: false })
      rawSql = client as typeof rawSql
      const migrationSql = readFileSync(
        path.resolve(
          import.meta.dirname,
          '../../../../../packages/khala-sync-server/migrations/0005_pylon_dispatch.sql',
        ),
        'utf8',
      )
      await client.unsafe(migrationSql)
      const controlPlaneMigrationSql = readFileSync(
        path.resolve(
          import.meta.dirname,
          '../../../../../packages/khala-sync-server/migrations/0009_pylon_control_plane_remainder.sql',
        ),
        'utf8',
      )
      await client.unsafe(controlPlaneMigrationSql)

      store = makePostgresPylonDispatchStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: client as never,
          }),
      })
    }, 120_000)

    afterAll(async () => {
      await rawSql?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    specContractSuite(() => store)
  },
)
