import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeArtanisOperatorTools } from './artanis-operator-tools'
import type {
  ArtanisMemoryAppendInput,
  ArtanisMemoryEntry,
  ArtanisMemoryLoadInput,
  ArtanisOwnerMemoryStore,
} from './artanis-owner-memory'
import { ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK } from './artanis-operator'
import { makeOperatorArtanisChatRoutes } from './artanis-operator-chat-routes'
import type {
  InferenceRequest,
  InferenceResult,
} from './inference/provider-adapter'
import { InferenceAdapterError } from './inference/provider-adapter'

type Session = Readonly<{ user: Readonly<{ email: string; userId: string }> }>

const ctx = {} as ExecutionContext

// An in-memory owner-memory store so the route test exercises persistence
// without the D1/sqlite harness. Owner-scoped: load only returns the owner's
// own entries.
const makeFakeStore = (): {
  store: ArtanisOwnerMemoryStore
  entries: Array<ArtanisMemoryEntry>
} => {
  const entries: Array<ArtanisMemoryEntry> = []
  const store: ArtanisOwnerMemoryStore = {
    append: async (input: ArtanisMemoryAppendInput) => {
      const turn = input.turn
      const entry: ArtanisMemoryEntry = {
        body: turn.body,
        createdAt: input.createdAt,
        kind: turn.kind,
        memoryRef: input.memoryRef,
        noteCategory: turn.kind === 'note' ? turn.noteCategory : null,
        ownerId: input.ownerId,
        role: turn.kind === 'turn' ? turn.role : null,
      }
      entries.unshift(entry)
      return entry
    },
    load: async (input: ArtanisMemoryLoadInput) =>
      entries
        .filter(entry => entry.ownerId === input.ownerId)
        .filter(entry => input.notesOnly !== true || entry.kind === 'note')
        .slice(0, input.limit),
  }
  return { entries, store }
}

const baseDeps = (
  overrides: Partial<
    Parameters<typeof makeOperatorArtanisChatRoutes<Session, { OPENAGENTS_DB: D1Database }>>[0]
  > = {},
) => {
  const fake = makeFakeStore()
  const deps = {
    appendRefreshedSessionCookies: (response: Response) => response,
    isOpenAgentsAdminEmail: (email: string) => email.endsWith('@openagents.com'),
    makeKhalaClient: () => (request: import('./inference/provider-adapter').InferenceRequest) =>
      Effect.succeed({
        content: 'I dispatched two Codex assignments this morning.',
        finishReason: 'stop',
        // Echo the requested model so the test can confirm it was openagents/khala.
        servedModel: request.model === 'openagents/khala' ? 'gpt-oss-120b' : 'wrong',
        usage: { completionTokens: 9, promptTokens: 100, totalTokens: 109 },
      }),
    makeMemoryStore: () => fake.store,
    requireBrowserSession: async (): Promise<Session | undefined> => ({
      user: { email: 'chris@openagents.com', userId: 'github:14167547' },
    }),
    ...overrides,
  }
  return { deps, fake }
}

const post = (body: unknown): Request =>
  new Request('https://openagents.com/api/operator/artanis/chat', {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const runRoute = async (
  deps: ReturnType<typeof baseDeps>['deps'],
  request: Request,
) => {
  const routes = makeOperatorArtanisChatRoutes(deps)
  const effect = routes.routeOperatorArtanisChatRequest(
    request,
    { OPENAGENTS_DB: {} as D1Database },
    ctx,
  )
  expect(effect).toBeDefined()
  return Effect.runPromise(effect!)
}

describe('POST /api/operator/artanis/chat — owner auth', () => {
  test('only matches the chat path', () => {
    const { deps } = baseDeps()
    const routes = makeOperatorArtanisChatRoutes(deps)
    const other = routes.routeOperatorArtanisChatRequest(
      new Request('https://openagents.com/api/operator/artanis/console'),
      { OPENAGENTS_DB: {} as D1Database },
      ctx,
    )
    expect(other).toBeUndefined()
  })

  test('rejects non-POST with 405', async () => {
    const { deps } = baseDeps()
    const routes = makeOperatorArtanisChatRoutes(deps)
    const response = await Effect.runPromise(
      routes.routeOperatorArtanisChatRequest(
        new Request('https://openagents.com/api/operator/artanis/chat'),
        { OPENAGENTS_DB: {} as D1Database },
        ctx,
      )!,
    )
    expect(response.status).toBe(405)
  })

  test('401 when there is no session', async () => {
    const { deps } = baseDeps({
      requireBrowserSession: async () => undefined,
    })
    const response = await runRoute(deps, post({ messages: [] }))
    expect(response.status).toBe(401)
  })

  test('200 for any authenticated browser session, scoped to that user', async () => {
    const { deps } = baseDeps({
      requireBrowserSession: async () => ({
        user: { email: 'someone@example.com', userId: 'github:9' },
      }),
    })
    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'hi', role: 'user' }] }),
    )
    expect(response.status).toBe(200)
  })

  test('non-admin browser sessions persist only to their own owner memory', async () => {
    const { deps, fake } = baseDeps({
      requireBrowserSession: async () => ({
        user: { email: 'community@example.com', userId: 'github:community' },
      }),
    })
    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'tenant status', role: 'user' }] }),
    )
    expect(response.status).toBe(200)
    const owner = fake.entries.find(entry => entry.role === 'owner')
    expect(owner?.body).toBe('tenant status')
    expect(owner?.ownerId).toBe('owner:github:community')
  })

  test('200 when an owner-linked agent bearer resolves (no browser session needed)', async () => {
    const { deps } = baseDeps({
      requireBrowserSession: async () => undefined,
      resolveOwnerAgentBearer: async () => ({
        user: { email: 'chris@openagents.com', userId: 'github:14167547' },
      }),
    })
    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'hi', role: 'user' }] }),
    )
    expect(response.status).toBe(200)
  })

  test('401 when the agent bearer is not owner-linked and there is no session', async () => {
    const { deps } = baseDeps({
      requireBrowserSession: async () => undefined,
      resolveOwnerAgentBearer: async () => undefined,
    })
    const response = await runRoute(deps, post({ messages: [] }))
    expect(response.status).toBe(401)
  })

  test('400 on an empty / owner-less body', async () => {
    const { deps } = baseDeps()
    const empty = await runRoute(deps, post({ messages: [] }))
    expect(empty.status).toBe(400)

    const noOwner = await runRoute(
      deps,
      post({ messages: [{ content: 'x', role: 'assistant' }] }),
    )
    expect(noOwner.status).toBe(400)
  })
})

