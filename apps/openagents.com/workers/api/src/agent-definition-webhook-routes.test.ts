import {
  decodeAgentDefinition,
  type AgentDefinition,
} from '@openagentsinc/agent-runtime-schema'
import { fulfillmentLoopAgentDefinitionFixture } from '@openagentsinc/agent-runtime-schema/fixtures'
import { describe, expect, test } from 'vitest'

import {
  type AgentDefinitionWebhookRouteDependencies,
  handleAgentDefinitionWebhookRequest,
} from './agent-definition-webhook-routes'
import type {
  AgentDefinitionRunDispatchDependencies,
  AgentDefinitionRunDispatchOutcome,
} from './agent-definition-run-routes'
import type { DueAgentDefinitionTriggerRecord } from './agent-definition-trigger-store'

const encoder = new TextEncoder()
const ownerAgentUserId = 'agent_user_webhook_owner'
const nowIso = '2026-07-03T16:15:00.000Z'
const secret = 'github-webhook-secret'

const hex = (bytes: ArrayBuffer): string =>
  [...new Uint8Array(bytes)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer =>
  bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer

const signGitHubBody = async (body: string, signingSecret = secret) => {
  const key = await crypto.subtle.importKey(
    'raw',
    arrayBufferFromBytes(encoder.encode(signingSecret)),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )
  const digest = await crypto.subtle.sign('HMAC', key, encoder.encode(body))

  return `sha256=${hex(digest)}`
}

const githubIssuePayload = {
  action: 'opened',
  issue: {
    html_url: 'https://github.com/OpenAgentsInc/openagents/issues/8195',
    number: 8195,
    state: 'open',
    title: 'BA-B3 webhook ingress',
  },
  repository: {
    full_name: 'OpenAgentsInc/openagents',
    id: 123,
    name: 'openagents',
    owner: {
      id: 456,
      login: 'OpenAgentsInc',
    },
  },
  sender: {
    id: 789,
    login: 'AtlantisPleb',
  },
}

const makeDefinition = (
  repository = 'OpenAgentsInc/openagents',
): AgentDefinition =>
  decodeAgentDefinition({
    ...fulfillmentLoopAgentDefinitionFixture,
    id: 'agent_definition.public.webhook_test',
    ownerRef: `agent:${ownerAgentUserId}`,
    lane: 'own_pylon',
    triggers: [
      {
        kind: 'inbound_webhook',
        triggerRef: 'trigger.public.webhook.github_issue',
        source: 'github',
        conditions: [
          {
            kind: 'event_type',
            equals: 'issues.opened',
          },
          {
            kind: 'json_path_equals',
            path: '$.repository.full_name',
            equals: repository,
          },
        ],
      },
    ],
  })

const triggerRecord = (
  definition: AgentDefinition,
): DueAgentDefinitionTriggerRecord => {
  const trigger = definition.triggers[0]
  expect(trigger).toBeDefined()

  return {
    schema: 'openagents.agent_definition_trigger.v1',
    consecutiveFailures: 0,
    createdAt: '2026-07-03T16:00:00.000Z',
    definitionId: definition.id,
    ownerAgentUserId,
    ownerRef: definition.ownerRef,
    state: 'enabled',
    trigger: trigger!,
    triggerId: `${definition.id}:${trigger!.triggerRef}`,
    triggerRef: trigger!.triggerRef,
    updatedAt: '2026-07-03T16:00:00.000Z',
  }
}

class MemoryTriggerStore {
  readonly failures: Array<{
    readonly ownerAgentUserId: string
    readonly triggerRef: string
    readonly updatedAt: string
  }> = []
  readonly successes: Array<{
    readonly nextRunAt: string | undefined
    readonly ownerAgentUserId: string
    readonly triggerRef: string
    readonly updatedAt: string
  }> = []

  constructor(private readonly rows: ReadonlyArray<DueAgentDefinitionTriggerRecord>) {}

  listInboundWebhookTriggers(
    source: string,
    _limit: number,
  ): Promise<ReadonlyArray<DueAgentDefinitionTriggerRecord>> {
    return Promise.resolve(
      this.rows.filter(row =>
        row.trigger.kind === 'inbound_webhook' &&
        row.trigger.source === source
      ),
    )
  }

  recordTriggerFailure(
    owner: string,
    triggerRef: string,
    updatedAt: string,
  ): Promise<boolean> {
    this.failures.push({ ownerAgentUserId: owner, triggerRef, updatedAt })

    return Promise.resolve(true)
  }

  recordTriggerSuccess(
    owner: string,
    triggerRef: string,
    nextRunAt: string | undefined,
    updatedAt: string,
  ): Promise<boolean> {
    this.successes.push({
      nextRunAt,
      ownerAgentUserId: owner,
      triggerRef,
      updatedAt,
    })

    return Promise.resolve(true)
  }
}

const dispatchedOutcome = (): AgentDefinitionRunDispatchOutcome => ({
  assignmentRef: 'assignment.background.webhook',
  durableStreamUrl: 'https://openagents.com/v1/stream/webhook',
  kind: 'dispatched',
  record: {} as never,
  seeded: false,
})

const dependenciesFor = (
  input: Readonly<{
    definition: AgentDefinition
    dispatchRun: AgentDefinitionWebhookRouteDependencies['dispatchRun']
    triggerStore: MemoryTriggerStore
  }>,
): AgentDefinitionWebhookRouteDependencies => ({
  definitionStore: {
    readDefinition: (owner, definitionId) =>
      Promise.resolve(
        owner === ownerAgentUserId && definitionId === input.definition.id
          ? input.definition
          : undefined,
      ),
  },
  dispatchDependencies: {
    forgeStore: {} as never,
    pylonStore: {} as never,
    runStore: {} as never,
  },
  dispatchRun: input.dispatchRun,
  githubSecret: secret,
  nowIso: () => nowIso,
  triggerStore: input.triggerStore,
})

const githubRequest = async (
  payload: Record<string, unknown>,
  signingSecret = secret,
) => {
  const body = JSON.stringify(payload)

  return new Request(
    'https://openagents.com/v1/agent-definitions/webhooks/github',
    {
      body,
      headers: {
        'Content-Type': 'application/json',
        'X-GitHub-Delivery': 'delivery-8195',
        'X-GitHub-Event': 'issues',
        'X-Hub-Signature-256': await signGitHubBody(body, signingSecret),
      },
      method: 'POST',
    },
  )
}

describe('agent definition webhook routes', () => {
  test('verifies GitHub signatures, matches trigger conditions, and dispatches owner-scoped runs', async () => {
    const definition = makeDefinition()
    const triggerStore = new MemoryTriggerStore([triggerRecord(definition)])
    const dispatches: Array<{
      readonly dependencies: AgentDefinitionRunDispatchDependencies
      readonly triggerPayload: Record<string, unknown> | undefined
      readonly triggerRef: string | undefined
    }> = []
    const response = await handleAgentDefinitionWebhookRequest(
      await githubRequest(githubIssuePayload),
      dependenciesFor({
        definition,
        dispatchRun: (dependencies, input) => {
          dispatches.push({
            dependencies,
            triggerPayload: input.request.triggerPayload,
            triggerRef: input.request.triggerRef,
          })

          return Promise.resolve(dispatchedOutcome())
        },
        triggerStore,
      }),
    )
    const body = await response?.json()

    expect(response?.status).toBe(202)
    expect(body).toMatchObject({
      schema: 'openagents.agent_definition_webhook_ingress.v1',
      deliveryId: 'delivery-8195',
      dispatched: 1,
      eventType: 'issues.opened',
      matched: 1,
      source: 'github',
    })
    expect(dispatches).toHaveLength(1)
    expect(dispatches[0]?.dependencies.linkedAgents).toEqual([
      { agentUserId: ownerAgentUserId },
    ])
    expect(dispatches[0]?.triggerRef).toBe(
      'trigger.public.webhook.github_issue',
    )
    expect(dispatches[0]?.triggerPayload).toMatchObject({
      schema: 'openagents.agent_definition_webhook_event.v1',
      eventType: 'issues.opened',
      triggerRef: 'trigger.public.webhook.github_issue',
      payload: {
        repository: {
          full_name: 'OpenAgentsInc/openagents',
        },
      },
    })
    expect(triggerStore.successes).toEqual([
      {
        nextRunAt: undefined,
        ownerAgentUserId,
        triggerRef: 'trigger.public.webhook.github_issue',
        updatedAt: nowIso,
      },
    ])
  })

  test('rejects invalid GitHub signatures before reading triggers', async () => {
    const definition = makeDefinition()
    const triggerStore = new MemoryTriggerStore([triggerRecord(definition)])
    const response = await handleAgentDefinitionWebhookRequest(
      await githubRequest(githubIssuePayload, 'wrong-secret'),
      dependenciesFor({
        definition,
        dispatchRun: () => Promise.resolve(dispatchedOutcome()),
        triggerStore,
      }),
    )

    expect(response?.status).toBe(401)
    expect(triggerStore.successes).toEqual([])
    expect(triggerStore.failures).toEqual([])
  })

  test('skips enabled webhook triggers whose normalized conditions do not match', async () => {
    const definition = makeDefinition('OpenAgentsInc/not-this-repo')
    const triggerStore = new MemoryTriggerStore([triggerRecord(definition)])
    const response = await handleAgentDefinitionWebhookRequest(
      await githubRequest(githubIssuePayload),
      dependenciesFor({
        definition,
        dispatchRun: () => Promise.resolve(dispatchedOutcome()),
        triggerStore,
      }),
    )
    const body = await response?.json()

    expect(response?.status).toBe(202)
    expect(body).toMatchObject({
      dispatched: 0,
      matched: 0,
      skipped: 1,
    })
    expect(triggerStore.successes).toEqual([])
    expect(triggerStore.failures).toEqual([])
  })
})
