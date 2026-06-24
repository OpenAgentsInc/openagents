import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import type {
  AgentRegistrationStore,
  AgentUserRecord,
} from './agent-registration'
import { ATIF_PINNED_SCHEMA_VERSION } from './atif-trace-schema'
import { makeTraceStoreRoutes } from './trace-store-routes'
import type {
  CreateTraceInput,
  TraceRecord,
  TraceStore,
} from './trace-store-d1'

type Bindings = Record<string, never>

const ENV: Bindings = {}
const CTX = {} as ExecutionContext
const NOW = '2026-06-24T00:00:00.000Z'

const userFor = (id: string): AgentUserRecord => ({
  avatarUrl: null,
  createdAt: NOW,
  displayName: id,
  id,
  kind: 'agent',
  primaryEmail: null,
  status: 'active',
  updatedAt: NOW,
})

// Authenticates the bearer as `userId` (or none when undefined).
const makeAgentStore = (userId: string | undefined): AgentRegistrationStore => ({
  createAgentRegistration: () => Promise.resolve(),
  findAgentByTokenHash: () =>
    Promise.resolve(
      userId === undefined
        ? undefined
        : {
            credentialId: `cred-${userId}`,
            profileMetadataJson: '{}',
            tokenPrefix: 'oa_agent_',
            user: userFor(userId),
          },
    ),
  touchAgentCredential: () => Promise.resolve(),
  updateAgentDisplayName: () => Promise.resolve(0),
})

// Minimal in-memory TraceStore (no D1).
const makeMemoryStore = (): TraceStore & { rows: Map<string, TraceRecord> } => {
  const rows = new Map<string, TraceRecord>()
  const byIdem = new Map<string, string>()
  const recordFrom = (input: CreateTraceInput): TraceRecord => ({
    traceUuid: input.traceUuid,
    ownerUserId: input.ownerUserId,
    agentRef: input.agentRef,
    schemaVersion: input.schemaVersion,
    trajectoryId: input.trajectoryId,
    sessionId: input.sessionId,
    visibility: input.visibility,
    stepCount: input.stepCount,
    trajectory: input.trajectory,
    blobRefs: input.blobRefs,
    idempotencyKey: input.idempotencyKey,
    createdAt: input.nowIso,
    updatedAt: input.nowIso,
  })
  return {
    rows,
    createTrace: input => {
      if (input.idempotencyKey !== null) {
        const key = `${input.ownerUserId}:${input.idempotencyKey}`
        const existingUuid = byIdem.get(key)
        if (existingUuid !== undefined) {
          return Promise.resolve({ record: rows.get(existingUuid)!, created: false })
        }
        const record = recordFrom(input)
        rows.set(record.traceUuid, record)
        byIdem.set(key, record.traceUuid)
        return Promise.resolve({ record, created: true })
      }
      const record = recordFrom(input)
      rows.set(record.traceUuid, record)
      return Promise.resolve({ record, created: true })
    },
    readTraceByUuid: uuid => Promise.resolve(rows.get(uuid)),
    listTracesForOwner: (ownerUserId, limit) =>
      Promise.resolve(
        [...rows.values()]
          .filter(row => row.ownerUserId === ownerUserId)
          .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
          .slice(0, limit),
      ),
  }
}

type Session = Readonly<{ user: { email?: string; userId: string } }>

const makeDeps = (options: {
  store: TraceStore
  agentUserId?: string | undefined
  browserSession?: Session | undefined
  adminEmails?: ReadonlyArray<string>
}) =>
  makeTraceStoreRoutes<Bindings, Session>({
    agentStore: () => makeAgentStore(options.agentUserId),
    appendRefreshedSessionCookies: response => response,
    isAdminEmail: email => (options.adminEmails ?? []).includes(email),
    makeStore: () => options.store,
    makeId: () => 'trace-uuid-fixed',
    nowIso: () => NOW,
    requireBrowserSession: () => Promise.resolve(options.browserSession),
  })

const cleanTrajectory = () => ({
  schema_version: ATIF_PINNED_SCHEMA_VERSION,
  trajectory_id: 'traj-1',
  agent: { name: 'Raynor', version: '1.0.0', model_name: 'openagents/khala' },
  steps: [
    { step_id: 1, source: 'user', message: 'Log in.' },
    {
      step_id: 2,
      source: 'agent',
      message: 'Clicking.',
      tool_calls: [
        {
          tool_call_id: 'call-1',
          function_name: 'click',
          arguments: { selector: '#go' },
        },
      ],
      observation: { results: [{ source_call_id: 'call-1', content: 'ok' }] },
    },
  ],
})

