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
  readonly activeTeamMemberships = new Set<string>()
  readonly workspaces = new Map<string, PrefilledWorkspaceRecord>()

  addActiveTeamMembership(teamId: string, userId: string): void {
    this.activeTeamMemberships.add(`${teamId}:${userId}`)
  }

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

  readOrClaimWorkspaceForHolder = async (
    workspaceId: string,
    holderUserId: string,
  ) => {
    const record = this.workspaces.get(workspaceId)

    if (record === undefined) {
      return undefined
    }

    if (record.holderUserId !== null && record.holderUserId !== holderUserId) {
      return undefined
    }

    if (record.holderUserId === null && record.status !== 'invited') {
      return undefined
    }

    const nextRecord: PrefilledWorkspaceRecord = {
      ...record,
      holderUserId,
      status: record.holderUserId === null ? 'active' : record.status,
      engagement: {
        ...record.engagement,
        firstViewedAt: record.engagement.firstViewedAt ?? fixtureNowIso,
        firstClaimedAt:
          record.holderUserId === null
            ? (record.engagement.firstClaimedAt ?? fixtureNowIso)
            : record.engagement.firstClaimedAt,
        lastViewedAt: fixtureNowIso,
        revisitCount:
          record.engagement.firstViewedAt === null
            ? record.engagement.revisitCount
            : record.engagement.revisitCount + 1,
      },
      updatedAt: fixtureNowIso,
    }
    this.workspaces.set(workspaceId, nextRecord)

    return nextRecord
  }

  readPrivateWorkspaceForTeamMember = async (
    workspaceId: string,
    userId: string,
  ) => {
    const record = this.workspaces.get(workspaceId)

    if (
      record === undefined ||
      record.accessMode !== 'private_team' ||
      record.privateTeamId === null ||
      !this.activeTeamMemberships.has(`${record.privateTeamId}:${userId}`)
    ) {
      return undefined
    }

    const nextRecord: PrefilledWorkspaceRecord = {
      ...record,
      engagement: {
        ...record.engagement,
        firstViewedAt: record.engagement.firstViewedAt ?? fixtureNowIso,
        lastViewedAt: fixtureNowIso,
        revisitCount:
          record.engagement.firstViewedAt === null
            ? record.engagement.revisitCount
            : record.engagement.revisitCount + 1,
      },
      updatedAt: fixtureNowIso,
    }
    this.workspaces.set(workspaceId, nextRecord)

    return nextRecord
  }

  readPrivateWorkspaceByTarget = async (
    privateTeamId: string,
    privateProjectId: string | null,
  ) =>
    [...this.workspaces.values()].find(
      record =>
        record.accessMode === 'private_team' &&
        record.privateTeamId === privateTeamId &&
        record.privateProjectId === privateProjectId,
    )

  recordFirstRunForHolder = async (
    workspaceId: string,
    holderUserId: string,
  ) => {
    const record = this.workspaces.get(workspaceId)

    if (record === undefined || record.holderUserId !== holderUserId) {
      return undefined
    }

    const nextRecord: PrefilledWorkspaceRecord = {
      ...record,
      engagement: {
        ...record.engagement,
        firstRunAt: record.engagement.firstRunAt ?? fixtureNowIso,
      },
      updatedAt: fixtureNowIso,
    }
    this.workspaces.set(workspaceId, nextRecord)

    return nextRecord
  }

  recordFirstRunForOperator = async (workspaceId: string) => {
    const record = this.workspaces.get(workspaceId)

    if (record === undefined) {
      return undefined
    }

    const nextRecord: PrefilledWorkspaceRecord = {
      ...record,
      engagement: {
        ...record.engagement,
        firstRunAt: record.engagement.firstRunAt ?? fixtureNowIso,
      },
      updatedAt: fixtureNowIso,
    }
    this.workspaces.set(workspaceId, nextRecord)

    return nextRecord
  }

  recordFirstRunForPrivateTeamMember = async (
    workspaceId: string,
    userId: string,
  ) => {
    const record = await this.readPrivateWorkspaceForTeamMember(
      workspaceId,
      userId,
    )

    if (record === undefined) {
      return undefined
    }

    const nextRecord: PrefilledWorkspaceRecord = {
      ...record,
      engagement: {
        ...record.engagement,
        firstRunAt: record.engagement.firstRunAt ?? fixtureNowIso,
      },
      updatedAt: fixtureNowIso,
    }
    this.workspaces.set(workspaceId, nextRecord)

    return nextRecord
  }
}

