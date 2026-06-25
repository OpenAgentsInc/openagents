import { describe, expect, it } from 'vitest'

import {
  atifTraceTripwire,
  decodeAtifTrajectorySync,
  validateAtifTrajectory,
} from '../atif-trace-schema'
import type {
  CreateTraceInput,
  CreateTraceResult,
  TraceRecord,
  TraceStore,
} from '../trace-store-d1'
import type { InferenceResult } from './provider-adapter'
import {
  type CompletedKhalaChatSession,
  KHALA_TRACE_MODEL_NAME,
  emitKhalaChatTrace,
  isKhalaChatTraceEmitEnabled,
  khalaChatSessionToAtifTrajectory,
  resolveKhalaChatTraceOptIn,
  resolveTraceDemandColumns,
} from './khala-chat-trace-emitter'

const fakeResult = (
  overrides?: Partial<InferenceResult>,
): InferenceResult => ({
  content: 'Here is the answer to your question.',
  finishReason: 'stop',
  servedModel: 'vertex/gemini-2.5-flash',
  usage: { promptTokens: 42, completionTokens: 17, totalTokens: 59 },
  ...overrides,
})

const fakeSession = (
  overrides?: Partial<CompletedKhalaChatSession>,
): CompletedKhalaChatSession => ({
  requestedModel: 'openagents/khala',
  requestMessages: [
    { role: 'system', content: 'You are Khala, the OpenAgents agent.' },
    { role: 'user', content: 'What is 2 + 2?' },
  ],
  result: fakeResult(),
  responseId: 'chatcmpl-abc123',
  ...overrides,
})

// A minimal in-memory TraceStore stub. Only `createTrace` is exercised by the
// emitter; the rest are present to satisfy the interface.
const makeFakeStore = (): TraceStore & {
  readonly created: Array<CreateTraceInput>
} => {
  const created: Array<CreateTraceInput> = []
  const recordFromInput = (input: CreateTraceInput): TraceRecord => ({
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
    demandKind: input.demandKind,
    demandSource: input.demandSource,
    createdAt: input.nowIso,
    updatedAt: input.nowIso,
  })
  return {
    created,
    createTrace: async (input): Promise<CreateTraceResult> => {
      // Mirror the real D1 store's idempotency contract: a repeat
      // (ownerUserId, idempotencyKey) returns the existing record unchanged.
      if (input.idempotencyKey !== null && input.idempotencyKey !== undefined) {
        const existing = created.find(
          c =>
            c.ownerUserId === input.ownerUserId &&
            c.idempotencyKey === input.idempotencyKey,
        )
        if (existing !== undefined) {
          return { record: recordFromInput(existing), created: false }
        }
      }
      created.push(input)
      return { record: recordFromInput(input), created: true }
    },
    readTraceByUuid: async () => undefined,
    listTracesForOwner: async () => [],
    findTraceByOwnerDigest: async () => undefined,
    countTracesForOwnerSince: async () => 0,
    listTracesForOwnerByDemand: async () => [],
    countTracesForOwnerByDemand: async () => ({
      external: 0,
      internal: 0,
      own_capacity: 0,
      unlabeled: 0,
    }),
  }
}

describe('khalaChatSessionToAtifTrajectory', () => {
  it('maps a completed session to a VALID, public-safe ATIF trajectory', () => {
    const trajectory = khalaChatSessionToAtifTrajectory(fakeSession())

    // Decodes against the pinned schema (round-trips through the boundary).
    expect(() => decodeAtifTrajectorySync(trajectory)).not.toThrow()

    // Passes the SAME structural validator + tripwire the ingest route runs.
    expect(validateAtifTrajectory(trajectory)).toHaveLength(0)
    expect(atifTraceTripwire(trajectory)).toHaveLength(0)
  })

  it('projects agent.model_name to openagents/khala (never a raw backend)', () => {
    const trajectory = khalaChatSessionToAtifTrajectory(
      fakeSession({
        result: fakeResult({ servedModel: 'fireworks/raw-secret-backend' }),
      }),
    )
    expect(trajectory.agent.model_name).toBe('openagents/khala')
    expect(trajectory.agent.model_name).toBe(KHALA_TRACE_MODEL_NAME)
    // The raw served backend id is NOT present anywhere in the projection.
    expect(JSON.stringify(trajectory)).not.toContain('fireworks/raw-secret-backend')
    // The assistant step is also attributed to the gateway model.
    const assistantStep = trajectory.steps[trajectory.steps.length - 1]
    expect(assistantStep?.source).toBe('agent')
    expect(assistantStep?.model_name).toBe('openagents/khala')
  })

  it('drops gateway-injected system scaffolding but keeps the conversation', () => {
    const trajectory = khalaChatSessionToAtifTrajectory(fakeSession())
    // system (Khala identity) dropped; user + final assistant kept => 2 steps.
    expect(trajectory.steps).toHaveLength(2)
    expect(trajectory.steps[0]?.source).toBe('user')
    expect(trajectory.steps[0]?.message).toBe('What is 2 + 2?')
    // step_id is sequential from 1 after dropping.
    expect(trajectory.steps.map(step => step.step_id)).toEqual([1, 2])
  })
})

