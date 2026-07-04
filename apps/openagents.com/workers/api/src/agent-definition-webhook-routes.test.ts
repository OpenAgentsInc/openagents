import {
  decodeAgentDefinition,
  type AgentDefinition,
} from '@openagentsinc/agent-runtime-schema'
import { fulfillmentLoopAgentDefinitionFixture } from '@openagentsinc/agent-runtime-schema/fixtures'
import { describe, expect, test } from 'vitest'

import {
  type AgentDefinitionWebhookRouteDependencies,
  handleAgentDefinitionForumCompletionRequest,
  handleAgentDefinitionForumWebhookRequest,
  handleAgentDefinitionGitHubCompletionRequest,
  handleAgentDefinitionWebhookRequest,
} from './agent-definition-webhook-routes'
import type {
  AgentDefinitionForumCompletionCallback,
  AgentDefinitionForumCompletionForumStore,
  AgentDefinitionGitHubCompletionCallback,
  AgentDefinitionGitHubCompletionGitHubStore,
} from './agent-definition-bot-integration'
import type {
  AgentDefinitionRunDispatchDependencies,
  AgentDefinitionRunDispatchOutcome,
  AgentDefinitionRunRecord,
} from './agent-definition-run-routes'
import type { DueAgentDefinitionTriggerRecord } from './agent-definition-trigger-store'
import type { EventLedgerIngestQueueMessage } from './event-ledger'
import { Effect } from 'effect'

const encoder = new TextEncoder()
const ownerAgentUserId = 'agent_user_webhook_owner'
const nowIso = '2026-07-03T16:15:00.000Z'
const secret = 'github-webhook-secret'
const forumSecret = 'forum-webhook-secret'

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

const signOpenAgentsBody = async (
  body: string,
  signingSecret = forumSecret,
) => signGitHubBody(body, signingSecret)

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

