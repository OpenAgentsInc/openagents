import { MutationId, SyncCommand, SyncScope } from '@openagentsinc/sync-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  SyncOutboxStore,
  makeD1SyncOutboxRepository,
  makeD1SyncOutboxStore,
  personalWorkroomScope,
} from './index'
import { makeSyncOutboxStoreFixture } from './test-fixtures'

type StoredScope = Readonly<{
  created_at: string
  last_seq: number
  scope: string
  updated_at: string
}>

type StoredChange = Readonly<{
  actor_id: string | null
  collection: string
  created_at: string
  entity_id: string
  mutation_id: string | null
  op: 'put' | 'patch' | 'delete' | 'invalidate'
  patch_json: string | null
  scope: string
  seq: number
  value_json: string | null
}>

type StoredMutation = Readonly<{
  actor_id: string
  created_at: string
  mutation_id: string
  result_json: string | null
  scope: string
  status: string
}>

type MemoryD1State = Readonly<{
  changes: Array<StoredChange>
  mutations: Map<string, StoredMutation>
  scopes: Map<string, StoredScope>
}>

type MemoryD1Options = Readonly<{
  failSequenceAllocation?: boolean
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

const cloneForD1 = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const storedChangeOp = (value: unknown): StoredChange['op'] => {
  if (
    value === 'put' ||
    value === 'patch' ||
    value === 'delete' ||
    value === 'invalidate'
  ) {
    return value
  }

  throw new Error(`Invalid stored change operation: ${String(value)}`)
}

function rawD1Rows<T = unknown[]>(options: {
  columnNames: true
}): Promise<[Array<string>, ...Array<T>]>
function rawD1Rows<T = unknown[]>(options?: {
  columnNames?: false
}): Promise<Array<T>>
function rawD1Rows<T = unknown[]>(options?: {
  columnNames?: boolean
}): Promise<Array<T> | [Array<string>, ...Array<T>]> {
  if (options?.columnNames === true) {
    return Promise.resolve([[]])
  }

  return Promise.resolve([])
}

const makeStatement = (
  state: MemoryD1State,
  query: string,
  options: MemoryD1Options = {},
): D1PreparedStatement => {
  let values: ReadonlyArray<unknown> = []
  let statement: D1PreparedStatement

  statement = {
    all: async <T = Record<string, unknown>>() => {
      const [scope, cursor, limit] = values
      const filtered = state.changes
        .filter(change => change.scope === String(scope))
        .filter(change =>
          query.includes('seq >') ? change.seq > Number(cursor) : true,
        )
        .sort((left, right) => left.seq - right.seq)
        .slice(0, limit === undefined ? undefined : Number(limit))

      return makeResult(cloneForD1(filtered))
    },
    bind: (...nextValues: ReadonlyArray<unknown>) => {
      values = nextValues

      return statement
    },
    first: async <T = Record<string, unknown>>() => {
      if (query.includes('INSERT INTO sync_scopes')) {
        if (options.failSequenceAllocation === true) {
          return null
        }

        const [scope, createdAt, updatedAt] = values
        const key = String(scope)
        const previous = state.scopes.get(key)
        const next = {
          created_at: previous?.created_at ?? String(createdAt),
          last_seq: (previous?.last_seq ?? 0) + 1,
          scope: key,
          updated_at: String(updatedAt),
        }

        state.scopes.set(key, next)

        return cloneForD1<T>({ last_seq: next.last_seq })
      }

      if (query.includes('FROM sync_changes') && query.includes('seq =')) {
        const [scope, seq] = values
        const row = state.changes.find(
          change =>
            change.scope === String(scope) && change.seq === Number(seq),
        )

        return row === undefined ? null : cloneForD1<T>(row)
      }

      if (query.includes('FROM sync_mutations')) {
        const [mutationId] = values
        const row = state.mutations.get(String(mutationId))

        return row === undefined ? null : cloneForD1<T>(row)
      }

      return null
    },
    raw: rawD1Rows,
    run: async <T = Record<string, unknown>>() => {
      if (query.includes('INSERT INTO sync_changes')) {
        const [
          scope,
          seq,
          collection,
          op,
          entityId,
          valueJson,
          patchJson,
          mutationId,
          actorId,
          createdAt,
        ] = values

        state.changes.push({
          actor_id: actorId === null ? null : String(actorId),
          collection: String(collection),
          created_at: String(createdAt),
          entity_id: String(entityId),
          mutation_id: mutationId === null ? null : String(mutationId),
          op: storedChangeOp(op),
          patch_json: patchJson === null ? null : String(patchJson),
          scope: String(scope),
          seq: Number(seq),
          value_json: valueJson === null ? null : String(valueJson),
        })
      }

      if (query.includes('INSERT OR IGNORE INTO sync_mutations')) {
        const [mutationId, scope, actorId, resultJson, createdAt] = values
        const key = String(mutationId)

        if (!state.mutations.has(key)) {
          state.mutations.set(key, {
            actor_id: String(actorId),
            created_at: String(createdAt),
            mutation_id: key,
            result_json: String(resultJson),
            scope: String(scope),
            status: 'accepted',
          })
        }
      }

      if (query.includes('INSERT OR REPLACE INTO sync_mutations')) {
        const [mutationId, scope, actorId, resultJson, createdAt] = values
        const key = String(mutationId)

        state.mutations.set(key, {
          actor_id: String(actorId),
          created_at: String(createdAt),
          mutation_id: key,
          result_json: String(resultJson),
          scope: String(scope),
          status: 'rejected',
        })
      }

      return makeResult<T>()
    },
  } satisfies D1PreparedStatement

  return statement
}

const makeMemoryD1 = (
  options: MemoryD1Options = {},
): D1Database & MemoryD1State => {
  const state: MemoryD1State = {
    changes: [],
    mutations: new Map(),
    scopes: new Map(),
  }
  const database = {
    ...state,
    batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (query: string) => makeStatement(state, query, options),
    withSession: () =>
      ({
        batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(statement => statement.run<T>())),
        getBookmark: () => null,
        prepare: (query: string) => makeStatement(state, query, options),
      }) satisfies D1DatabaseSession,
  } satisfies D1Database & MemoryD1State

  return database
}