describe('emitKhalaChatTrace gating', () => {
  it('is an honest no-op when the flag is OFF (no store call)', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: false,
      optedIn: true,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
    })
    expect(result).toEqual({ emitted: false, reason: 'disabled' })
    expect(store.created).toHaveLength(0)
  })

  it('is a no-op when the request did not opt in', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: false,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
    })
    expect(result).toEqual({ emitted: false, reason: 'not_opted_in' })
    expect(store.created).toHaveLength(0)
  })

  it('is a no-op for a non-Khala model', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(
      fakeSession({ requestedModel: 'gpt-4o' }),
      {
        enabled: true,
        optedIn: true,
        store,
        owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
      },
    )
    expect(result).toEqual({ emitted: false, reason: 'not_khala' })
    expect(store.created).toHaveLength(0)
  })

  it('is a no-op when no owner is resolved', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: true,
      store,
      owner: undefined,
    })
    expect(result).toEqual({ emitted: false, reason: 'no_owner' })
    expect(store.created).toHaveLength(0)
  })
})

describe('emitKhalaChatTrace persistence (flag ON, opted in)', () => {
  it('persists via the trace store and returns a uuid + /trace url', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: true,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
      makeId: () => 'fixed-uuid-0001',
      nowIso: () => '2026-06-24T00:00:00.000Z',
    })
    expect(result.emitted).toBe(true)
    if (result.emitted) {
      expect(result.uuid).toBe('fixed-uuid-0001')
      expect(result.url).toBe('/trace/fixed-uuid-0001')
      // Redaction always runs on the capture path; a clean session reports zero.
      expect(result.redactionReport?.total).toBe(0)
    }
    expect(store.created).toHaveLength(1)
    const stored = store.created[0]!
    expect(stored.ownerUserId).toBe('u1')
    expect(stored.agentRef).toBe('agent:u1')
    expect(stored.visibility).toBe('unlisted')
    expect(stored.uploadSource).toBe('agent')
    // Data-market reward stays INERT on the gateway emit path.
    expect(stored.trainingConsent).toBe(false)
    expect(stored.rewardEligible).toBe(false)
    expect(stored.rewardAmountSats).toBeNull()
    // Idempotency keyed by the chat response id.
    expect(stored.idempotencyKey).toBe('chatcmpl-abc123')
    // The stored trajectory is the gateway projection.
    const trajectory = stored.trajectory as { agent: { model_name: string } }
    expect(trajectory.agent.model_name).toBe('openagents/khala')
  })

  it('REDACTS (then stores) a session that would otherwise trip the tripwire', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(
      fakeSession({
        result: fakeResult({
          // A real bearer-token VALUE plus an email + key. Under default-on
          // capture these are SCRUBBED before the tripwire (redact-before-trip),
          // so the trace is captured-and-safe rather than dropped.
          content:
            'Use header Authorization: Bearer abcdef0123456789ABCDEF and mail bob@example.com key sk-abcdefghijklmnop0123456789ABCD',
        }),
      }),
      {
        enabled: true,
        optedIn: true,
        store,
        owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
        makeId: () => 'fixed-uuid-redacted',
      },
    )
    expect(result.emitted).toBe(true)
    expect(store.created).toHaveLength(1)
    const stored = store.created[0]!
    const serialized = JSON.stringify(stored.trajectory)
    // NOTHING sensitive survives in the stored trajectory.
    expect(serialized).not.toContain('abcdef0123456789ABCDEF')
    expect(serialized).not.toContain('bob@example.com')
    expect(serialized).not.toContain('sk-abcdefghijklmnop')
    expect(serialized).toContain('[REDACTED:')
    if (result.emitted) {
      expect((result.redactionReport?.total ?? 0)).toBeGreaterThanOrEqual(3)
    }
  })

  it('returns store_error (never throws) when the store fails', async () => {
    const failingStore: TraceStore = {
      createTrace: async () => {
        throw new Error('d1 down')
      },
      readTraceByUuid: async () => undefined,
      listTracesForOwner: async () => [],
      findTraceByOwnerDigest: async () => undefined,
      countTracesForOwnerSince: async () => 0,
      listTracesForOwnerByDemand: async () => [],
      countTracesForOwnerByDemand: async () => ({
        external: 0,
        internal: 0,
        own_capacity: 0,
        unlabeled: 0,
      }),
    }
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: true,
      store: failingStore,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
    })
    expect(result.emitted).toBe(false)
    if (!result.emitted) {
      expect(result.reason).toBe('store_error')
    }
  })
})

