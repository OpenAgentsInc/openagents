import {
  agentDefinitionWebhookConditionsMatch,
  type AgentDefinitionWebhookNormalizedEvent,
} from '@openagentsinc/agent-runtime-schema/webhooks'
import type { AgentDefinition } from '@openagentsinc/agent-runtime-schema'
import { Data, Effect, Schema as S } from 'effect'

import type { AgentDefinitionStore } from './agent-definition-routes'
import {
  type AgentDefinitionRunDispatchDependencies,
  type AgentDefinitionRunDispatchOutcome,
  type AgentDefinitionRunRecord,
  dispatchAgentDefinitionRun,
} from './agent-definition-run-routes'
import type {
  AgentDefinitionTriggerStore,
  DueAgentDefinitionTriggerRecord,
} from './agent-definition-trigger-store'
import {
  buildForumWriterContext,
  canonicalForumTopicHref,
  createForumReplyPost,
  evaluateForumWritePolicy,
  ForumWritePolicyMaxLookupWindowSeconds,
  listRecentForumWritesForActor,
  readForumPostByIdempotencyKey,
  readForumPostDetail,
  readForumSummaryByRef,
  readForumTopicById,
  type ForumCreateReplyPostRecordInput,
  type ForumForumSummaryType,
  type ForumPostDetailResponseType,
  type ForumPostSummaryType,
  type ForumPublicProjectionType,
  type ForumTopicSummaryType,
} from './forum'
import {
  currentEpochMillis,
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
  randomUuid,
} from './runtime-primitives'

export const AGENT_DEFINITION_BOT_INTEGRATION_RESULT_SCHEMA =
  'openagents.agent_definition_webhook_ingress.v1' as const
export const AGENT_DEFINITION_FORUM_COMPLETION_RESULT_SCHEMA =
  'openagents.agent_definition_forum_completion.v1' as const
export const AGENT_DEFINITION_GITHUB_COMPLETION_RESULT_SCHEMA =
  'openagents.agent_definition_github_completion.v1' as const
const AGENT_DEFINITION_FORUM_COMPLETION_CALLBACK_SCHEMA =
  'openagents.agent_definition_forum_completion_callback.v1' as const
const AGENT_DEFINITION_GITHUB_COMPLETION_CALLBACK_SCHEMA =
  'openagents.agent_definition_github_completion_callback.v1' as const

const PublicSafeText = S.Trim.check(S.isNonEmpty(), S.isMaxLength(4000))

class AgentDefinitionGitHubCompletionRestError extends Data.TaggedError(
  'AgentDefinitionGitHubCompletionRestError',
)<{
  readonly cause?: unknown
  readonly reason: string
}> {}

const githubCompletionRestError = (
  reason: string,
  cause?: unknown,
): AgentDefinitionGitHubCompletionRestError =>
  new AgentDefinitionGitHubCompletionRestError({
    ...(cause === undefined ? {} : { cause }),
    reason,
  })

export const AgentDefinitionForumCompletionCallback = S.Struct({
  schema: S.Literal(AGENT_DEFINITION_FORUM_COMPLETION_CALLBACK_SCHEMA),
  forumId: S.String,
  kind: S.Literal('forum_reply'),
  source: S.Literal('forum'),
  sourcePostId: S.String,
  sourceUrl: S.optionalKey(S.String),
  subjectRef: S.String,
  topicId: S.String,
})
export type AgentDefinitionForumCompletionCallback =
  typeof AgentDefinitionForumCompletionCallback.Type

export const AgentDefinitionGitHubCompletionCallback = S.Struct({
  schema: S.Literal(AGENT_DEFINITION_GITHUB_COMPLETION_CALLBACK_SCHEMA),
  kind: S.Literal('github_issue_comment'),
  mentionLogin: S.String,
  number: S.Number,
  owner: S.String,
  repo: S.String,
  repositoryFullName: S.String,
  source: S.Literal('github'),
  sourceCommentId: S.Number,
  sourceUrl: S.optionalKey(S.String),
  subjectKind: S.Literals(['issue', 'pull_request']),
  subjectRef: S.String,
})
export type AgentDefinitionGitHubCompletionCallback =
  typeof AgentDefinitionGitHubCompletionCallback.Type

