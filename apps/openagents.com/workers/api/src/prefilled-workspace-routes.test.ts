import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type CreatePrefilledWorkspaceInput,
  type PrefilledWorkspaceRecord,
  type PrefilledWorkspaceServiceShape,
  makePrefilledWorkspaceRecord,
} from './prefilled-workspace'
import { makePrefilledWorkspaceRoutes } from './prefilled-workspace-routes'

const fixtureNowIso = '2026-06-16T12:00:00.000Z'

type Bindings = Readonly<{ holderUserId?: string; operatorToken?: string }>

const ctx = {
  waitUntil: () => {},
  passThroughOnException: () => {},
} as unknown as ExecutionContext

const recordRuntime = {
  makeId: (prefix: string) => `${prefix}_1`,
  nowIso: () => fixtureNowIso,
}

// In-memory store implementing the service shape so the route test exercises
// routing, operator create, operator-vs-holder read, holder scoping, and auth.
class MemoryWorkspaceStore implements PrefilledWorkspaceServiceShape {
  readonly workspaces = new Map<string, PrefilledWorkspaceRecord>()

  seed(record: PrefilledWorkspaceRecord): void {
    this.workspaces.set(record.id, record)
  }

  createWorkspace = async (input: CreatePrefilledWorkspaceInput) => {
    const record = makePrefilledWorkspaceRecord(input, recordRuntime)
    this.workspaces.set(record.id, record)
    return record
  }

  readWorkspace = async (workspaceId: string) =>
    this.workspaces.get(workspaceId)

  readWorkspaceForHolder = async (workspaceId: string, holderUserId: string) => {
    const record = this.workspaces.get(workspaceId)
    return record !== undefined && record.holderUserId === holderUserId
      ? record
      : undefined
  }
}

const seededRecord = (
  overrides: Partial<PrefilledWorkspaceRecord> = {},
): PrefilledWorkspaceRecord => ({
  id: 'workspace_seed',
  holderUserId: 'github:holder',
  holderRef: 'prospect-ref-001',
  projectName: 'The Hardware Shop',
  status: 'invited',
  seededMemory: [
    { label: 'voice', value: 'friendly', publicSourceRef: 'https://src' },
  ],
  starterWorkflows: [
    {
      title: 'Run a campaign',
      description: 'A starter campaign.',
      outcomeKind: 'campaign',
      status: 'queued',
    },
  ],
  introReceipt: {
    summary: 'Seeded from public sources.',
    publicSourceRefs: ['https://src'],
  },
  createdAt: fixtureNowIso,
  updatedAt: fixtureNowIso,
  ...overrides,
})

const makeRoutes = (store: MemoryWorkspaceStore) =>
  makePrefilledWorkspaceRoutes<Bindings>({
    makeStore: () => store,
    nowIso: () => fixtureNowIso,
    requireHolderUserId: async (_request, env) => env.holderUserId,
    requireOperator: async (request, env) =>
      env.operatorToken !== undefined &&
      request.headers.get('authorization') === `Bearer ${env.operatorToken}`,
  })

const run = (effect: Effect.Effect<Response> | undefined) => {
  if (effect === undefined) {
    throw new Error('route did not match')
  }
  return Effect.runPromise(effect)
}

describe('prefilled workspace routes', () => {
  test('operator can create a workspace', async () => {
    const store = new MemoryWorkspaceStore()
    const routes = makeRoutes(store)
    const env: Bindings = { operatorToken: 'op-secret' }

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces', {
          method: 'POST',
          headers: { authorization: 'Bearer op-secret' },
          body: JSON.stringify({
            projectName: 'The Hardware Shop',
            holderRef: 'prospect-ref-001',
            holderUserId: 'github:holder',
            status: 'invited',
            introReceipt: {
              summary: 'Seeded from public storefront.',
              publicSourceRefs: ['https://example.com'],
            },
            seededMemory: [
              {
                label: 'voice',
                value: 'friendly',
                publicSourceRef: 'https://example.com/about',
              },
            ],
            starterWorkflows: [
              {
                title: 'Run a campaign',
                description: 'A starter campaign.',
                outcomeKind: 'campaign',
              },
            ],
          }),
        }),
        env,
        ctx,
      ),
    )

    expect(response.status).toBe(201)
    const body = (await response.json()) as {
      workspace: {
        id: string
        projectName: string
        starterWorkflows: ReadonlyArray<{ status: string }>
      }
    }
    expect(body.workspace.projectName).toBe('The Hardware Shop')
    expect(body.workspace.starterWorkflows[0]?.status).toBe('queued')
    expect(store.workspaces.size).toBe(1)
  })

  test('create requires operator auth', async () => {
    const store = new MemoryWorkspaceStore()
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces', {
          method: 'POST',
          body: JSON.stringify({
            projectName: 'X',
            introReceipt: { summary: 's', publicSourceRefs: [] },
          }),
        }),
        { operatorToken: 'op-secret' },
        ctx,
      ),
    )

    expect(response.status).toBe(403)
    expect(store.workspaces.size).toBe(0)
  })

  test('create rejects invalid body', async () => {
    const store = new MemoryWorkspaceStore()
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces', {
          method: 'POST',
          headers: { authorization: 'Bearer op-secret' },
          body: JSON.stringify({ projectName: 'X' }),
        }),
        { operatorToken: 'op-secret' },
        ctx,
      ),
    )

    expect(response.status).toBe(400)
  })

  test('operator read returns full workspace view', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(seededRecord())
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces/workspace_seed', {
          headers: { authorization: 'Bearer op-secret' },
        }),
        { operatorToken: 'op-secret' },
        ctx,
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      viewer: string
      workspace: { holderRef: string; holderUserId: string | null }
    }
    expect(body.viewer).toBe('operator')
    expect(body.workspace.holderRef).toBe('prospect-ref-001')
    expect(body.workspace.holderUserId).toBe('github:holder')
  })

  test('bound holder gets the public-safe projection without operator fields', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(seededRecord())
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces/workspace_seed'),
        { holderUserId: 'github:holder', operatorToken: 'op-secret' },
        ctx,
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      viewer: string
      workspace: Record<string, unknown>
    }
    expect(body.viewer).toBe('holder')
    expect(body.workspace).not.toHaveProperty('holderUserId')
    expect(body.workspace).not.toHaveProperty('holderRef')
    expect(body.workspace.projectName).toBe('The Hardware Shop')
  })

  test('a different holder cannot read someone else workspace', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(seededRecord())
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces/workspace_seed'),
        { holderUserId: 'github:other' },
        ctx,
      ),
    )

    expect(response.status).toBe(404)
  })

  test('anonymous non-operator read is unauthorized', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(seededRecord())
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces/workspace_seed'),
        {},
        ctx,
      ),
    )

    expect(response.status).toBe(401)
  })

  test('unmatched path returns undefined', () => {
    const store = new MemoryWorkspaceStore()
    const routes = makeRoutes(store)
    expect(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/other'),
        {},
        ctx,
      ),
    ).toBeUndefined()
  })
})
