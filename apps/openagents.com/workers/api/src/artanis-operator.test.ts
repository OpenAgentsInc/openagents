import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK,
  ARTANIS_OPERATOR_APPROVAL_GATE_REF,
  ARTANIS_OPERATOR_KHALA_MODEL,
  ARTANIS_OPERATOR_MAX_TOOL_ITERATIONS,
  ARTANIS_OPERATOR_SYSTEM_PROMPT,
  ARTANIS_OPERATOR_TOOL_RESULT_CONTEXT_MAX_CHARS,
  type ArtanisMemoryEntry,
  type ArtanisOperatorGatedResult,
  type ArtanisOperatorGatedTool,
  type ArtanisOperatorKhalaClient,
  type ArtanisOperatorReadTool,
  type ArtanisOperatorRiskyTool,
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
import {
  makeArtanisGetKhalaFeedbackTool,
  makeArtanisRepoPathExistsTool,
  makeArtanisTriggerSyntheticLoadTool,
} from './artanis-operator-tools'
import { ARTANIS_GROUNDING_ADDENDUM_HEADER } from './artanis-operator-grounding-gate'

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
    tokenPace: {
      behindPace: true,
      day: '2026-06-27',
      fractionOfCentralDayElapsed: 0.5,
      gapToTarget4x: 1_112_400_000,
      paceProjection: 200_000_000,
      target10x: 3_281_000_000,
      target4x: 1_312_400_000,
      todayTokens: 100_000_000,
      yesterdayTokens: 328_100_000,
    },
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
    tokenPace: null,
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
      'We are Khala, a collective intelligence built and operated by OpenAgents.',
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

// ---------------------------------------------------------------------------
// #6364 — bounded tool-calling loop.
// ---------------------------------------------------------------------------

// A read tool that returns a canned file body and records its calls.
const makeFakeReadTool = (
  body: string,
): { tool: ArtanisOperatorReadTool; calls: Array<unknown> } => {
  const calls: Array<unknown> = []
  const tool: ArtanisOperatorReadTool = {
    definition: {
      description: 'read a repo file',
      name: 'read_repo_file',
      parameters: {
        properties: { path: { type: 'string' } },
        required: ['path'],
        type: 'object',
      },
    },
    execute: (args: unknown) => {
      calls.push(args)
      return Effect.succeed(body)
    },
    kind: 'read',
  }
  return { calls, tool }
}

// A risky tool that records whether plan() ran (and asserts it NEVER executes).
const makeFakeRiskyTool = (): {
  tool: ArtanisOperatorRiskyTool
  planned: Array<unknown>
} => {
  const planned: Array<unknown> = []
  const tool: ArtanisOperatorRiskyTool = {
    definition: {
      description: 'dispatch a codex task',
      name: 'dispatch_codex_task',
      parameters: {
        properties: { objective: { type: 'string' } },
        required: ['objective'],
        type: 'object',
      },
    },
    kind: 'risky',
    plan: (args: unknown) => {
      planned.push(args)
      return Effect.succeed('pylon khala request --workflow codex_agent_task ...')
    },
    riskyActionKind: 'pylon_job_dispatch',
  }
  return { planned, tool }
}

// A gated tool that records its args and returns a scripted outcome (executed
// or deferred). Mirrors the real `dispatch_codex_task` gated shape.
const makeFakeGatedTool = (
  outcome: ArtanisOperatorGatedResult,
): {
  tool: ArtanisOperatorGatedTool
  ran: Array<unknown>
} => {
  const ran: Array<unknown> = []
  const tool: ArtanisOperatorGatedTool = {
    definition: {
      description: 'dispatch a codex task',
      name: 'dispatch_codex_task',
      parameters: {
        properties: { objective: { type: 'string' } },
        required: ['objective'],
        type: 'object',
      },
    },
    kind: 'gated',
    riskyActionKind: 'pylon_job_dispatch',
    run: (args: unknown) => {
      ran.push(args)
      return Effect.succeed(outcome)
    },
  }
  return { ran, tool }
}

