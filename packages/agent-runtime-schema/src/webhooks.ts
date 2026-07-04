import { Schema as S } from "effect"

import type { AgentDefinitionInboundWebhookCondition } from "./index.js"

export const AgentDefinitionWebhookNormalizedEventSchemaLiteral =
  "openagents.agent_definition_webhook_event.v1" as const

export const AgentDefinitionWebhookSource = S.Literals(["github", "forum"])
export type AgentDefinitionWebhookSource =
  typeof AgentDefinitionWebhookSource.Type

export const AgentDefinitionWebhookNormalizedEvent = S.Struct({
  schema: S.Literal(AgentDefinitionWebhookNormalizedEventSchemaLiteral),
  source: AgentDefinitionWebhookSource,
  eventType: S.String,
  deliveryId: S.String,
  subjectRef: S.String,
  receivedAt: S.String,
  payload: S.Record(S.String, S.Unknown),
  sourceRefs: S.Array(S.String),
})
export type AgentDefinitionWebhookNormalizedEvent =
  typeof AgentDefinitionWebhookNormalizedEvent.Type

const asRecord = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined

const stringValue = (value: unknown): string | undefined =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : undefined

const numberValue = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined

const booleanValue = (value: unknown): boolean | undefined =>
  typeof value === "boolean" ? value : undefined

const compactRefSegment = (value: string): string =>
  value.trim().replaceAll(/[^A-Za-z0-9_.:/=-]+/g, "_").slice(0, 180)

const optionalObject = (
  entries: ReadonlyArray<readonly [string, unknown | undefined]>,
): Record<string, unknown> => {
  const output: Record<string, unknown> = {}

  for (const [key, value] of entries) {
    if (value !== undefined) {
      output[key] = value
    }
  }

  return output
}

