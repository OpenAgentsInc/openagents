import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
} from './agent-registration'
import {
  OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES,
} from './autopilot-work-request'
import {
  type AutopilotWorkOrderRecord,
  type AutopilotWorkStore,
  makeAutopilotWorkRoutes,
} from './autopilot-work-routes'

class MemoryAutopilotWorkStore implements AutopilotWorkStore {
  readonly records = new Map<string, AutopilotWorkOrderRecord>()
  readonly recordsByOwnerIdempotency = new Map<string, AutopilotWorkOrderRecord>()

  createWorkOrder = async (record: AutopilotWorkOrderRecord) => {
    const key = `${record.ownerUserId}:${record.idempotencyKeyHash}`
    const existing = this.recordsByOwnerIdempotency.get(key)

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    this.records.set(record.workOrderRef, record)
    this.recordsByOwnerIdempotency.set(key, record)

    return { idempotent: false, record }
  }

  readWorkOrder = async (workOrderRef: string) =>
    this.records.get(workOrderRef)

  readWorkOrderByIdempotency = async (
    ownerUserId: string,
    idempotencyKeyHash: string,
  ) => this.recordsByOwnerIdempotency.get(`${ownerUserId}:${idempotencyKeyHash}`)
}

const agentToken = `${AGENT_TOKEN_PREFIX}autopilot-work-test`

const agentStoreForScopes = (
  scopes: ReadonlyArray<string> = [
    'customer_orders.read',
    'customer_orders.write',
  ],
): AgentRegistrationStore => ({
  createAgentRegistration: () => Promise.resolve(),
  findAgentByTokenHash: () =>
    Promise.resolve({
      credentialId: 'agent_credential_autopilot_work_test',
      profileMetadataJson: JSON.stringify({
        customerOrderGrants: [
          {
            expiresAt: null,
            ownerUserId: 'github:autopilot-owner',
            scopes,
            status: 'active',
          },
        ],
      }),
      tokenPrefix: `${AGENT_TOKEN_PREFIX}autopilot`,
      user: {
        avatarUrl: null,
        createdAt: '2026-06-09T17:30:00.000Z',
        displayName: 'Autopilot Work Agent',
        id: 'agent_user_autopilot_work',
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: '2026-06-09T17:30:00.000Z',
      },
    }),
  touchAgentCredential: () => Promise.resolve(),
})

const route = async (
  store: MemoryAutopilotWorkStore,
  path: string,
  options: Readonly<{
    body?: unknown
    idempotencyKey?: string
    method?: string
    scopes?: ReadonlyArray<string>
    token?: string
  }> = {},
) => {
  let counter = 0
  const routes = makeAutopilotWorkRoutes<Record<string, unknown>>({
    agentStore: () => agentStoreForScopes(options.scopes),
    makeId: () => `autopilot_work_order.test_${++counter}`,
    makeStore: () => store,
    nowIso: () => '2026-06-09T17:30:00.000Z',
  })
  const body = options.body === undefined
    ? {}
    : { body: JSON.stringify(options.body) }
  const request = new Request(`https://openagents.com${path}`, {
    ...body,
    headers: {
      ...(options.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(options.idempotencyKey === undefined
        ? {}
        : { 'Idempotency-Key': options.idempotencyKey }),
      ...(options.token === undefined
        ? { authorization: `Bearer ${agentToken}` }
        : options.token === ''
          ? {}
          : { authorization: `Bearer ${options.token}` }),
    },
    method: options.method ?? (options.body === undefined ? 'GET' : 'POST'),
  })
  const response = routes.routeAutopilotWorkRequest(request, {})

  if (response === undefined) {
    throw new Error(`No Autopilot work route matched ${path}`)
  }

  return Effect.runPromise(response)
}

const responseJson = async (response: Response) =>
  response.json() as Promise<Readonly<{
    error?: string
    work?: Readonly<{
      idempotent: boolean
      state: string
      taskRefs: ReadonlyArray<string>
      workOrderRef: string
    }>
  }>>

describe('Autopilot work routes', () => {
  test('creates and recovers the same work projection with an idempotency key', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
        },
      ],
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-create',
    })
    const replay = await route(store, '/api/autopilot/work', {
      body: {
        prompt: 'This malformed replay body should not replace the record.',
      },
      idempotencyKey: 'idem-autopilot-work-create',
    })
    const firstJson = await responseJson(first)
    const replayJson = await responseJson(replay)

    expect(first.status).toBe(202)
    expect(replay.status).toBe(200)
    expect(firstJson.work).toMatchObject({
      idempotent: false,
      state: 'accepted_free_slice',
      taskRefs: ['task.autopilot_coder.docs_contract'],
      workOrderRef: 'autopilot_work_order.test_1',
    })
    expect(replayJson.work).toEqual({
      ...firstJson.work,
      idempotent: true,
    })

    const detail = await route(
      store,
      `/api/autopilot/work/${firstJson.work?.workOrderRef}`,
      { method: 'GET' },
    )
    const detailJson = await responseJson(detail)

    expect(detail.status).toBe(200)
    expect(detailJson.work).toEqual(firstJson.work)
  })

  test('requires idempotency on create', async () => {
    const response = await route(
      new MemoryAutopilotWorkStore(),
      '/api/autopilot/work',
      {
        body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      },
    )
    const body = await responseJson(response)

    expect(response.status).toBe(400)
    expect(body.error).toBe('autopilot_work_validation_error')
  })

  test('requires a registered agent grant for create and read', async () => {
    const create = await route(
      new MemoryAutopilotWorkStore(),
      '/api/autopilot/work',
      {
        body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        idempotencyKey: 'idem-autopilot-work-unauthorized',
        token: '',
      },
    )
    const read = await route(
      new MemoryAutopilotWorkStore(),
      '/api/autopilot/work/autopilot_work_order.test_1',
      {
        method: 'GET',
        token: '',
      },
    )

    expect(create.status).toBe(401)
    expect(read.status).toBe(401)
  })

  test('requires read scope for detail recovery', async () => {
    const store = new MemoryAutopilotWorkStore()
    const create = await route(store, '/api/autopilot/work', {
      body: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [
          {
            ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
            accessRequests: [],
          },
        ],
      },
      idempotencyKey: 'idem-autopilot-work-read-scope',
    })
    const createJson = await responseJson(create)
    const read = await route(
      store,
      `/api/autopilot/work/${createJson.work?.workOrderRef}`,
      {
        method: 'GET',
        scopes: ['customer_orders.write'],
      },
    )

    expect(read.status).toBe(401)
  })
})