type AgentDefinitionBotIntegrationCompletionCallback =
  | AgentDefinitionForumCompletionCallback
  | AgentDefinitionGitHubCompletionCallback

export const AgentDefinitionForumCompletionRequest = S.Struct({
  assignmentRef: S.String,
  bodyText: PublicSafeText,
  evidenceRefs: S.optionalKey(S.Array(S.String)),
})
export type AgentDefinitionForumCompletionRequest =
  typeof AgentDefinitionForumCompletionRequest.Type

export const AgentDefinitionGitHubCompletionRequest = S.Struct({
  assignmentRef: S.String,
  bodyText: PublicSafeText,
  evidenceRefs: S.optionalKey(S.Array(S.String)),
})
export type AgentDefinitionGitHubCompletionRequest =
  typeof AgentDefinitionGitHubCompletionRequest.Type

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

export type AgentDefinitionBotIntegrationDependencies = Readonly<{
  definitionStore: Pick<AgentDefinitionStore, 'readDefinition'>
  dispatchDependencies: WebhookDispatchDependencies
  dispatchRun?: WebhookDispatch | undefined
  triggerStore: Pick<
    AgentDefinitionTriggerStore,
    | 'listInboundWebhookTriggers'
    | 'recordTriggerFailure'
    | 'recordTriggerSuccess'
  >
}>

export type AgentDefinitionBotIntegrationResult = Readonly<{
  deliveryId: string
  dispatched: number
  eventType: string
  failed: number
  matched: number
  refused: number
  skipped: number
  source: AgentDefinitionWebhookNormalizedEvent['source']
  subjectRef: string
}>

export type AgentDefinitionForumCompletionPolicyDenial = Readonly<{
  reason: string
  statusCode: 409 | 429
}>

export type AgentDefinitionForumCompletionForumStore = Readonly<{
  createReplyPost: (
    input: ForumCreateReplyPostRecordInput,
  ) => Effect.Effect<ForumPostSummaryType, unknown>
  enforceReplyPolicy: (
    input: Readonly<{
      actorRef: string
      bodyText: string
      nowEpochMillis: number
    }>,
  ) => Effect.Effect<AgentDefinitionForumCompletionPolicyDenial | null, unknown>
  readPostByIdempotencyKey: (
    idempotencyKey: string,
  ) => Effect.Effect<ForumPostSummaryType | null, unknown>
  readPostDetail: (
    postId: string,
  ) => Effect.Effect<ForumPostDetailResponseType | null, unknown>
  readSummaryByRef: (
    forumRef: string,
  ) => Effect.Effect<ForumForumSummaryType | null, unknown>
  readTopicById: (
    topicId: string,
  ) => Effect.Effect<ForumTopicSummaryType | null, unknown>
}>

export type AgentDefinitionForumCompletionDependencies = Readonly<{
  forum: AgentDefinitionForumCompletionForumStore
  makeId?: (() => string) | undefined
  nowEpochMillis?: (() => number) | undefined
  nowIso?: (() => string) | undefined
  runStore: Pick<
    AgentDefinitionRunDispatchDependencies['runStore'],
    'readRunByAssignmentRef'
  >
}>

export type AgentDefinitionGitHubCompletionComment = Readonly<{
  commentId: number
  htmlUrl: string
}>

export type AgentDefinitionGitHubCompletionGitHubStore = Readonly<{
  createIssueComment: (
    input: Readonly<{
      bodyText: string
      callback: AgentDefinitionGitHubCompletionCallback
      idempotencyKey: string
    }>,
  ) => Effect.Effect<AgentDefinitionGitHubCompletionComment, unknown>
  readCompletionComment: (
    input: Readonly<{
      callback: AgentDefinitionGitHubCompletionCallback
      idempotencyKey: string
    }>,
  ) => Effect.Effect<AgentDefinitionGitHubCompletionComment | null, unknown>
}>

export type AgentDefinitionGitHubCompletionDependencies = Readonly<{
  github: AgentDefinitionGitHubCompletionGitHubStore
  runStore: Pick<
    AgentDefinitionRunDispatchDependencies['runStore'],
    'readRunByAssignmentRef'
  >
}>