const ingestRequest = (
  body: unknown,
  headers: Record<string, string> = {},
): Request =>
  new Request('https://openagents.com/api/traces', {
    method: 'POST',
    headers: {
      authorization: 'Bearer oa_agent_test',
      'content-type': 'application/json',
      'idempotency-key': 'idem-1',
      ...headers,
    },
    body: JSON.stringify(body),
  })

const run = (effect: Effect.Effect<Response> | undefined): Promise<Response> => {
  expect(effect).toBeDefined()
  return Effect.runPromise(effect!)
}

describe('POST /api/traces ingest', () => {
  it('stores a public-safe trace and returns a uuid', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: cleanTrajectory(), visibility: 'public' }),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(201)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.uuid).toBe('trace-uuid-fixed')
    expect(json.url).toBe('/trace/trace-uuid-fixed')
    expect(json.visibility).toBe('public')
    expect(store.rows.get('trace-uuid-fixed')?.ownerUserId).toBe('agent-1')
  })

  it('rejects an unauthenticated ingest with 401', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: undefined })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: cleanTrajectory() }),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(401)
    expect(store.rows.size).toBe(0)
  })

  it('rejects ingest without an Idempotency-Key with 400', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const request = new Request('https://openagents.com/api/traces', {
      method: 'POST',
      headers: {
        authorization: 'Bearer oa_agent_test',
        'content-type': 'application/json',
      },
      body: JSON.stringify({ trajectory: cleanTrajectory() }),
    })
    const response = await run(routes.routeTraceRequest(request, ENV, CTX))
    expect(response.status).toBe(400)
    expect(store.rows.size).toBe(0)
  })

  it('tripwires a leaky payload (raw provider model id) with 422 and stores nothing', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const leaky = {
      ...cleanTrajectory(),
      agent: { name: 'a', version: '1', model_name: 'claude-opus-4' },
    }
    const response = await run(
      routes.routeTraceRequest(ingestRequest({ trajectory: leaky }), ENV, CTX),
    )
    expect(response.status).toBe(422)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.error).toBe('trace_public_safety_rejected')
    expect(json.findings).toContain('raw_provider_model_id')
    expect(store.rows.size).toBe(0)
  })

  it('tripwires a secret in a message with 422', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const leaky = {
      ...cleanTrajectory(),
      steps: [
        { step_id: 1, source: 'agent', message: 'key sk-abcdef0123456789abcd' },
      ],
    }
    const response = await run(
      routes.routeTraceRequest(ingestRequest({ trajectory: leaky }), ENV, CTX),
    )
    expect(response.status).toBe(422)
    expect(store.rows.size).toBe(0)
  })

  it('rejects an invalid (non-sequential) trajectory with 400', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const bad = {
      ...cleanTrajectory(),
      steps: [
        { step_id: 1, source: 'user', message: 'a' },
        { step_id: 5, source: 'agent', message: 'b' },
      ],
    }
    const response = await run(
      routes.routeTraceRequest(ingestRequest({ trajectory: bad }), ENV, CTX),
    )
    expect(response.status).toBe(400)
    expect(store.rows.size).toBe(0)
  })

  it('is idempotent on a repeated Idempotency-Key (200 replay)', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const first = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: cleanTrajectory() }),
        ENV,
        CTX,
      ),
    )
    expect(first.status).toBe(201)
    const second = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: cleanTrajectory() }),
        ENV,
        CTX,
      ),
    )
    expect(second.status).toBe(200)
    const json = (await second.json()) as Record<string, unknown>
    expect(json.replay).toBe(true)
    expect(store.rows.size).toBe(1)
  })

  it('defaults visibility to unlisted', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: cleanTrajectory() }),
        ENV,
        CTX,
      ),
    )
    const json = (await response.json()) as Record<string, unknown>
    expect(json.visibility).toBe('unlisted')
  })
})

const seedTrace = async (
  store: TraceStore,
  visibility: 'public' | 'unlisted' | 'owner_only',
  ownerUserId = 'agent-1',
): Promise<string> => {
  const result = await store.createTrace({
    traceUuid: `uuid-${visibility}`,
    ownerUserId,
    agentRef: `agent:${ownerUserId}`,
    schemaVersion: ATIF_PINNED_SCHEMA_VERSION,
    trajectoryId: 'traj-1',
    sessionId: null,
    visibility,
    stepCount: 2,
    trajectory: cleanTrajectory(),
    blobRefs: [{ kind: 'video', r2Key: 'traces/uuid/video.mp4' }],
    idempotencyKey: null,
    nowIso: NOW,
  })
  return result.record.traceUuid
}