describe('POST /api/operator/artanis/chat — grounded Khala-powered reply', () => {
  test('returns a non-roleplay Artanis reply served via Khala', async () => {
    const { deps } = baseDeps()
    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'what are you doing?', role: 'user' }] }),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.servedVia).toBe('openagents_khala')
    expect(json.requestedModel).toBe('openagents/khala')
    expect(json.servedModel).toBe('gpt-oss-120b')
    expect(typeof json.reply).toBe('string')
    expect(json.channelRef).toBe('operator.artanis.chat')
    // The persona verdict is present and clean.
    expect((json.persona as { satisfied: boolean }).satisfied).toBe(true)
    expect(json.deferredToApprovalGate).toBe(false)
  })

  test('persists the owner turn AND the Artanis reply to owner memory', async () => {
    const { deps, fake } = baseDeps()
    await runRoute(
      deps,
      post({ messages: [{ content: 'status please', role: 'user' }] }),
    )
    expect(fake.entries.length).toBe(2)
    const owner = fake.entries.find(entry => entry.role === 'owner')
    const artanis = fake.entries.find(entry => entry.role === 'artanis')
    expect(owner?.body).toBe('status please')
    expect(artanis?.body).toBe(
      'I dispatched two Codex assignments this morning.',
    )
    // Owner-scoped.
    expect(owner?.ownerId).toBe('owner:github:14167547')
  })

  test('a spend ask surfaces the approval-gate hint', async () => {
    const { deps } = baseDeps()
    const response = await runRoute(
      deps,
      post({
        messages: [{ content: 'pay the worker their payout now', role: 'user' }],
      }),
    )
    const json = (await response.json()) as Record<string, unknown>
    expect(json.deferredToApprovalGate).toBe(true)
    expect(json.approvalGateRef).toBe('gate.operator.artanis.spend_destructive')
  })

  test('503 when the Khala client is not configured (no provider fallback)', async () => {
    const { deps } = baseDeps({ makeKhalaClient: () => undefined })
    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'hi', role: 'user' }] }),
    )
    expect(response.status).toBe(503)
  })

  test('503 when the Khala client fails (never falls back to a provider)', async () => {
    const { deps } = baseDeps({
      makeKhalaClient: () => () =>
        Effect.fail(
          new InferenceAdapterError({
            adapterId: 'test',
            httpStatus: 503,
            reason: 'overloaded',
            retryable: true,
          }),
        ),
    })
    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'hi', role: 'user' }] }),
    )
    expect(response.status).toBe(503)
  })
})


// ---------------------------------------------------------------------------
// #6365 acceptance — read_repo_file drives the REAL bounded tool-calling loop
// through the chat endpoint. This is Artanis's iteration-1 self-improvement
// capability: he can read a public repo file himself and reason over its real
// contents, owner-scoped and Khala-powered.
// ---------------------------------------------------------------------------

const ROADMAP_PATH =
  'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md'

// The REAL roadmap file from the repo, read from disk so the asserted "first
// line" is the actual one Artanis was asked to read — not a fixture.
const roadmapContents = readFileSync(
  join(__dirname, '..', '..', '..', '..', '..', ROADMAP_PATH),
  'utf8',
)
const roadmapFirstLine = roadmapContents.split('\n')[0] ?? ''

// A fetch stub the REAL read_repo_file tool fetches through, so we can feed it
// the real roadmap bytes (happy path) or a 404 (nonexistent path)
// deterministically without hitting the network in CI.
const stubRepoFetch = (
  handler: (url: string) => Response,
): { fetchImpl: typeof fetch; urls: Array<string> } => {
  const urls: Array<string> = []
  const fetchImpl = (async (input: RequestInfo | URL) => {
    const url = typeof input === 'string' ? input : input.toString()
    urls.push(url)
    return handler(url)
  }) as typeof fetch
  return { fetchImpl, urls }
}

const toolCallResult = (name: string, args: string): InferenceResult => ({
  content: '',
  finishReason: 'tool_calls',
  servedModel: 'gpt-oss-120b',
  toolCalls: [
    { function: { arguments: args, name }, id: `call_${name}`, type: 'function' },
  ],
  usage: { completionTokens: 4, promptTokens: 100, totalTokens: 104 },
})

// Most recent tool-result message content in a Khala request. The scripted
// client echoes its first line to PROVE the tool's bytes actually flowed back
// through the loop, instead of fabricating a reply.
const lastToolResultContent = (
  request: InferenceRequest,
): string | undefined =>
  [...request.messages].reverse().find(message => message.role === 'tool')
    ?.content

