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
  TraceMediaBlobObject,
  TraceMediaBlobStore,
  TraceRecord,
  TraceStore,
  TraceTrajectoryBlobStore,
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
    trajectoryR2Key: input.trajectoryR2Key,
    blobRefs: input.blobRefs,
    idempotencyKey: input.idempotencyKey,
    trainingConsent: input.trainingConsent,
    license: input.license,
    contentDigest: input.contentDigest,
    rewardEligible: input.rewardEligible,
    rewardAmountSats: input.rewardAmountSats,
    uploadSource: input.uploadSource,
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
    findTraceByOwnerDigest: (ownerUserId, contentDigest) =>
      Promise.resolve(
        [...rows.values()].find(
          row =>
            row.ownerUserId === ownerUserId &&
            row.contentDigest === contentDigest,
        ),
      ),
    countTracesForOwnerSince: (ownerUserId, sinceIso) =>
      Promise.resolve(
        [...rows.values()].filter(
          row => row.ownerUserId === ownerUserId && row.createdAt >= sinceIso,
        ).length,
      ),
  }
}

type Session = Readonly<{ user: { email?: string; userId: string } }>

// In-memory trajectory blob store double (R2 stand-in for the large-trace path).
const makeMemoryBlobStore = (): TraceTrajectoryBlobStore & {
  objects: Map<string, string>
} => {
  const objects = new Map<string, string>()
  return {
    objects,
    putTrajectory: (traceUuid, json) => {
      const key = `traces/${traceUuid}/trajectory.json`
      objects.set(key, json)
      return Promise.resolve(key)
    },
    getTrajectory: key => Promise.resolve(objects.get(key) ?? null),
  }
}

// In-memory media blob store double (R2 stand-in for the #6223 media path).
const makeMemoryMediaStore = (): TraceMediaBlobStore & {
  objects: Map<string, { bytes: Uint8Array; contentType: string | undefined }>
} => {
  const objects = new Map<
    string,
    { bytes: Uint8Array; contentType: string | undefined }
  >()
  const keyFor = (uuid: string, r2Key: string) => `trace-blobs/${uuid}/${r2Key}`
  return {
    objects,
    putBlob: (uuid, r2Key, bytes, contentType) => {
      const key = keyFor(uuid, r2Key)
      const buf =
        bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes)
      objects.set(key, { bytes: buf, contentType })
      return Promise.resolve(key)
    },
    getBlob: (uuid, r2Key) => {
      const stored = objects.get(keyFor(uuid, r2Key))
      if (stored === undefined) {
        return Promise.resolve(null)
      }
      const object: TraceMediaBlobObject = {
        body: new Response(stored.bytes.buffer as ArrayBuffer).body as ReadableStream,
        size: stored.bytes.byteLength,
        contentType: stored.contentType,
        httpEtag: `"etag-${r2Key}"`,
      }
      return Promise.resolve(object)
    },
  }
}

let idCounter = 0