const githubRepositoryPayload = (
  repository: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined => {
  if (repository === undefined) {
    return undefined
  }

  const owner = asRecord(repository.owner)

  return optionalObject([
    ["full_name", stringValue(repository.full_name)],
    ["id", numberValue(repository.id)],
    ["name", stringValue(repository.name)],
    [
      "owner",
      owner === undefined
        ? undefined
        : optionalObject([
            ["id", numberValue(owner.id)],
            ["login", stringValue(owner.login)],
          ]),
    ],
  ])
}

const githubIssueLikePayload = (
  value: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined =>
  value === undefined
    ? undefined
    : optionalObject([
        ["html_url", stringValue(value.html_url)],
        ["number", numberValue(value.number)],
        ["state", stringValue(value.state)],
        ["title", stringValue(value.title)],
      ])

const githubIssueSubjectPayload = (
  payload: Record<string, unknown>,
): Record<string, unknown> | undefined => {
  const issue = asRecord(payload.issue)
  const pullRequest = asRecord(payload.pull_request)

  if (issue !== undefined) {
    const pullRequestMarker = asRecord(issue.pull_request)

    return optionalObject([
      ["html_url", stringValue(issue.html_url)],
      ["kind", pullRequestMarker === undefined ? "issue" : "pull_request"],
      ["number", numberValue(issue.number)],
      ["state", stringValue(issue.state)],
      ["title", stringValue(issue.title)],
    ])
  }

  if (pullRequest !== undefined) {
    return optionalObject([
      ["html_url", stringValue(pullRequest.html_url)],
      ["kind", "pull_request"],
      ["number", numberValue(pullRequest.number)],
      ["state", stringValue(pullRequest.state)],
      ["title", stringValue(pullRequest.title)],
    ])
  }

  return undefined
}

const githubSenderPayload = (
  sender: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined =>
  sender === undefined
    ? undefined
    : optionalObject([
        ["id", numberValue(sender.id)],
        ["login", stringValue(sender.login)],
      ])

const githubIssueCommentPayload = (
  value: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined => {
  if (value === undefined) {
    return undefined
  }

  return optionalObject([
    ["author_association", stringValue(value.author_association)],
    ["html_url", stringValue(value.html_url)],
    ["id", numberValue(value.id)],
    ["user", githubSenderPayload(asRecord(value.user))],
  ])
}

const defaultGitHubMentionLogins = ["openagents", "openagentsinc"] as const

const normalizedMentionTargets = (
  mentionLogins: ReadonlyArray<string> | undefined,
): ReadonlySet<string> => {
  const targets = (mentionLogins ?? defaultGitHubMentionLogins)
    .map(login => login.trim().toLowerCase())
    .filter(login => login !== "")

  return new Set(targets)
}

const githubMentionFromComment = (
  input: Readonly<{
    action: string | undefined
    body: string | undefined
    eventName: string
    mentionLogins: ReadonlyArray<string> | undefined
  }>,
): Record<string, unknown> | undefined => {
  if (
    input.eventName !== "issue_comment" ||
    input.action !== "created" ||
    input.body === undefined
  ) {
    return undefined
  }

  const targets = normalizedMentionTargets(input.mentionLogins)

  if (targets.size === 0) {
    return undefined
  }

  const mentionPattern = /@([A-Za-z0-9-]{1,39})\b/g

  for (const match of input.body.matchAll(mentionPattern)) {
    const login = match[1]?.toLowerCase()
    if (login !== undefined && targets.has(login)) {
      return {
        present: true,
        source: "comment",
        target_login: login,
      }
    }
  }

  return undefined
}

const githubSubjectRef = (
  payload: Record<string, unknown>,
  deliveryId: string,
): string => {
  const repository = asRecord(payload.repository)
  const fullName = stringValue(repository?.full_name)
  const issueNumber = numberValue(asRecord(payload.issue)?.number)
  const pullNumber = numberValue(asRecord(payload.pull_request)?.number)
  const refPrefix =
    fullName === undefined
      ? "github.delivery"
      : `github.repository.${compactRefSegment(fullName)}`

  if (issueNumber !== undefined) {
    return `${refPrefix}.issue.${issueNumber}`
  }

  if (pullNumber !== undefined) {
    return `${refPrefix}.pull_request.${pullNumber}`
  }

  return `${refPrefix}.delivery.${compactRefSegment(deliveryId)}`
}

const githubSourceRefs = (
  payload: Record<string, unknown>,
  deliveryId: string,
): ReadonlyArray<string> => {
  const repository = asRecord(payload.repository)
  const fullName = stringValue(repository?.full_name)
  const issueNumber = numberValue(asRecord(payload.issue)?.number)
  const pullNumber = numberValue(asRecord(payload.pull_request)?.number)
  const commentId = numberValue(asRecord(payload.comment)?.id)
  const refs = [`github.delivery.${compactRefSegment(deliveryId)}`]

  if (fullName !== undefined) {
    refs.push(`github.repository.${compactRefSegment(fullName)}`)
  }

  if (fullName !== undefined && issueNumber !== undefined) {
    refs.push(`github.issue.${compactRefSegment(fullName)}.${issueNumber}`)
  }

  if (fullName !== undefined && pullNumber !== undefined) {
    refs.push(
      `github.pull_request.${compactRefSegment(fullName)}.${pullNumber}`,
    )
  }

  if (fullName !== undefined && commentId !== undefined) {
    refs.push(`github.comment.${compactRefSegment(fullName)}.${commentId}`)
  }

  return refs
}

const forumPayload = (
  payload: Record<string, unknown>,
): Record<string, unknown> => {
  const forum = optionalObject([
    ["id", stringValue(payload.forumId)],
    ["slug", stringValue(payload.forumSlug)],
    ["title", stringValue(payload.forumTitle)],
  ])
  const topic = optionalObject([
    ["id", stringValue(payload.topicId)],
    ["slug", stringValue(payload.topicSlug)],
    ["state", stringValue(payload.topicState)],
    ["title", stringValue(payload.topicTitle)],
  ])
  const post = optionalObject([
    ["id", stringValue(payload.postId)],
    ["number", numberValue(payload.postNumber)],
    ["state", stringValue(payload.postState)],
  ])
  const actor = optionalObject([
    ["ref", stringValue(payload.actorRef)],
    ["slug", stringValue(payload.actorSlug)],
    ["display_name", stringValue(payload.actorDisplayName)],
  ])

  return optionalObject([
    ["event", stringValue(payload.eventType)],
    ["forum", Object.keys(forum).length === 0 ? undefined : forum],
    ["topic", Object.keys(topic).length === 0 ? undefined : topic],
    ["post", Object.keys(post).length === 0 ? undefined : post],
    ["actor", Object.keys(actor).length === 0 ? undefined : actor],
    ["source_url", stringValue(payload.sourceUrl)],
  ])
}

const forumSubjectRef = (
  payload: Record<string, unknown>,
  deliveryId: string,
): string => {
  const topicId = stringValue(payload.topicId)
  const postId = stringValue(payload.postId)
  const forumId = stringValue(payload.forumId) ?? stringValue(payload.forumSlug)

  if (topicId !== undefined && postId !== undefined) {
    return `forum.topic.${compactRefSegment(topicId)}.post.${compactRefSegment(postId)}`
  }

  if (topicId !== undefined) {
    return `forum.topic.${compactRefSegment(topicId)}`
  }

  if (forumId !== undefined) {
    return `forum.${compactRefSegment(forumId)}.delivery.${compactRefSegment(deliveryId)}`
  }

  return `forum.delivery.${compactRefSegment(deliveryId)}`
}

const forumSourceRefs = (
  payload: Record<string, unknown>,
  deliveryId: string,
): ReadonlyArray<string> => {
  const refs = [`forum.delivery.${compactRefSegment(deliveryId)}`]
  const forumId = stringValue(payload.forumId)
  const forumSlug = stringValue(payload.forumSlug)
  const topicId = stringValue(payload.topicId)
  const postId = stringValue(payload.postId)

  if (forumId !== undefined) {
    refs.push(`forum.forum.${compactRefSegment(forumId)}`)
  }

  if (forumSlug !== undefined) {
    refs.push(`forum.slug.${compactRefSegment(forumSlug)}`)
  }

  if (topicId !== undefined) {
    refs.push(`forum.topic.${compactRefSegment(topicId)}`)
  }

  if (postId !== undefined) {
    refs.push(`forum.post.${compactRefSegment(postId)}`)
  }

  return refs
}

export const normalizeGitHubWebhookEvent = (
  input: Readonly<{
    deliveryId: string
    eventName: string
    mentionLogins?: ReadonlyArray<string> | undefined
    payload: Record<string, unknown>
    receivedAt: string
  }>,
): AgentDefinitionWebhookNormalizedEvent | undefined => {
  const eventName = input.eventName.trim()
  const deliveryId = input.deliveryId.trim()

  if (eventName === "" || deliveryId === "") {
    return undefined
  }

  const action = stringValue(input.payload.action)
  const repository = githubRepositoryPayload(asRecord(input.payload.repository))
  const comment = asRecord(input.payload.comment)
  const mention = githubMentionFromComment({
    action,
    body: stringValue(comment?.body),
    eventName,
    mentionLogins: input.mentionLogins,
  })
  const normalizedPayload = optionalObject([
    ["action", action],
    ["comment", githubIssueCommentPayload(comment)],
    ["event", eventName],
    ["issue", githubIssueLikePayload(asRecord(input.payload.issue))],
    [
      "pull_request",
      githubIssueLikePayload(asRecord(input.payload.pull_request)),
    ],
    ["mention", mention],
    ["repository", repository],
    ["sender", githubSenderPayload(asRecord(input.payload.sender))],
    ["subject", githubIssueSubjectPayload(input.payload)],
  ])
  const baseEventType =
    action === undefined ? eventName : `${eventName}.${action}`

  return {
    schema: AgentDefinitionWebhookNormalizedEventSchemaLiteral,
    source: "github",
    eventType:
      booleanValue(mention?.present) === true
        ? `${baseEventType}.mention`
        : baseEventType,
    deliveryId,
    subjectRef: githubSubjectRef(input.payload, deliveryId),
    receivedAt: input.receivedAt,
    payload: normalizedPayload,
    sourceRefs: githubSourceRefs(input.payload, deliveryId),
  }
}

export const normalizeForumWebhookEvent = (
  input: Readonly<{
    deliveryId: string
    eventType: string
    payload: Record<string, unknown>
    receivedAt: string
  }>,
): AgentDefinitionWebhookNormalizedEvent | undefined => {
  const eventType = input.eventType.trim()
  const deliveryId = input.deliveryId.trim()
  const forumId = stringValue(input.payload.forumId)
  const topicId = stringValue(input.payload.topicId)
  const postId = stringValue(input.payload.postId)

  if (
    eventType === "" ||
    deliveryId === "" ||
    forumId === undefined ||
    topicId === undefined ||
    postId === undefined
  ) {
    return undefined
  }

  return {
    schema: AgentDefinitionWebhookNormalizedEventSchemaLiteral,
    source: "forum",
    eventType,
    deliveryId,
    subjectRef: forumSubjectRef(input.payload, deliveryId),
    receivedAt: input.receivedAt,
    payload: forumPayload({ ...input.payload, eventType }),
    sourceRefs: forumSourceRefs(input.payload, deliveryId),
  }
}

const pathSegments = (path: string): ReadonlyArray<string> | undefined => {
  if (!path.startsWith("$")) {
    return undefined
  }

  const tail = path.slice(1)

  if (tail === "") {
    return []
  }

  if (!tail.startsWith(".")) {
    return undefined
  }

  const segments = tail
    .slice(1)
    .split(".")
    .map(segment => segment.trim())

  return segments.every(segment => /^[A-Za-z0-9_]+$/.test(segment))
    ? segments
    : undefined
}

const jsonPathValue = (
  root: Record<string, unknown>,
  path: string,
): unknown => {
  const segments = pathSegments(path)

  if (segments === undefined) {
    return undefined
  }

  let current: unknown = root

  for (const segment of segments) {
    const record = asRecord(current)
    if (record === undefined || !(segment in record)) {
      return undefined
    }
    current = record[segment]
  }

  return current
}

const comparableString = (value: unknown): string | undefined =>
  typeof value === "string" ||
  typeof value === "number" ||
  typeof value === "boolean"
    ? String(value)
    : undefined

const conditionMatches = (
  event: AgentDefinitionWebhookNormalizedEvent,
  condition: AgentDefinitionInboundWebhookCondition,
): boolean => {
  if (condition.kind === "event_type") {
    return event.eventType === condition.equals
  }

  const value = comparableString(jsonPathValue(event.payload, condition.path))

  if (value === undefined) {
    return false
  }

  if (condition.kind === "json_path_equals") {
    return value === condition.equals
  }

  if (condition.kind === "json_path_in") {
    return condition.values.includes(value)
  }

  try {
    return new RegExp(condition.pattern).test(value)
  } catch {
    return false
  }
}

export const agentDefinitionWebhookConditionsMatch = (
  event: AgentDefinitionWebhookNormalizedEvent,
  conditions: ReadonlyArray<AgentDefinitionInboundWebhookCondition>,
): boolean => conditions.every(condition => conditionMatches(event, condition))
