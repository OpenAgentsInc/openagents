import {
  normalizeForumWebhookEvent,
  normalizeGitHubWebhookEvent,
  normalizeSlackWebhookEvent,
  type AgentDefinitionWebhookNormalizedEvent,
} from '@openagentsinc/agent-runtime-schema/webhooks'
import { Effect, Schema as S } from 'effect'

import {
  AGENT_DEFINITION_BOT_INTEGRATION_RESULT_SCHEMA,
  AGENT_DEFINITION_FORUM_COMPLETION_RESULT_SCHEMA,
  AGENT_DEFINITION_GITHUB_COMPLETION_RESULT_SCHEMA,
  AgentDefinitionForumCompletionRequest,
  AgentDefinitionGitHubCompletionRequest,
  type AgentDefinitionBotIntegrationDependencies,
  type AgentDefinitionForumCompletionDependencies,
  type AgentDefinitionForumCompletionForumStore,
  type AgentDefinitionGitHubCompletionDependencies,
  type AgentDefinitionGitHubCompletionGitHubStore,
  forumCompletionCallbackForEvent,
  githubCompletionCallbackForEvent,
  postAgentDefinitionForumCompletion,
  postAgentDefinitionGitHubCompletion,
  runAgentDefinitionBotIntegration,
} from './agent-definition-bot-integration'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonRecord } from './json-boundary'
import { currentEpochMillis, currentIsoTimestamp } from './runtime-primitives'

const encoder = new TextEncoder()
const GITHUB_WEBHOOK_PATH = '/v1/agent-definitions/webhooks/github'
const GITHUB_COMPLETION_PATH =
  '/v1/agent-definitions/webhooks/github/completions'
const FORUM_WEBHOOK_PATH = '/v1/agent-definitions/webhooks/forum'
const FORUM_COMPLETION_PATH =
  '/v1/agent-definitions/webhooks/forum/completions'
const SLACK_WEBHOOK_PATH = '/v1/agent-definitions/webhooks/slack'
const SLACK_SIGNATURE_VERSION = 'v0'
const SLACK_SIGNATURE_MAX_SKEW_SECONDS = 5 * 60

export type AgentDefinitionWebhookRouteDependencies =
  AgentDefinitionBotIntegrationDependencies & Readonly<{
    githubMentionLogins?: ReadonlyArray<string> | undefined
    githubSecret?: string | undefined
    nowIso?: (() => string) | undefined
  }>

export type AgentDefinitionSlackWebhookRouteDependencies =
  AgentDefinitionBotIntegrationDependencies & Readonly<{
    nowEpochSeconds?: (() => number) | undefined
    nowIso?: (() => string) | undefined
    slackSecret?: string | undefined
  }>

export type AgentDefinitionForumWebhookRouteDependencies =
  AgentDefinitionBotIntegrationDependencies & Readonly<{
    forumEventSourceVerifier: (
      event: AgentDefinitionWebhookNormalizedEvent,
    ) => Effect.Effect<boolean, unknown>
    forumSecret?: string | undefined
    nowIso?: (() => string) | undefined
  }>

export type AgentDefinitionForumCompletionRouteDependencies =
  AgentDefinitionForumCompletionDependencies & Readonly<{
    forumSecret?: string | undefined
  }>