const makeDeps = (options: {
  store: TraceStore
  agentUserId?: string | undefined
  browserSession?: Session | undefined
  adminEmails?: ReadonlyArray<string>
  rewardArmed?: boolean
  uniqueIds?: boolean
  blobStore?: TraceTrajectoryBlobStore
  mediaStore?: TraceMediaBlobStore
}) =>
  makeTraceStoreRoutes<Bindings, Session>({
    agentStore: () => makeAgentStore(options.agentUserId),
    appendRefreshedSessionCookies: response => response,
    dataMarketRewardArmed: () => options.rewardArmed ?? false,
    isAdminEmail: email => (options.adminEmails ?? []).includes(email),
    makeStore: () => options.store,
    makeId: () =>
      options.uniqueIds === true
        ? `trace-uuid-${(idCounter += 1)}`
        : 'trace-uuid-fixed',
    nowIso: () => NOW,
    requireBrowserSession: () => Promise.resolve(options.browserSession),
    ...(options.blobStore === undefined
      ? {}
      : { trajectoryBlobStore: () => options.blobStore }),
    ...(options.mediaStore === undefined
      ? {}
      : { mediaBlobStore: () => options.mediaStore }),
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

// A user web-session upload: NO agent bearer header (auth is the browser
// session), at `/api/traces/upload` (#6221 variant) or `/api/traces`.
const userUploadRequest = (
  body: unknown,
  options: { path?: string; headers?: Record<string, string> } = {},
): Request =>
  new Request(`https://openagents.com${options.path ?? '/api/traces'}`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'idempotency-key': 'user-idem-1',
      ...(options.headers ?? {}),
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

  it('ACCEPTS a model id (content, not a leak) and stores the trace', async () => {
    // The model that ran is trace content (a user-uploaded Claude Code session's
    // model IS claude-*). The openagents/khala-only rule is a Khala GATEWAY
    // invariant, not a trace one — the tripwire only rejects real value-leaks.
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const withModelId = {
      ...cleanTrajectory(),
      agent: { name: 'a', version: '1', model_name: 'claude-opus-4' },
    }
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: withModelId }),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(201)
    expect(store.rows.size).toBe(1)
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

const dataMarketBlock = (
  json: Record<string, unknown>,
): Record<string, unknown> => json.dataMarket as Record<string, unknown>

describe('trace data market (#6221)', () => {
  it('lets an authenticated user web session upload a trace they own', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({
      store,
      // no agent bearer: a human web session
      browserSession: { user: { userId: 'user-7', email: 'u@x.com' } },
    })
    const response = await run(
      routes.routeTraceRequest(
        userUploadRequest(
          { trajectory: cleanTrajectory(), trainingConsent: true },
          { path: '/api/traces/upload' },
        ),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(201)
    const stored = store.rows.get('trace-uuid-fixed')
    expect(stored?.ownerUserId).toBe('user-7')
    expect(stored?.uploadSource).toBe('user_session')
    expect(stored?.agentRef).toBe('user:user-7')
  })

  it('rejects an unauthenticated user upload with 401', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, browserSession: undefined })
    const response = await run(
      routes.routeTraceRequest(
        userUploadRequest({ trajectory: cleanTrajectory() }),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(401)
    expect(store.rows.size).toBe(0)
  })

  it('round-trips training_consent + license through store and read', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({
      store,
      browserSession: { user: { userId: 'user-7' } },
    })
    const upload = await run(
      routes.routeTraceRequest(
        userUploadRequest({
          trajectory: cleanTrajectory(),
          visibility: 'public',
          trainingConsent: true,
          license: 'CC-BY-4.0',
        }),
        ENV,
        CTX,
      ),
    )
    const uploadJson = (await upload.json()) as Record<string, unknown>
    expect(dataMarketBlock(uploadJson).trainingConsent).toBe(true)
    expect(dataMarketBlock(uploadJson).license).toBe('CC-BY-4.0')

    const read = await run(
      routes.routeTraceRequest(readRequest('trace-uuid-fixed'), ENV, CTX),
    )
    const readJson = (await read.json()) as { trace: Record<string, unknown> }
    const market = readJson.trace.dataMarket as Record<string, unknown>
    expect(market.trainingConsent).toBe(true)
    expect(market.license).toBe('CC-BY-4.0')
    expect(market.uploadSource).toBe('user_session')
  })

  it('defaults training_consent to withheld when not granted', async () => {
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
    expect(dataMarketBlock(json).trainingConsent).toBe(false)
    expect(store.rows.get('trace-uuid-fixed')?.trainingConsent).toBe(false)
  })

  it('records the revshare reward marker INERT (eligible-only, amount TBD, no money)', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({
      store,
      agentUserId: 'agent-1',
      rewardArmed: true,
    })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: cleanTrajectory(), trainingConsent: true }),
        ENV,
        CTX,
      ),
    )
    const json = (await response.json()) as Record<string, unknown>
    const reward = dataMarketBlock(json).reward as Record<string, unknown>
    expect(reward.eligible).toBe(true)
    // INERT: amount is always null ("reward TBD"); no money path.
    expect(reward.amountSats).toBeNull()
    expect(reward.status).toBe('tbd')
    expect(store.rows.get('trace-uuid-fixed')?.rewardEligible).toBe(true)
    expect(store.rows.get('trace-uuid-fixed')?.rewardAmountSats).toBeNull()
  })

  it('keeps reward ineligible when the data-market flag is not armed', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({
      store,
      agentUserId: 'agent-1',
      rewardArmed: false,
    })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: cleanTrajectory(), trainingConsent: true }),
        ENV,
        CTX,
      ),
    )
    const json = (await response.json()) as Record<string, unknown>
    const reward = dataMarketBlock(json).reward as Record<string, unknown>
    expect(reward.eligible).toBe(false)
  })

  it('keeps reward ineligible when consent is withheld even if armed', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({
      store,
      agentUserId: 'agent-1',
      rewardArmed: true,
    })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: cleanTrajectory(), trainingConsent: false }),
        ENV,
        CTX,
      ),
    )
    const json = (await response.json()) as Record<string, unknown>
    const reward = dataMarketBlock(json).reward as Record<string, unknown>
    expect(reward.eligible).toBe(false)
  })

  it('dedups a duplicate content digest with 409 (no double store, no double reward)', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({
      store,
      agentUserId: 'agent-1',
      uniqueIds: true,
    })
    const first = await run(
      routes.routeTraceRequest(
        ingestRequest(
          { trajectory: cleanTrajectory() },
          { 'idempotency-key': 'k-1' },
        ),
        ENV,
        CTX,
      ),
    )
    expect(first.status).toBe(201)
    // Same payload, DIFFERENT idempotency key -> content dedup rejects it.
    const second = await run(
      routes.routeTraceRequest(
        ingestRequest(
          { trajectory: cleanTrajectory() },
          { 'idempotency-key': 'k-2' },
        ),
        ENV,
        CTX,
      ),
    )
    expect(second.status).toBe(409)
    const json = (await second.json()) as Record<string, unknown>
    expect(json.error).toBe('trace_duplicate')
    expect(json.duplicate).toBe(true)
    expect(store.rows.size).toBe(1)
  })

  it('rate-limits per-user uploads with 429', async () => {
    const store = makeMemoryStore()
    // Pre-seed the owner to/above the rolling-window cap.
    for (let index = 0; index < 120; index += 1) {
      await store.createTrace({
        traceUuid: `seed-${index}`,
        ownerUserId: 'agent-1',
        agentRef: 'agent:agent-1',
        schemaVersion: ATIF_PINNED_SCHEMA_VERSION,
        trajectoryId: `t-${index}`,
        sessionId: null,
        visibility: 'unlisted',
        stepCount: 1,
        trajectory: {},
        trajectoryR2Key: null,
        blobRefs: [],
        idempotencyKey: null,
        trainingConsent: false,
        license: null,
        contentDigest: `seed-digest-${index}`,
        rewardEligible: false,
        rewardAmountSats: null,
        uploadSource: 'agent',
        nowIso: NOW,
      })
    }
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest(
          { trajectory: cleanTrajectory() },
          { 'idempotency-key': 'fresh' },
        ),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(429)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.error).toBe('trace_rate_limited')
  })
})

