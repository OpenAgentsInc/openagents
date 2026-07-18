import {
  FullAutoRunControlAuthorityError,
  type FullAutoRunControlAuthorityRepositoryShape,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'
import type { FullAutoRunControlIntent } from '@openagentsinc/khala-sync'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { materializeHttpResult } from './http/responses'
import {
  FULL_AUTO_RUN_CONTROL_INTENTS_PATH,
  FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF,
  makeFullAutoRunControlRoutes,
  type FullAutoRunControlRoutesAuthenticatedOwner,
} from './full-auto-run-control-routes'

const CONNECTION_STRING = 'postgresql://operator:private-password@private-host/khala_sync'
const TIMESTAMP = '2026-07-18T02:00:00.000Z'

/** In-memory fake, mirroring `full-auto-run-routes.test.ts`'s
 * `makeFakeRepository()` harness pattern for the projection route. */
const makeFakeAuthority = (): FullAutoRunControlAuthorityRepositoryShape => {
  const rowsByOwner = new Map<string, Map<string, FullAutoRunControlIntent>>()

  const dispatch: FullAutoRunControlAuthorityRepositoryShape['dispatch'] = input =>
    Effect.sync(() => {
      const owned = rowsByOwner.get(input.ownerUserId) ?? new Map<string, FullAutoRunControlIntent>()
      const existing = [...owned.values()].find(row =>
        row.intentId === input.request.intentId || row.idempotencyKey === input.request.idempotencyKey)
      if (existing !== undefined) return existing
      const intent: FullAutoRunControlIntent = {
        schema: 'full_auto_run.control_intent.v1',
        intentId: input.request.intentId,
        idempotencyKey: input.request.idempotencyKey,
        runRef: input.request.runRef,
        action: input.request.action,
        surface: 'mobile',
        createdAt: TIMESTAMP,
        status: 'pending',
        appliedAt: null,
        rejectionReason: null,
        resultLifecycleState: null,
      }
      owned.set(intent.intentId, intent)
      rowsByOwner.set(input.ownerUserId, owned)
      return intent
    })

  const list: FullAutoRunControlAuthorityRepositoryShape['list'] = input =>
    Effect.sync(() => [...(rowsByOwner.get(input.ownerUserId)?.values() ?? [])])

  const reportOutcome: FullAutoRunControlAuthorityRepositoryShape['reportOutcome'] = input =>
    Effect.gen(function* () {
      const owned = rowsByOwner.get(input.ownerUserId)
      const existing = owned?.get(input.outcome.intentId)
      if (existing === undefined) {
        return yield* new FullAutoRunControlAuthorityError({ kind: 'intent_not_found', reason: 'no such intent' })
      }
      const updated: FullAutoRunControlIntent = {
        ...existing,
        status: input.outcome.status,
        appliedAt: input.outcome.status === 'applied' ? TIMESTAMP : null,
        rejectionReason: input.outcome.status === 'rejected' ? (input.outcome.rejectionReason ?? null) : null,
        resultLifecycleState: input.outcome.resultLifecycleState ?? null,
      }
      owned!.set(updated.intentId, updated)
      return updated
    })

  return { dispatch, list, reportOutcome }
}

type Env = Readonly<{ KHALA_SYNC_DB?: Readonly<{ connectionString: string }> }>

const makeHarness = (
  options: Readonly<{
    authenticationError?: boolean
    owner?: FullAutoRunControlRoutesAuthenticatedOwner | undefined
    env?: Env
    authority?: FullAutoRunControlAuthorityRepositoryShape
  }> = {},
) => {
  const authority = options.authority ?? makeFakeAuthority()
  const calls = { clients: 0, ended: 0 }
  const routes = makeFullAutoRunControlRoutes<Env>({
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
    makeAuthority: () => authority,
  })
  const run = (request: Request) =>
    routes.handle(request, options.env ?? { KHALA_SYNC_DB: { connectionString: CONNECTION_STRING } }, {} as ExecutionContext)
      .pipe(Effect.map(materializeHttpResult), Effect.runPromise)
  return { calls, run, authority }
}

const ownerA: FullAutoRunControlRoutesAuthenticatedOwner = { userId: 'owner-a' }
const ownerB: FullAutoRunControlRoutesAuthenticatedOwner = { userId: 'owner-b' }

describe('makeFullAutoRunControlRoutes', () => {
  test('unauthenticated GET is refused before storage is touched', async () => {
    const harness = makeHarness({ owner: undefined })
    const response = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`))
    expect(response.status).toBe(401)
    expect(harness.calls.clients).toBe(0)
  })

  test('unsupported HTTP method is rejected', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, { method: 'DELETE' }),
    )
    expect(response.status).toBe(405)
  })

  test('GET with no intents returns an empty list', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`))
    expect(response.status).toBe(200)
    const body = await response.json() as Readonly<{ ok: boolean; intents: ReadonlyArray<unknown> }>
    expect(body.ok).toBe(true)
    expect(body.intents).toEqual([])
  })

  test('mobile dispatches a Pause intent: POST { intent } returns it pending, then GET lists it', async () => {
    const harness = makeHarness({ owner: ownerA })
    const dispatchResponse = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          intent: {
            intentId: 'intent.mobile.1', idempotencyKey: 'idem.mobile.1',
            runRef: 'run.full-auto.abc', action: 'pause',
          },
        }),
      }),
    )
    expect(dispatchResponse.status).toBe(200)
    const dispatchBody = await dispatchResponse.json() as Readonly<{ ok: boolean; intent: { status: string; action: string } }>
    expect(dispatchBody.ok).toBe(true)
    expect(dispatchBody.intent.status).toBe('pending')
    expect(dispatchBody.intent.action).toBe('pause')

    const listResponse = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`))
    const listBody = await listResponse.json() as Readonly<{ intents: ReadonlyArray<{ intentId: string }> }>
    expect(listBody.intents).toHaveLength(1)
    expect(listBody.intents[0]?.intentId).toBe('intent.mobile.1')
  })

  test('desktop reports an applied outcome: POST { outcome } transitions the intent, visible on the next GET', async () => {
    const authority = makeFakeAuthority()
    const harness = makeHarness({ owner: ownerA, authority })
    await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          intent: { intentId: 'intent.mobile.1', idempotencyKey: 'idem.mobile.1', runRef: 'run.full-auto.abc', action: 'resume' },
        }),
      }),
    )
    const outcomeResponse = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          outcome: { intentId: 'intent.mobile.1', status: 'applied', resultLifecycleState: 'running' },
        }),
      }),
    )
    expect(outcomeResponse.status).toBe(200)
    const outcomeBody = await outcomeResponse.json() as Readonly<{ intent: { status: string; resultLifecycleState: string } }>
    expect(outcomeBody.intent.status).toBe('applied')
    expect(outcomeBody.intent.resultLifecycleState).toBe('running')

    const listResponse = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`))
    const listBody = await listResponse.json() as Readonly<{ intents: ReadonlyArray<{ status: string }> }>
    expect(listBody.intents[0]?.status).toBe('applied')
  })

  test('desktop reports a rejected outcome with a typed reason, never silently dropped', async () => {
    const authority = makeFakeAuthority()
    const harness = makeHarness({ owner: ownerA, authority })
    await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          intent: { intentId: 'intent.mobile.1', idempotencyKey: 'idem.mobile.1', runRef: 'run.full-auto.abc', action: 'stop' },
        }),
      }),
    )
    const outcomeResponse = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          outcome: { intentId: 'intent.mobile.1', status: 'rejected', rejectionReason: 'illegal_transition' },
        }),
      }),
    )
    const outcomeBody = await outcomeResponse.json() as Readonly<{ intent: { status: string; rejectionReason: string } }>
    expect(outcomeBody.intent.status).toBe('rejected')
    expect(outcomeBody.intent.rejectionReason).toBe('illegal_transition')
  })

  test('reporting an outcome for an unknown intentId returns 404 intent_not_found, never a silent no-op', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ outcome: { intentId: 'intent.mobile.ghost', status: 'applied' } }),
      }),
    )
    expect(response.status).toBe(404)
    const body = await response.json() as Readonly<{ error: { code: string } }>
    expect(body.error.code).toBe('intent_not_found')
  })

  test('cross-account isolation: owner B never observes owner A dispatched intents', async () => {
    const authority = makeFakeAuthority()
    const harnessA = makeHarness({ owner: ownerA, authority })
    await harnessA.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          intent: { intentId: 'intent.mobile.1', idempotencyKey: 'idem.mobile.1', runRef: 'run.full-auto.abc', action: 'pause' },
        }),
      }),
    )
    const harnessB = makeHarness({ owner: ownerB, authority })
    const response = await harnessB.run(new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`))
    const body = await response.json() as Readonly<{ intents: ReadonlyArray<unknown> }>
    expect(body.intents).toEqual([])
  })

  test('malformed dispatch body (missing action) is rejected as invalid_request', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ intent: { intentId: 'x', idempotencyKey: 'y', runRef: 'z' } }),
      }),
    )
    expect(response.status).toBe(400)
    const body = await response.json() as Readonly<{ error: { code: string } }>
    expect(body.error.code).toBe('invalid_request')
  })

  test('malformed body with neither intent nor outcome is rejected', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ somethingElse: true }),
      }),
    )
    expect(response.status).toBe(400)
  })

  test('fails closed with no configured Postgres binding, without opening a client', async () => {
    const harness = makeHarness({ owner: ownerA, env: {} })
    const response = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`))
    expect(response.status).toBe(503)
    expect(harness.calls.clients).toBe(0)
  })

  test('rejects unexpected query parameters', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(
      new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}?runRef=whatever`),
    )
    expect(response.status).toBe(400)
  })

  test('every response is decorated with the routeRef and never leaks another owner id', async () => {
    const harness = makeHarness({ owner: ownerA })
    const response = await harness.run(new Request(`https://openagents.com${FULL_AUTO_RUN_CONTROL_INTENTS_PATH}`))
    const body = await response.json() as Readonly<{ routeRef: string }>
    expect(body.routeRef).toBe(FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF)
    expect(JSON.stringify(body)).not.toContain('owner-a')
  })
})
