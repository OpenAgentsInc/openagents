import { describe, expect, test } from 'vitest'

import {
  GYM_RUN_PROGRESS_SYNC_COLLECTION,
  publishGymRunProgressSnapshot,
} from './run-progress-sync'
import { buildGymRunProgress, type GymRunProgress } from './run-progress'

// Minimal in-memory D1 + sync room fixture.
// that records the rows appended to the outbox and the scopes poked, so the
// publisher's upsert-by-runRef, public-safe, and fail-soft behavior is exercised
// against the REAL outbox append path.

type StoredChange = Readonly<{
  collection: string
  entity_id: string
  op: string
  scope: string
  seq: number
  value_json: string | null
}>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: true,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 0,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

type MemoryD1 = D1Database & Readonly<{ changes: Array<StoredChange> }>

const makeStatement = (
  state: Pick<MemoryD1, 'changes'> & { lastSeq: number },
  query: string,
): D1PreparedStatement => {
  let values: ReadonlyArray<unknown> = []
  let statement: D1PreparedStatement

  statement = {
    all: async <T = Record<string, unknown>>() => makeResult<T>(),
    bind: (...nextValues: ReadonlyArray<unknown>) => {
      values = nextValues

      return statement
    },
    first: async <T = Record<string, unknown>>() => {
      if (query.includes('INSERT INTO sync_scopes')) {
        state.lastSeq = state.lastSeq + 1

        return JSON.parse(JSON.stringify({ last_seq: state.lastSeq })) as T
      }

      return null
    },
    raw: async () => [] as never,
    run: async <T = Record<string, unknown>>() => {
      if (query.includes('INSERT INTO sync_changes')) {
        const [scope, seq, collection, op, entityId, valueJson] = values

        state.changes.push({
          collection: String(collection),
          entity_id: String(entityId),
          op: String(op),
          scope: String(scope),
          seq: Number(seq),
          value_json: valueJson === null ? null : String(valueJson),
        })
      }

      return makeResult<T>()
    },
  } satisfies D1PreparedStatement

  return statement
}

const makeMemoryD1 = (): MemoryD1 => {
  const state = {
    changes: [] as Array<StoredChange>,
    lastSeq: 0,
  }

  return {
    changes: state.changes,
    batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (query: string) => makeStatement(state, query),
    withSession: () =>
      ({
        batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(statement => statement.run<T>())),
        getBookmark: () => null,
        prepare: (query: string) => makeStatement(state, query),
      }) satisfies D1DatabaseSession,
  } as unknown as MemoryD1
}

const makeSyncRoom = (notifiedScopes: Array<string>): DurableObjectNamespace =>
  ({
    getByName: (scope: string) => ({
      fetch: async (request: Request) => {
        notifiedScopes.push(
          request.headers.get('x-openagents-sync-scope') ?? scope,
        )

        return new Response(null, { status: 204 })
      },
    }),
    idFromName: (scope: string) => scope,
    get: (scope: string) => ({
      fetch: async (request: Request) => {
        notifiedScopes.push(
          request.headers.get('x-openagents-sync-scope') ?? scope,
        )

        return new Response(null, { status: 204 })
      },
    }),
  }) as never

const webAuthorized = (
  runRef: string,
  overrides: Partial<Parameters<typeof buildGymRunProgress>[0]> = {},
): GymRunProgress =>
  buildGymRunProgress({
    runRef,
    jobRef: `job.${runRef}`,
    configId: `config.${runRef}`,
    profileRef: 'khala-public-heuristic',
    agent: 'opencode',
    phase: 'running',
    publication: 'web_authorized',
    officialDenominator: 89,
    completedPassed: 13,
    completedFailed: 0,
    running: 2,
    pending: 74,
    error: 0,
    cancelled: 0,
    promptTokens: null,
    completionTokens: null,
    elapsedMs: 540_000,
    lastUpdatedAt: '2026-06-25T00:00:00.000Z',
    caveatRefs: [],
    blockerRefs: [],
    ...overrides,
  })

const localOnly = (runRef: string): GymRunProgress =>
  webAuthorized(runRef, { publication: 'local_only' })