// A public-safe trajectory whose serialized JSON exceeds the ~768KB inline
// ceiling (a real full agent session is a few MB). Benign repeated prose only,
// so the tripwire does not fire. Stays under MAX_STEPS (2000).
const largeTrajectory = () => {
  const chunk = 'the agent reviewed the code and continued the task. '.repeat(40)
  const steps = Array.from({ length: 1500 }, (_value, index) => ({
    step_id: index + 1,
    source: index % 2 === 0 ? ('user' as const) : ('agent' as const),
    message: `step ${index + 1}: ${chunk}`,
  }))
  return {
    schema_version: ATIF_PINNED_SCHEMA_VERSION,
    trajectory_id: 'traj-large',
    agent: { name: 'Raynor', version: '1.0.0', model_name: 'openagents/khala' },
    steps,
  }
}

describe('large trajectory R2 offload (#6221)', () => {
  it('offloads a >768KB trajectory to R2 and keeps a pointer + placeholder in D1', async () => {
    const store = makeMemoryStore()
    const blobStore = makeMemoryBlobStore()
    const trajectory = largeTrajectory()
    const bytes = new TextEncoder().encode(JSON.stringify(trajectory)).length
    expect(bytes).toBeGreaterThan(768 * 1024)

    const routes = makeDeps({ store, agentUserId: 'agent-1', blobStore })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory, visibility: 'public' }),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(201)
    const stored = store.rows.get('trace-uuid-fixed')
    // D1 holds only a pointer + placeholder; R2 holds the real JSON.
    expect(stored?.trajectoryR2Key).toBe(
      'traces/trace-uuid-fixed/trajectory.json',
    )
    expect(stored?.trajectory).toEqual({})
    expect(blobStore.objects.has('traces/trace-uuid-fixed/trajectory.json')).toBe(
      true,
    )
  })

  it('rehydrates the full trajectory from R2 on read', async () => {
    const store = makeMemoryStore()
    const blobStore = makeMemoryBlobStore()
    const trajectory = largeTrajectory()
    const routes = makeDeps({ store, agentUserId: 'agent-1', blobStore })
    await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory, visibility: 'public' }),
        ENV,
        CTX,
      ),
    )
    const read = await run(
      routes.routeTraceRequest(readRequest('trace-uuid-fixed'), ENV, CTX),
    )
    expect(read.status).toBe(200)
    const json = (await read.json()) as {
      trace: { trajectory: { steps: ReadonlyArray<unknown> } }
    }
    // The read projection is identical to an inline trace: full trajectory back.
    expect(json.trace.trajectory.steps).toHaveLength(1500)
  })

  it('413s a too-large trajectory when no R2 store is configured', async () => {
    const store = makeMemoryStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1' })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: largeTrajectory() }),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(413)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.error).toBe('trace_payload_too_large')
    expect(store.rows.size).toBe(0)
  })

  it('stores a small trajectory inline (no R2 pointer) even when R2 is available', async () => {
    const store = makeMemoryStore()
    const blobStore = makeMemoryBlobStore()
    const routes = makeDeps({ store, agentUserId: 'agent-1', blobStore })
    const response = await run(
      routes.routeTraceRequest(
        ingestRequest({ trajectory: cleanTrajectory() }),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(201)
    const stored = store.rows.get('trace-uuid-fixed')
    expect(stored?.trajectoryR2Key).toBeNull()
    expect(blobStore.objects.size).toBe(0)
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
    trajectoryR2Key: null,
    blobRefs: [{ kind: 'video', r2Key: 'traces/uuid/video.mp4' }],
    idempotencyKey: null,
    trainingConsent: true,
    license: 'CC-BY-4.0',
    contentDigest: `digest-${visibility}-${ownerUserId}`,
    rewardEligible: false,
    rewardAmountSats: null,
    uploadSource: 'agent',
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
      trajectoryR2Key: null,
      blobRefs: [],
      idempotencyKey: null,
      trainingConsent: false,
      license: null,
      contentDigest: 'digest-other',
      rewardEligible: false,
      rewardAmountSats: null,
      uploadSource: 'agent',
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

// ---------------------------------------------------------------------------
// Trace media blobs (#6223): self-hosted recording + screenshots.
// ---------------------------------------------------------------------------

const seedTraceWithBlobs = async (
  store: TraceStore,
  visibility: 'public' | 'unlisted' | 'owner_only',
  blobRefs: TraceRecord['blobRefs'],
  ownerUserId = 'agent-1',
): Promise<string> => {
  const result = await store.createTrace({
    traceUuid: `uuid-blob-${visibility}`,
    ownerUserId,
    agentRef: `agent:${ownerUserId}`,
    schemaVersion: ATIF_PINNED_SCHEMA_VERSION,
    trajectoryId: 'traj-1',
    sessionId: null,
    visibility,
    stepCount: 2,
    trajectory: cleanTrajectory(),
    trajectoryR2Key: null,
    blobRefs,
    idempotencyKey: null,
    trainingConsent: true,
    license: null,
    contentDigest: `digest-blob-${visibility}-${ownerUserId}`,
    rewardEligible: false,
    rewardAmountSats: null,
    uploadSource: 'agent',
    nowIso: NOW,
  })
  return result.record.traceUuid
}

const blobServeRequest = (uuid: string, r2Key: string): Request =>
  new Request(`https://openagents.com/api/traces/${uuid}/blob/${r2Key}`)

const blobUploadRequest = (
  uuid: string,
  r2Key: string,
  bytes: Uint8Array,
  headers: Record<string, string> = { authorization: 'Bearer oa_agent_test' },
): Request =>
  new Request(`https://openagents.com/api/traces/${uuid}/blob/${r2Key}`, {
    method: 'POST',
    headers,
    body: bytes.buffer as ArrayBuffer,
  })

describe('GET /api/traces/{uuid}/blob/{r2Key} serve (#6223)', () => {
  it('streams the bytes for a public trace with the right Content-Type (no auth)', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    const bytes = new Uint8Array([0, 1, 2, 3, 4])
    await media.putBlob(uuid, 'session.mp4', bytes, 'video/mp4')
    const routes = makeDeps({ store, mediaStore: media })
    const response = await run(
      routes.routeTraceRequest(blobServeRequest(uuid, 'session.mp4'), ENV, CTX),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe('video/mp4')
    expect(response.headers.get('accept-ranges')).toBe('bytes')
    expect(response.headers.get('cache-control')).toContain('immutable')
    const served = new Uint8Array(await response.arrayBuffer())
    expect([...served]).toEqual([...bytes])
  })

  it('prefers the stored blobRef Content-Type over the R2 object metadata', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.webm', contentType: 'video/webm' },
    ])
    // R2 object has a different content type; the blobRef wins.
    await media.putBlob(uuid, 'session.webm', new Uint8Array([9]), 'application/octet-stream')
    const routes = makeDeps({ store, mediaStore: media })
    const response = await run(
      routes.routeTraceRequest(blobServeRequest(uuid, 'session.webm'), ENV, CTX),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe('video/webm')
  })

  it('serves a screenshot whose r2Key has a slash path segment', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'screenshot', r2Key: 'shots/00-login.png', contentType: 'image/png' },
    ])
    await media.putBlob(uuid, 'shots/00-login.png', new Uint8Array([7, 7]), 'image/png')
    const routes = makeDeps({ store, mediaStore: media })
    const response = await run(
      routes.routeTraceRequest(
        blobServeRequest(uuid, 'shots/00-login.png'),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe('image/png')
  })

  it('404s when the blob bytes are missing (trace exists, no R2 object)', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    // never uploaded the bytes
    const routes = makeDeps({ store, mediaStore: media })
    const response = await run(
      routes.routeTraceRequest(blobServeRequest(uuid, 'session.mp4'), ENV, CTX),
    )
    expect(response.status).toBe(404)
  })

  it('404s when the trace does not exist', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const routes = makeDeps({ store, mediaStore: media })
    const response = await run(
      routes.routeTraceRequest(blobServeRequest('nope', 'session.mp4'), ENV, CTX),
    )
    expect(response.status).toBe(404)
  })

  it('is visibility-gated: 404s an owner_only blob for an anonymous caller', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'owner_only', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    await media.putBlob(uuid, 'session.mp4', new Uint8Array([1]), 'video/mp4')
    const routes = makeDeps({ store, mediaStore: media, browserSession: undefined })
    const response = await run(
      routes.routeTraceRequest(blobServeRequest(uuid, 'session.mp4'), ENV, CTX),
    )
    expect(response.status).toBe(404)
  })

  it('is visibility-gated: 404s an owner_only blob for a non-owner session', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'owner_only', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    await media.putBlob(uuid, 'session.mp4', new Uint8Array([1]), 'video/mp4')
    const routes = makeDeps({
      store,
      mediaStore: media,
      browserSession: { user: { userId: 'someone-else' } },
    })
    const response = await run(
      routes.routeTraceRequest(blobServeRequest(uuid, 'session.mp4'), ENV, CTX),
    )
    expect(response.status).toBe(404)
  })

  it('is visibility-gated: serves an owner_only blob to its owner (private cache)', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'owner_only', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    await media.putBlob(uuid, 'session.mp4', new Uint8Array([1, 2]), 'video/mp4')
    const routes = makeDeps({
      store,
      mediaStore: media,
      browserSession: { user: { userId: 'agent-1' } },
    })
    const response = await run(
      routes.routeTraceRequest(blobServeRequest(uuid, 'session.mp4'), ENV, CTX),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('private')
  })

  it('404s when no media store is configured', async () => {
    const store = makeMemoryStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    const routes = makeDeps({ store }) // no mediaStore
    const response = await run(
      routes.routeTraceRequest(blobServeRequest(uuid, 'session.mp4'), ENV, CTX),
    )
    expect(response.status).toBe(404)
  })
})

