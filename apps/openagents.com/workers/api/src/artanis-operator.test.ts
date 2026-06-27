import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_OPERATOR_KHALA_MODEL,
  ARTANIS_OPERATOR_SYSTEM_PROMPT,
  type ArtanisMemoryEntry,
  type ArtanisOperatorKhalaClient,
  type ArtanisSituationalAwareness,
  artanisOperatorTurn,
  buildArtanisOperatorContextBlock,
  buildArtanisOperatorKhalaRequest,
  isArtanisOperatorPersonaClean,
  verifyArtanisOperatorPersona,
} from './artanis-operator'
import { KHALA_IDENTITY_SYSTEM_PROMPT } from './inference/khala-identity'
import {
  InferenceAdapterError,
  type InferenceRequest,
  type InferenceResult,
} from './inference/provider-adapter'

// Real owner-memory entries (most-recent-first, as the store returns them).
const exampleMemory: ReadonlyArray<ArtanisMemoryEntry> = [
  {
    body: 'Three assignments merged, four queued.',
    createdAt: '2026-06-25T10:00:05.000Z',
    kind: 'turn',
    memoryRef: 'mem-2',
    noteCategory: null,
    ownerId: 'owner:github:14167547',
    role: 'artanis',
  },
  {
    body: 'How is the burndown going?',
    createdAt: '2026-06-25T10:00:00.000Z',
    kind: 'turn',
    memoryRef: 'mem-1',
    noteCategory: null,
    ownerId: 'owner:github:14167547',
    role: 'owner',
  },
  {
    body: 'Owner prefers concise direct answers, no marketing copy.',
    createdAt: '2026-06-24T09:00:00.000Z',
    kind: 'note',
    memoryRef: 'mem-0',
    noteCategory: 'preference',
    ownerId: 'owner:github:14167547',
    role: null,
  },
]

// A real situational-awareness bundle (the shape buildArtanisSituationalAwareness
// returns).
const exampleAwareness: ArtanisSituationalAwareness = {
  generatedAt: '2026-06-26T12:00:00.000Z',
  goals: {
    epics: [
      {
        mandate: 'Own the Khala improvement loop autonomously.',
        number: 6359,
        title: 'Artanis: autonomous owner of the loop',
      },
    ],
    roadmapRef: 'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
    roadmapSummary: 'Master roadmap for the open Khala issue set.',
  },
  kind: 'artanis_situational_awareness',
  ongoingOps: {
    activeAssignments: [
      {
        assignmentRef: 'assignment.codex.abc123',
        phase: 'proof-ready',
        startedAt: '2026-06-26T11:50:00.000Z',
        state: 'accepted',
      },
    ],
    fleetReadiness: { readyReplicas: 3, status: 'ready', totalReplicas: 3 },
    publicCounter: null,
    recentDeploys: [],
  },
  ownerId: 'owner:github:14167547',
  ownerOnly: true,
  recentActions: {
    assignments: [],
    commits: [],
    issueChanges: [],
    ticks: [
      {
        assignmentRef: 'assignment.codex.abc123',
        at: '2026-06-26T11:55:00.000Z',
        decisionRef: 'tick.artanis.20260626115500',
        state: 'dispatched',
      },
    ],
  },
}

const emptyAwareness: ArtanisSituationalAwareness = {
  generatedAt: '2026-06-26T12:00:00.000Z',
  goals: {
    epics: [],
    roadmapRef: 'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
    roadmapSummary: 'Master roadmap for the open Khala issue set.',
  },
  kind: 'artanis_situational_awareness',
  ongoingOps: {
    activeAssignments: [],
    fleetReadiness: null,
    publicCounter: null,
    recentDeploys: [],
  },
  ownerId: 'owner:x',
  ownerOnly: true,
  recentActions: {
    assignments: [],
    commits: [],
    issueChanges: [],
    ticks: [],
  },
}

// A fake Khala client that records the request it was handed and returns a
// canned operator-style reply. This is the dogfood seam under test: the real
// client routes through the Khala pool, but the contract the core depends on is
// exactly this shape.
const makeRecordingKhalaClient = (
  reply: string,
): {
  client: ArtanisOperatorKhalaClient
  captured: { request: InferenceRequest | null }
} => {
  const captured: { request: InferenceRequest | null } = { request: null }
  const client: ArtanisOperatorKhalaClient = (request: InferenceRequest) => {
    captured.request = request
    const result: InferenceResult = {
      content: reply,
      finishReason: 'stop',
      servedModel: 'gpt-oss-120b',
      usage: { completionTokens: 12, promptTokens: 200, totalTokens: 212 },
    }
    return Effect.succeed(result)
  }
  return { captured, client }
}

