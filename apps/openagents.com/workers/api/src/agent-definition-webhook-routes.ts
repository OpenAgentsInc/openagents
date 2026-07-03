import {
  agentDefinitionWebhookConditionsMatch,
  normalizeGitHubWebhookEvent,
  type AgentDefinitionWebhookNormalizedEvent,
} from '@openagentsinc/agent-runtime-schema/webhooks'
import type { AgentDefinition } from '@openagentsinc/agent-runtime-schema'

import type { AgentDefinitionStore } from './agent-definition-routes'
import {
  type AgentDefinitionRunDispatchDependencies,
  type AgentDefinitionRunDispatchOutcome,
  dispatchAgentDefinitionRun,
} from './agent-definition-run-routes'
import {
  type AgentDefinitionTriggerStore,
  type DueAgentDefinitionTriggerRecord,
} from './agent-definition-trigger-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonRecord } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

const encoder = new TextEncoder()
const AGENT_DEFINITION_WEBHOOK_RESULT_SCHEMA =
  'openagents.agent_definition_webhook_ingress.v1' as const
const GITHUB_WEBHOOK_PATH = '/v1/agent-definitions/webhooks/github'

type WebhookDispatchDependencies = Omit<
  AgentDefinitionRunDispatchDependencies,
  'linkedAgents' | 'nowIso'
>

type WebhookDispatch = (
  dependencies: AgentDefinitionRunDispatchDependencies,
  input: Readonly<{
    definition: AgentDefinition
    request: Parameters<typeof dispatchAgentDefinitionRun>[1]['request']
  }>,
) => Promise<AgentDefinitionRunDispatchOutcome>

export type AgentDefinitionWebhookRouteDependencies = Readonly<{
  definitionStore: Pick<AgentDefinitionStore, 'readDefinition'>
  dispatchDependencies: WebhookDispatchDependencies
  dispatchRun?: WebhookDispatch | undefined
  githubSecret?: string | undefined
  nowIso?: (() => string) | undefined
  triggerStore: Pick<
    AgentDefinitionTriggerStore,
    | 'listInboundWebhookTriggers'
    | 'recordTriggerFailure'
    | 'recordTriggerSuccess'
  >
}>

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer =>
  bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer

