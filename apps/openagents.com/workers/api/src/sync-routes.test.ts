import { MutationId, SyncCommand, SyncScope } from '@openagentsinc/sync-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { Env } from './index'
import { makeSyncRoutes } from './sync-routes'

type TestSession = Readonly<{
  user: Readonly<{
    userId: string
  }>
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

type MemoryD1 = D1Database &
  Readonly<{
    changes: Array<StoredChange>
    mutations: Map<string, StoredMutation>
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
  state: Pick<MemoryD1, 'changes' | 'mutations'>,
  query: string,
): D1PreparedStatement => {
  let values: ReadonlyArray<unknown> = []
  let statement: D1PreparedStatement

  statement = {
    all: async <T = Record<string, unknown>>() => {
      if (query.includes('FROM sync_changes')) {
        const [scope] = values
        const rows = state.changes
          .filter(change => change.scope === String(scope))
          .sort((left, right) => left.seq - right.seq)

        return makeResult(cloneForD1(rows))
      }

      return makeResult<T>()
    },
    bind: (...nextValues: ReadonlyArray<unknown>) => {
      values = nextValues

      return statement
    },
    first: async <T = Record<string, unknown>>() => {
      if (query.includes('FROM sync_mutations')) {
        const [mutationId] = values
        const row = state.mutations.get(String(mutationId))

        return row === undefined ? null : cloneForD1<T>(row)
      }

      return null
    },
    raw: rawD1Rows,
    run: async <T = Record<string, unknown>>() => {
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

      return makeResult<T>()
    },
  } satisfies D1PreparedStatement

  return statement
}

const makeMemoryD1 = (): MemoryD1 => {
  const state: Pick<MemoryD1, 'changes' | 'mutations'> = {
    changes: [],
    mutations: new Map(),
  }
  const db = {
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

  return db
}

const makeSyncRoom = (capturedScopes: Array<string>): DurableObjectNamespace =>
  ({
    getByName: (scope: string) => ({
      fetch: async (request: Request) => {
        capturedScopes.push(
          request.headers.get('x-openagents-sync-scope') ?? scope,
        )

        return new Response(null, { status: 204 })
      },
    }),
    get: (id: DurableObjectId) => ({
      fetch: async (request: Request) => {
        capturedScopes.push(
          request.headers.get('x-openagents-sync-scope') ?? String(id),
        )

        return new Response(null, { status: 204 })
      },
    }),
    idFromName: (scope: string) => scope,
  }) as never

const makeExecutionContext = (): ExecutionContext =>
  ({
    passThroughOnException: () => undefined,
    props: undefined,
    waitUntil: () => undefined,
  }) satisfies ExecutionContext

const makeEnv = (
  db: D1Database,
  syncRoom: DurableObjectNamespace = makeSyncRoom([]),
): Env =>
  ({
    OPENAGENTS_DB: db,
    SYNC_ROOM: syncRoom,
  }) as Env

const makeRoutes = (session: TestSession | undefined) =>
  makeSyncRoutes<TestSession>({
    appendRefreshedSessionCookies: response => response,
    authorizeSyncPath: () => Effect.sync(() => undefined),
    requireBrowserSession: () => Effect.succeed(session),
  })

const defaultSession: TestSession = {
  user: { userId: 'user_1' },
}

const runRoute = (
  request: Request,
  env: Env,
  session: TestSession | undefined,
): Promise<Response> =>
  Effect.runPromise(
    makeRoutes(session).routeSyncRequest(request, env, makeExecutionContext()),
  )

describe('sync routes', () => {
  test('returns no-store unauthorized responses before sync store access', async () => {
    const response = await runRoute(
      new Request('https://openagents.test/api/sync/workspace/user_1/snapshot'),
      makeEnv(makeMemoryD1()),
      undefined,
    )

    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toContain('no-store')
  })

  test('reads snapshots through SyncOutboxStore', async () => {
    const db = makeMemoryD1()
    db.changes.push({
      actor_id: null,
      collection: 'missions',
      created_at: '2026-06-04T00:00:00.000Z',
      entity_id: 'run_1',
      mutation_id: null,
      op: 'put',
      patch_json: null,
      scope: 'workspace:user_1',
      seq: 1,
      value_json: JSON.stringify({ status: 'queued' }),
    })

    const response = await runRoute(
      new Request('https://openagents.test/api/sync/workspace/user_1/snapshot'),
      makeEnv(db),
      defaultSession,
    )

    await expect(response.json()).resolves.toMatchObject({
      collections: {
        missions: {
          run_1: {
            status: 'queued',
          },
        },
      },
      cursor: 1,
      scope: 'workspace:user_1',
    })
    expect(response.status).toBe(200)
  })

  test('maps mutation scope mismatches through the route error mapper', async () => {
    const command = new SyncCommand({
      command: 'mission.create',
      mutationId: MutationId.make('mutation_1'),
      payload: { prompt: 'Run tests' },
      scope: SyncScope.make('workspace:user_2'),
    })
    const response = await runRoute(
      new Request('https://openagents.test/api/sync/workspace/user_1/mutate', {
        body: JSON.stringify(command),
        method: 'POST',
      }),
      makeEnv(makeMemoryD1()),
      defaultSession,
    )

    await expect(response.json()).resolves.toEqual({
      error: 'scope_mismatch',
      expectedScope: 'workspace:user_1',
    })
    expect(response.status).toBe(409)
  })

  test('dispatches stream requests with the resolved sync scope header', async () => {
    const capturedScopes: Array<string> = []
    const response = await runRoute(
      new Request('https://openagents.test/api/sync/workspace/user_1/stream'),
      makeEnv(makeMemoryD1(), makeSyncRoom(capturedScopes)),
      defaultSession,
    )

    expect(response.status).toBe(204)
    expect(capturedScopes).toEqual(['workspace:user_1'])
  })

  test('allows anonymous public goal snapshots', async () => {
    const db = makeMemoryD1()
    db.changes.push({
      actor_id: null,
      collection: 'public_agent_goals',
      created_at: '2026-06-04T00:00:00.000Z',
      entity_id: 'goal_public_1',
      mutation_id: null,
      op: 'put',
      patch_json: null,
      scope: 'public-goal:goal_public_1',
      seq: 1,
      value_json: JSON.stringify({ objective: 'Publish safe progress' }),
    })

    const response = await runRoute(
      new Request(
        'https://openagents.test/api/sync/public-goal/goal_public_1/snapshot',
      ),
      makeEnv(db),
      undefined,
    )

    await expect(response.json()).resolves.toMatchObject({
      collections: {
        public_agent_goals: {
          goal_public_1: {
            objective: 'Publish safe progress',
          },
        },
      },
      cursor: 1,
      scope: 'public-goal:goal_public_1',
    })
    expect(response.status).toBe(200)
  })

  test('allows anonymous public agent streams but rejects public mutations', async () => {
    const capturedScopes: Array<string> = []
    const streamResponse = await runRoute(
      new Request(
        'https://openagents.test/api/sync/public-agent/agent_artanis/stream',
      ),
      makeEnv(makeMemoryD1(), makeSyncRoom(capturedScopes)),
      undefined,
    )
    const command = new SyncCommand({
      command: 'public.mutate',
      mutationId: MutationId.make('mutation_public_1'),
      payload: {},
      scope: SyncScope.make('public-agent:agent_artanis'),
    })
    const mutationResponse = await runRoute(
      new Request(
        'https://openagents.test/api/sync/public-agent/agent_artanis/mutate',
        { body: JSON.stringify(command), method: 'POST' },
      ),
      makeEnv(makeMemoryD1()),
      undefined,
    )

    expect(streamResponse.status).toBe(204)
    expect(capturedScopes).toEqual(['public-agent:agent_artanis'])
    await expect(mutationResponse.json()).resolves.toEqual({
      error: 'not_found',
    })
    expect(mutationResponse.status).toBe(404)
  })

  test('public-settled-feed is a retired legacy kind (KS-6.10, #8420): snapshot and stream return 404, not a double-served room', async () => {
    // KS-6.4 (#8414) fully cut the settled feed over to the khala-sync engine
    // on BOTH ends: the legacy `notifySyncScopes` producer was deleted
    // (`tassadar-settled-feed-sync.ts`) and the web client now opens
    // `WS /api/sync/connect?scope=scope.public.settled-feed` instead of this
    // legacy DO-room path (`apps/web/src/subscriptions.ts`). With zero
    // remaining producers AND zero remaining consumers, `'public-settled-feed'`
    // was removed from `SyncScopeKind` entirely (KS-6.10, #8420) — this is
    // the acceptance criteria's own "confirm 404/gone rather than silently
    // double-serving" check, scoped to the one legacy kind that is provably
    // dead today (unlike team chat, thread files, agent goals, gym
    // run-progress, or the workspace/team/thread socket, which are all still
    // live legacy consumers/producers and must NOT be retired yet).
    const db = makeMemoryD1()
    db.changes.push({
      actor_id: 'system',
      collection: 'settled_events',
      created_at: '2026-06-17T00:00:00.000Z',
      entity_id: 'settled.window.0',
      mutation_id: null,
      op: 'put',
      patch_json: null,
      scope: 'public-settled-feed:tassadar',
      seq: 1,
      value_json: JSON.stringify({
        amountSats: 5,
        contributorRef: 'pylon.worker.orrery',
        totalSettledSats: 5,
      }),
    })

    const snapshotResponse = await runRoute(
      new Request(
        'https://openagents.test/api/sync/public-settled-feed/tassadar/snapshot',
      ),
      makeEnv(db),
      undefined,
    )

    expect(snapshotResponse.status).toBe(404)

    const capturedScopes: Array<string> = []
    const streamResponse = await runRoute(
      new Request(
        'https://openagents.test/api/sync/public-settled-feed/tassadar/stream',
      ),
      makeEnv(makeMemoryD1(), makeSyncRoom(capturedScopes)),
      undefined,
    )

    expect(streamResponse.status).toBe(404)
    // The retired kind must never reach the DO room at all — not merely
    // return a not-found status while still poking the room underneath.
    expect(capturedScopes).toEqual([])
  })

  test('allows anonymous public gym run-progress snapshots and streams (#6261)', async () => {
    const db = makeMemoryD1()
    db.changes.push({
      actor_id: 'system',
      collection: 'gym_run_progress',
      created_at: '2026-06-25T00:00:00.000Z',
      entity_id: 'run.gym.terminal_bench.web.test',
      mutation_id: null,
      op: 'put',
      patch_json: null,
      scope: 'public-gym-run-progress:network',
      seq: 1,
      value_json: JSON.stringify({
        schemaVersion: 'openagents.gym.run_progress.v1',
        runRef: 'run.gym.terminal_bench.web.test',
        publication: 'web_authorized',
      }),
    })

    const snapshotResponse = await runRoute(
      new Request(
        'https://openagents.test/api/sync/public-gym-run-progress/network/snapshot',
      ),
      makeEnv(db),
      undefined,
    )

    await expect(snapshotResponse.json()).resolves.toMatchObject({
      collections: {
        gym_run_progress: {
          'run.gym.terminal_bench.web.test': {
            runRef: 'run.gym.terminal_bench.web.test',
            publication: 'web_authorized',
          },
        },
      },
      scope: 'public-gym-run-progress:network',
    })
    expect(snapshotResponse.status).toBe(200)

    const capturedScopes: Array<string> = []
    const streamResponse = await runRoute(
      new Request(
        'https://openagents.test/api/sync/public-gym-run-progress/network/stream',
      ),
      makeEnv(makeMemoryD1(), makeSyncRoom(capturedScopes)),
      undefined,
    )

    expect(streamResponse.status).toBe(204)
    expect(capturedScopes).toEqual(['public-gym-run-progress:network'])
  })
})
