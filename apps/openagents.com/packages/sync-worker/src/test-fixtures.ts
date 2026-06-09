import { Layer } from 'effect'

import {
  SyncOutboxStore,
  type SyncWorkerRuntime,
  makeD1SyncOutboxStore,
} from './index'

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

export type MemorySyncD1State = Readonly<{
  changes: Array<StoredChange>
  mutations: Map<string, StoredMutation>
  scopes: Map<string, StoredScope>
}>

export type MemorySyncD1 = D1Database & MemorySyncD1State

export type MemorySyncD1Options = Readonly<{
  failSequenceAllocation?: boolean
}>

export const syncWorkerTestRuntime = (
  iso = '2026-06-04T00:01:00.000Z',
): SyncWorkerRuntime => ({
  nowIso: () => iso,
})

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

const cloneForD1 = <T>(value: unknown): T =>
  // The D1 fake mirrors Cloudflare's caller-selected row generic.
  // eslint-disable-next-line @typescript-eslint/consistent-type-assertions
  structuredClone(value) as T

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

function rawD1Rows<T = Array<unknown>>(options: {
  columnNames: true
}): Promise<[Array<string>, ...Array<T>]>
function rawD1Rows<T = Array<unknown>>(options?: {
  columnNames?: false
}): Promise<Array<T>>
function rawD1Rows<T = Array<unknown>>(options?: {
  columnNames?: boolean
}): Promise<Array<T> | [Array<string>, ...Array<T>]> {
  if (options?.columnNames === true) {
    return Promise.resolve([[]])
  }

  return Promise.resolve([])
}

const makeStatement = (
  state: MemorySyncD1State,
  query: string,
  options: MemorySyncD1Options = {},
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

      return makeResult<T>(cloneForD1<Array<T>>(filtered))
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

export const makeMemorySyncD1 = (
  options: MemorySyncD1Options = {},
): MemorySyncD1 => {
  const state: MemorySyncD1State = {
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
  } satisfies MemorySyncD1

  return database
}

export type SyncOutboxStoreFixture = Readonly<{
  db: MemorySyncD1
  layer: Layer.Layer<SyncOutboxStore>
  runtime: SyncWorkerRuntime
}>

export const makeSyncOutboxStoreFixture = (
  options: Readonly<{
    dbOptions?: MemorySyncD1Options
    nowIso?: string
  }> = {},
): SyncOutboxStoreFixture => {
  const db = makeMemorySyncD1(options.dbOptions)
  const runtime = syncWorkerTestRuntime(options.nowIso)

  return {
    db,
    layer: SyncOutboxStore.layer(db, runtime),
    runtime,
  }
}

export const makeMemorySyncOutboxStore = (
  options: Readonly<{
    dbOptions?: MemorySyncD1Options
    nowIso?: string
  }> = {},
) => {
  const db = makeMemorySyncD1(options.dbOptions)
  const runtime = syncWorkerTestRuntime(options.nowIso)

  return {
    db,
    runtime,
    store: makeD1SyncOutboxStore(db, runtime),
  }
}