const hex = (bytes: ArrayBuffer): string =>
  [...new Uint8Array(bytes)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')

const timingSafeEqual = (left: string, right: string): boolean => {
  const leftBytes = encoder.encode(left)
  const rightBytes = encoder.encode(right)

  if (leftBytes.length !== rightBytes.length) {
    return false
  }

  return (
    leftBytes.reduce(
      (diff, byte, index) => diff | (byte ^ (rightBytes[index] ?? 0)),
      0,
    ) === 0
  )
}

const hmacSha256Hex = async (
  secret: string,
  body: string,
): Promise<string> => {
  const key = await crypto.subtle.importKey(
    'raw',
    arrayBufferFromBytes(encoder.encode(secret)),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )
  const digest = await crypto.subtle.sign('HMAC', key, encoder.encode(body))

  return hex(digest)
}

export const verifyGitHubWebhookSignature = async (
  input: Readonly<{
    body: string
    headers: Headers
    secret: string
  }>,
): Promise<boolean> => {
  const signature = input.headers.get('x-hub-signature-256')
  const supplied = signature?.trim().replace(/^sha256=/iu, '')

  if (supplied === undefined || supplied === '') {
    return false
  }

  const expected = await hmacSha256Hex(input.secret, input.body)

  return timingSafeEqual(supplied, expected)
}

const webhookResponse = (body: unknown, init: ResponseInit = {}) =>
  noStoreJsonResponse(body, init)

const invalidWebhook = (reason: string) =>
  webhookResponse(
    { error: 'invalid_agent_definition_webhook', reason },
    { status: 400 },
  )

const unauthorizedWebhook = () =>
  webhookResponse(
    { error: 'agent_definition_webhook_unauthorized' },
    { status: 401 },
  )

const webhookSecretMissing = () =>
  webhookResponse(
    { error: 'agent_definition_webhook_secret_not_configured' },
    { status: 503 },
  )

const triggerMatches = (
  trigger: DueAgentDefinitionTriggerRecord,
  event: AgentDefinitionWebhookNormalizedEvent,
): boolean =>
  trigger.trigger.kind === 'inbound_webhook' &&
  trigger.trigger.source === event.source &&
  agentDefinitionWebhookConditionsMatch(event, trigger.trigger.conditions)

const markWebhookFailure = (
  dependencies: AgentDefinitionWebhookRouteDependencies,
  trigger: DueAgentDefinitionTriggerRecord,
  nowIso: string,
): Promise<boolean> =>
  dependencies.triggerStore.recordTriggerFailure(
    trigger.ownerAgentUserId,
    trigger.triggerRef,
    nowIso,
  )

const markWebhookSuccess = (
  dependencies: AgentDefinitionWebhookRouteDependencies,
  trigger: DueAgentDefinitionTriggerRecord,
  nowIso: string,
): Promise<boolean> =>
  dependencies.triggerStore.recordTriggerSuccess(
    trigger.ownerAgentUserId,
    trigger.triggerRef,
    undefined,
    nowIso,
  )

const dispatchMatchedWebhookTrigger = async (
  dependencies: AgentDefinitionWebhookRouteDependencies,
  input: Readonly<{
    definition: AgentDefinition
    event: AgentDefinitionWebhookNormalizedEvent
    nowIso: string
    trigger: DueAgentDefinitionTriggerRecord
  }>,
): Promise<AgentDefinitionRunDispatchOutcome> =>
  (dependencies.dispatchRun ?? dispatchAgentDefinitionRun)({
    ...dependencies.dispatchDependencies,
    linkedAgents: [{ agentUserId: input.trigger.ownerAgentUserId }],
    nowIso: () => input.nowIso,
  }, {
    definition: input.definition,
    request: {
      triggerPayload: {
        ...input.event,
        triggerRef: input.trigger.triggerRef,
      },
      triggerRef: input.trigger.triggerRef,
    },
  })

export const handleAgentDefinitionWebhookRequest = async (
  request: Request,
  dependencies: AgentDefinitionWebhookRouteDependencies,
) => {
  const url = new URL(request.url)

  if (url.pathname !== GITHUB_WEBHOOK_PATH) {
    return webhookResponse({ error: 'not_found' }, { status: 404 })
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const secret = dependencies.githubSecret?.trim()

  if (secret === undefined || secret === '') {
    return webhookSecretMissing()
  }

  const body = await request.text()
  const authorized = await verifyGitHubWebhookSignature({
    body,
    headers: request.headers,
    secret,
  })

  if (!authorized) {
    return unauthorizedWebhook()
  }

  const payload = parseJsonRecord(body)
  const eventName = request.headers.get('x-github-event')?.trim() ?? ''
  const deliveryId = request.headers.get('x-github-delivery')?.trim() ?? ''
  const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
  const event =
    payload === undefined
      ? undefined
      : normalizeGitHubWebhookEvent({
          deliveryId,
          eventName,
          payload,
          receivedAt: nowIso,
        })

  if (event === undefined) {
    return invalidWebhook('GitHub webhook payload could not be normalized.')
  }

  const triggers = await dependencies.triggerStore.listInboundWebhookTriggers(
    event.source,
    250,
  )
  let dispatched = 0
  let failed = 0
  let matched = 0
  let refused = 0
  let skipped = 0

  for (const trigger of triggers) {
    if (!triggerMatches(trigger, event)) {
      skipped += 1
      continue
    }

    matched += 1
    const definition = await dependencies.definitionStore
      .readDefinition(trigger.ownerAgentUserId, trigger.definitionId)
      .catch(() => undefined)

    if (definition === undefined) {
      failed += 1
      await markWebhookFailure(dependencies, trigger, nowIso).catch(
        () => undefined,
      )
      continue
    }

    const dispatch = await dispatchMatchedWebhookTrigger(dependencies, {
      definition,
      event,
      nowIso,
      trigger,
    }).catch((): AgentDefinitionRunDispatchOutcome => ({
      kind: 'storage_error',
    }))

    if (dispatch.kind === 'dispatched') {
      const marked = await markWebhookSuccess(
        dependencies,
        trigger,
        nowIso,
      ).catch(() => false)
      if (marked) {
        dispatched += 1
      } else {
        failed += 1
      }
      continue
    }

    const marked = await markWebhookFailure(
      dependencies,
      trigger,
      nowIso,
    ).catch(() => false)

    if (!marked) {
      failed += 1
    } else if (dispatch.kind === 'refused') {
      refused += 1
    } else {
      failed += 1
    }
  }

  return webhookResponse(
    {
      schema: AGENT_DEFINITION_WEBHOOK_RESULT_SCHEMA,
      deliveryId: event.deliveryId,
      dispatched,
      eventType: event.eventType,
      failed,
      matched,
      refused,
      skipped,
      source: event.source,
      subjectRef: event.subjectRef,
    },
    { status: 202 },
  )
}