describe('default-on free-tier capture (#6293/#6294)', () => {
  it('captures WITHOUT a per-request opt-in when captureDefault is true, stored owner_only', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: false,
      captureDefault: true,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
      makeId: () => 'fixed-uuid-auto',
    })
    expect(result.emitted).toBe(true)
    expect(store.created).toHaveLength(1)
    // PRIVATE-BY-DEFAULT: an auto-captured trace is owner_only, not unlisted.
    expect(store.created[0]!.visibility).toBe('owner_only')
  })

  it('an EXPLICIT opt-in is stored unlisted (shareable link), not owner_only', async () => {
    const store = makeFakeStore()
    await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: true,
      captureDefault: true,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
    })
    expect(store.created[0]!.visibility).toBe('unlisted')
  })

  it('is a no-op (not_opted_in) when neither opted in NOR captureDefault', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: false,
      captureDefault: false,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
    })
    expect(result).toEqual({ emitted: false, reason: 'not_opted_in' })
    expect(store.created).toHaveLength(0)
  })

  it('the MASTER flag still hard-disables even with captureDefault on', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: false,
      optedIn: false,
      captureDefault: true,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
    })
    expect(result).toEqual({ emitted: false, reason: 'disabled' })
    expect(store.created).toHaveLength(0)
  })

  it('idempotent per responseId on the capture-default path', async () => {
    const store = makeFakeStore()
    const deps = {
      enabled: true,
      optedIn: false,
      captureDefault: true,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' as const },
    }
    const a = await emitKhalaChatTrace(fakeSession(), deps)
    const b = await emitKhalaChatTrace(fakeSession(), deps)
    expect(a.emitted).toBe(true)
    expect(b.emitted).toBe(true)
    // Same responseId => the store dedups on idempotencyKey; one row.
    expect(store.created).toHaveLength(1)
  })
})

describe('flag + opt-in parsers', () => {
  it('isKhalaChatTraceEmitEnabled defaults OFF', () => {
    expect(isKhalaChatTraceEmitEnabled(undefined)).toBe(false)
    expect(isKhalaChatTraceEmitEnabled('')).toBe(false)
    expect(isKhalaChatTraceEmitEnabled('false')).toBe(false)
    expect(isKhalaChatTraceEmitEnabled('on')).toBe(true)
    expect(isKhalaChatTraceEmitEnabled('1')).toBe(true)
    expect(isKhalaChatTraceEmitEnabled('TRUE')).toBe(true)
  })

  it('resolveKhalaChatTraceOptIn reads the header and body switch', () => {
    expect(
      resolveKhalaChatTraceOptIn({
        request: new Request('https://x/', {
          headers: { 'x-oa-emit-trace': 'on' },
        }),
        rawBody: {},
      }),
    ).toBe(true)
    expect(
      resolveKhalaChatTraceOptIn({
        request: new Request('https://x/'),
        rawBody: { oa_emit_trace: true },
      }),
    ).toBe(true)
    expect(
      resolveKhalaChatTraceOptIn({
        request: new Request('https://x/'),
        rawBody: {},
      }),
    ).toBe(false)
  })
})

describe('demand-origin attribution on a captured trace (#6298)', () => {
  it('persists the threaded demand_kind + demand_source on the stored trace', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: true,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
      demandAttribution: { demandKind: 'internal', demandSource: 'heartbeat' },
    })
    expect(result.emitted).toBe(true)
    expect(store.created).toHaveLength(1)
    expect(store.created[0]!.demandKind).toBe('internal')
    expect(store.created[0]!.demandSource).toBe('heartbeat')
  })

  it('an internal-tagged request => trace demand_kind=internal', async () => {
    const store = makeFakeStore()
    await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      captureDefault: true,
      optedIn: false,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
      demandAttribution: {
        demandKind: 'internal',
        demandSource: 'harbor_terminal_bench',
      },
    })
    expect(store.created[0]!.demandKind).toBe('internal')
    expect(store.created[0]!.demandSource).toBe('harbor_terminal_bench')
  })

  it('an external completion with no header => demand_kind=external', async () => {
    const store = makeFakeStore()
    await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: true,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
      // The chat path resolves a header-less external request to `external`.
      demandAttribution: { demandKind: 'external' },
    })
    expect(store.created[0]!.demandKind).toBe('external')
    expect(store.created[0]!.demandSource).toBeNull()
  })

  it('FAIL-SOFT: missing attribution => unlabeled, capture still succeeds', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(fakeSession(), {
      enabled: true,
      optedIn: true,
      store,
      owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
      // No demandAttribution at all.
    })
    expect(result.emitted).toBe(true)
    expect(store.created[0]!.demandKind).toBe('unlabeled')
    expect(store.created[0]!.demandSource).toBeNull()
  })

  it('resolveTraceDemandColumns is fail-soft over bad/missing input', () => {
    expect(resolveTraceDemandColumns(undefined)).toEqual({
      demandKind: 'unlabeled',
      demandSource: null,
    })
    // An unbounded / malformed source is dropped to null.
    expect(
      resolveTraceDemandColumns({
        demandKind: 'internal',
        demandSource: 'has spaces and !!!',
      }),
    ).toEqual({ demandKind: 'internal', demandSource: null })
    // A bounded source slug is kept.
    expect(
      resolveTraceDemandColumns({
        demandKind: 'own_capacity',
        demandSource: 'khala_coding_delegation',
      }),
    ).toEqual({
      demandKind: 'own_capacity',
      demandSource: 'khala_coding_delegation',
    })
  })
})