export type AgentDefinitionForumCompletionOutcome =
  | Readonly<{
      kind: 'posted'
      idempotent: boolean
      post: ForumPostSummaryType
      run: AgentDefinitionRunRecord
      topic: ForumTopicSummaryType
    }>
  | Readonly<{
      kind: 'invalid'
      reason: string
      statusCode: number
    }>

export type AgentDefinitionGitHubCompletionOutcome =
  | Readonly<{
      comment: AgentDefinitionGitHubCompletionComment
      idempotent: boolean
      kind: 'posted'
      run: AgentDefinitionRunRecord
      target: AgentDefinitionGitHubCompletionCallback
    }>
  | Readonly<{
      kind: 'invalid'
      reason: string
      statusCode: number
    }>

export const forumCompletionCallbackForEvent = (
  event: AgentDefinitionWebhookNormalizedEvent,
): AgentDefinitionForumCompletionCallback | undefined => {
  if (event.source !== 'forum') {
    return undefined
  }

  const forum = recordValue(event.payload.forum)
  const topic = recordValue(event.payload.topic)
  const post = recordValue(event.payload.post)
  const forumId = stringValue(forum?.id)
  const topicId = stringValue(topic?.id)
  const postId = stringValue(post?.id)
  const sourceUrl = stringValue(event.payload.source_url)

  return forumId === undefined ||
    topicId === undefined ||
    postId === undefined
    ? undefined
    : {
        schema: AGENT_DEFINITION_FORUM_COMPLETION_CALLBACK_SCHEMA,
        forumId,
        kind: 'forum_reply',
        source: 'forum',
        sourcePostId: postId,
        ...(sourceUrl === undefined ? {} : { sourceUrl }),
        subjectRef: event.subjectRef,
        topicId,
      }
}

const recordValue = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined

const stringValue = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const numberValue = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

const subjectKindValue = (
  value: unknown,
): AgentDefinitionGitHubCompletionCallback['subjectKind'] | undefined =>
  value === 'issue' || value === 'pull_request' ? value : undefined

const splitRepositoryFullName = (
  fullName: string,
): Readonly<{ owner: string; repo: string }> | undefined => {
  const [owner, repo, ...extra] = fullName.split('/')

  return owner === undefined ||
    owner.trim() === '' ||
    repo === undefined ||
    repo.trim() === '' ||
    extra.length > 0
    ? undefined
    : { owner, repo }
}

export const githubCompletionCallbackForEvent = (
  event: AgentDefinitionWebhookNormalizedEvent,
): AgentDefinitionGitHubCompletionCallback | undefined => {
  if (
    event.source !== 'github' ||
    event.eventType !== 'issue_comment.created.mention'
  ) {
    return undefined
  }

  const repository = recordValue(event.payload.repository)
  const subject = recordValue(event.payload.subject)
  const comment = recordValue(event.payload.comment)
  const mention = recordValue(event.payload.mention)
  const repositoryFullName = stringValue(repository?.full_name)
  const repositoryParts =
    repositoryFullName === undefined
      ? undefined
      : splitRepositoryFullName(repositoryFullName)
  const subjectKind = subjectKindValue(subject?.kind)
  const subjectNumber = numberValue(subject?.number)
  const sourceCommentId = numberValue(comment?.id)
  const sourceUrl = stringValue(comment?.html_url)
  const mentionLogin = stringValue(mention?.target_login)

  return repositoryFullName === undefined ||
    repositoryParts === undefined ||
    subjectKind === undefined ||
    subjectNumber === undefined ||
    sourceCommentId === undefined ||
    mentionLogin === undefined
    ? undefined
    : {
        schema: AGENT_DEFINITION_GITHUB_COMPLETION_CALLBACK_SCHEMA,
        kind: 'github_issue_comment',
        mentionLogin,
        number: subjectNumber,
        owner: repositoryParts.owner,
        repo: repositoryParts.repo,
        repositoryFullName,
        source: 'github',
        sourceCommentId,
        ...(sourceUrl === undefined ? {} : { sourceUrl }),
        subjectKind,
        subjectRef: event.subjectRef,
      }
}