// A two-step scripted Khala client: round 1 asks for read_repo_file(path);
// round 2 reads the tool result it received and replies with that result's first
// line. The reply only contains the real first line IF the loop executed the
// tool and fed the contents back — that is the end-to-end acceptance.
const makeReadThenEchoKhalaClient = (path: string) => {
  const requests: Array<InferenceRequest> = []
  let round = 0
  const client = (request: InferenceRequest) => {
    requests.push(request)
    round += 1
    if (round === 1) {
      return Effect.succeed(
        toolCallResult('read_repo_file', JSON.stringify({ path })),
      )
    }
    const firstLine = (lastToolResultContent(request) ?? '').split('\n')[0] ?? ''
    return Effect.succeed({
      content: `The first line of that roadmap file is: "${firstLine}".`,
      finishReason: 'stop' as const,
      servedModel:
        request.model === 'openagents/khala' ? 'gpt-oss-120b' : 'wrong',
      usage: { completionTokens: 20, promptTokens: 300, totalTokens: 320 },
    } satisfies InferenceResult)
  }
  return { client, requests }
}

describe('POST /api/operator/artanis/chat — #6365 read_repo_file acceptance', () => {
  test('reads the roadmap through the loop and replies with its real first line', async () => {
    const { fetchImpl, urls } = stubRepoFetch(
      () => new Response(roadmapContents, { status: 200 }),
    )
    const { client, requests } = makeReadThenEchoKhalaClient(ROADMAP_PATH)
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({ repoRead: { fetchImpl } }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [
          {
            content:
              'Read docs/khala/2026-06-26-khala-open-issues-master-roadmap.md and tell me its first line.',
            role: 'user',
          },
        ],
      }),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>

    // The loop executed read_repo_file against the roadmap path.
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
      deferredToApprovalGate: boolean
    }>
    const readInvocation = invocations.find(
      invocation => invocation.name === 'read_repo_file',
    )
    expect(readInvocation).toBeDefined()
    expect(readInvocation?.executed).toBe(true)
    expect(readInvocation?.deferredToApprovalGate).toBe(false)

    // It was the REAL read tool: it fetched the roadmap from the public repo.
    expect(urls[0]).toBe(
      `https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/${ROADMAP_PATH}`,
    )

    // The loop resolved the tool with the real file contents: at least two Khala
    // calls (request the tool -> feed result -> final reply).
    expect(json.iterations as number).toBeGreaterThanOrEqual(2)

    // The final reply contains the EXACT first line of the real roadmap file.
    expect(roadmapFirstLine.length).toBeGreaterThan(0)
    expect(json.reply as string).toContain(roadmapFirstLine)

    // Persona separation still holds (no Khala-collective / StarCraft leak).
    expect((json.persona as { satisfied: boolean }).satisfied).toBe(true)

    // Dogfood: served via Khala, no provider bypass; not deferred to a gate.
    expect(json.servedVia).toBe('openagents_khala')
    expect(json.requestedModel).toBe('openagents/khala')
    expect(json.deferredToApprovalGate).toBe(false)

    // The second Khala request carried the tool result back into context.
    expect(
      requests[1]?.messages.some(
        message =>
          message.role === 'tool' &&
          message.content.includes(roadmapFirstLine),
      ),
    ).toBe(true)
  })

  test('a path-traversal read returns a typed, bounded block (never a raw fs error)', async () => {
    const { fetchImpl, urls } = stubRepoFetch(
      () => new Response('SHOULD NOT BE READ', { status: 200 }),
    )
    const { client } = makeReadThenEchoKhalaClient('../../etc/passwd')
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({ repoRead: { fetchImpl } }),
    })

    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'read ../../etc/passwd', role: 'user' }] }),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>

    // The traversal path was NEVER fetched, and the reply is a bounded block.
    expect(urls).toHaveLength(0)
    const reply = json.reply as string
    expect(reply).toContain('blocked')
    expect(reply).not.toContain('ENOENT')
    expect(reply).not.toMatch(/Error:/)
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
    }>
    expect(
      invocations.find(invocation => invocation.name === 'read_repo_file')
        ?.executed,
    ).toBe(true)
  })

  test('a nonexistent path reads as an honest "file not found", not a thrown turn', async () => {
    const { fetchImpl } = stubRepoFetch(
      () => new Response('Not Found', { status: 404 }),
    )
    const { client } = makeReadThenEchoKhalaClient('docs/does-not-exist.md')
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({ repoRead: { fetchImpl } }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [
          { content: 'read docs/does-not-exist.md', role: 'user' },
        ],
      }),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.reply as string).toContain('file not found')
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
    }>
    expect(
      invocations.find(invocation => invocation.name === 'read_repo_file')
        ?.executed,
    ).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// read_github_issue acceptance — Artanis's iteration-2 self-improvement
// capability drives the REAL bounded tool-calling loop through the chat
// endpoint. He can read a public GitHub issue himself (title, state, body,
// bounded comments) and reason over its real contents, owner-scoped and
// Khala-powered, BEFORE drafting a dispatch plan.
// ---------------------------------------------------------------------------