// A scripted Khala client: returns the queued InferenceResults in order. Each
// call captures the request so tests can assert tools were advertised + the tool
// result round-tripped back into the conversation.
const makeScriptedKhalaClient = (
  script: ReadonlyArray<InferenceResult>,
): {
  client: ArtanisOperatorKhalaClient
  requests: Array<InferenceRequest>
} => {
  const requests: Array<InferenceRequest> = []
  let index = 0
  const fallback = textResult('(end of script)')
  const client: ArtanisOperatorKhalaClient = (request: InferenceRequest) => {
    requests.push(request)
    const result = script[index] ?? script[script.length - 1] ?? fallback
    index += 1
    return Effect.succeed(result)
  }
  return { client, requests }
}

const toolCallResult = (
  name: string,
  args: string,
): InferenceResult => ({
  content: '',
  finishReason: 'tool_calls',
  servedModel: 'gpt-oss-120b',
  toolCalls: [
    { function: { arguments: args, name }, id: `call_${name}`, type: 'function' },
  ],
  usage: { completionTokens: 4, promptTokens: 100, totalTokens: 104 },
})

const textResult = (content: string): InferenceResult => ({
  content,
  finishReason: 'stop',
  servedModel: 'gpt-oss-120b',
  usage: { completionTokens: 20, promptTokens: 300, totalTokens: 320 },
})