const triggerMatches = (
  trigger: DueAgentDefinitionTriggerRecord,
  event: AgentDefinitionWebhookNormalizedEvent,
): boolean =>
  trigger.trigger.kind === 'inbound_webhook' &&
  trigger.trigger.source === event.source &&
  agentDefinitionWebhookConditionsMatch(event, trigger.trigger.conditions)

const markTriggerFailure = (
  dependencies: AgentDefinitionBotIntegrationDependencies,
  trigger: DueAgentDefinitionTriggerRecord,
  nowIso: string,
): Promise<boolean> =>
  dependencies.triggerStore.recordTriggerFailure(
    trigger.ownerAgentUserId,
    trigger.triggerRef,
    nowIso,
  )

const markTriggerSuccess = (
  dependencies: AgentDefinitionBotIntegrationDependencies,
  trigger: DueAgentDefinitionTriggerRecord,
  nowIso: string,
): Promise<boolean> =>
  dependencies.triggerStore.recordTriggerSuccess(
    trigger.ownerAgentUserId,
    trigger.triggerRef,
    undefined,
    nowIso,
  )

const dispatchMatchedTrigger = async (
  dependencies: AgentDefinitionBotIntegrationDependencies,
  input: Readonly<{
    completionCallback?: AgentDefinitionBotIntegrationCompletionCallback | undefined
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
        ...(input.completionCallback === undefined
          ? {}
          : { completionCallback: input.completionCallback }),
        triggerRef: input.trigger.triggerRef,
      },
      triggerRef: input.trigger.triggerRef,
    },
  })

export const runAgentDefinitionBotIntegration = async (
  dependencies: AgentDefinitionBotIntegrationDependencies,
  input: Readonly<{
    completionCallback?: AgentDefinitionBotIntegrationCompletionCallback | undefined
    event: AgentDefinitionWebhookNormalizedEvent
    nowIso: string
  }>,
): Promise<AgentDefinitionBotIntegrationResult> => {
  const triggers = await dependencies.triggerStore.listInboundWebhookTriggers(
    input.event.source,
    250,
  )
  let dispatched = 0
  let failed = 0
  let matched = 0
  let refused = 0
  let skipped = 0

  for (const trigger of triggers) {
    if (!triggerMatches(trigger, input.event)) {
      skipped += 1
      continue
    }

    matched += 1
    const definition = await dependencies.definitionStore
      .readDefinition(trigger.ownerAgentUserId, trigger.definitionId)
      .catch(() => undefined)

    if (definition === undefined) {
      failed += 1
      await markTriggerFailure(dependencies, trigger, input.nowIso).catch(
        () => undefined,
      )
      continue
    }

    const dispatch = await dispatchMatchedTrigger(dependencies, {
      completionCallback: input.completionCallback,
      definition,
      event: input.event,
      nowIso: input.nowIso,
      trigger,
    }).catch((): AgentDefinitionRunDispatchOutcome => ({
      kind: 'storage_error',
    }))

    if (dispatch.kind === 'dispatched') {
      const marked = await markTriggerSuccess(
        dependencies,
        trigger,
        input.nowIso,
      ).catch(() => false)
      if (marked) {
        dispatched += 1
      } else {
        failed += 1
      }
      continue
    }

    const marked = await markTriggerFailure(
      dependencies,
      trigger,
      input.nowIso,
    ).catch(() => false)

    if (!marked) {
      failed += 1
    } else if (dispatch.kind === 'refused') {
      refused += 1
    } else {
      failed += 1
    }
  }

  return {
    deliveryId: input.event.deliveryId,
    dispatched,
    eventType: input.event.eventType,
    failed,
    matched,
    refused,
    skipped,
    source: input.event.source,
    subjectRef: input.event.subjectRef,
  }
}

const decodeForumCompletionCallback = (
  value: unknown,
): AgentDefinitionForumCompletionCallback | undefined => {
  try {
    return S.decodeUnknownSync(AgentDefinitionForumCompletionCallback)(value)
  } catch {
    return undefined
  }
}

const decodeGitHubCompletionCallback = (
  value: unknown,
): AgentDefinitionGitHubCompletionCallback | undefined => {
  try {
    return S.decodeUnknownSync(AgentDefinitionGitHubCompletionCallback)(value)
  } catch {
    return undefined
  }
}