describe('POST /api/traces/{uuid}/blob/{r2Key} upload (#6223)', () => {
  it('stores the bytes for a declared blobRef by the owner', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    const routes = makeDeps({ store, mediaStore: media, agentUserId: 'agent-1' })
    const bytes = new Uint8Array([5, 6, 7, 8])
    const response = await run(
      routes.routeTraceRequest(
        blobUploadRequest(uuid, 'session.mp4', bytes),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(201)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.bytes).toBe(4)
    expect(json.r2Key).toBe('session.mp4')
    const stored = media.objects.get(`trace-blobs/${uuid}/session.mp4`)
    expect(stored).toBeDefined()
    expect([...(stored?.bytes ?? [])]).toEqual([...bytes])
  })

  it('rejects an r2Key not declared on the trace blobRefs with 400', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    const routes = makeDeps({ store, mediaStore: media, agentUserId: 'agent-1' })
    const response = await run(
      routes.routeTraceRequest(
        blobUploadRequest(uuid, 'evil.sh', new Uint8Array([1])),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(400)
    expect(media.objects.size).toBe(0)
  })

  it('404s an upload from a non-owner', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ], 'agent-1')
    const routes = makeDeps({ store, mediaStore: media, agentUserId: 'agent-2' })
    const response = await run(
      routes.routeTraceRequest(
        blobUploadRequest(uuid, 'session.mp4', new Uint8Array([1])),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(404)
    expect(media.objects.size).toBe(0)
  })

  it('rejects an unauthenticated upload with 401', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    const routes = makeDeps({ store, mediaStore: media, agentUserId: undefined })
    const response = await run(
      routes.routeTraceRequest(
        blobUploadRequest(uuid, 'session.mp4', new Uint8Array([1]), {}),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(401)
  })

  it('rejects an empty upload body with 400', async () => {
    const store = makeMemoryStore()
    const media = makeMemoryMediaStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    const routes = makeDeps({ store, mediaStore: media, agentUserId: 'agent-1' })
    const response = await run(
      routes.routeTraceRequest(
        blobUploadRequest(uuid, 'session.mp4', new Uint8Array([])),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(400)
  })

  it('501s an upload when no media store is configured', async () => {
    const store = makeMemoryStore()
    const uuid = await seedTraceWithBlobs(store, 'public', [
      { kind: 'video', r2Key: 'session.mp4', contentType: 'video/mp4' },
    ])
    const routes = makeDeps({ store, agentUserId: 'agent-1' }) // no mediaStore
    const response = await run(
      routes.routeTraceRequest(
        blobUploadRequest(uuid, 'session.mp4', new Uint8Array([1])),
        ENV,
        CTX,
      ),
    )
    expect(response.status).toBe(501)
  })
})