describe('#6364 artanis operator bounded tool-calling loop', () => {
  test('a tool_calls round-trips: read tool executes, then a final text reply', async () => {
    const { calls, tool } = makeFakeReadTool('First priority: the #6316 track.')
    const { client, requests } = makeScriptedKhalaClient([
      toolCallResult('read_repo_file', '{"path":"docs/roadmap.md"}'),
      textResult('The roadmap says the first priority is the #6316 track.'),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'read docs/roadmap.md and summarize', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    // The read tool was actually executed with the model's arguments.
    expect(calls).toEqual([{ path: 'docs/roadmap.md' }])
    // Two Khala calls: one that requested the tool, one that produced the reply.
    expect(result.iterations).toBe(2)
    expect(result.reply).toContain('#6316')
    expect(result.toolInvocations).toEqual([
      {
        deferredToApprovalGate: false,
        executed: true,
        executedRef: null,
        name: 'read_repo_file',
        riskyActionKind: null,
      },
    ])
    // First request advertised the tools; second carried the tool result back.
    expect(requests[0]?.passthroughParams.tools).toBeDefined()
    const secondConversation = requests[1]?.messages ?? []
    expect(
      secondConversation.some(
        message =>
          message.role === 'tool' &&
          message.content.includes('First priority'),
      ),
    ).toBe(true)
    expect(
      secondConversation.some(
        message => message.role === 'assistant' && message.toolCalls !== undefined,
      ),
    ).toBe(true)
  })

  test('the REAL get_khala_feedback tool executes and Artanis summarizes + triages it (iteration 6)', async () => {
    // The owner says: "Get the recent Khala CLI feedback." The fake Khala client
    // requests get_khala_feedback(limit:10); the tool resolves to mock feedback
    // from an injected reader; Artanis's final reply summarizes + proposes triage.
    let askedLimit: number | undefined
    const tool = makeArtanisGetKhalaFeedbackTool({
      reader: async limit => {
        askedLimit = limit
        return [
          {
            clientVersion: '0.4.2',
            createdAt: '2026-06-27T11:00:00.000Z',
            feedback: 'too wordy, prefer more conversational',
            feedbackRef: 'khala_feedback:fb_aaa111',
            source: 'khala-cli',
          },
          {
            clientVersion: null,
            createdAt: '2026-06-27T10:30:00.000Z',
            feedback: 'wish it could read my local git diff before answering',
            feedbackRef: 'khala_feedback:fb_bbb222',
            source: 'khala-cli',
          },
        ]
      },
    })
    const { client, requests } = makeScriptedKhalaClient([
      toolCallResult('get_khala_feedback', '{"limit":10}'),
      textResult(
        'Two recent notes: users find replies too wordy (prefer more conversational), and one wants me to read their local git diff first. Triage: tighten the default reply style, and route the git-diff ask to the unsupported-requests track (#6357).',
      ),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [
          { content: 'Get the recent Khala CLI feedback.', role: 'user' },
        ],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    // The read tool actually executed with the model's bounded limit arg.
    expect(askedLimit).toBe(10)
    expect(result.iterations).toBe(2)
    // It is a plain read: no approval-gate deferral.
    expect(result.deferredToApprovalGate).toBe(false)
    expect(result.toolInvocations).toEqual([
      {
        deferredToApprovalGate: false,
        executed: true,
        executedRef: null,
        name: 'get_khala_feedback',
        riskyActionKind: null,
      },
    ])
    // Artanis's final reply summarizes the feedback and proposes triage actions.
    expect(result.reply).toContain('wordy')
    expect(result.reply).toContain('#6357')
    // The real feedback round-tripped back to the model as a tool message.
    const secondConversation = requests[1]?.messages ?? []
    expect(
      secondConversation.some(
        message =>
          message.role === 'tool' &&
          message.content.includes('too wordy, prefer more conversational') &&
          message.content.includes('Recent Khala CLI feedback'),
      ),
    ).toBe(true)
  })

  test('a risky tool is PLANNED, never executed, and defers to the approval gate', async () => {
    const { planned, tool } = makeFakeRiskyTool()
    const { client } = makeScriptedKhalaClient([
      toolCallResult('dispatch_codex_task', '{"objective":"burn down #6320"}'),
      textResult('I planned the dispatch; it needs your approval before it runs.'),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'dispatch the backlog', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    // plan() ran (the public-safe plan was built) but nothing was executed.
    expect(planned).toEqual([{ objective: 'burn down #6320' }])
    expect(result.deferredToApprovalGate).toBe(true)
    expect(result.pendingApprovalGates).toEqual([
      {
        gateRef: ARTANIS_OPERATOR_APPROVAL_GATE_REF,
        gateSystem: 'artanis-approval-gates',
        riskyActionKind: 'pylon_job_dispatch',
        state: 'pending',
        toolName: 'dispatch_codex_task',
      },
    ])
    expect(result.toolInvocations).toEqual([
      {
        deferredToApprovalGate: true,
        executed: false,
        executedRef: null,
        name: 'dispatch_codex_task',
        riskyActionKind: 'pylon_job_dispatch',
      },
    ])
  })

  test('the REAL trigger_synthetic_load tool defers (planned, never run live)', async () => {
    const tool = makeArtanisTriggerSyntheticLoadTool()
    const { client, requests } = makeScriptedKhalaClient([
      toolCallResult(
        'trigger_synthetic_load',
        '{"type":"terminal-bench","targetTokens":500000000}',
      ),
      textResult(
        'I planned a Terminal-Bench synthetic-load run; it needs your approval before it runs.',
      ),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [
          { content: "we're behind pace, spin up synthetic load", role: 'user' },
        ],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    // It deferred to the approval gate and never executed live.
    expect(result.deferredToApprovalGate).toBe(true)
    expect(result.toolInvocations).toEqual([
      {
        deferredToApprovalGate: true,
        executed: false,
        executedRef: null,
        name: 'trigger_synthetic_load',
        riskyActionKind: 'eval_launch',
      },
    ])
    // The plan round-tripped back to the model as a tool message that names the
    // bounded run AND the "REQUIRES OWNER APPROVAL — NOT EXECUTED" framing.
    const secondConversation = requests[1]?.messages ?? []
    expect(
      secondConversation.some(
        message =>
          message.role === 'tool' &&
          message.content.includes('type=terminal-bench') &&
          message.content.includes('NOT EXECUTED'),
      ),
    ).toBe(true)
  })

  test('a gated tool that EXECUTES reports the created assignment ref', async () => {
    const { ran, tool } = makeFakeGatedTool({
      assignmentRef: 'assignment.public.khala_coding.abc123',
      durableRequestId: 'req-abc123',
      outcome: 'executed',
      summary: 'assignmentRef: assignment.public.khala_coding.abc123',
    })
    const { client } = makeScriptedKhalaClient([
      toolCallResult('dispatch_codex_task', '{"objective":"burn down #6320"}'),
      textResult('Dispatched. The assignment is running on your linked Pylon.'),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'dispatch the backlog', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    // The gated tool ran (it really fired), and reports the created ref.
    expect(ran).toEqual([{ objective: 'burn down #6320' }])
    expect(result.deferredToApprovalGate).toBe(false)
    expect(result.pendingApprovalGates).toEqual([])
    expect(result.toolInvocations).toEqual([
      {
        deferredToApprovalGate: false,
        executed: true,
        executedRef: 'assignment.public.khala_coding.abc123',
        name: 'dispatch_codex_task',
        riskyActionKind: 'pylon_job_dispatch',
      },
    ])
  })

  test('a gated tool that DEFERS returns the plan and never executes', async () => {
    const { ran, tool } = makeFakeGatedTool({
      outcome: 'deferred',
      plan: 'pylon khala request --workflow codex_agent_task ...',
      reason: 'no_effective_owner_approval',
    })
    const { client } = makeScriptedKhalaClient([
      toolCallResult('dispatch_codex_task', '{"objective":"burn down #6320"}'),
      textResult('That dispatch needs your approval before it runs.'),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'dispatch the backlog', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(ran).toEqual([{ objective: 'burn down #6320' }])
    expect(result.deferredToApprovalGate).toBe(true)
    expect(result.pendingApprovalGates).toEqual([
      {
        gateRef: ARTANIS_OPERATOR_APPROVAL_GATE_REF,
        gateSystem: 'artanis-approval-gates',
        riskyActionKind: 'pylon_job_dispatch',
        state: 'pending',
        toolName: 'dispatch_codex_task',
      },
    ])
    expect(result.toolInvocations).toEqual([
      {
        deferredToApprovalGate: true,
        executed: false,
        executedRef: null,
        name: 'dispatch_codex_task',
        riskyActionKind: 'pylon_job_dispatch',
      },
    ])
  })

  test('the loop caps iterations when the model keeps asking for tools', async () => {
    const { tool } = makeFakeReadTool('some file body')
    // A client that ALWAYS asks for the tool — the cap must force termination.
    const alwaysToolClient: ArtanisOperatorKhalaClient = () =>
      Effect.succeed(toolCallResult('read_repo_file', '{"path":"docs/x.md"}'))
    let calls = 0
    const counted: ArtanisOperatorKhalaClient = request => {
      calls += 1
      return alwaysToolClient(request)
    }

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: counted,
        memory: exampleMemory,
        messages: [{ content: 'keep reading forever', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    // Bounded: the LOOP caps at MAX tool rounds + one final (tools-suppressed)
    // call; the robust empty-reply guard then makes at most two more compact
    // composition calls. Everything terminates — it never spins forever.
    expect(result.iterations).toBe(ARTANIS_OPERATOR_MAX_TOOL_ITERATIONS + 3)
    expect(calls).toBe(ARTANIS_OPERATOR_MAX_TOOL_ITERATIONS + 3)
  })

  test('with no tools configured the turn is a single Khala call (unchanged behavior)', async () => {
    const { client } = makeRecordingKhalaClient('All green.')
    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'status?', role: 'user' }],
        ownerId: 'owner:github:14167547',
      }),
    )
    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.iterations).toBe(1)
    expect(result.toolInvocations).toEqual([])
  })
})


// ---------------------------------------------------------------------------
// epic #6359 — live token-pace awareness + mission framing.
// ---------------------------------------------------------------------------

describe('#6359 token-pace mission framing + awareness injection', () => {
  test('persona makes the daily token target explicit (>=4x prior day, goal 10x)', () => {
    const lower = ARTANIS_OPERATOR_SYSTEM_PROMPT.toLowerCase()
    expect(lower).toContain('at least 4x the prior day')
    expect(lower).toContain('goal of 10x')
    expect(lower).toContain('behindpace')
    // The biggest token-per-action levers are named so Artanis knows what to do.
    expect(lower).toContain('mirrorcode')
  })

  test('a behind-pace tokenPace block renders into the grounded context with URGENT framing', () => {
    const block = buildArtanisOperatorContextBlock({
      awareness: exampleAwareness,
      memory: exampleMemory,
    })
    expect(block).toContain('token pace')
    expect(block).toContain('BEHIND PACE')
    expect(block).toContain('[URGENT]')
    expect(block).toContain('2026-06-27')
  })

  test('a null tokenPace renders no pace line (honest absence)', () => {
    const block = buildArtanisOperatorContextBlock({
      awareness: emptyAwareness,
      memory: [],
    })
    expect(block).not.toContain('[token pace]')
    expect(block).not.toContain('[URGENT]')
  })
})

describe('#6359 the turn never returns an empty reply (cap / blank-completion guard)', () => {
  test('a blank completion (no tool calls) forces a final tools-suppressed answer', async () => {
    const { tool } = makeFakeReadTool('some file body')
    // First completion: blank text, no tool calls, while tools are advertised.
    // The guard must force ONE more tools-suppressed call that answers.
    const { client, requests } = makeScriptedKhalaClient([
      textResult(''),
      textResult('Here is my real answer about the token pace.'),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'what is our token pace?', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.reply).toBe('Here is my real answer about the token pace.')
    expect(result.reply.length).toBeGreaterThan(0)
    expect(result.iterations).toBe(2)
    // The forced final call suppressed tools.
    expect(requests[1]?.passthroughParams.tools).toBeUndefined()
  })

  test('when every completion is blank, the reply is the fallback, never empty', async () => {
    const { tool } = makeFakeReadTool('body')
    const { client } = makeScriptedKhalaClient([textResult(''), textResult('')])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'status?', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.reply).toBe(ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK)
    expect(result.reply.trim().length).toBeGreaterThan(0)
  })

  test('hitting the iteration cap with real tool work yields a GROUNDED synthesized reply (not the apology)', async () => {
    const { tool } = makeFakeReadTool('FILE-BODY-CONTENT from the roadmap')
    // Always asks for a tool (content empty); the cap forces termination and the
    // forced compositions also come back empty. Because real tool work WAS
    // gathered, the reply must SYNTHESIZE from that gathered state — never the
    // bare "couldn't compose" apology, and never empty.
    const alwaysToolClient: ArtanisOperatorKhalaClient = () =>
      Effect.succeed(toolCallResult('read_repo_file', '{"path":"docs/x.md"}'))

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: alwaysToolClient,
        memory: exampleMemory,
        messages: [{ content: 'keep reading forever', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    // MAX+1 loop calls, then two forced (empty) composition retries.
    expect(result.iterations).toBe(ARTANIS_OPERATOR_MAX_TOOL_ITERATIONS + 3)
    expect(result.reply.trim().length).toBeGreaterThan(0)
    // It is the grounded synthesis, citing the gathered tool result, NOT apology.
    expect(result.reply).not.toBe(ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK)
    expect(result.reply).toContain('FILE-BODY-CONTENT from the roadmap')
    expect(result.reply).toContain('read_repo_file')
    // The dogfood proof is intact even on the synthesized path.
    expect(result.servedVia).toBe('openagents_khala')
    expect(result.persona.satisfied).toBe(true)
  })

  test('a LATE Khala failure after gathering tool work recovers with a grounded reply, never mind_unavailable', async () => {
    // The live failure mode: the FIRST call succeeds and triggers a tool; the
    // tool returns real content; then EVERY subsequent Khala call fails (context
    // bloat). The turn must NOT hard-bail with artanis_operator_mind_unavailable
    // — it must compose/synthesize a grounded reply from the gathered work.
    const { tool } = makeFakeReadTool('TOOL-RESULT: the burndown has 4 open lanes')
    let call = 0
    const failAfterFirstToolClient: ArtanisOperatorKhalaClient = () => {
      call += 1
      if (call === 1) {
        return Effect.succeed(
          toolCallResult('read_repo_file', '{"path":"docs/roadmap.md"}'),
        )
      }
      return Effect.fail(
        new InferenceAdapterError({
          adapterId: 'test',
          httpStatus: 413,
          kind: 'service_overloaded',
          reason: 'context_too_large',
          retryable: false,
        }),
      )
    }

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: failAfterFirstToolClient,
        memory: exampleMemory,
        messages: [{ content: 'assess the burndown', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    // Critically: NOT a hard failure.
    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.reply.trim().length).toBeGreaterThan(0)
    expect(result.reply).not.toBe(ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK)
    // The reply is grounded in the real tool result that was gathered.
    expect(result.reply).toContain('TOOL-RESULT: the burndown has 4 open lanes')
    expect(result.servedVia).toBe('openagents_khala')
    expect(result.toolInvocations).toHaveLength(1)
    expect(result.persona.satisfied).toBe(true)
  })

  test('a LATE Khala failure recovers via a SUCCESSFUL compact composition when the model can still answer small requests', async () => {
    // The first call triggers a tool; the second (full-conversation) call fails
    // from bloat; but the compact composition request is small enough to answer.
    const { tool } = makeFakeReadTool('gathered detail')
    let call = 0
    const client: ArtanisOperatorKhalaClient = () => {
      call += 1
      if (call === 1) {
        return Effect.succeed(
          toolCallResult('read_repo_file', '{"path":"docs/roadmap.md"}'),
        )
      }
      if (call === 2) {
        return Effect.fail(
          new InferenceAdapterError({
            adapterId: 'test',
            httpStatus: 413,
            kind: 'service_overloaded',
            reason: 'context_too_large',
            retryable: false,
          }),
        )
      }
      // The compact composition call succeeds.
      return Effect.succeed(
        textResult('Decision: focus the next window on the #6316 track.'),
      )
    }

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'assess and decide', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.reply).toBe(
      'Decision: focus the next window on the #6316 track.',
    )
    expect(result.servedVia).toBe('openagents_khala')
  })

  test('a large tool result is bounded before it is fed back into the conversation', async () => {
    // A tool returning a huge body must NOT be re-sent verbatim (that is what
    // bloats a later call into failing); it is truncated with an explicit marker.
    const huge = 'X'.repeat(
      ARTANIS_OPERATOR_TOOL_RESULT_CONTEXT_MAX_CHARS + 50_000,
    )
    const { tool } = makeFakeReadTool(huge)
    const { client, requests } = makeScriptedKhalaClient([
      toolCallResult('read_repo_file', '{"path":"docs/big.md"}'),
      textResult('Summarized the large file.'),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'read the big file', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    const secondConversation = requests[1]?.messages ?? []
    const toolMessage = secondConversation.find(
      message => message.role === 'tool',
    )
    expect(toolMessage).toBeDefined()
    // Bounded well under the raw size, with the truncation marker.
    expect((toolMessage?.content.length ?? 0)).toBeLessThan(
      ARTANIS_OPERATOR_TOOL_RESULT_CONTEXT_MAX_CHARS + 200,
    )
    expect(toolMessage?.content).toContain('truncated at')
  })
})


// ---------------------------------------------------------------------------
// Full-Blueprint-set wiring, slice 1: the operator loop ENFORCES the Blueprint
// Signature-6 grounding gate over the final reply. A fabricated path/endpoint is
// structurally tagged SPECULATIVE; a verified one passes GROUNDED. This is the
// "headless but full Blueprint set" guarantee — grounding is no longer merely a
// prompt instruction the model can ignore.
// ---------------------------------------------------------------------------

describe('artanis operator grounding gate enforcement (Blueprint Signature 6)', () => {
  test('a turn that fabricates a script path AND an API endpoint is gated SPECULATIVE', async () => {
    // The model ignores the GROUNDED-ASSERTION RULE and names two artifacts it
    // never looked up. With no grounding lookups performed, the loop must gate
    // them — the prompt-level rule is now a structural gate.
    const { client } = makeScriptedKhalaClient([
      textResult(
        'Regenerate the traces with `bun scripts/distill_traces.ts --since 24h`, then mint via POST /api/admin/khala/mint.',
      ),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'how do I regenerate the traces?', role: 'user' }],
        ownerId: 'owner:github:14167547',
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.groundingGate.allGrounded).toBe(false)
    expect(result.groundingGate.enforced).toBe(true)
    // Both fabricated artifacts are recorded as SPECULATIVE on the turn.
    const refs = result.groundingGate.speculativeArtifacts.map(a => a.artifactRef)
    expect(refs).toContain('scripts/distill_traces.ts')
    expect(refs).toContain('/api/admin/khala/mint')
    // The reply itself now carries the structural SPECULATIVE addendum, so the
    // owner can never read a fabricated path as runnable.
    expect(result.reply).toContain(ARTANIS_GROUNDING_ADDENDUM_HEADER)
    expect(result.reply).toContain('SPECULATIVE')
    // None of the artifacts reached GROUNDED.
    expect(result.groundingGate.evaluated.every(v => !v.grounded)).toBe(true)
  })

  test('a turn that VERIFIES a path via repo_path_exists passes GROUNDED with no addendum', async () => {
    const realPath = 'apps/pylon/scripts/multi-session-campaign.ts'
    // repo_path_exists fetch returns a 200 file object => GROUNDED existence.
    const fetchImpl = (async () =>
      new Response(JSON.stringify({ size: 900, type: 'file' }), {
        status: 200,
      })) as unknown as typeof fetch
    const tool = makeArtanisRepoPathExistsTool({ fetchImpl })

    const { client } = makeScriptedKhalaClient([
      toolCallResult('repo_path_exists', JSON.stringify({ path: realPath })),
      textResult(`Use ${realPath} for the campaign run.`),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'which script runs the campaign?', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.groundingGate.allGrounded).toBe(true)
    expect(result.groundingGate.enforced).toBe(false)
    expect(result.groundingGate.speculativeArtifacts).toEqual([])
    // The reply is delivered untouched: no SPECULATIVE addendum.
    expect(result.reply).not.toContain(ARTANIS_GROUNDING_ADDENDUM_HEADER)
    expect(result.reply).toContain(realPath)
    const verdict = result.groundingGate.evaluated.find(
      v => v.artifactRef === realPath,
    )
    expect(verdict?.state).toBe('GROUNDED')
    expect(verdict?.grounded).toBe(true)
    expect(verdict?.lookupTool).toBe('repo_path_exists')
  })

  test('a fabricated path looked up with a NEGATIVE result is still gated', async () => {
    const fakePath = 'scripts/distill_traces.ts'
    // repo_path_exists fetch returns 404 => existence UNGROUNDED.
    const fetchImpl = (async () =>
      new Response('Not Found', { status: 404 })) as unknown as typeof fetch
    const tool = makeArtanisRepoPathExistsTool({ fetchImpl })

    const { client } = makeScriptedKhalaClient([
      toolCallResult('repo_path_exists', JSON.stringify({ path: fakePath })),
      textResult(`I would run ${fakePath} to rebuild the traces.`),
    ])

    const result = await Effect.runPromise(
      artanisOperatorTurn({
        awareness: exampleAwareness,
        khalaClient: client,
        memory: exampleMemory,
        messages: [{ content: 'rebuild the traces', role: 'user' }],
        ownerId: 'owner:github:14167547',
        tools: [tool],
      }),
    )

    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.groundingGate.allGrounded).toBe(false)
    expect(result.groundingGate.enforced).toBe(true)
    const verdict = result.groundingGate.evaluated.find(
      v => v.artifactRef === fakePath,
    )
    expect(verdict?.state).toBe('LOOKED_UP')
    expect(verdict?.lookupResult).toBe('negative')
    expect(result.reply).toContain('SPECULATIVE')
  })
})