const publicProjection = (artifactRef: string): ForumPublicProjectionType => ({
  classificationCaveatRef: 'classification.public_forum_projection',
  customerSafe: true,
  dataClassification: 'public',
  excludedPrivateRefs: [],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.public.v1',
  safeArtifactRefs: [artifactRef],
  safeReceiptRefs: [],
  trustTier: 'reviewed',
})

const forumCompletionActor = {
  _tag: 'Operator' as const,
  operator: {
    displayName: 'OpenAgents Background Agents',
    operatorId: 'background-agents',
    slug: 'openagents-background-agents',
  },
}

const requiredForumWriteScope = (
  forum: ForumForumSummaryType,
): 'forum.write' | 'forum.void.write' =>
  forum.slug === 'void' ? 'forum.void.write' : 'forum.write'

export const postAgentDefinitionForumCompletion = (
  dependencies: AgentDefinitionForumCompletionDependencies,
  input: AgentDefinitionForumCompletionRequest,
): Effect.Effect<AgentDefinitionForumCompletionOutcome, unknown> =>
  Effect.gen(function* () {
    const run = yield* Effect.promise(() =>
      dependencies.runStore.readRunByAssignmentRef(input.assignmentRef),
    )

    if (run === undefined) {
      return {
        kind: 'invalid' as const,
        reason: 'agent definition run was not found for assignmentRef',
        statusCode: 404,
      }
    }

    const callback = decodeForumCompletionCallback(
      run.triggerPayload.completionCallback,
    )

    if (callback === undefined) {
      return {
        kind: 'invalid' as const,
        reason: 'agent definition run does not carry a Forum completion callback',
        statusCode: 409,
      }
    }

    const topic = yield* dependencies.forum.readTopicById(callback.topicId)

    if (
      topic === null ||
      topic.forumId !== callback.forumId ||
      topic.state === 'archived' ||
      topic.state === 'hidden'
    ) {
      return {
        kind: 'invalid' as const,
        reason: 'source Forum topic is not writable',
        statusCode: 404,
      }
    }

    if (topic.state === 'locked') {
      return {
        kind: 'invalid' as const,
        reason: 'source Forum topic is locked',
        statusCode: 423,
      }
    }

    const sourcePost = yield* dependencies.forum.readPostDetail(
      callback.sourcePostId,
    )

    if (
      sourcePost === null ||
      sourcePost.containingTopicId !== callback.topicId ||
      sourcePost.post.state === 'tombstoned'
    ) {
      return {
        kind: 'invalid' as const,
        reason: 'source Forum post is not writable',
        statusCode: 404,
      }
    }

    const forum = yield* dependencies.forum.readSummaryByRef(callback.forumId)

    if (forum === null) {
      return {
        kind: 'invalid' as const,
        reason: 'source Forum is not readable',
        statusCode: 404,
      }
    }

    if (forum.locked) {
      return {
        kind: 'invalid' as const,
        reason: 'source Forum is locked',
        statusCode: 423,
      }
    }

    const idempotencyKey = `agent-definition-completion:${run.runId}`
    const existingPost = yield* dependencies.forum.readPostByIdempotencyKey(
      idempotencyKey,
    )

    if (existingPost !== null) {
      return {
        kind: 'posted' as const,
        idempotent: true,
        post: existingPost,
        run,
        topic,
      }
    }

    const nowEpochMillis =
      dependencies.nowEpochMillis ?? currentEpochMillis
    const writer = yield* buildForumWriterContext({
      actor: forumCompletionActor,
      grant: {
        expiresAtEpochMillis: null,
        forumIds: [forum.forumId],
        ownerUserId: null,
        scopes: [requiredForumWriteScope(forum)],
        status: 'active',
        teamId: null,
      },
      nowEpochMillis,
      paymentProofRef: null,
      requiredScope: requiredForumWriteScope(forum),
      targetForumId: forum.forumId,
      targetOwnerUserId: null,
      targetTeamId: null,
    })
    const makeId = dependencies.makeId ?? randomUuid
    const postId = makeId()
    const artifactRef = `artifact.forum.agent_definition_completion.${postId}`
    const writePolicyDenial = yield* dependencies.forum.enforceReplyPolicy({
      actorRef: writer.actor.actorRef,
      bodyText: input.bodyText,
      nowEpochMillis: nowEpochMillis(),
    })

    if (writePolicyDenial !== null) {
      return {
        kind: 'invalid' as const,
        reason: writePolicyDenial.reason,
        statusCode: writePolicyDenial.statusCode,
      }
    }

    const post = yield* dependencies.forum.createReplyPost({
      actor: writer.actor,
      bodyText: input.bodyText,
      contextLinks: [
        {
          contextId: run.runId,
          contextKind: 'workroom',
          contextSlug: run.definitionId,
          contextTitle: run.definitionRef,
          forumId: forum.forumId,
          id: makeId(),
          postId,
          publicProjection: publicProjection(
            `artifact.forum.context.${run.runId}`,
          ),
          publicUrl: canonicalForumTopicHref(topic.topicId),
          sourceRef: run.assignmentRef,
          targetKind: 'post',
          topicId: topic.topicId,
        },
      ],
      contentRef: `content.forum.post.${postId}`,
      forumId: forum.forumId,
      idempotencyKey,
      parentPostId: callback.sourcePostId,
      postId,
      publicProjection: publicProjection(artifactRef),
      quotePostId: null,
      topicId: topic.topicId,
    })

    return {
      kind: 'posted' as const,
      idempotent: false,
      post,
      run,
      topic,
    }
  })

