import { Schema as S } from "effect"

import type { AgentDefinitionInboundWebhookCondition } from "./index.js"

export const AgentDefinitionWebhookNormalizedEventSchemaLiteral =
  "openagents.agent_definition_webhook_event.v1" as const

export const AgentDefinitionWebhookSource = S.Literals(["github"])
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

const githubSenderPayload = (
  sender: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined =>
  sender === undefined
    ? undefined
    : optionalObject([
        ["id", numberValue(sender.id)],
        ["login", stringValue(sender.login)],
      ])

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

  return refs
}

export const normalizeGitHubWebhookEvent = (
  input: Readonly<{
    deliveryId: string
    eventName: string
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
  const normalizedPayload = optionalObject([
    ["action", action],
    ["event", eventName],
    ["issue", githubIssueLikePayload(asRecord(input.payload.issue))],
    [
      "pull_request",
      githubIssueLikePayload(asRecord(input.payload.pull_request)),
    ],
    ["repository", repository],
    ["sender", githubSenderPayload(asRecord(input.payload.sender))],
  ])

  return {
    schema: AgentDefinitionWebhookNormalizedEventSchemaLiteral,
    source: "github",
    eventType: action === undefined ? eventName : `${eventName}.${action}`,
    deliveryId,
    subjectRef: githubSubjectRef(input.payload, deliveryId),
    receivedAt: input.receivedAt,
    payload: normalizedPayload,
    sourceRefs: githubSourceRefs(input.payload, deliveryId),
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