// A scripted Khala client: round 1 asks for read_github_issue(issue_number);
// round 2 reads the tool result and replies with the title line it received. The
// reply only carries the real title IF the loop executed the tool and fed the
// contents back — that is the end-to-end acceptance.
const makeReadIssueThenSummarizeKhalaClient = (issueNumber: number) => {
  const requests: Array<InferenceRequest> = []
  let round = 0
  const client = (request: InferenceRequest) => {
    requests.push(request)
    round += 1
    if (round === 1) {
      return Effect.succeed(
        toolCallResult(
          'read_github_issue',
          JSON.stringify({ issue_number: issueNumber }),
        ),
      )
    }
    const toolResult = lastToolResultContent(request) ?? ''
    const titleLine =
      toolResult.split('\n').find(line => line.startsWith('Issue #')) ?? ''
    return Effect.succeed({
      content: `Here is what that issue requires. ${titleLine}`,
      finishReason: 'stop' as const,
      servedModel:
        request.model === 'openagents/khala' ? 'gpt-oss-120b' : 'wrong',
      usage: { completionTokens: 20, promptTokens: 300, totalTokens: 320 },
    } satisfies InferenceResult)
  }
  return { client, requests }
}

describe('POST /api/operator/artanis/chat — read_github_issue acceptance', () => {
  test('reads a public issue through the loop and summarizes its real requirements', async () => {
    const issueJson = JSON.stringify({
      body: 'Build a read-only read_github_issue(issue_number) operator tool.',
      comments: 1,
      state: 'open',
      title: 'read_github_issue operator tool',
    })
    const { fetchImpl, urls } = stubRepoFetch(url =>
      url.includes('/comments')
        ? new Response(
            JSON.stringify([
              {
                body: 'Acceptance: fake-fetch unit test + endpoint proof.',
                created_at: '2026-06-27T00:00:00Z',
                user: { login: 'chris' },
              },
            ]),
            { status: 200 },
          )
        : new Response(issueJson, { status: 200 }),
    )
    const { client, requests } = makeReadIssueThenSummarizeKhalaClient(6311)
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({ issueRead: { fetchImpl } }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [
          {
            content: 'Read GitHub issue #6311 and summarize its requirements',
            role: 'user',
          },
        ],
      }),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>

    // The loop executed read_github_issue.
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
      deferredToApprovalGate: boolean
    }>
    const readInvocation = invocations.find(
      invocation => invocation.name === 'read_github_issue',
    )
    expect(readInvocation).toBeDefined()
    expect(readInvocation?.executed).toBe(true)
    expect(readInvocation?.deferredToApprovalGate).toBe(false)

    // It was the REAL read tool: it fetched the issue from the public repo API.
    expect(urls[0]).toBe(
      'https://api.github.com/repos/OpenAgentsInc/openagents/issues/6311',
    )

    // At least two Khala calls (request the tool -> feed result -> reply).
    expect(json.iterations as number).toBeGreaterThanOrEqual(2)

    // The final reply summarizes the REAL issue (its actual title line).
    expect(json.reply as string).toContain(
      'Issue #6311: read_github_issue operator tool',
    )

    // Persona separation holds; dogfood via Khala; not deferred to a gate.
    expect((json.persona as { satisfied: boolean }).satisfied).toBe(true)
    expect(json.servedVia).toBe('openagents_khala')
    expect(json.requestedModel).toBe('openagents/khala')
    expect(json.deferredToApprovalGate).toBe(false)

    // The second Khala request carried the real issue body + comment back.
    expect(
      requests[1]?.messages.some(
        message =>
          message.role === 'tool' &&
          message.content.includes(
            'Build a read-only read_github_issue(issue_number) operator tool.',
          ) &&
          message.content.includes(
            'Acceptance: fake-fetch unit test + endpoint proof.',
          ),
      ),
    ).toBe(true)
  })

  test('a non-numeric issue input returns a typed, bounded block (never a raw error)', async () => {
    const { fetchImpl, urls } = stubRepoFetch(
      () => new Response('SHOULD NOT BE READ', { status: 200 }),
    )
    const { client } = (() => {
      let round = 0
      const c = (request: InferenceRequest) => {
        round += 1
        if (round === 1) {
          return Effect.succeed(
            toolCallResult(
              'read_github_issue',
              JSON.stringify({ issue_number: 'the .secrets file' }),
            ),
          )
        }
        const toolResult = lastToolResultContent(request) ?? ''
        return Effect.succeed({
          content: toolResult,
          finishReason: 'stop' as const,
          servedModel: 'gpt-oss-120b',
          usage: { completionTokens: 5, promptTokens: 50, totalTokens: 55 },
        } satisfies InferenceResult)
      }
      return { client: c }
    })()
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({ issueRead: { fetchImpl } }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [{ content: 'read the secrets issue', role: 'user' }],
      }),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>
    expect(urls).toHaveLength(0)
    const reply = json.reply as string
    expect(reply).toContain('blocked')
    expect(reply).not.toMatch(/Error:/)
  })

  test('a nonexistent issue reads as honest "(issue not found: #N)"', async () => {
    const { fetchImpl } = stubRepoFetch(
      () => new Response('Not Found', { status: 404 }),
    )
    const { client, requests } =
      makeReadIssueThenSummarizeKhalaClient(999999)
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({ issueRead: { fetchImpl } }),
    })

    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'read issue #999999', role: 'user' }] }),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
    }>
    expect(
      invocations.find(invocation => invocation.name === 'read_github_issue')
        ?.executed,
    ).toBe(true)
    // The honest not-found string flowed back into the loop as the tool result.
    expect(
      requests[1]?.messages.some(
        message =>
          message.role === 'tool' &&
          message.content.includes('(issue not found: #999999)'),
      ),
    ).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// get_pylon_job_status acceptance — Artanis's iteration-3 self-improvement