export const postAgentDefinitionGitHubCompletion = (
  dependencies: AgentDefinitionGitHubCompletionDependencies,
  input: AgentDefinitionGitHubCompletionRequest,
): Effect.Effect<AgentDefinitionGitHubCompletionOutcome, unknown> =>
  Effect.gen(function* () {
    const run = yield* Effect.promise(() =>
      dependencies.runStore.readRunByAssignmentRef(input.assignmentRef),
    )

    if (run === undefined) {
      return {
        kind: 'invalid' as const,
        reason: 'agent definition run was not found for assignmentRef',
        statusCode: 404,
      }
    }

    const callback = decodeGitHubCompletionCallback(
      run.triggerPayload.completionCallback,
    )

    if (callback === undefined) {
      return {
        kind: 'invalid' as const,
        reason:
          'agent definition run does not carry a GitHub completion callback',
        statusCode: 409,
      }
    }

    const idempotencyKey = `agent-definition-github-completion:${run.runId}`
    const existingComment = yield* dependencies.github.readCompletionComment({
      callback,
      idempotencyKey,
    })

    if (existingComment !== null) {
      return {
        comment: existingComment,
        idempotent: true,
        kind: 'posted' as const,
        run,
        target: callback,
      }
    }

    const comment = yield* dependencies.github.createIssueComment({
      bodyText: input.bodyText,
      callback,
      idempotencyKey,
    })

    return {
      comment,
      idempotent: false,
      kind: 'posted' as const,
      run,
      target: callback,
    }
  })

const GitHubIssueCommentApiItem = S.Struct({
  body: S.optionalKey(S.String),
  html_url: S.String,
  id: S.Number,
})
const GitHubIssueCommentApiItems = S.Array(GitHubIssueCommentApiItem)

const githubCompletionMarker = (idempotencyKey: string): string =>
  `<!-- openagents:${idempotencyKey} -->`

const githubCompletionCommentBody = (
  bodyText: string,
  idempotencyKey: string,
): string => `${bodyText}\n\n${githubCompletionMarker(idempotencyKey)}`

const githubIssueCommentsUrl = (
  callback: AgentDefinitionGitHubCompletionCallback,
  page: number,
): string => {
  const url = new URL(
    `https://api.github.com/repos/${encodeURIComponent(callback.owner)}/${encodeURIComponent(callback.repo)}/issues/${callback.number}/comments`,
  )
  url.searchParams.set('page', String(page))
  url.searchParams.set('per_page', '100')

  return url.toString()
}

const decodeGitHubIssueComment = (
  value: unknown,
): AgentDefinitionGitHubCompletionComment => {
  const decoded = S.decodeUnknownSync(GitHubIssueCommentApiItem)(value)

  return {
    commentId: decoded.id,
    htmlUrl: decoded.html_url,
  }
}

const decodeGitHubIssueComments = (
  value: unknown,
): ReadonlyArray<typeof GitHubIssueCommentApiItem.Type> =>
  S.decodeUnknownSync(GitHubIssueCommentApiItems)(value)

