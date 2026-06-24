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
    createdAt: input.nowIso,
    updatedAt: input.nowIso,
  })
  return {
    created,
    createTrace: async (input): Promise<CreateTraceResult> => {
      created.push(input)
      return { record: recordFromInput(input), created: true }
    },
    readTraceByUuid: async () => undefined,
    listTracesForOwner: async () => [],
    findTraceByOwnerDigest: async () => undefined,
    countTracesForOwnerSince: async () => 0,
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
    expect(result).toEqual({
      emitted: true,
      uuid: 'fixed-uuid-0001',
      url: '/trace/fixed-uuid-0001',
    })
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

  it('rejects (does not store) a session that trips the public-safety tripwire', async () => {
    const store = makeFakeStore()
    const result = await emitKhalaChatTrace(
      fakeSession({
        result: fakeResult({
          // A real bearer-token VALUE: caught by the SAME tripwire the ingest
          // route runs, so it is rejected before persistence.
          content: 'Use header Authorization: Bearer abcdef0123456789ABCDEF',
        }),
      }),
      {
        enabled: true,
        optedIn: true,
        store,
        owner: { ownerUserId: 'u1', agentRef: 'agent:u1', uploadSource: 'agent' },
      },
    )
    expect(result.emitted).toBe(false)
    if (!result.emitted) {
      expect(result.reason).toBe('public_safety_rejected')
    }
    expect(store.created).toHaveLength(0)
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