export type AgentDefinitionGitHubCompletionRouteDependencies =
  Omit<AgentDefinitionGitHubCompletionDependencies, 'github'> &
    Readonly<{
      github?: AgentDefinitionGitHubCompletionGitHubStore | undefined
      githubSecret?: string | undefined
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

export const verifyOpenAgentsWebhookSignature = async (
  input: Readonly<{
    body: string
    headers: Headers
    secret: string
  }>,
): Promise<boolean> => {
  const signature = input.headers.get('x-openagents-signature-256')
  const supplied = signature?.trim().replace(/^sha256=/iu, '')

  if (supplied === undefined || supplied === '') {
    return false
  }

  const expected = await hmacSha256Hex(input.secret, input.body)

  return timingSafeEqual(supplied, expected)
}

export const verifySlackWebhookSignature = async (
  input: Readonly<{
    body: string
    headers: Headers
    nowEpochSeconds: number
    secret: string
  }>,
): Promise<boolean> => {
  const signature = input.headers.get('x-slack-signature')?.trim()
  const timestampText = input.headers
    .get('x-slack-request-timestamp')
    ?.trim()
  const timestamp =
    timestampText === undefined || timestampText === ''
      ? Number.NaN
      : Number(timestampText)

  if (
    signature === undefined ||
    signature === '' ||
    timestampText === undefined ||
    timestampText === '' ||
    !Number.isFinite(timestamp) ||
    Math.abs(input.nowEpochSeconds - timestamp) >
      SLACK_SIGNATURE_MAX_SKEW_SECONDS
  ) {
    return false
  }

  const base = `${SLACK_SIGNATURE_VERSION}:${timestampText}:${input.body}`
  const expected = `${SLACK_SIGNATURE_VERSION}=${await hmacSha256Hex(
    input.secret,
    base,
  )}`

  return timingSafeEqual(signature, expected)
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

const githubCompletionNotConfigured = () =>
  webhookResponse(
    { error: 'agent_definition_github_completion_not_configured' },
    { status: 503 },
  )

const sourceString = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const decodeForumCompletionRequest = (
  value: unknown,
): AgentDefinitionForumCompletionRequest | undefined => {
  try {
    return S.decodeUnknownSync(AgentDefinitionForumCompletionRequest)(value)
  } catch {
    return undefined
  }
}

const decodeGitHubCompletionRequest = (
  value: unknown,
): AgentDefinitionGitHubCompletionRequest | undefined => {
  try {
    return S.decodeUnknownSync(AgentDefinitionGitHubCompletionRequest)(value)
  } catch {
    return undefined
  }
}

export const verifyAgentDefinitionForumEventSource = (
  forum: Pick<
    AgentDefinitionForumCompletionForumStore,
    'readPostDetail' | 'readSummaryByRef' | 'readTopicById'
  >,
  event: AgentDefinitionWebhookNormalizedEvent,
): Effect.Effect<boolean, unknown> =>
  Effect.gen(function* () {
    const callback = forumCompletionCallbackForEvent(event)

    if (callback === undefined) {
      return false
    }

    const post = yield* forum.readPostDetail(callback.sourcePostId)

    if (
      post === null ||
      post.containingTopicId !== callback.topicId ||
      post.post.state === 'tombstoned'
    ) {
      return false
    }

    const topic = yield* forum.readTopicById(callback.topicId)

    if (
      topic === null ||
      topic.forumId !== callback.forumId ||
      topic.state === 'archived' ||
      topic.state === 'hidden' ||
      topic.state === 'locked'
    ) {
      return false
    }

    const sourceForum = yield* forum.readSummaryByRef(callback.forumId)

    return sourceForum !== null && !sourceForum.locked
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
          mentionLogins: dependencies.githubMentionLogins,
          payload,
          receivedAt: nowIso,
        })

  if (event === undefined) {
    return invalidWebhook('GitHub webhook payload could not be normalized.')
  }

  const completionCallback = githubCompletionCallbackForEvent(event)

  if (
    event.eventType === 'issue_comment.created.mention' &&
    completionCallback === undefined
  ) {
    return invalidWebhook(
      'GitHub mention event does not carry a callback source.',
    )
  }

  const result = await runAgentDefinitionBotIntegration(dependencies, {
    ...(completionCallback === undefined ? {} : { completionCallback }),
    event,
    nowIso,
  })

  return webhookResponse(
    {
      schema: AGENT_DEFINITION_BOT_INTEGRATION_RESULT_SCHEMA,
      ...result,
      ...(completionCallback === undefined
        ? {}
        : {
            completionCallback: {
              kind: completionCallback.kind,
              number: completionCallback.number,
              source: completionCallback.source,
              subjectKind: completionCallback.subjectKind,
            },
          }),
    },
    { status: 202 },
  )
}

export const handleAgentDefinitionSlackWebhookRequest = async (
  request: Request,
  dependencies: AgentDefinitionSlackWebhookRouteDependencies,
) => {
  const url = new URL(request.url)

  if (url.pathname !== SLACK_WEBHOOK_PATH) {
    return webhookResponse({ error: 'not_found' }, { status: 404 })
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const secret = dependencies.slackSecret?.trim()

  if (secret === undefined || secret === '') {
    return webhookSecretMissing()
  }

  const body = await request.text()
  const nowEpochSeconds =
    dependencies.nowEpochSeconds?.() ?? Math.floor(currentEpochMillis() / 1000)
  const authorized = await verifySlackWebhookSignature({
    body,
    headers: request.headers,
    nowEpochSeconds,
    secret,
  })

  if (!authorized) {
    return unauthorizedWebhook()
  }

  const payload = parseJsonRecord(body)

  if (payload?.type === 'url_verification') {
    const challenge = sourceString(payload.challenge)

    return challenge === undefined
      ? invalidWebhook('Slack URL verification challenge is missing.')
      : webhookResponse({ challenge })
  }

  const deliveryId = sourceString(payload?.event_id) ?? ''
  const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
  const event =
    payload === undefined
      ? undefined
      : normalizeSlackWebhookEvent({
          deliveryId,
          payload,
          receivedAt: nowIso,
        })

  if (event === undefined) {
    return invalidWebhook('Slack event payload could not be normalized.')
  }

  const result = await runAgentDefinitionBotIntegration(dependencies, {
    event,
    nowIso,
  })

  return webhookResponse(
    {
      schema: AGENT_DEFINITION_BOT_INTEGRATION_RESULT_SCHEMA,
      ...result,
    },
    { status: 202 },
  )
}

export const handleAgentDefinitionForumWebhookRequest = (
  request: Request,
  dependencies: AgentDefinitionForumWebhookRouteDependencies,
) =>
  Effect.gen(function* () {
    const url = new URL(request.url)

    if (url.pathname !== FORUM_WEBHOOK_PATH) {
      return webhookResponse({ error: 'not_found' }, { status: 404 })
    }

    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const secret = dependencies.forumSecret?.trim()

    if (secret === undefined || secret === '') {
      return webhookSecretMissing()
    }

    const body = yield* Effect.promise(() => request.text())
    const authorized = yield* Effect.promise(() =>
      verifyOpenAgentsWebhookSignature({
        body,
        headers: request.headers,
        secret,
      }),
    )

    if (!authorized) {
      return unauthorizedWebhook()
    }

    const payload = parseJsonRecord(body)
    const eventType =
      request.headers.get('x-openagents-event')?.trim() ??
      sourceString(payload?.eventType) ??
      ''
    const deliveryId =
      request.headers.get('x-openagents-delivery')?.trim() ??
      sourceString(payload?.deliveryId) ??
      ''
    const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
    const event =
      payload === undefined
        ? undefined
        : normalizeForumWebhookEvent({
            deliveryId,
            eventType,
            payload,
            receivedAt: nowIso,
          })

    if (event === undefined) {
      return invalidWebhook('Forum event payload could not be normalized.')
    }

    const completionCallback = forumCompletionCallbackForEvent(event)

    if (completionCallback === undefined) {
      return invalidWebhook('Forum event does not carry a callback source.')
    }

    const verified = yield* dependencies.forumEventSourceVerifier(event)

    if (!verified) {
      return invalidWebhook('Forum source post could not be verified.')
    }

    const result = yield* Effect.promise(() =>
      runAgentDefinitionBotIntegration(dependencies, {
        completionCallback,
        event,
        nowIso,
      }),
    )

    return webhookResponse(
      {
        schema: AGENT_DEFINITION_BOT_INTEGRATION_RESULT_SCHEMA,
        ...result,
        completionCallback: {
          kind: completionCallback.kind,
          source: completionCallback.source,
          topicId: completionCallback.topicId,
        },
      },
      { status: 202 },
    )
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(
        webhookResponse(
          { error: 'agent_definition_forum_webhook_failed' },
          { status: 500 },
        ),
      ),
    ),
  )

export const handleAgentDefinitionGitHubCompletionRequest = (
  request: Request,
  dependencies: AgentDefinitionGitHubCompletionRouteDependencies,
) =>
  Effect.gen(function* () {
    const url = new URL(request.url)

    if (url.pathname !== GITHUB_COMPLETION_PATH) {
      return webhookResponse({ error: 'not_found' }, { status: 404 })
    }

    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const secret = dependencies.githubSecret?.trim()

    if (secret === undefined || secret === '') {
      return webhookSecretMissing()
    }

    if (dependencies.github === undefined) {
      return githubCompletionNotConfigured()
    }

    const body = yield* Effect.promise(() => request.text())
    const authorized = yield* Effect.promise(() =>
      verifyOpenAgentsWebhookSignature({
        body,
        headers: request.headers,
        secret,
      }),
    )

    if (!authorized) {
      return unauthorizedWebhook()
    }

    const requestBody = decodeGitHubCompletionRequest(parseJsonRecord(body))

    if (requestBody === undefined) {
      return invalidWebhook('GitHub completion payload could not be decoded.')
    }

    const outcome = yield* postAgentDefinitionGitHubCompletion(
      {
        github: dependencies.github,
        runStore: dependencies.runStore,
      },
      requestBody,
    )

    if (outcome.kind === 'invalid') {
      return webhookResponse(
        {
          error: 'invalid_agent_definition_github_completion',
          reason: outcome.reason,
        },
        { status: outcome.statusCode },
      )
    }

    return webhookResponse(
      {
        schema: AGENT_DEFINITION_GITHUB_COMPLETION_RESULT_SCHEMA,
        assignmentRef: requestBody.assignmentRef,
        commentId: outcome.comment.commentId,
        htmlUrl: outcome.comment.htmlUrl,
        idempotent: outcome.idempotent,
        number: outcome.target.number,
        runId: outcome.run.runId,
        subjectKind: outcome.target.subjectKind,
      },
      { status: outcome.idempotent ? 200 : 201 },
    )
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(
        webhookResponse(
          { error: 'agent_definition_github_completion_failed' },
          { status: 500 },
        ),
      ),
    ),
  )

export const handleAgentDefinitionForumCompletionRequest = (
  request: Request,
  dependencies: AgentDefinitionForumCompletionRouteDependencies,
) =>
  Effect.gen(function* () {
    const url = new URL(request.url)

    if (url.pathname !== FORUM_COMPLETION_PATH) {
      return webhookResponse({ error: 'not_found' }, { status: 404 })
    }

    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const secret = dependencies.forumSecret?.trim()

    if (secret === undefined || secret === '') {
      return webhookSecretMissing()
    }

    const body = yield* Effect.promise(() => request.text())
    const authorized = yield* Effect.promise(() =>
      verifyOpenAgentsWebhookSignature({
        body,
        headers: request.headers,
        secret,
      }),
    )

    if (!authorized) {
      return unauthorizedWebhook()
    }

    const requestBody = decodeForumCompletionRequest(parseJsonRecord(body))

    if (requestBody === undefined) {
      return invalidWebhook('Forum completion payload could not be decoded.')
    }

    const outcome = yield* postAgentDefinitionForumCompletion(
      dependencies,
      requestBody,
    )

    if (outcome.kind === 'invalid') {
      return webhookResponse(
        {
          error: 'invalid_agent_definition_forum_completion',
          reason: outcome.reason,
        },
        { status: outcome.statusCode },
      )
    }

    return webhookResponse(
      {
        schema: AGENT_DEFINITION_FORUM_COMPLETION_RESULT_SCHEMA,
        assignmentRef: requestBody.assignmentRef,
        idempotent: outcome.idempotent,
        postId: outcome.post.postId,
        runId: outcome.run.runId,
        topicId: outcome.topic.topicId,
      },
      { status: outcome.idempotent ? 200 : 201 },
    )
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(
        webhookResponse(
          { error: 'agent_definition_forum_completion_failed' },
          { status: 500 },
        ),
      ),
    ),
  )
