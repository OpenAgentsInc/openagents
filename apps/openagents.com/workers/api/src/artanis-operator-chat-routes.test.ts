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

  test('403 when the session email is not an admin', async () => {
    const { deps } = baseDeps({
      requireBrowserSession: async () => ({
        user: { email: 'someone@example.com', userId: 'github:9' },
      }),
    })
    const response = await runRoute(
      deps,
      post({ messages: [{ content: 'hi', role: 'user' }] }),
    )
    expect(response.status).toBe(403)
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

