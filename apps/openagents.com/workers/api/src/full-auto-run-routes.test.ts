import { FullAutoRunClientRunProjection } from '@openagentsinc/khala-sync'
import {
  FullAutoRunProjectionAuthorityError,
  type FullAutoRunProjectionAuthorityRepositoryShape,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'
import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { materializeHttpResult } from './http/responses'
import {
  FULL_AUTO_RUNS_PATH,
  FULL_AUTO_RUNS_ROUTE_REF,
  makeFullAutoRunRoutes,
  type FullAutoRunRoutesAuthenticatedOwner,
} from './full-auto-run-routes'

const CONNECTION_STRING = 'postgresql://operator:private-password@private-host/khala_sync'
const TIMESTAMP = '2026-07-17T21:00:00.000Z'

const validRunBody = {
  runRef: 'run.full-auto.abc123.def456',
  threadRef: 'thread.abc123',
  objective: 'Ship the mobile live-run projection.',
  doneCondition: 'The new endpoint round-trips a projection end to end.',
  lifecycleState: 'running',
  workspaceLabel: 'openagents',
  startedAt: TIMESTAMP,
  updatedAt: TIMESTAMP,
  lastTransition: { actor: 'control_api', at: TIMESTAMP },
  laneRef: 'codex-local',
  accountRef: null,
  turnCap: 20,
  successfulAttempts: 3,
  failedAttempts: 0,
  rotationCount: 0,
  receiptSummary: null,
}

/** In-memory fake, mirroring `sarah-fleet-run-routes.test.ts`'s
 * `makeFakeAuthority()` harness pattern for the FleetRun authority route. */
const makeFakeRepository = (): FullAutoRunProjectionAuthorityRepositoryShape => {
  const rowsByOwner = new Map<string, typeof FullAutoRunClientRunProjection.Type>()

  const publish: FullAutoRunProjectionAuthorityRepositoryShape['publish'] = input =>
    Effect.gen(function* () {
      if (input.run === null) {
        rowsByOwner.delete(input.ownerUserId)
      } else {
        const decoded = yield* Effect.try({
          try: () => S.decodeUnknownSync(FullAutoRunClientRunProjection)(input.run, { onExcessProperty: 'error' }),
          catch: () => new FullAutoRunProjectionAuthorityError({ kind: 'invalid_request', reason: 'fixed invalid run' }),
        })
        rowsByOwner.set(input.ownerUserId, decoded)
      }
      return {
        projection: S.decodeUnknownSync(S.Struct({
          schema: S.Literal('full_auto_run.mobile_projection.v1'),
          privateMaterialExcluded: S.Literal(true),
          generatedAt: S.String,
          run: S.NullOr(FullAutoRunClientRunProjection),
        }))({
          schema: 'full_auto_run.mobile_projection.v1',
          privateMaterialExcluded: true,
          generatedAt: TIMESTAMP,
          run: input.run === null ? null : (rowsByOwner.get(input.ownerUserId) ?? null),
        }),
      }
    })

  const observe: FullAutoRunProjectionAuthorityRepositoryShape['observe'] = input =>
    Effect.succeed({
      projection: S.decodeUnknownSync(S.Struct({
        schema: S.Literal('full_auto_run.mobile_projection.v1'),
        privateMaterialExcluded: S.Literal(true),
        generatedAt: S.String,
        run: S.NullOr(FullAutoRunClientRunProjection),
      }))({
        schema: 'full_auto_run.mobile_projection.v1',
        privateMaterialExcluded: true,
        generatedAt: TIMESTAMP,
        run: rowsByOwner.get(input.ownerUserId) ?? null,
      }),
    })

  return { publish, observe }
}

type Env = Readonly<{ KHALA_SYNC_DB?: Readonly<{ connectionString: string }> }>

const makeHarness = (
  options: Readonly<{
    authenticationError?: boolean
    owner?: FullAutoRunRoutesAuthenticatedOwner | undefined
    env?: Env
    repository?: FullAutoRunProjectionAuthorityRepositoryShape
  }> = {},
) => {
  const repository = options.repository ?? makeFakeRepository()
  const calls = { clients: 0, ended: 0 }
  const routes = makeFullAutoRunRoutes<Env>({
    authenticateOwner: async () => {
      if (options.authenticationError === true) throw new Error('auth boom')
      return options.owner
    },
    makeSqlClient: async () => {
      calls.clients += 1
      return {
        sql: (async () => []) as unknown as SyncSql,
        end: async () => {
          calls.ended += 1
        },
      }
    },
    makeRepository: () => repository,
  })
  const run = (request: Request) =>
    routes.handle(request, options.env ?? { KHALA_SYNC_DB: { connectionString: CONNECTION_STRING } }, {} as ExecutionContext)
      .pipe(Effect.map(materializeHttpResult), Effect.runPromise)
  return { calls, run, repository }
}

const ownerA: FullAutoRunRoutesAuthenticatedOwner = { userId: 'owner-a' }
const ownerB: FullAutoRunRoutesAuthenticatedOwner = { userId: 'owner-b' }

describe('makeFullAutoRunRoutes', () => {
  test('unauthenticated GET is refused before storage is touched', async () => {
    const harness = makeHarness({ owner: undefined })
    const response = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`))
    expect(response.status).toBe(401)
    const body = await response.json() as Readonly<{ ok: boolean; error: { code: string } }>
    expect(body.ok).toBe(false)
    expect(body.error.code).toBe('unauthenticated')
    expect(harness.calls.clients).toBe(0)
  })

  test('authentication failure returns 503 distinct from 401', async () => {
    const harness = makeHarness({ authenticationError: true })
    const response = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`))
    expect(response.status).toBe(503)
    const body = await response.json() as Readonly<{ error: { code: string } }>
    expect(body.error.code).toBe('authentication_unavailable')
  })

  test('unsupported HTTP method is rejected without authenticating or opening storage', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`, { method: 'DELETE' }),
    )
    expect(response.status).toBe(405)
    expect(harness.calls.clients).toBe(0)
  })

  test('GET with no published run returns run: null (not an error)', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`))
    expect(response.status).toBe(200)
    const body = await response.json() as Readonly<{ ok: boolean; projection: { run: unknown; schema: string } }>
    expect(body.ok).toBe(true)
    expect(body.projection.schema).toBe('full_auto_run.mobile_projection.v1')
    expect(body.projection.run).toBeNull()
  })

  test('POST publishes a run, then GET observes exactly that run for the same owner', async () => {
    const harness = makeHarness({ owner: ownerA })
    const publishResponse = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ run: validRunBody }),
      }),
    )
    expect(publishResponse.status).toBe(200)
    const publishBody = await publishResponse.json() as Readonly<{ ok: boolean; projection: { run: { runRef: string } } }>
    expect(publishBody.ok).toBe(true)
    expect(publishBody.projection.run.runRef).toBe(validRunBody.runRef)

    const getResponse = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`))
    const getBody = await getResponse.json() as Readonly<{ projection: { run: { runRef: string; lifecycleState: string } } }>
    expect(getBody.projection.run.runRef).toBe(validRunBody.runRef)
    expect(getBody.projection.run.lifecycleState).toBe('running')
  })

  test('POST with run: null clears a previously published run', async () => {
    const repository = makeFakeRepository()
    const harness = makeHarness({ owner: ownerA, repository })
    await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ run: validRunBody }),
      }),
    )
    await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ run: null }),
      }),
    )
    const getResponse = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`))
    const getBody = await getResponse.json() as Readonly<{ projection: { run: unknown } }>
    expect(getBody.projection.run).toBeNull()
  })

  test('cross-account isolation: owner B never observes owner A published run', async () => {
    const repository = makeFakeRepository()
    const harnessA = makeHarness({ owner: ownerA, repository })
    await harnessA.run(
      new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ run: validRunBody }),
      }),
    )
    const harnessB = makeHarness({ owner: ownerB, repository })
    const response = await harnessB.run(new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`))
    const body = await response.json() as Readonly<{ projection: { run: unknown } }>
    expect(body.projection.run).toBeNull()
  })

  test('malformed publish body (missing run field) is rejected as invalid_request', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ notRun: true }),
      }),
    )
    expect(response.status).toBe(400)
    const body = await response.json() as Readonly<{ error: { code: string } }>
    expect(body.error.code).toBe('invalid_request')
  })

  test('malformed run projection (bad lifecycleState) is rejected as invalid_request', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ run: { ...validRunBody, lifecycleState: 'bogus' } }),
      }),
    )
    expect(response.status).toBe(400)
    const body = await response.json() as Readonly<{ error: { code: string } }>
    expect(body.error.code).toBe('invalid_request')
  })

  test('rejects a raw local filesystem path smuggled into workspaceLabel', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ run: { ...validRunBody, workspaceLabel: '/Users/private/repo' } }),
      }),
    )
    expect(response.status).toBe(400)
  })

  test('fails closed with no configured Postgres binding, without opening a client', async () => {
    const harness = makeHarness({ owner: ownerA, env: {} })
    const response = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`))
    expect(response.status).toBe(503)
    const body = await response.json() as Readonly<{ error: { code: string } }>
    expect(body.error.code).toBe('storage_unavailable')
    expect(harness.calls.clients).toBe(0)
  })

  test('rejects unexpected query parameters', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}?runRef=whatever`),
    )
    expect(response.status).toBe(400)
  })

  test('every response is decorated with the routeRef and never leaks another owner id', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUNS_PATH}`))
    const body = await response.json() as Readonly<{ routeRef: string }>
    expect(body.routeRef).toBe(FULL_AUTO_RUNS_ROUTE_REF)
    expect(JSON.stringify(body)).not.toContain('owner-a')
  })
})