// capability drives the REAL bounded tool-calling loop through the chat
// endpoint. He pulls the public-safe closeout/proof status of ONE specific
// owner-scoped assignment ref, reads the verify verdict, and summarizes it —
// owner-scoped, Khala-powered, NON-RISKY (a read tool, never the approval gate).
// ---------------------------------------------------------------------------

const KNOWN_ASSIGNMENT_REF = 'assignment.public.pylon_api.known_001'

// A scripted Khala client: round 1 asks for get_pylon_job_status(assignmentRef);
// round 2 reads the tool result and replies with the State + verify lines it
// received. The reply only carries the real status IF the loop executed the tool
// and fed the result back — that is the end-to-end acceptance.
const makeStatusThenSummarizeKhalaClient = (assignmentRef: string) => {
  const requests: Array<InferenceRequest> = []
  let round = 0
  const client = (request: InferenceRequest) => {
    requests.push(request)
    round += 1
    if (round === 1) {
      return Effect.succeed(
        toolCallResult(
          'get_pylon_job_status',
          JSON.stringify({ assignmentRef }),
        ),
      )
    }
    const toolResult = lastToolResultContent(request) ?? ''
    const stateLine =
      toolResult.split('\n').find(line => line.startsWith('- State:')) ?? ''
    const verifyLine =
      toolResult.split('\n').find(line => line.startsWith('- Verify/proof:')) ??
      ''
    return Effect.succeed({
      content: `Status for that assignment. ${stateLine} ${verifyLine}`,
      finishReason: 'stop' as const,
      servedModel:
        request.model === 'openagents/khala' ? 'gpt-oss-120b' : 'wrong',
      usage: { completionTokens: 20, promptTokens: 300, totalTokens: 320 },
    } satisfies InferenceResult)
  }
  return { client, requests }
}

describe('POST /api/operator/artanis/chat — get_pylon_job_status acceptance', () => {
  test('reads ONE assignment status through the loop and summarizes its real verdict (non-risky, executed)', async () => {
    const { client, requests } =
      makeStatusThenSummarizeKhalaClient(KNOWN_ASSIGNMENT_REF)
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({
          pylonJobStatus: {
            // Injected reader: resolves the KNOWN ref to a public-safe PASS
            // status; any other ref is honest absence.
            reader: async ref =>
              ref === KNOWN_ASSIGNMENT_REF
                ? {
                    artifactRefs: ['artifact.public.pylon_assignment.001'],
                    assignmentRef: KNOWN_ASSIGNMENT_REF,
                    blockerRefs: [],
                    closeoutRefs: ['closeout.public.pylon_assignment.001'],
                    closeoutSubmitted: true,
                    failureSummary: null,
                    jobKind: 'codex_agent_task',
                    leaseState: 'terminal',
                    proofObserved: true,
                    proofRefs: ['proof.public.pylon_assignment.001'],
                    rejectionRefs: [],
                    state: 'closeout_submitted',
                    updatedAt: 'a few minutes ago',
                    verifyResult: 'pass',
                  }
                : null,
          },
        }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [
          {
            content: `What is the status of assignment ${KNOWN_ASSIGNMENT_REF}?`,
            role: 'user',
          },
        ],
      }),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>

    // The loop executed get_pylon_job_status — executed:true, NON-RISKY (a read
    // tool: not deferred to the approval gate, no risky-action kind).
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
      deferredToApprovalGate: boolean
      riskyActionKind: string | null
    }>
    const statusInvocation = invocations.find(
      invocation => invocation.name === 'get_pylon_job_status',
    )
    expect(statusInvocation).toBeDefined()
    expect(statusInvocation?.executed).toBe(true)
    expect(statusInvocation?.deferredToApprovalGate).toBe(false)
    expect(statusInvocation?.riskyActionKind).toBeNull()

    // At least two Khala calls (request the tool -> feed result -> reply).
    expect(json.iterations as number).toBeGreaterThanOrEqual(2)

    // The final reply summarizes the REAL status sourced from the tool: the
    // closeout state and the verify PASS verdict — not invented.
    expect(json.reply as string).toContain('closeout_submitted')
    expect(json.reply as string).toContain('PASS')

    // Persona separation holds; dogfood via Khala; not deferred to a gate.
    expect((json.persona as { satisfied: boolean }).satisfied).toBe(true)
    expect(json.servedVia).toBe('openagents_khala')
    expect(json.requestedModel).toBe('openagents/khala')
    expect(json.deferredToApprovalGate).toBe(false)

    // The second Khala request carried the real status block back into context.
    expect(
      requests[1]?.messages.some(
        message =>
          message.role === 'tool' &&
          message.content.includes(
            `Pylon job status for ${KNOWN_ASSIGNMENT_REF}`,
          ) &&
          message.content.includes('proof.public.pylon_assignment.001'),
      ),
    ).toBe(true)
  })

  test('an other-owner / unknown assignment reads as honest "(no assignment found …)"', async () => {
    // A client that echoes the FULL tool result so the honest not-found string
    // is observable both in the loop context and the final reply.
    const requests: Array<InferenceRequest> = []
    let round = 0
    const client = (request: InferenceRequest) => {
      requests.push(request)
      round += 1
      if (round === 1) {
        return Effect.succeed(
          toolCallResult(
            'get_pylon_job_status',
            JSON.stringify({
              assignmentRef: 'assignment.public.pylon_api.not_mine',
            }),
          ),
        )
      }
      return Effect.succeed({
        content: lastToolResultContent(request) ?? '',
        finishReason: 'stop' as const,
        servedModel: 'gpt-oss-120b',
        usage: { completionTokens: 5, promptTokens: 50, totalTokens: 55 },
      } satisfies InferenceResult)
    }
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({
          pylonJobStatus: { reader: async () => null },
        }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [
          {
            content:
              'What is the status of assignment assignment.public.pylon_api.not_mine?',
            role: 'user',
          },
        ],
      }),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
    }>
    expect(
      invocations.find(
        invocation => invocation.name === 'get_pylon_job_status',
      )?.executed,
    ).toBe(true)
    expect(json.reply as string).toContain('no assignment found')
    expect(
      requests[1]?.messages.some(
        message =>
          message.role === 'tool' &&
          message.content.includes(
            '(no assignment found for "assignment.public.pylon_api.not_mine")',
          ),
      ),
    ).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// update_unsupported_request acceptance — Artanis's iteration-9 self-improvement