const githubMentionCommentPayload = {
  action: 'created',
  comment: {
    author_association: 'MEMBER',
    body:
      '@OpenAgents please run the bounded definition. Secret-ish prose must not leak.',
    html_url:
      'https://github.com/OpenAgentsInc/openagents/issues/8209#issuecomment-1001',
    id: 1001,
    user: {
      id: 790,
      login: 'AtlantisPleb',
    },
  },
  issue: {
    html_url: 'https://github.com/OpenAgentsInc/openagents/issues/8209',
    number: 8209,
    state: 'open',
    title: 'BA-G2 GitHub @mention runs',
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

const forumPostPayload = {
  actorDisplayName: 'OpenAgents Operator',
  actorRef: 'user:owner_123',
  actorSlug: 'openagents-operator',
  deliveryId: 'forum-delivery-8208',
  eventType: 'forum.post.created',
  forumId: 'forum_product_promises',
  forumSlug: 'product-promises',
  forumTitle: 'Product Promises',
  postId: 'post_forum_trigger_001',
  postNumber: 3,
  postState: 'visible',
  sourceUrl:
    'https://openagents.com/forum/t/topic_forum_trigger_001#post_forum_trigger_001',
  topicId: 'topic_forum_trigger_001',
  topicSlug: 'ship-background-agents',
  topicState: 'open',
  topicTitle: 'Ship background agents',
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

const makeGitHubMentionDefinition = (): AgentDefinition =>
  decodeAgentDefinition({
    ...fulfillmentLoopAgentDefinitionFixture,
    id: 'agent_definition.public.github_mention_webhook_test',
    ownerRef: `agent:${ownerAgentUserId}`,
    lane: 'own_pylon',
    triggers: [
      {
        kind: 'inbound_webhook',
        triggerRef: 'trigger.public.webhook.github_mention',
        source: 'github',
        conditions: [
          {
            kind: 'event_type',
            equals: 'issue_comment.created.mention',
          },
          {
            kind: 'json_path_equals',
            path: '$.repository.full_name',
            equals: 'OpenAgentsInc/openagents',
          },
          {
            kind: 'json_path_equals',
            path: '$.mention.target_login',
            equals: 'openagents',
          },
        ],
      },
    ],
  })

const makeForumDefinition = (): AgentDefinition =>
  decodeAgentDefinition({
    ...fulfillmentLoopAgentDefinitionFixture,
    id: 'agent_definition.public.forum_webhook_test',
    ownerRef: `agent:${ownerAgentUserId}`,
    lane: 'own_pylon',
    triggers: [
      {
        kind: 'inbound_webhook',
        triggerRef: 'trigger.public.webhook.forum_post',
        source: 'forum',
        conditions: [
          {
            kind: 'event_type',
            equals: 'forum.post.created',
          },
          {
            kind: 'json_path_equals',
            path: '$.forum.slug',
            equals: 'product-promises',
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
    eventLedgerMessages?: Array<EventLedgerIngestQueueMessage> | undefined
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
    forgeGitAuthStore: {} as never,
    forgeStore: {} as never,
    pylonStore: {} as never,
    runStore: {} as never,
  },
  dispatchRun: input.dispatchRun,
  eventLedgerEnqueue:
    input.eventLedgerMessages === undefined
      ? undefined
      : message => {
          input.eventLedgerMessages?.push(message)

          return Promise.resolve()
        },
  githubSecret: secret,
  nowIso: () => nowIso,
  triggerStore: input.triggerStore,
})

const githubRequest = async (
  payload: Record<string, unknown>,
  signingSecret = secret,
  eventName = 'issues',
  deliveryId = 'delivery-8195',
) => {
  const body = JSON.stringify(payload)

  return new Request(
    'https://openagents.com/v1/agent-definitions/webhooks/github',
    {
      body,
      headers: {
        'Content-Type': 'application/json',
        'X-GitHub-Delivery': deliveryId,
        'X-GitHub-Event': eventName,
        'X-Hub-Signature-256': await signGitHubBody(body, signingSecret),
      },
      method: 'POST',
    },
  )
}

const githubCompletionRequest = async (
  payload: Record<string, unknown>,
  signingSecret = secret,
) => {
  const body = JSON.stringify(payload)

  return new Request(
    'https://openagents.com/v1/agent-definitions/webhooks/github/completions',
    {
      body,
      headers: {
        'Content-Type': 'application/json',
        'X-OpenAgents-Signature-256': await signOpenAgentsBody(
          body,
          signingSecret,
        ),
      },
      method: 'POST',
    },
  )
}

const forumRequest = async (
  payload: Record<string, unknown>,
  signingSecret = forumSecret,
) => {
  const body = JSON.stringify(payload)

  return new Request(
    'https://openagents.com/v1/agent-definitions/webhooks/forum',
    {
      body,
      headers: {
        'Content-Type': 'application/json',
        'X-OpenAgents-Delivery': 'forum-delivery-8208',
        'X-OpenAgents-Event': 'forum.post.created',
        'X-OpenAgents-Signature-256': await signOpenAgentsBody(
          body,
          signingSecret,
        ),
      },
      method: 'POST',
    },
  )
}

const forumCompletionRequest = async (
  payload: Record<string, unknown>,
  signingSecret = forumSecret,
) => {
  const body = JSON.stringify(payload)

  return new Request(
    'https://openagents.com/v1/agent-definitions/webhooks/forum/completions',
    {
      body,
      headers: {
        'Content-Type': 'application/json',
        'X-OpenAgents-Signature-256': await signOpenAgentsBody(
          body,
          signingSecret,
        ),
      },
      method: 'POST',
    },
  )
}

class MemoryForumStore implements AgentDefinitionForumCompletionForumStore {
  readonly createdReplies: Array<Parameters<
    AgentDefinitionForumCompletionForumStore['createReplyPost']
  >[0]> = []
  readonly policyChecks: Array<{
    readonly actorRef: string
    readonly bodyText: string
  }> = []
  existingPost: unknown = null

  createReplyPost = (input: Parameters<
    AgentDefinitionForumCompletionForumStore['createReplyPost']
  >[0]) => {
    this.createdReplies.push(input)

    return Effect.succeed({
      postId: input.postId,
      topicId: input.topicId,
    } as never)
  }

  enforceReplyPolicy: AgentDefinitionForumCompletionForumStore['enforceReplyPolicy'] =
    input => {
      this.policyChecks.push({
        actorRef: input.actorRef,
        bodyText: input.bodyText,
      })

      return Effect.succeed(null)
    }

  readPostByIdempotencyKey: AgentDefinitionForumCompletionForumStore['readPostByIdempotencyKey'] =
    () => Effect.succeed(this.existingPost as never)

  readPostDetail: AgentDefinitionForumCompletionForumStore['readPostDetail'] =
    postId =>
      Effect.succeed({
        containingTopicId: 'topic_forum_trigger_001',
        post: {
          postId,
          state: 'visible',
        },
      } as never)

  readSummaryByRef: AgentDefinitionForumCompletionForumStore['readSummaryByRef'] =
    forumRef =>
      Effect.succeed({
        forumId: forumRef,
        locked: false,
        slug: 'product-promises',
      } as never)

  readTopicById: AgentDefinitionForumCompletionForumStore['readTopicById'] =
    topicId =>
      Effect.succeed({
        forumId: 'forum_product_promises',
        state: 'open',
        topicId,
      } as never)
}

const forumCallback = (): AgentDefinitionForumCompletionCallback => ({
  schema: 'openagents.agent_definition_forum_completion_callback.v1',
  forumId: 'forum_product_promises',
  kind: 'forum_reply',
  source: 'forum',
  sourcePostId: 'post_forum_trigger_001',
  sourceUrl:
    'https://openagents.com/forum/t/topic_forum_trigger_001#post_forum_trigger_001',
  subjectRef:
    'forum.topic.topic_forum_trigger_001.post.post_forum_trigger_001',
  topicId: 'topic_forum_trigger_001',
})

const forumRunRecord = (): AgentDefinitionRunRecord =>
  ({
    assignmentRef: 'assignment.background.webhook',
    definitionId: 'agent_definition.public.forum_webhook_test',
    definitionRef: 'agent_definition.agent_definition.public.forum_webhook_test',
    runId: 'agent_definition_run.forum_001',
    triggerPayload: {
      completionCallback: forumCallback(),
    },
  }) as unknown as AgentDefinitionRunRecord

class MemoryGitHubCompletionStore
  implements AgentDefinitionGitHubCompletionGitHubStore
{
  readonly createdComments: Array<Parameters<
    AgentDefinitionGitHubCompletionGitHubStore['createIssueComment']
  >[0]> = []
  readonly existingByKey = new Map<
    string,
    { readonly commentId: number; readonly htmlUrl: string }
  >()

  createIssueComment: AgentDefinitionGitHubCompletionGitHubStore['createIssueComment'] =
    input => {
      this.createdComments.push(input)
      const comment = {
        commentId: 10010,
        htmlUrl:
          'https://github.com/OpenAgentsInc/openagents/issues/8209#issuecomment-10010',
      }
      this.existingByKey.set(input.idempotencyKey, comment)

      return Effect.succeed(comment)
    }

  readCompletionComment: AgentDefinitionGitHubCompletionGitHubStore['readCompletionComment'] =
    input => Effect.succeed(this.existingByKey.get(input.idempotencyKey) ?? null)
}

const githubCallback = (): AgentDefinitionGitHubCompletionCallback => ({
  schema: 'openagents.agent_definition_github_completion_callback.v1',
  kind: 'github_issue_comment',
  mentionLogin: 'openagents',
  number: 8209,
  owner: 'OpenAgentsInc',
  repo: 'openagents',
  repositoryFullName: 'OpenAgentsInc/openagents',
  source: 'github',
  sourceCommentId: 1001,
  sourceUrl:
    'https://github.com/OpenAgentsInc/openagents/issues/8209#issuecomment-1001',
  subjectKind: 'issue',
  subjectRef: 'github.repository.OpenAgentsInc/openagents.issue.8209',
})

const githubRunRecord = (): AgentDefinitionRunRecord =>
  ({
    assignmentRef: 'assignment.background.webhook.github',
    definitionId: 'agent_definition.public.github_mention_webhook_test',
    definitionRef:
      'agent_definition.agent_definition.public.github_mention_webhook_test',
    runId: 'agent_definition_run.github_001',
    triggerPayload: {
      completionCallback: githubCallback(),
    },
  }) as unknown as AgentDefinitionRunRecord

describe('agent definition webhook routes', () => {
  test('verifies GitHub signatures, matches trigger conditions, and dispatches owner-scoped runs', async () => {
    const definition = makeDefinition()
    const triggerStore = new MemoryTriggerStore([triggerRecord(definition)])
    const eventLedgerMessages: Array<EventLedgerIngestQueueMessage> = []
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
        eventLedgerMessages,
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
      ledgerEnqueued: 1,
      ledgerFailed: 0,
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
    expect(eventLedgerMessages).toHaveLength(1)
    expect(eventLedgerMessages[0]).toMatchObject({
      schemaVersion: 'openagents.event_ledger_ingest.v1',
      actorRef: 'github.user.AtlantisPleb',
      contentRef: 'github.issue.OpenAgentsInc/openagents.8195',
      externalRef: 'github.delivery.delivery-8195',
      ownerAgentUserId,
      ownerRef: `agent:${ownerAgentUserId}`,
      source: 'github',
      subjectRef: 'github.repository.OpenAgentsInc/openagents.issue.8195',
      trainingConsent: false,
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

  test('dispatches GitHub @mention comments with a stored completion callback', async () => {
    // background_agents.integrations.github_mention_callback.v1
    const definition = makeGitHubMentionDefinition()
    const triggerStore = new MemoryTriggerStore([triggerRecord(definition)])
    const dispatches: Array<{
      readonly triggerPayload: Record<string, unknown> | undefined
      readonly triggerRef: string | undefined
    }> = []
    const response = await handleAgentDefinitionWebhookRequest(
      await githubRequest(
        githubMentionCommentPayload,
        secret,
        'issue_comment',
        'delivery-8209',
      ),
      dependenciesFor({
        definition,
        dispatchRun: (_dependencies, input) => {
          dispatches.push({
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
      deliveryId: 'delivery-8209',
      dispatched: 1,
      eventType: 'issue_comment.created.mention',
      matched: 1,
      source: 'github',
      completionCallback: {
        kind: 'github_issue_comment',
        number: 8209,
        source: 'github',
        subjectKind: 'issue',
      },
    })
    expect(dispatches).toHaveLength(1)
    expect(dispatches[0]?.triggerRef).toBe(
      'trigger.public.webhook.github_mention',
    )
    expect(dispatches[0]?.triggerPayload).toMatchObject({
      schema: 'openagents.agent_definition_webhook_event.v1',
      eventType: 'issue_comment.created.mention',
      source: 'github',
      triggerRef: 'trigger.public.webhook.github_mention',
      payload: {
        comment: {
          id: 1001,
        },
        mention: {
          target_login: 'openagents',
        },
        repository: {
          full_name: 'OpenAgentsInc/openagents',
        },
        subject: {
          kind: 'issue',
          number: 8209,
        },
      },
      completionCallback: {
        schema: 'openagents.agent_definition_github_completion_callback.v1',
        kind: 'github_issue_comment',
        number: 8209,
        owner: 'OpenAgentsInc',
        repo: 'openagents',
        source: 'github',
        sourceCommentId: 1001,
        subjectKind: 'issue',
      },
    })
    expect(JSON.stringify(dispatches[0]?.triggerPayload)).not.toContain(
      'Secret-ish prose must not leak',
    )
    expect(triggerStore.successes).toEqual([
      {
        nextRunAt: undefined,
        ownerAgentUserId,
        triggerRef: 'trigger.public.webhook.github_mention',
        updatedAt: nowIso,
      },
    ])
  })

  test('does not dispatch ordinary GitHub issue comments as mentions', async () => {
    const definition = makeGitHubMentionDefinition()
    const triggerStore = new MemoryTriggerStore([triggerRecord(definition)])
    const dispatches: Array<Record<string, unknown> | undefined> = []
    const response = await handleAgentDefinitionWebhookRequest(
      await githubRequest(
        {
          ...githubMentionCommentPayload,
          comment: {
            ...githubMentionCommentPayload.comment,
            body: 'ordinary discussion comment',
            id: 1002,
          },
        },
        secret,
        'issue_comment',
        'delivery-8210',
      ),
      dependenciesFor({
        definition,
        dispatchRun: (_dependencies, input) => {
          dispatches.push(input.request.triggerPayload)

          return Promise.resolve(dispatchedOutcome())
        },
        triggerStore,
      }),
    )
    const body = await response?.json()

    expect(response?.status).toBe(202)
    expect(body).toMatchObject({
      dispatched: 0,
      eventType: 'issue_comment.created',
      matched: 0,
      skipped: 1,
    })
    expect(dispatches).toEqual([])
    expect(triggerStore.successes).toEqual([])
    expect(triggerStore.failures).toEqual([])
  })

  test('verifies Forum source events and dispatches runs with completion callbacks', async () => {
    // background_agents.integrations.forum_trigger_callback.v1
    const definition = makeForumDefinition()
    const triggerStore = new MemoryTriggerStore([triggerRecord(definition)])
    const dispatches: Array<{
      readonly triggerPayload: Record<string, unknown> | undefined
      readonly triggerRef: string | undefined
    }> = []
    const response = await Effect.runPromise(
      handleAgentDefinitionForumWebhookRequest(
        await forumRequest(forumPostPayload),
        {
          ...dependenciesFor({
            definition,
            dispatchRun: (_dependencies, input) => {
              dispatches.push({
                triggerPayload: input.request.triggerPayload,
                triggerRef: input.request.triggerRef,
              })

              return Promise.resolve(dispatchedOutcome())
            },
            triggerStore,
          }),
          forumEventSourceVerifier: () => Effect.succeed(true),
          forumSecret,
        },
      ),
    )
    const body = await response.json()

    expect(response.status).toBe(202)
    expect(body).toMatchObject({
      schema: 'openagents.agent_definition_webhook_ingress.v1',
      deliveryId: 'forum-delivery-8208',
      dispatched: 1,
      eventType: 'forum.post.created',
      matched: 1,
      source: 'forum',
      completionCallback: {
        kind: 'forum_reply',
        source: 'forum',
        topicId: 'topic_forum_trigger_001',
      },
    })
    expect(dispatches).toHaveLength(1)
    expect(dispatches[0]?.triggerRef).toBe(
      'trigger.public.webhook.forum_post',
    )
    expect(dispatches[0]?.triggerPayload).toMatchObject({
      schema: 'openagents.agent_definition_webhook_event.v1',
      source: 'forum',
      triggerRef: 'trigger.public.webhook.forum_post',
      payload: {
        forum: {
          slug: 'product-promises',
        },
        post: {
          id: 'post_forum_trigger_001',
        },
      },
      completionCallback: {
        schema: 'openagents.agent_definition_forum_completion_callback.v1',
        forumId: 'forum_product_promises',
        kind: 'forum_reply',
        source: 'forum',
        sourcePostId: 'post_forum_trigger_001',
        topicId: 'topic_forum_trigger_001',
      },
    })
    expect(triggerStore.successes).toEqual([
      {
        nextRunAt: undefined,
        ownerAgentUserId,
        triggerRef: 'trigger.public.webhook.forum_post',
        updatedAt: nowIso,
      },
    ])
  })

  test('posts Forum completion callbacks only through the stored run callback', async () => {
    // background_agents.integrations.forum_trigger_callback.v1
    const forum = new MemoryForumStore()
    const response = await Effect.runPromise(
      handleAgentDefinitionForumCompletionRequest(
        await forumCompletionRequest({
          assignmentRef: 'assignment.background.webhook',
          bodyText: 'Background agent run completed. Evidence: receipt.public.1',
          evidenceRefs: ['receipt.public.1'],
        }),
        {
          forum,
          forumSecret,
          makeId: () => 'post_forum_completion_001',
          runStore: {
            readRunByAssignmentRef: assignmentRef =>
              Promise.resolve(
                assignmentRef === 'assignment.background.webhook'
                  ? forumRunRecord()
                  : undefined,
              ),
          },
        },
      ),
    )
    const body = await response.json()

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      schema: 'openagents.agent_definition_forum_completion.v1',
      assignmentRef: 'assignment.background.webhook',
      idempotent: false,
      postId: 'post_forum_completion_001',
      runId: 'agent_definition_run.forum_001',
      topicId: 'topic_forum_trigger_001',
    })
    expect(forum.policyChecks).toEqual([
      {
        actorRef: 'operator:background-agents',
        bodyText:
          'Background agent run completed. Evidence: receipt.public.1',
      },
    ])
    expect(forum.createdReplies).toHaveLength(1)
    expect(forum.createdReplies[0]).toMatchObject({
      bodyText: 'Background agent run completed. Evidence: receipt.public.1',
      forumId: 'forum_product_promises',
      idempotencyKey:
        'agent-definition-completion:agent_definition_run.forum_001',
      parentPostId: 'post_forum_trigger_001',
      topicId: 'topic_forum_trigger_001',
    })
  })

  test('posts GitHub completions through the stored callback without duplicates', async () => {
    // background_agents.integrations.github_mention_callback.v1
    const github = new MemoryGitHubCompletionStore()
    const dependencies = {
      github,
      githubSecret: secret,
      runStore: {
        readRunByAssignmentRef: (assignmentRef: string) =>
          Promise.resolve(
            assignmentRef === 'assignment.background.webhook.github'
              ? githubRunRecord()
              : undefined,
          ),
      },
    }
    const payload = {
      assignmentRef: 'assignment.background.webhook.github',
      bodyText: 'Background agent run completed. Evidence: receipt.public.2',
      evidenceRefs: ['receipt.public.2'],
    }
    const first = await Effect.runPromise(
      handleAgentDefinitionGitHubCompletionRequest(
        await githubCompletionRequest(payload),
        dependencies,
      ),
    )
    const firstBody = await first.json()
    const second = await Effect.runPromise(
      handleAgentDefinitionGitHubCompletionRequest(
        await githubCompletionRequest(payload),
        dependencies,
      ),
    )
    const secondBody = await second.json()

    expect(first.status).toBe(201)
    expect(firstBody).toMatchObject({
      schema: 'openagents.agent_definition_github_completion.v1',
      assignmentRef: 'assignment.background.webhook.github',
      commentId: 10010,
      idempotent: false,
      number: 8209,
      runId: 'agent_definition_run.github_001',
      subjectKind: 'issue',
    })
    expect(second.status).toBe(200)
    expect(secondBody).toMatchObject({
      assignmentRef: 'assignment.background.webhook.github',
      commentId: 10010,
      idempotent: true,
      runId: 'agent_definition_run.github_001',
    })
    expect(github.createdComments).toHaveLength(1)
    expect(github.createdComments[0]).toMatchObject({
      bodyText: 'Background agent run completed. Evidence: receipt.public.2',
      callback: {
        number: 8209,
        owner: 'OpenAgentsInc',
        repo: 'openagents',
        subjectKind: 'issue',
      },
      idempotencyKey:
        'agent-definition-github-completion:agent_definition_run.github_001',
    })
  })
})