const runStore = <A>(effect: Effect.Effect<A, unknown>) =>
  Effect.runPromise(effect)

const testRuntime = (iso: string) => ({
  nowIso: () => iso,
})

const makeCommand = (scope: string, mutationId = 'mutation_1'): SyncCommand =>
  new SyncCommand({
    command: 'mission.create',
    mutationId: MutationId.make(mutationId),
    payload: { prompt: 'Run tests' },
    scope: SyncScope.make(scope),
  })

describe('SyncOutboxStore', () => {
  test('appends ordered patches and materializes snapshots', async () => {
    const db = makeMemoryD1()
    const store = makeD1SyncOutboxStore(
      db,
      testRuntime('2026-06-04T00:00:00.000Z'),
    )
    const scope = personalWorkroomScope('user_1')

    const first = await runStore(
      store.appendChange({
        collection: 'missions',
        id: 'run_1',
        op: 'put',
        scope,
        value: { status: 'queued', title: 'Run tests' },
      }),
    )
    const second = await runStore(
      store.appendChange({
        collection: 'missions',
        id: 'run_1',
        op: 'patch',
        patch: { status: 'active' },
        scope,
      }),
    )

    expect(first.seq).toBe(1)
    expect(second.seq).toBe(2)
    expect(db.changes.map(change => change.created_at)).toEqual([
      '2026-06-04T00:00:00.000Z',
      '2026-06-04T00:00:00.000Z',
    ])

    await expect(
      runStore(store.readChangesAfter(scope, 0)),
    ).resolves.toHaveLength(2)
    await expect(runStore(store.readSnapshot(scope))).resolves.toMatchObject({
      collections: {
        missions: {
          run_1: {
            status: 'active',
            title: 'Run tests',
          },
        },
      },
      cursor: 2,
      scope,
    })
  })

  test('records accepted and rejected mutations idempotently', async () => {
    const db = makeMemoryD1()
    const store = makeD1SyncOutboxStore(
      db,
      testRuntime('2026-06-04T00:01:00.000Z'),
    )
    const scope = personalWorkroomScope('user_1')
    const command = makeCommand(scope)

    await runStore(store.acceptMutation(command, 'user_1'))
    await runStore(store.acceptMutation(command, 'user_1'))

    expect(db.mutations).toHaveLength(1)

    await expect(
      runStore(store.rejectMutation(command, 'user_1', 'unsupported command')),
    ).rejects.toMatchObject({
      _tag: 'SyncMutationAlreadyAccepted',
      mutationId: 'mutation_1',
    })

    const rejectedCommand = makeCommand(scope, 'mutation_2')
    const rejection = await runStore(
      store.rejectMutation(rejectedCommand, 'user_1', 'unsupported command'),
    )

    expect(rejection.reason).toBe('unsupported command')
    expect(db.mutations.get('mutation_2')).toMatchObject({
      created_at: '2026-06-04T00:01:00.000Z',
      status: 'rejected',
    })
  })

  test('fails with typed scope mismatch before accepting a mutation', async () => {
    const store = makeD1SyncOutboxStore(makeMemoryD1())
    const scope = personalWorkroomScope('user_1')
    const command = makeCommand(scope)

    await expect(
      runStore(
        store.acceptMutationForScope(
          personalWorkroomScope('user_2'),
          command,
          'user_1',
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'SyncScopeMismatch',
      actualScope: scope,
      expectedScope: personalWorkroomScope('user_2'),
    })
  })

  test('fails with typed decode errors for malformed stored payloads', async () => {
    const db = makeMemoryD1()
    const store = makeD1SyncOutboxStore(db)
    const scope = personalWorkroomScope('user_1')

    db.changes.push({
      actor_id: null,
      collection: 'missions',
      created_at: '2026-06-04T00:02:00.000Z',
      entity_id: 'run_1',
      mutation_id: null,
      op: 'put',
      patch_json: null,
      scope,
      seq: 1,
      value_json: '{bad json',
    })

    await expect(runStore(store.readSnapshot(scope))).rejects.toMatchObject({
      _tag: 'SyncPayloadDecodeError',
      field: 'sync_changes.value_json',
    })
  })

  test('fails with typed sequence allocation errors', async () => {
    const db = makeMemoryD1({ failSequenceAllocation: true })
    const store = makeD1SyncOutboxStore(db)
    const scope = personalWorkroomScope('user_1')

    await expect(
      runStore(
        store.appendChange({
          collection: 'missions',
          id: 'run_1',
          op: 'put',
          scope,
          value: { status: 'queued' },
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'SyncSequenceAllocationFailed',
      scope,
    })
  })

  test('provides the store from an effect-cf D1 service effect', async () => {
    const db = makeMemoryD1()
    const scope = personalWorkroomScope('user_1')
    const storeLayer = SyncOutboxStore.effectCfLayer(
      Effect.succeed(db),
      testRuntime('2026-06-04T00:03:00.000Z'),
    )

    const snapshot = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* SyncOutboxStore

        yield* store.appendChange({
          collection: 'missions',
          id: 'run_1',
          op: 'put',
          scope,
          value: { status: 'queued' },
        })

        return yield* store.readSnapshot(scope)
      }).pipe(Effect.provide(storeLayer)),
    )

    expect(snapshot.collections).toMatchObject({
      missions: {
        run_1: {
          status: 'queued',
        },
      },
    })
  })

  test('provides the store from the reusable sync fixture layer', async () => {
    const fixture = makeSyncOutboxStoreFixture()
    const scope = personalWorkroomScope('user_1')

    const snapshot = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* SyncOutboxStore

        return yield* store.readSnapshot(scope)
      }).pipe(Effect.provide(fixture.layer)),
    )

    expect(snapshot.scope).toBe(scope)
    expect(snapshot.cursor).toBe(0)
    expect(fixture.db.changes).toEqual([])
  })

  test('reports missing change and required snapshot states', async () => {
    const store = makeD1SyncOutboxStore(makeMemoryD1())
    const scope = personalWorkroomScope('user_1')

    await expect(runStore(store.readChange(scope, 7))).rejects.toMatchObject({
      _tag: 'SyncChangeMissing',
      scope,
      seq: 7,
    })
    await expect(
      runStore(store.readRequiredSnapshot(scope)),
    ).rejects.toMatchObject({
      _tag: 'SyncSnapshotMissing',
      scope,
    })
  })
})

describe('D1 sync outbox repository compatibility facade', () => {
  test('keeps current Promise callers working', async () => {
    const db = makeMemoryD1()
    const repository = makeD1SyncOutboxRepository(
      db,
      testRuntime('2026-06-04T00:04:00.000Z'),
    )
    const scope = personalWorkroomScope('user_1')

    await repository.appendChange({
      collection: 'missions',
      id: 'run_1',
      op: 'put',
      scope,
      value: { status: 'queued' },
    })

    await expect(repository.readSnapshot(scope)).resolves.toMatchObject({
      collections: {
        missions: {
          run_1: {
            status: 'queued',
          },
        },
      },
      cursor: 1,
      scope,
    })
  })
})