// capability drives the REAL bounded tool-calling loop through the chat
// endpoint. iteration-8 lets him READ the unsupported-request ledger; this lets
// him ACT on it in the same turn — moving a gap entry through its lifecycle and
// linking the GitHub issue he dispatched to fix it. Owner-scoped, Khala-powered,
// internal-ledger-only (no spend/destructive/outward authority).
// ---------------------------------------------------------------------------

// A scripted Khala client: round 1 asks for update_unsupported_request with the
// triage change; round 2 reads the tool result it received and summarizes the
// change. The reply only carries the real updated state IF the loop executed the
// write tool and fed the result back — that is the end-to-end acceptance.
const makeTriageThenSummarizeKhalaClient = (args: unknown) => {
  const requests: Array<InferenceRequest> = []
  let round = 0
  const client = (request: InferenceRequest) => {
    requests.push(request)
    round += 1
    if (round === 1) {
      return Effect.succeed(
        toolCallResult('update_unsupported_request', JSON.stringify(args)),
      )
    }
    const toolResult = lastToolResultContent(request) ?? ''
    const statusLine =
      toolResult.split('\n').find(line => line.startsWith('- status:')) ?? ''
    const issueLine =
      toolResult
        .split('\n')
        .find(line => line.startsWith('- linked issue:')) ?? ''
    return Effect.succeed({
      content: `Done. I moved that gap to ${statusLine.replace('- status: ', '')} and ${issueLine.replace('- ', '')}.`,
      finishReason: 'stop' as const,
      servedModel: 'gpt-oss-120b',
      usage: { completionTokens: 20, promptTokens: 300, totalTokens: 320 },
    } satisfies InferenceResult)
  }
  return { client, requests }
}

describe('POST /api/operator/artanis/chat — update_unsupported_request acceptance', () => {
  test('triages gap_987 to issue_opened and links issue 6310 through the loop (write, executed, non-risky)', async () => {
    const KNOWN_REF = 'gap_987'
    const { client, requests } = makeTriageThenSummarizeKhalaClient({
      issue: 6310,
      ref: KNOWN_REF,
      status: 'issue_opened',
    })

    let received: unknown
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({
          // Injected writer: resolves the KNOWN ref, applies the merge, and
          // returns the updated public-safe record (issue_opened -> next 'none').
          unsupportedRequestWriter: async update => {
            received = update
            if (update.ref !== KNOWN_REF) return null
            return {
              githubIssueRef:
                update.githubIssueRef ?? null,
              nextAction: 'none',
              requestRef: KNOWN_REF,
              sourceKind: 'trace_review',
              status: update.status ?? 'needs_issue',
              summary:
                'Users want Khala to read their local git diff before answering.',
              title: 'Khala cannot read the local git diff',
              triageKind: 'missing_capability',
              updatedAt: 'just now',
            }
          },
        }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [
          {
            content:
              'Triage unsupported request gap_987 to issue_opened and link issue 6310',
            role: 'user',
          },
        ],
      }),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>

    // The loop executed update_unsupported_request — executed:true, NON-RISKY (a
    // write tool: not deferred to the approval gate, no risky-action kind).
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
      deferredToApprovalGate: boolean
      riskyActionKind: string | null
    }>
    const triageInvocation = invocations.find(
      invocation => invocation.name === 'update_unsupported_request',
    )
    expect(triageInvocation).toBeDefined()
    expect(triageInvocation?.executed).toBe(true)
    expect(triageInvocation?.deferredToApprovalGate).toBe(false)
    expect(triageInvocation?.riskyActionKind).toBeNull()

    // The tool handed the writer the validated, normalized update.
    expect(received).toEqual({
      githubIssueRef: 'OpenAgentsInc/openagents#6310',
      ref: 'gap_987',
      status: 'issue_opened',
      triageKind: undefined,
    })

    // At least two Khala calls (request the tool -> feed result -> reply).
    expect(json.iterations as number).toBeGreaterThanOrEqual(2)

    // The final reply summarizes the REAL triage change sourced from the tool.
    expect(json.reply as string).toContain('issue_opened')
    expect(json.reply as string).toContain('OpenAgentsInc/openagents#6310')

    // Persona separation holds; dogfood via Khala; not deferred to a gate.
    expect((json.persona as { satisfied: boolean }).satisfied).toBe(true)
    expect(json.servedVia).toBe('openagents_khala')
    expect(json.requestedModel).toBe('openagents/khala')
    expect(json.deferredToApprovalGate).toBe(false)

    // The second Khala request carried the updated record block back into context.
    expect(
      requests[1]?.messages.some(
        message =>
          message.role === 'tool' &&
          message.content.includes('Updated unsupported request gap_987') &&
          message.content.includes('OpenAgentsInc/openagents#6310'),
      ),
    ).toBe(true)
  })

  test('an unknown ref reads as honest "(not found …)", never a fabricated update', async () => {
    const requests: Array<InferenceRequest> = []
    let round = 0
    const client = (request: InferenceRequest) => {
      requests.push(request)
      round += 1
      if (round === 1) {
        return Effect.succeed(
          toolCallResult(
            'update_unsupported_request',
            JSON.stringify({ ref: 'gap_missing', status: 'closed' }),
          ),
        )
      }
      return Effect.succeed({
        content: lastToolResultContent(request) ?? '',
        finishReason: 'stop' as const,
        servedModel: 'gpt-oss-120b',
        usage: { completionTokens: 5, promptTokens: 50, totalTokens: 55 },
      } satisfies InferenceResult)
    }
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      makeOperatorTools: () =>
        makeArtanisOperatorTools({
          unsupportedRequestWriter: async () => null,
        }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [
          { content: 'Close unsupported request gap_missing', role: 'user' },
        ],
      }),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
    }>
    expect(
      invocations.find(
        invocation => invocation.name === 'update_unsupported_request',
      )?.executed,
    ).toBe(true)
    expect(json.reply as string).toContain('not found')
  })
})

