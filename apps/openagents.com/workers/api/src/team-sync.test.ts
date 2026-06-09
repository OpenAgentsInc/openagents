import { describe, expect, test } from 'vitest'

import {
  publishTeamChatMessageSync,
  publishTeamThreadFileSync,
} from './sync-notifier'

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

type StoredScope = Readonly<{
  created_at: string
  last_seq: number
  scope: string
  updated_at: string
}>

type MemoryD1 = D1Database &
  Readonly<{
    changes: Array<StoredChange>
    scopes: Map<string, StoredScope>
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
  state: Pick<MemoryD1, 'changes' | 'scopes'>,
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

        const decoded: T = JSON.parse(
          JSON.stringify({ last_seq: next.last_seq }),
        )

        return decoded
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

      return makeResult<T>()
    },
  } satisfies D1PreparedStatement

  return statement
}

const makeMemoryD1 = (): MemoryD1 => {
  const state: Pick<MemoryD1, 'changes' | 'scopes'> = {
    changes: [],
    scopes: new Map<string, StoredScope>(),
  }

  return {
    ...state,
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
  } satisfies MemoryD1
}

const makeSyncRoom = (notifiedScopes: Array<string>): DurableObjectNamespace =>
  // eslint-disable-next-line @typescript-eslint/consistent-type-assertions
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

const makeExecutionContext = () => {
  const waits: Array<Promise<unknown>> = []
  const ctx = {
    passThroughOnException: () => undefined,
    props: undefined,
    waitUntil: (promise: Promise<unknown>) => {
      waits.push(promise)
    },
  } satisfies ExecutionContext

  return { ctx, waits }
}

describe('team sync publication', () => {
  test('publishes team chat messages to the team sync scope', async () => {
    const db = makeMemoryD1()
    const notifiedScopes: Array<string> = []
    const { ctx, waits } = makeExecutionContext()

    await publishTeamChatMessageSync(
      {
        OPENAGENTS_DB: db,
        SYNC_ROOM: makeSyncRoom(notifiedScopes),
      },
      ctx,
      {
        agentRunId: null,
        author: {
          avatarUrl: null,
          githubUsername: 'chris',
          name: 'Christopher David',
          userId: 'github:14167547',
        },
        autopilotThreadId: null,
        body: 'hello team',
        createdAt: '2026-06-03T00:00:00.000Z',
        id: 'team_chat_1',
        kind: 'message',
        projectId: null,
        teamId: 'team_openagents_core',
      },
      'github:14167547',
    )
    await Promise.all(waits)

    expect(db.changes).toHaveLength(1)
    expect(db.changes[0]).toMatchObject({
      actor_id: 'github:14167547',
      collection: 'team_chat_messages',
      entity_id: 'team_chat_1',
      op: 'put',
      scope: 'team:team_openagents_core',
      seq: 1,
    })
    expect(JSON.parse(db.changes[0]?.value_json ?? '{}')).toMatchObject({
      body: 'hello team',
      teamId: 'team_openagents_core',
    })
    expect(notifiedScopes).toEqual(['team:team_openagents_core'])
  })

  test('publishes team files to the team sync scope', async () => {
    const db = makeMemoryD1()
    const notifiedScopes: Array<string> = []
    const { ctx, waits } = makeExecutionContext()

    await publishTeamThreadFileSync(
      {
        OPENAGENTS_DB: db,
        SYNC_ROOM: makeSyncRoom(notifiedScopes),
      },
      ctx,
      {
        contentType: 'text/plain',
        createdAt: '2026-06-03T00:00:00.000Z',
        detailUrl: '/teams/team_openagents_core/files/file_1',
        downloadEnabled: true,
        downloadUrl: '/api/thread-files/file_1/download',
        filename: 'notes.txt',
        id: 'file_1',
        ownerUserId: 'github:14167547',
        scope: 'team',
        sizeBytes: 12,
        teamId: 'team_openagents_core',
        threadId: 'team:team_openagents_core:chat',
      },
      'github:14167547',
    )
    await Promise.all(waits)

    expect(db.changes).toHaveLength(1)
    expect(db.changes[0]).toMatchObject({
      actor_id: 'github:14167547',
      collection: 'thread_files',
      entity_id: 'file_1',
      op: 'put',
      scope: 'team:team_openagents_core',
      seq: 1,
    })
    expect(JSON.parse(db.changes[0]?.value_json ?? '{}')).toMatchObject({
      filename: 'notes.txt',
      teamId: 'team_openagents_core',
      threadId: 'team:team_openagents_core:chat',
    })
    expect(notifiedScopes).toEqual(['team:team_openagents_core'])
  })
})