const seededRecord = (
  overrides: Partial<PrefilledWorkspaceRecord> = {},
): PrefilledWorkspaceRecord => ({
  accessMode: 'public_safe',
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
  privateProjectId: null,
  privateTeamId: null,
  engagement: {
    invitedAt: fixtureNowIso,
    firstViewedAt: null,
    firstClaimedAt: null,
    firstRunAt: null,
    lastViewedAt: null,
    revisitCount: 0,
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
        engagement: { invitedAt: string | null }
        id: string
        inviteUrl: string
        projectName: string
        starterWorkflows: ReadonlyArray<{ status: string }>
      }
    }
    expect(body.workspace.projectName).toBe('The Hardware Shop')
    expect(body.workspace.inviteUrl).toBe(
      'https://openagents.com/workspaces/workspace_1',
    )
    expect(body.workspace.engagement.invitedAt).toBe(fixtureNowIso)
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
      workspace: {
        engagement: { firstRunAt: string | null }
        holderRef: string
        holderUserId: string | null
        inviteUrl: string
      }
    }
    expect(body.viewer).toBe('operator')
    expect(body.workspace.holderRef).toBe('prospect-ref-001')
    expect(body.workspace.holderUserId).toBe('github:holder')
    expect(body.workspace.inviteUrl).toBe(
      'https://openagents.com/workspaces/workspace_seed',
    )
    expect(body.workspace.engagement.firstRunAt).toBe(null)
  })

  test('operator view includes private workspace access metadata', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(
      seededRecord({
        accessMode: 'private_team',
        holderUserId: null,
        privateProjectId: 'team_project_private',
        privateTeamId: 'team_private',
      }),
    )
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
    const body = (await response.json()) as {
      viewer: string
      workspace: {
        accessMode: string
        privateProjectId: string | null
        privateTeamId: string | null
      }
    }

    expect(response.status).toBe(200)
    expect(body.viewer).toBe('operator')
    expect(body.workspace).toMatchObject({
      accessMode: 'private_team',
      privateProjectId: 'team_project_private',
      privateTeamId: 'team_private',
    })
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
    expect(body.workspace).not.toHaveProperty('inviteUrl')
    expect(body.workspace.projectName).toBe('The Hardware Shop')
  })

  test('first signed-in holder claims an unbound invited workspace and revisits are counted', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(seededRecord({ holderUserId: null }))
    const routes = makeRoutes(store)

    const firstResponse = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces/workspace_seed'),
        { holderUserId: 'github:first-holder' },
        ctx,
      ),
    )
    const secondResponse = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces/workspace_seed'),
        { holderUserId: 'github:first-holder' },
        ctx,
      ),
    )

    expect(firstResponse.status).toBe(200)
    expect(secondResponse.status).toBe(200)
    expect(store.workspaces.get('workspace_seed')).toMatchObject({
      holderUserId: 'github:first-holder',
      status: 'active',
      engagement: {
        firstClaimedAt: fixtureNowIso,
        firstViewedAt: fixtureNowIso,
        lastViewedAt: fixtureNowIso,
        revisitCount: 1,
      },
    })
  })

  test('private team workspace rejects signed-in users without active membership', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(
      seededRecord({
        accessMode: 'private_team',
        holderUserId: null,
        privateProjectId: 'team_project_private',
        privateTeamId: 'team_private',
      }),
    )
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces/workspace_seed'),
        { holderUserId: 'github:not-member' },
        ctx,
      ),
    )

    expect(response.status).toBe(403)
    expect(store.workspaces.get('workspace_seed')).toMatchObject({
      holderUserId: null,
      engagement: {
        firstViewedAt: null,
        lastViewedAt: null,
      },
    })
  })

  test('private team workspace returns seeded material only to active members', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(
      seededRecord({
        accessMode: 'private_team',
        holderUserId: null,
        privateProjectId: 'team_project_private',
        privateTeamId: 'team_private',
      }),
    )
    store.addActiveTeamMembership('team_private', 'github:member')
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request('https://openagents.com/api/workspaces/workspace_seed'),
        { holderUserId: 'github:member' },
        ctx,
      ),
    )
    const body = (await response.json()) as {
      viewer: string
      workspace: Record<string, unknown>
    }

    expect(response.status).toBe(200)
    expect(body.viewer).toBe('team_member')
    expect(body.workspace).not.toHaveProperty('holderUserId')
    expect(body.workspace).not.toHaveProperty('holderRef')
    expect(body.workspace).not.toHaveProperty('privateTeamId')
    expect(body.workspace.accessMode).toBe('private_team')
    expect(body.workspace.seededMemory).toEqual([
      { label: 'voice', value: 'friendly', publicSourceRef: 'https://src' },
    ])
    expect(store.workspaces.get('workspace_seed')).toMatchObject({
      holderUserId: null,
      engagement: {
        firstViewedAt: fixtureNowIso,
        lastViewedAt: fixtureNowIso,
      },
    })
  })

  test('private team first-run engagement requires active membership', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(
      seededRecord({
        accessMode: 'private_team',
        holderUserId: null,
        privateProjectId: 'team_project_private',
        privateTeamId: 'team_private',
      }),
    )
    store.addActiveTeamMembership('team_private', 'github:member')
    const routes = makeRoutes(store)

    const rejected = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request(
          'https://openagents.com/api/workspaces/workspace_seed/engagement',
          {
            body: JSON.stringify({ event: 'first_run' }),
            method: 'POST',
          },
        ),
        { holderUserId: 'github:not-member' },
        ctx,
      ),
    )
    const accepted = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request(
          'https://openagents.com/api/workspaces/workspace_seed/engagement',
          {
            body: JSON.stringify({ event: 'first_run' }),
            method: 'POST',
          },
        ),
        { holderUserId: 'github:member' },
        ctx,
      ),
    )

    expect(rejected.status).toBe(403)
    expect(accepted.status).toBe(200)
    expect(store.workspaces.get('workspace_seed')?.engagement.firstRunAt).toBe(
      fixtureNowIso,
    )
  })

  test('holder first-run engagement is recorded without exposing operator fields', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(seededRecord())
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request(
          'https://openagents.com/api/workspaces/workspace_seed/engagement',
          {
            body: JSON.stringify({ event: 'first_run' }),
            method: 'POST',
          },
        ),
        { holderUserId: 'github:holder' },
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
    expect(store.workspaces.get('workspace_seed')?.engagement.firstRunAt).toBe(
      fixtureNowIso,
    )
  })

  test('operator can record first-run engagement for inspection', async () => {
    const store = new MemoryWorkspaceStore()
    store.seed(seededRecord({ holderUserId: null }))
    const routes = makeRoutes(store)

    const response = await run(
      routes.routePrefilledWorkspaceRequest(
        new Request(
          'https://openagents.com/api/workspaces/workspace_seed/engagement',
          {
            body: JSON.stringify({ event: 'first_run' }),
            headers: { authorization: 'Bearer op-secret' },
            method: 'POST',
          },
        ),
        { operatorToken: 'op-secret' },
        ctx,
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      viewer: string
      workspace: { engagement: { firstRunAt: string | null } }
    }
    expect(body.viewer).toBe('operator')
    expect(body.workspace.engagement.firstRunAt).toBe(fixtureNowIso)
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