// ---------------------------------------------------------------------------
// get_trace_review acceptance — Artanis's iteration-11 self-improvement
// capability drives the REAL bounded tool-calling loop through the chat
// endpoint. A turn asking for the latest trace review invokes get_trace_review,
// which reads the live trace-review report and feeds its real bytes back into
// the loop. Owner-scoped, Khala-powered, read-only (no spend/destructive/outward
// authority). Proves makeArtanisOperatorTools() now includes get_trace_review.
// ---------------------------------------------------------------------------

// A scripted Khala client: round 1 asks for get_trace_review(); round 2 reads
// the tool result it received and echoes its first line. The reply only carries
// the real report header IF the loop executed the tool and fed the bytes back.
const makeTraceReviewThenEchoKhalaClient = () => {
  const requests: Array<InferenceRequest> = []
  let round = 0
  const client = (request: InferenceRequest) => {
    requests.push(request)
    round += 1
    if (round === 1) {
      return Effect.succeed(toolCallResult('get_trace_review', '{}'))
    }
    const firstLine = (lastToolResultContent(request) ?? '').split('\n')[0] ?? ''
    return Effect.succeed({
      content: `Here is the latest trace review: ${firstLine}`,
      finishReason: 'stop' as const,
      servedModel:
        request.model === 'openagents/khala' ? 'gpt-oss-120b' : 'wrong',
      usage: { completionTokens: 20, promptTokens: 300, totalTokens: 320 },
    } satisfies InferenceResult)
  }
  return { client, requests }
}