const failingKhalaClient: ArtanisOperatorKhalaClient = () =>
  Effect.fail(
    new InferenceAdapterError({
      adapterId: 'test',
      httpStatus: 503,
      kind: 'service_overloaded',
      reason: 'test_failure',
      retryable: true,
    }),
  )

describe('artanis operator persona (NOT the public Khala identity)', () => {
  test('persona prompt is the Artanis operator agent, not the Khala collective', () => {
    // First-person operator agent.
    expect(ARTANIS_OPERATOR_SYSTEM_PROMPT).toContain('You are Artanis')
    expect(ARTANIS_OPERATOR_SYSTEM_PROMPT.toLowerCase()).toContain(
      'first person singular',
    )
    // Explicitly forbids the Khala collective voice + StarCraft roleplay.
    expect(ARTANIS_OPERATOR_SYSTEM_PROMPT.toLowerCase()).toContain('daelaam')
    expect(ARTANIS_OPERATOR_SYSTEM_PROMPT.toLowerCase()).toContain('hierarch')
    expect(ARTANIS_OPERATOR_SYSTEM_PROMPT.toLowerCase()).toContain(
      'not public khala chat',
    )
  })

  test('persona is a DISTINCT prompt from the public Khala identity', () => {
    // The two prompts must never be the same object/text — persona separation.
    expect(ARTANIS_OPERATOR_SYSTEM_PROMPT).not.toEqual(
      KHALA_IDENTITY_SYSTEM_PROMPT,
    )
    // The public Khala prompt mandates first-person PLURAL ("we are Khala");
    // the operator persona mandates first-person SINGULAR. They are opposites.
    expect(KHALA_IDENTITY_SYSTEM_PROMPT.toLowerCase()).toContain(
      'first-person plural',
    )
    expect(ARTANIS_OPERATOR_SYSTEM_PROMPT.toLowerCase()).toContain(
      'first person singular',
    )
  })

  test('persona guard accepts a clean operator reply', () => {
    const verdict = verifyArtanisOperatorPersona(
      'I dispatched two Codex assignments this morning and merged one PR.',
    )
    expect(verdict.satisfied).toBe(true)
    expect(verdict.leaks).toEqual([])
  })

  test('persona guard accepts legitimate "the Khala …" product references (#6363)', () => {
    // The operator persona is REQUIRED to talk about "the Khala improvement
    // loop" / "the Khala API" / "the Khala surface". The earlier bare "the
    // khala" guard term falsely failed these grounded replies. They must pass.
    for (const reply of [
      'I run the Khala improvement loop and keep the Khala API solid.',
      'My current goal is driving the Khala surface to ship.',
      'I dispatch and verify work through the Khala -> Pylon -> Codex burndown.',
    ]) {
      const verdict = verifyArtanisOperatorPersona(reply)
      expect(verdict.satisfied).toBe(true)
      expect(verdict.leaks).toEqual([])
    }
  })

  test('persona guard flags StarCraft / collective-intelligence roleplay leaks', () => {
    const verdict = verifyArtanisOperatorPersona(
      'En Taro Adun! I am Hierarch Artanis of the Daelaam, leading the Protoss.',
    )
    expect(verdict.satisfied).toBe(false)
    expect(verdict.leaks).toContain('hierarch')
    expect(verdict.leaks).toContain('daelaam')
    expect(verdict.leaks).toContain('protoss')
    expect(verdict.leaks).toContain('en taro')
  })

  test('persona guard flags the Khala collective "we are Khala" voice', () => {
    const verdict = verifyArtanisOperatorPersona(
      'We are Khala, a collective intelligence. How can we help you?',
    )
    expect(verdict.satisfied).toBe(false)
    expect(verdict.leaks).toContain('we are khala')
    expect(verdict.leaks).toContain('collective intelligence')
  })

  test('isArtanisOperatorPersonaClean is the boolean form of the guard', () => {
    expect(isArtanisOperatorPersonaClean('For the Daelaam!')).toBe(false)
    expect(
      isArtanisOperatorPersonaClean(
        'I merged the PR and opened a follow-up issue.',
      ),
    ).toBe(true)
  })
})

