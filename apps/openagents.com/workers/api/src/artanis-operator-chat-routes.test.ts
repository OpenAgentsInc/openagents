import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  ArtanisMemoryAppendInput,
  ArtanisMemoryEntry,
  ArtanisMemoryLoadInput,
  ArtanisOwnerMemoryStore,
} from './artanis-owner-memory'
import { makeOperatorArtanisChatRoutes } from './artanis-operator-chat-routes'
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