describe('POST /api/operator/artanis/chat — get_trace_review acceptance', () => {
  test('reads the live trace-review report through the loop and replies with its real summary', async () => {
    const { client, requests } = makeTraceReviewThenEchoKhalaClient()
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      // Inject an in-worker loadReport seam returning the live report shape, so
      // the route does not depend on an HTTP hop to its own admin-gated zone.
      makeOperatorTools: () =>
        makeArtanisOperatorTools({
          traceReview: {
            loadReport: async () => ({
              aggregates: {
                rawCodexEvents: { rowCount: 3 },
                tokens: { eventCount: 42, totalTokens: 12000 },
                traces: { traceCount: 17 },
              },
              failureModes: [
                {
                  count: 4,
                  failureRef: 'failure.khala_trace_review.empty_response',
                  label: 'Token rows with zero completion/output tokens',
                  severity: 'warning',
                },
              ],
              modelMix: [
                {
                  count: 30,
                  model: 'khala',
                  provider: 'openagents',
                  totalTokens: 9000,
                },
              ],
              outcomes: [{ count: 38, outcome: 'stop', totalTokens: 11000 }],
              window: {
                hours: 24,
                since: '2026-06-26T00:00:00.000Z',
                until: '2026-06-27T00:00:00.000Z',
              },
            }),
          },
        }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [
          { content: 'show the latest trace review', role: 'user' },
        ],
      }),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>

    // The loop executed get_trace_review — read, executed, not deferred/risky.
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
      deferredToApprovalGate: boolean
      riskyActionKind: string | null
    }>
    const traceInvocation = invocations.find(
      invocation => invocation.name === 'get_trace_review',
    )
    expect(traceInvocation).toBeDefined()
    expect(traceInvocation?.executed).toBe(true)
    expect(traceInvocation?.deferredToApprovalGate).toBe(false)
    expect(traceInvocation?.riskyActionKind).toBeNull()

    // At least two Khala calls (request the tool -> feed result -> reply).
    expect(json.iterations as number).toBeGreaterThanOrEqual(2)

    // The final reply carries the REAL report header sourced from the tool.
    expect(json.reply as string).toContain('Khala trace review (last 24h')

    // Persona separation holds; dogfood via Khala; not deferred to a gate.
    expect((json.persona as { satisfied: boolean }).satisfied).toBe(true)
    expect(json.servedVia).toBe('openagents_khala')
    expect(json.requestedModel).toBe('openagents/khala')
    expect(json.deferredToApprovalGate).toBe(false)

    // The second Khala request carried the real report bytes back into context
    // (model mix + outcome/failure buckets), not a fabrication.
    expect(
      requests[1]?.messages.some(
        message =>
          message.role === 'tool' &&
          message.content.includes('Model mix (1):') &&
          message.content.includes('openagents/khala: 30 calls, 9,000 tokens') &&
          message.content.includes('Outcomes (1):') &&
          message.content.includes('Failure modes (1):'),
      ),
    ).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// get_synthetic_load_status acceptance — Artanis's iteration-12 self-improvement
// capability drives the REAL bounded tool-calling loop through the chat
// endpoint. A turn asking to show active synthetic loads invokes
// get_synthetic_load_status, which reads the active runs and feeds their
// public-safe summary back into the loop. Owner-scoped, Khala-powered, read-only
// (no spend/destructive/outward authority). Proves makeArtanisOperatorTools()
// now includes get_synthetic_load_status.
// ---------------------------------------------------------------------------

// A scripted Khala client: round 1 asks for get_synthetic_load_status(); round 2
// reads the tool result it received and echoes its first line. The reply only
// carries the real summary IF the loop executed the tool and fed the bytes back.
const makeSyntheticLoadStatusThenEchoKhalaClient = () => {
  const requests: Array<InferenceRequest> = []
  let round = 0
  const client = (request: InferenceRequest) => {
    requests.push(request)
    round += 1
    if (round === 1) {
      return Effect.succeed(toolCallResult('get_synthetic_load_status', '{}'))
    }
    const firstLine = (lastToolResultContent(request) ?? '').split('\n')[0] ?? ''
    return Effect.succeed({
      content: `Active synthetic loads: ${firstLine}`,
      finishReason: 'stop' as const,
      servedModel:
        request.model === 'openagents/khala' ? 'gpt-oss-120b' : 'wrong',
      usage: { completionTokens: 20, promptTokens: 300, totalTokens: 320 },
    } satisfies InferenceResult)
  }
  return { client, requests }
}

describe('POST /api/operator/artanis/chat — get_synthetic_load_status acceptance', () => {
  test('asking "show active synthetic loads" executes get_synthetic_load_status (executed:true, deferredToApprovalGate:false)', async () => {
    const { client, requests } = makeSyntheticLoadStatusThenEchoKhalaClient()
    const { deps } = baseDeps({
      makeKhalaClient: () => client,
      // Inject a stubbed synthetic-load status source returning one active run.
      makeOperatorTools: () =>
        makeArtanisOperatorTools({
          syntheticLoadStatus: {
            reader: async () => [
              {
                runRef: 'synthetic_load.terminal_bench.2026_06_27_01',
                runType: 'terminal-bench',
                state: 'running',
                targetTokens: 10_000_000,
                tokensBurned: 4_200_000,
              },
            ],
          },
        }),
    })

    const response = await runRoute(
      deps,
      post({
        messages: [{ content: 'show active synthetic loads', role: 'user' }],
      }),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>

    // The loop executed get_synthetic_load_status — read, executed, not deferred.
    const invocations = json.toolInvocations as ReadonlyArray<{
      name: string
      executed: boolean
      deferredToApprovalGate: boolean
      riskyActionKind: string | null
    }>
    const invocation = invocations.find(
      entry => entry.name === 'get_synthetic_load_status',
    )
    expect(invocation).toBeDefined()
    expect(invocation?.executed).toBe(true)
    expect(invocation?.deferredToApprovalGate).toBe(false)
    expect(invocation?.riskyActionKind).toBeNull()

    // At least two Khala calls (request the tool -> feed result -> reply).
    expect(json.iterations as number).toBeGreaterThanOrEqual(2)

    // The final reply carries the REAL summary header sourced from the tool.
    expect(json.reply as string).toContain('Synthetic-load runs (1 active):')

    // Persona separation holds; dogfood via Khala; not deferred to a gate.
    expect((json.persona as { satisfied: boolean }).satisfied).toBe(true)
    expect(json.servedVia).toBe('openagents_khala')
    expect(json.requestedModel).toBe('openagents/khala')
    expect(json.deferredToApprovalGate).toBe(false)

    // The second Khala request carried the real run bytes back into context
    // (run ref + state + token-burn progress), not a fabrication.
    expect(
      requests[1]?.messages.some(
        message =>
          message.role === 'tool' &&
          message.content.includes(
            'synthetic_load.terminal_bench.2026_06_27_01',
          ) &&
          message.content.includes('state=running') &&
          message.content.includes('4,200,000/10,000,000 tokens burned (42%)'),
      ),
    ).toBe(true)
  })
})


// Regression: the route must NEVER hand the owner an empty reply, even when the
// Khala model returns blank content every pass (epic #6359; the live recurrence
// observed as `reply: "" / tools: []`). The operator core forces a final
// tools-suppressed Khala call and, on a still-blank result, substitutes the
// fallback string. This route-level test guards the contract so a future deploy
// cannot silently drop it.
describe('POST /api/operator/artanis/chat — never returns an empty reply (#6359)', () => {
  test('a blank Khala completion yields the non-empty fallback, not ""', async () => {
    const { deps } = baseDeps({
      // Khala always returns blank content with no tool calls.
      makeKhalaClient:
        () =>
        (request: InferenceRequest): Effect.Effect<InferenceResult, InferenceAdapterError> =>
          Effect.succeed({
            content: '   ',
            finishReason: 'stop',
            servedModel:
              request.model === 'openagents/khala' ? 'gpt-oss-120b' : 'wrong',
            usage: { completionTokens: 0, promptTokens: 50, totalTokens: 50 },
          }),
    })
    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'status?', role: 'user' }] }),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>
    expect(typeof json.reply).toBe('string')
    expect((json.reply as string).trim()).not.toBe('')
    expect(json.reply).toBe(ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK)
    expect(json.servedVia).toBe('openagents_khala')
  })
})