describe('artanis operator reasoning routes through the Khala API (dogfood)', () => {
  test('the built request targets the openagents/khala model alias', () => {
    const request = buildArtanisOperatorKhalaRequest({
      contextBlock: 'context',
      messages: [{ content: 'what are you doing?', role: 'user' }],
    })
    expect(request.model).toBe(ARTANIS_OPERATOR_KHALA_MODEL)
    expect(ARTANIS_OPERATOR_KHALA_MODEL).toBe('openagents/khala')
    expect(request.stream).toBe(false)
    // Persona + context system messages precede the owner conversation.
    expect(request.messages[0]?.role).toBe('system')
    expect(request.messages[0]?.content).toBe(ARTANIS_OPERATOR_SYSTEM_PROMPT)
    expect(request.messages[1]?.role).toBe('system')
    expect(request.messages[2]?.content).toBe('what are you doing?')
  })

  test('a turn calls the Khala client and reports servedVia openagents_khala', async () => {
    const { captured, client } = makeRecordingKhalaClient(
      'I have two Codex assignments running and merged one PR today.',
    )
    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'what are you doing?', role: 'user' }],
        ownerId: 'owner:github:14167547',
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.servedVia).toBe('openagents_khala')
    expect(result.requestedModel).toBe('openagents/khala')
    expect(result.servedModel).toBe('gpt-oss-120b')
    expect(result.persona.satisfied).toBe(true)
    // Proof the reasoning actually went to the Khala client with the alias.
    expect(captured.request?.model).toBe('openagents/khala')
  })

  test('a Khala client failure yields typed unavailability, never a provider fallback', async () => {
    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: failingKhalaClient,
        memory: exampleMemory,
        messages: [{ content: 'status?', role: 'user' }],
        ownerId: 'owner:github:14167547',
      }),
    )
    expect('error' in result).toBe(true)
    if ('error' in result) {
      expect(result.error).toBe('artanis_operator_mind_unavailable')
    }
  })
})

describe('artanis operator grounding (memory + situational awareness)', () => {
  test('context block injects memory preferences and prior turns', () => {
    const block = buildArtanisOperatorContextBlock({
      awareness: exampleAwareness,
      memory: exampleMemory,
    })
    expect(block).toContain(
      'Owner prefers concise direct answers, no marketing copy.',
    )
    expect(block).toContain('How is the burndown going?')
    expect(block).toContain('Three assignments merged, four queued.')
  })

  test('context block injects recent actions, goals, and ongoing ops', () => {
    const block = buildArtanisOperatorContextBlock({
      awareness: exampleAwareness,
      memory: exampleMemory,
    })
    // A recent tick action.
    expect(block).toContain('tick.artanis.20260626115500')
    expect(block).toContain('dispatched')
    // A current goal epic.
    expect(block).toContain('Own the Khala improvement loop autonomously.')
    expect(block).toContain('#6359')
    // An ongoing active assignment + fleet readiness.
    expect(block).toContain('assignment.codex.abc123')
    expect(block).toContain('3/3 replicas ready')
  })

  test('empty memory + awareness produce an honest "none recorded" block, not invented state', () => {
    const block = buildArtanisOperatorContextBlock({
      awareness: emptyAwareness,
      memory: [],
    })
    expect(block).toContain('(none recorded yet)')
    expect(block).toContain('this is our first recorded conversation')
    expect(block).toContain('(none active right now)')
    // Goals stay grounded even when empty: the roadmap ref is always present.
    expect(block).toContain(
      'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
    )
  })

  test('the grounded context block reaches the Khala request as a system message', async () => {
    const { captured, client } = makeRecordingKhalaClient('ok')
    await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'what are you doing?', role: 'user' }],
        ownerId: 'owner:github:14167547',
      }),
    )
    const systemMessages = (captured.request?.messages ?? []).filter(
      message => message.role === 'system',
    )
    const joined = systemMessages.map(message => message.content).join('\n')
    expect(joined).toContain('tick.artanis.20260626115500')
    expect(joined).toContain('How is the burndown going?')
  })
})

describe('artanis operator authority boundary (defers spend/destructive)', () => {
  test('a spend/destructive owner ask sets deferredToApprovalGate', async () => {
    const { client } = makeRecordingKhalaClient(
      'That involves spend, so it needs to go through the owner approval gate.',
    )
    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [
          { content: 'go ahead and pay the worker their payout', role: 'user' },
        ],
        ownerId: 'owner:github:14167547',
      }),
    )
    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.deferredToApprovalGate).toBe(true)
  })

  test('a plain status ask does NOT set deferredToApprovalGate', async () => {
    const { client } = makeRecordingKhalaClient('All green.')
    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'how are things looking?', role: 'user' }],
        ownerId: 'owner:github:14167547',
      }),
    )
    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.deferredToApprovalGate).toBe(false)
  })
})