const readRequest = (uuid: string): Request =>
  new Request(`https://openagents.com/api/traces/${uuid}`)

describe('GET /api/traces/{uuid} read', () => {
  it('returns the public-safe projection for a public trace (no auth)', async () => {
    const store = makeMemoryStore()
    const uuid = await seedTrace(store, 'public')
    const routes = makeDeps({ store })
    const response = await run(
      routes.routeTraceRequest(readRequest(uuid), ENV, CTX),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as { trace: Record<string, unknown> }
    expect(json.trace.uuid).toBe(uuid)
    expect(json.trace.visibility).toBe('public')
    expect(json.trace.blobRefs).toHaveLength(1)
    expect((json.trace.authority as Record<string, unknown>).payoutAuthority).toBe(
      false,
    )
  })

  it('returns an unlisted trace to anyone with the link (no auth)', async () => {
    const store = makeMemoryStore()
    const uuid = await seedTrace(store, 'unlisted')
    const routes = makeDeps({ store })
    const response = await run(
      routes.routeTraceRequest(readRequest(uuid), ENV, CTX),
    )
    expect(response.status).toBe(200)
  })

  it('404s an owner_only trace for an anonymous caller', async () => {
    const store = makeMemoryStore()
    const uuid = await seedTrace(store, 'owner_only')
    const routes = makeDeps({ store, browserSession: undefined })
    const response = await run(
      routes.routeTraceRequest(readRequest(uuid), ENV, CTX),
    )
    expect(response.status).toBe(404)
  })

  it('404s an owner_only trace for a non-owner session', async () => {
    const store = makeMemoryStore()
    const uuid = await seedTrace(store, 'owner_only', 'agent-1')
    const routes = makeDeps({
      store,
      browserSession: { user: { userId: 'someone-else' } },
    })
    const response = await run(
      routes.routeTraceRequest(readRequest(uuid), ENV, CTX),
    )
    expect(response.status).toBe(404)
  })

  it('returns an owner_only trace to its owner', async () => {
    const store = makeMemoryStore()
    const uuid = await seedTrace(store, 'owner_only', 'agent-1')
    const routes = makeDeps({
      store,
      browserSession: { user: { userId: 'agent-1' } },
    })
    const response = await run(
      routes.routeTraceRequest(readRequest(uuid), ENV, CTX),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as { trace: Record<string, unknown> }
    expect(json.trace.uuid).toBe(uuid)
  })

  it('returns an owner_only trace to an admin', async () => {
    const store = makeMemoryStore()
    const uuid = await seedTrace(store, 'owner_only', 'agent-1')
    const routes = makeDeps({
      store,
      browserSession: { user: { userId: 'admin', email: 'chris@openagents.com' } },
      adminEmails: ['chris@openagents.com'],
    })
    const response = await run(
      routes.routeTraceRequest(readRequest(uuid), ENV, CTX),
    )
    expect(response.status).toBe(200)
  })

  it('404s an unknown uuid', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store })
    const response = await run(
      routes.routeTraceRequest(readRequest('does-not-exist'), ENV, CTX),
    )
    expect(response.status).toBe(404)
  })
})

describe('GET /api/traces owner list', () => {
  it('lists the owner\'s own traces', async () => {
    const store = makeMemoryStore()
    await seedTrace(store, 'public', 'agent-1')
    await seedTrace(store, 'owner_only', 'agent-1')
    await store.createTrace({
      traceUuid: 'other',
      ownerUserId: 'agent-2',
      agentRef: 'agent:agent-2',
      schemaVersion: ATIF_PINNED_SCHEMA_VERSION,
      trajectoryId: 't',
      sessionId: null,
      visibility: 'public',
      stepCount: 1,
      trajectory: {},
      blobRefs: [],
      idempotencyKey: null,
      nowIso: NOW,
    })
    const routes = makeDeps({
      store,
      browserSession: { user: { userId: 'agent-1' } },
    })
    const response = await run(
      routes.routeTraceRequest(
        new Request('https://openagents.com/api/traces'),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as { traces: ReadonlyArray<unknown> }
    expect(json.traces).toHaveLength(2)
  })

  it('401s an anonymous owner list', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, browserSession: undefined })
    const response = await run(
      routes.routeTraceRequest(
        new Request('https://openagents.com/api/traces'),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(401)
  })
})