describe('publishGymRunProgressSnapshot', () => {
  test('appends one put keyed by runRef onto the public scope and pokes it', async () => {
    const db = makeMemoryD1()
    const notifiedScopes: Array<string> = []
    const env = {
      OPENAGENTS_DB: db,
      SYNC_ROOM: makeSyncRoom(notifiedScopes),
    } as never

    await publishGymRunProgressSnapshot(env, webAuthorized('run.web.alpha'))

    expect(db.changes).toHaveLength(1)
    const change = db.changes[0]!
    expect(change.scope).toBe('public-gym-run-progress:network')
    expect(change.collection).toBe(GYM_RUN_PROGRESS_SYNC_COLLECTION)
    // Upsert-by-runRef: the put is keyed by the run's ref so the snapshot
    // collapses re-ingests of the same run.
    expect(change.entity_id).toBe('run.web.alpha')
    expect(change.op).toBe('put')

    const value = JSON.parse(change.value_json ?? 'null')
    expect(value.runRef).toBe('run.web.alpha')
    expect(value.publication).toBe('web_authorized')
    // Public-safe projection only: counts, never raw runner material.
    expect(value.counts.completedPassed).toBe(13)

    // The scope was poked exactly once so subscribers wake immediately.
    expect(notifiedScopes).toStrictEqual(['public-gym-run-progress:network'])
  })

  test('re-ingesting the same runRef reuses the same entity id (upsert, no dup)', async () => {
    const db = makeMemoryD1()
    const env = {
      OPENAGENTS_DB: db,
      SYNC_ROOM: makeSyncRoom([]),
    } as never

    await publishGymRunProgressSnapshot(env, webAuthorized('run.web.alpha'))
    await publishGymRunProgressSnapshot(
      env,
      webAuthorized('run.web.alpha', { completedPassed: 20, pending: 67 }),
    )

    // Two appends, BOTH keyed by the same entity id so the outbox snapshot
    // collapses them to the latest projection for that run.
    expect(db.changes).toHaveLength(2)
    expect(db.changes.every(change => change.entity_id === 'run.web.alpha')).toBe(
      true,
    )
  })

  test('publishes the degraded local_only projection (no live counts)', async () => {
    const db = makeMemoryD1()
    const env = {
      OPENAGENTS_DB: db,
      SYNC_ROOM: makeSyncRoom([]),
    } as never

    await publishGymRunProgressSnapshot(env, localOnly('run.local.beta'))

    expect(db.changes).toHaveLength(1)
    const value = JSON.parse(db.changes[0]!.value_json ?? 'null')
    expect(value.runRef).toBe('run.local.beta')
    expect(value.publication).toBe('local_only')
    // The degraded projection carries NO live numbers — just the honest marker.
    expect(value.counts).toBeUndefined()
    expect(value.blockerRefs).toContain(
      'blocker.gym.run_progress.not_authorized_for_web_publication',
    )
  })

  test('is fail-soft: a poke failure never breaks the ingest path', async () => {
    const db = makeMemoryD1()
    const failingRoom = {
      getByName: () => ({
        fetch: async () => {
          throw new Error('sync room unavailable')
        },
      }),
      idFromName: (scope: string) => scope,
      get: () => ({
        fetch: async () => {
          throw new Error('sync room unavailable')
        },
      }),
    } as never
    const env = { OPENAGENTS_DB: db, SYNC_ROOM: failingRoom } as never

    // The production ingest path passes the execution context, so the poke runs
    // DETACHED via `ctx.waitUntil` and a poke failure can never reject the publish
    // promise that the ingest awaits. The change is still appended first.
    const detached: Array<Promise<unknown>> = []
    const ctx = { waitUntil: (promise: Promise<unknown>) => detached.push(promise) }

    await expect(
      publishGymRunProgressSnapshot(env, webAuthorized('run.web.gamma'), {
        ctx,
      }),
    ).resolves.toBeUndefined()
    expect(db.changes).toHaveLength(1)
    // Each sync scope's Durable Object notify fetch is isolated (#8282
    // Promise.all landmine audit follow-up): a failing scope's fetch is
    // caught and logged internally by `notifySyncScopesPromise`, so the
    // detached poke promise itself now resolves rather than rejecting —
    // strictly more fail-soft than a rejecting `ctx.waitUntil` handle, which
    // a real Workers runtime would otherwise surface as an unhandled
    // rejection.
    await expect(Promise.all(detached)).resolves.toEqual(
      detached.map(() => undefined),
    )
  })
})