const githubCompletionHeaders = (
  accessToken: string,
): HeadersInit => ({
  Accept: 'application/vnd.github+json',
  Authorization: `Bearer ${accessToken}`,
  'Content-Type': 'application/json',
  'User-Agent': 'OpenAgents',
  'X-GitHub-Api-Version': '2022-11-28',
})

export const makeGitHubRestAgentDefinitionCompletionGitHubStore = (
  accessToken: string,
  fetcher: typeof fetch = fetch,
): AgentDefinitionGitHubCompletionGitHubStore => ({
  createIssueComment: input =>
    Effect.tryPromise({
      catch: error =>
        error instanceof AgentDefinitionGitHubCompletionRestError
          ? error
          : githubCompletionRestError(
              'GitHub issue comment create failed',
              error,
            ),
      try: async () => {
        const response = await fetcher(
          githubIssueCommentsUrl(input.callback, 1),
          {
            body: JSON.stringify({
              body: githubCompletionCommentBody(
                input.bodyText,
                input.idempotencyKey,
              ),
            }),
            headers: githubCompletionHeaders(accessToken),
            method: 'POST',
          },
        )

        if (!response.ok) {
          throw githubCompletionRestError('GitHub issue comment create failed')
        }

        return decodeGitHubIssueComment(await response.json())
      },
    }),
  readCompletionComment: input =>
    Effect.tryPromise({
      catch: error =>
        error instanceof AgentDefinitionGitHubCompletionRestError
          ? error
          : githubCompletionRestError(
              'GitHub issue comment lookup failed',
              error,
            ),
      try: async () => {
        const marker = githubCompletionMarker(input.idempotencyKey)

        for (let page = 1; page <= 5; page += 1) {
          const response = await fetcher(
            githubIssueCommentsUrl(input.callback, page),
            {
              headers: githubCompletionHeaders(accessToken),
              method: 'GET',
            },
          )

          if (!response.ok) {
            throw githubCompletionRestError('GitHub issue comment lookup failed')
          }

          const comments = decodeGitHubIssueComments(await response.json())
          const found = comments.find(comment => comment.body?.includes(marker))

          if (found !== undefined) {
            return {
              commentId: found.id,
              htmlUrl: found.html_url,
            }
          }

          if (comments.length < 100) {
            return null
          }
        }

        return null
      },
    }),
})

export const makeD1AgentDefinitionForumCompletionForumStore = (
  db: D1Database,
  runtime: Readonly<{
    makeId?: (() => string) | undefined
    nowIso?: (() => string) | undefined
  }> = {},
): AgentDefinitionForumCompletionForumStore => ({
  createReplyPost: input =>
    createForumReplyPost(db, input, {
      makeId: runtime.makeId ?? randomUuid,
      nowIso: runtime.nowIso ?? currentIsoTimestamp,
    }),
  enforceReplyPolicy: input =>
    Effect.gen(function* () {
      const sinceIso = epochMillisToIsoTimestamp(
        input.nowEpochMillis -
          ForumWritePolicyMaxLookupWindowSeconds * 1000,
      )
      const recentPosts = yield* listRecentForumWritesForActor(db, {
        actorRef: input.actorRef,
        limit: 25,
        sinceIso,
      })
      const decision = evaluateForumWritePolicy({
        actionKind: 'reply',
        bodyText: input.bodyText,
        nowEpochMillis: input.nowEpochMillis,
        recentPosts: recentPosts.map(post => ({
          bodyText: post.body_text ?? '',
          createdAt: post.created_at,
          postNumber: post.post_number,
        })),
      })

      return decision._tag === 'Allowed'
        ? null
        : {
            reason: decision.reason,
            statusCode:
              decision.denialKind === 'duplicate_content' ? 409 : 429,
          }
    }),
  readPostByIdempotencyKey: idempotencyKey =>
    readForumPostByIdempotencyKey(db, idempotencyKey),
  readPostDetail: postId => readForumPostDetail(db, postId),
  readSummaryByRef: forumRef =>
    readForumSummaryByRef(db, forumRef, { allowUnlisted: true }),
  readTopicById: topicId => readForumTopicById(db, topicId),
})
