import { createHmac, timingSafeEqual } from "node:crypto"
import { Schema as S } from "effect"

export const CONNECTOR_SIDECAR_SCHEMA_VERSION = "openagents.connector_sidecar.v1" as const

export const ConnectorProvider = S.Literal("github")
export type ConnectorProvider = typeof ConnectorProvider.Type

export const ConnectorEventKind = S.Literals([
  "github.issue",
  "github.pull_request",
])
export type ConnectorEventKind = typeof ConnectorEventKind.Type

export const ConnectorSubjectKind = S.Literals(["issue", "pull_request"])
export type ConnectorSubjectKind = typeof ConnectorSubjectKind.Type

export const ConnectorSubject = S.Struct({
  provider: ConnectorProvider,
  kind: ConnectorSubjectKind,
  owner: S.String,
  repo: S.String,
  number: S.Int,
  htmlUrl: S.String,
})
export type ConnectorSubject = typeof ConnectorSubject.Type

export const ConnectorSourceVerifiedEvent = S.Struct({
  schema: S.Literal(CONNECTOR_SIDECAR_SCHEMA_VERSION),
  provider: ConnectorProvider,
  eventKind: ConnectorEventKind,
  action: S.optional(S.String),
  deliveryId: S.String,
  dedupeKey: S.String,
  receivedAt: S.String,
  sourceVerified: S.Literal(true),
  subject: ConnectorSubject,
  sourceRefs: S.Array(S.String),
})
export type ConnectorSourceVerifiedEvent = typeof ConnectorSourceVerifiedEvent.Type

export const ConnectorWorkspaceLaneProjection = S.Struct({
  schema: S.Literal(CONNECTOR_SIDECAR_SCHEMA_VERSION),
  laneRef: S.String,
  eventRef: S.String,
  provider: ConnectorProvider,
  subject: ConnectorSubject,
  sourceRefs: S.Array(S.String),
  allowedWritebackToolRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
})
export type ConnectorWorkspaceLaneProjection = typeof ConnectorWorkspaceLaneProjection.Type

export const ConnectorWritebackToolRef = S.Literals([
  "tool.connector.github.issue.comment.create",
  "tool.connector.github.pull_request.comment.create",
])
export type ConnectorWritebackToolRef = typeof ConnectorWritebackToolRef.Type

export const ConnectorWritebackRequest = S.Struct({
  provider: ConnectorProvider,
  toolRef: S.String,
  owner: S.String,
  repo: S.String,
  subjectKind: ConnectorSubjectKind,
  number: S.Int,
})
export type ConnectorWritebackRequest = typeof ConnectorWritebackRequest.Type

export type ConnectorAuthorityDecision =
  | {
      allowed: true
      status: "allowed"
      reasonRef:
        | "reason.connector.dispatch.authority_bound"
        | "reason.connector.writeback.subject_bound"
      blockerRefs: []
    }
  | {
      allowed: false
      status: "denied"
      reasonRef:
        | "reason.connector.app_idempotency_required"
        | "reason.connector.generic_provider_tool_forbidden"
        | "reason.connector.model_context_contains_private_provider_material"
        | "reason.connector.platform_authority_forbidden"
        | "reason.connector.source_verification_required"
        | "reason.connector.tool_not_allowed"
        | "reason.connector.tool_forbidden_authority"
        | "reason.connector.writeback_subject_mismatch"
      blockerRefs: [string]
    }

export type GitHubWebhookHeaders = {
  event: string | null | undefined
  delivery: string | null | undefined
  signature256: string | null | undefined
}

export type NormalizeGitHubWebhookInput = {
  headers: GitHubWebhookHeaders
  rawBody: string | Uint8Array
  webhookSecret: string
  receivedAt: string
}

export type NormalizeGitHubWebhookResult =
  | {
      ok: true
      event: ConnectorSourceVerifiedEvent
    }
  | {
      ok: false
      reasonRef:
        | "reason.connector.github.missing_header"
        | "reason.connector.github.bad_signature"
        | "reason.connector.github.unsupported_event"
        | "reason.connector.github.unbounded_subject"
        | "reason.connector.github.invalid_json"
      blockerRefs: [string]
    }

export type ConnectorDispatchAuthorityInput = {
  event: ConnectorSourceVerifiedEvent
  appOwnedIdempotencyKey: string | null | undefined
  toolRefs: ReadonlyArray<string>
  modelContextItems?: ReadonlyArray<unknown> | undefined
  sessionHistoryItems?: ReadonlyArray<unknown> | undefined
  logItems?: ReadonlyArray<unknown> | undefined
  requestedPlatformAuthorityRefs?: ReadonlyArray<string> | undefined
}

const textEncoder = new TextEncoder()

const bytesOf = (value: string | Uint8Array): Uint8Array =>
  typeof value === "string" ? textEncoder.encode(value) : value

export const createGitHubWebhookSignature = (
  webhookSecret: string,
  rawBody: string | Uint8Array,
): string =>
  `sha256=${createHmac("sha256", webhookSecret)
    .update(bytesOf(rawBody))
    .digest("hex")}`

export const verifyGitHubWebhookSignature = (input: {
  webhookSecret: string
  rawBody: string | Uint8Array
  signature256: string | null | undefined
}): boolean => {
  const signature = input.signature256
  if (!signature?.startsWith("sha256=")) {
    return false
  }

  const expected = createGitHubWebhookSignature(input.webhookSecret, input.rawBody)
  const expectedBytes = Buffer.from(expected, "utf8")
  const actualBytes = Buffer.from(signature, "utf8")

  return (
    expectedBytes.length === actualBytes.length &&
    timingSafeEqual(expectedBytes, actualBytes)
  )
}

export const createConnectorDeliveryDedupeKey = (input: {
  provider: ConnectorProvider
  deliveryId: string
}): string => `connector.${input.provider}.delivery.${input.deliveryId}`

const safeRefPart = (value: string): string =>
  value.toLowerCase().replace(/[^a-z0-9_.-]+/g, "_")

const issueOrPrSubjectFromPayload = (
  providerEvent: string,
  payload: unknown,
): ConnectorSubject | null => {
  if (typeof payload !== "object" || payload === null) {
    return null
  }

  const record = payload as Record<string, unknown>
  const repository = record.repository as Record<string, unknown> | undefined
  const fullName = typeof repository?.full_name === "string" ? repository.full_name : null
  const [owner, repo, extra] = fullName?.split("/") ?? []
  if (!owner || !repo || extra) {
    return null
  }

  const subjectKey = providerEvent === "issues" ? "issue" : "pull_request"
  const subjectKind = providerEvent === "issues" ? "issue" : "pull_request"
  const subjectRecord = record[subjectKey] as Record<string, unknown> | undefined
  const number = subjectRecord?.number
  const htmlUrl = subjectRecord?.html_url
  if (typeof number !== "number" || !Number.isInteger(number) || typeof htmlUrl !== "string") {
    return null
  }

  return {
    provider: "github",
    kind: subjectKind,
    owner,
    repo,
    number,
    htmlUrl,
  }
}

const connectorEventKindFromGitHubEvent = (
  event: string,
): ConnectorEventKind | null => {
  if (event === "issues") {
    return "github.issue"
  }
  if (event === "pull_request") {
    return "github.pull_request"
  }
  return null
}

export const normalizeGitHubWebhookEvent = (
  input: NormalizeGitHubWebhookInput,
): NormalizeGitHubWebhookResult => {
  const { delivery, event, signature256 } = input.headers
  if (!delivery || !event || !signature256) {
    return {
      ok: false,
      reasonRef: "reason.connector.github.missing_header",
      blockerRefs: ["blocker.connector.github.missing_header"],
    }
  }

  if (!verifyGitHubWebhookSignature({
    webhookSecret: input.webhookSecret,
    rawBody: input.rawBody,
    signature256,
  })) {
    return {
      ok: false,
      reasonRef: "reason.connector.github.bad_signature",
      blockerRefs: ["blocker.connector.github.bad_signature"],
    }
  }

  const eventKind = connectorEventKindFromGitHubEvent(event)
  if (!eventKind) {
    return {
      ok: false,
      reasonRef: "reason.connector.github.unsupported_event",
      blockerRefs: ["blocker.connector.github.unsupported_event"],
    }
  }

  let payload: unknown
  try {
    payload = JSON.parse(
      typeof input.rawBody === "string"
        ? input.rawBody
        : new TextDecoder().decode(input.rawBody),
    )
  } catch {
    return {
      ok: false,
      reasonRef: "reason.connector.github.invalid_json",
      blockerRefs: ["blocker.connector.github.invalid_json"],
    }
  }

  const subject = issueOrPrSubjectFromPayload(event, payload)
  if (!subject) {
    return {
      ok: false,
      reasonRef: "reason.connector.github.unbounded_subject",
      blockerRefs: ["blocker.connector.github.unbounded_subject"],
    }
  }

  const action =
    typeof (payload as Record<string, unknown>).action === "string"
      ? ((payload as Record<string, unknown>).action as string)
      : undefined
  const dedupeKey = createConnectorDeliveryDedupeKey({
    provider: "github",
    deliveryId: delivery,
  })

  return {
    ok: true,
    event: {
      schema: CONNECTOR_SIDECAR_SCHEMA_VERSION,
      provider: "github",
      eventKind,
      action,
      deliveryId: delivery,
      dedupeKey,
      receivedAt: input.receivedAt,
      sourceVerified: true,
      subject,
      sourceRefs: [subject.htmlUrl, dedupeKey],
    },
  }
}

export const projectConnectorEventToWorkspaceLane = (
  event: ConnectorSourceVerifiedEvent,
): ConnectorWorkspaceLaneProjection => {
  const owner = safeRefPart(event.subject.owner)
  const repo = safeRefPart(event.subject.repo)
  const subjectKind = safeRefPart(event.subject.kind)
  const laneRef = `workspace_lane.connector.github.${owner}.${repo}.${subjectKind}.${event.subject.number}`

  return {
    schema: CONNECTOR_SIDECAR_SCHEMA_VERSION,
    laneRef,
    eventRef: `${laneRef}.event.${safeRefPart(event.deliveryId)}`,
    provider: event.provider,
    subject: event.subject,
    sourceRefs: event.sourceRefs,
    allowedWritebackToolRefs:
      event.subject.kind === "issue"
        ? ["tool.connector.github.issue.comment.create"]
        : ["tool.connector.github.pull_request.comment.create"],
    blockerRefs: [],
  }
}

const forbiddenAuthorityPattern = /\.(membership|payment|email|settlement|identity)\./
const platformAuthorityPattern =
  /(?:^|[._:-])(workspace|payment|email|membership|settlement|identity)(?:[._:-]|$)/i
const genericProviderToolPattern =
  /^(?:github|tool\.github|tool\.provider\.github|tool\.connector\.github\.(?:api|graphql|rest|generic|raw))(?:\.|$)/i
const privateProviderMaterialPattern =
  /(?:access[_-]?token|authorization|bearer|client[_-]?secret|cookie|gho_[A-Za-z0-9_]+|github[_-]?secret|hookshot|oauth|payload|private[_-]?key|provider[_-]?(credential|payload|secret)|raw[_-]?(body|headers|payload|webhook)|secret|signature256|token|webhook[_-]?(body|headers|payload|secret)|x-hub-signature)/i

const hasPrivateProviderMaterial = (value: unknown): boolean =>
  privateProviderMaterialPattern.test(JSON.stringify(value))

export const decideConnectorDispatchAuthority = (
  input: ConnectorDispatchAuthorityInput,
): ConnectorAuthorityDecision => {
  const { event } = input

  if (event.sourceVerified !== true) {
    return {
      allowed: false,
      status: "denied",
      reasonRef: "reason.connector.source_verification_required",
      blockerRefs: ["blocker.connector.source_verification_required"],
    }
  }

  if (input.appOwnedIdempotencyKey !== event.dedupeKey) {
    return {
      allowed: false,
      status: "denied",
      reasonRef: "reason.connector.app_idempotency_required",
      blockerRefs: ["blocker.connector.app_idempotency_required"],
    }
  }

  const contextEnvelope = {
    logs: input.logItems ?? [],
    modelContext: input.modelContextItems ?? [],
    sessionHistory: input.sessionHistoryItems ?? [],
  }
  if (hasPrivateProviderMaterial(contextEnvelope)) {
    return {
      allowed: false,
      status: "denied",
      reasonRef: "reason.connector.model_context_contains_private_provider_material",
      blockerRefs: [
        "blocker.connector.model_context_contains_private_provider_material",
      ],
    }
  }

  if (
    (input.requestedPlatformAuthorityRefs ?? []).some((authorityRef) =>
      platformAuthorityPattern.test(authorityRef),
    )
  ) {
    return {
      allowed: false,
      status: "denied",
      reasonRef: "reason.connector.platform_authority_forbidden",
      blockerRefs: ["blocker.connector.platform_authority_forbidden"],
    }
  }

  const projection = projectConnectorEventToWorkspaceLane(event)
  for (const toolRef of input.toolRefs) {
    if (
      genericProviderToolPattern.test(toolRef) ||
      forbiddenAuthorityPattern.test(toolRef)
    ) {
      return {
        allowed: false,
        status: "denied",
        reasonRef: genericProviderToolPattern.test(toolRef)
          ? "reason.connector.generic_provider_tool_forbidden"
          : "reason.connector.tool_forbidden_authority",
        blockerRefs: [
          genericProviderToolPattern.test(toolRef)
            ? "blocker.connector.generic_provider_tool_forbidden"
            : "blocker.connector.tool_forbidden_authority",
        ],
      }
    }

    if (!projection.allowedWritebackToolRefs.includes(toolRef)) {
      return {
        allowed: false,
        status: "denied",
        reasonRef: "reason.connector.tool_not_allowed",
        blockerRefs: ["blocker.connector.tool_not_allowed"],
      }
    }
  }

  return {
    allowed: true,
    status: "allowed",
    reasonRef: "reason.connector.dispatch.authority_bound",
    blockerRefs: [],
  }
}

export const decideConnectorWritebackToolAuthority = (input: {
  event: ConnectorSourceVerifiedEvent
  request: ConnectorWritebackRequest
}): ConnectorAuthorityDecision => {
  const { event, request } = input
  if (forbiddenAuthorityPattern.test(request.toolRef)) {
    return {
      allowed: false,
      status: "denied",
      reasonRef: "reason.connector.tool_forbidden_authority",
      blockerRefs: ["blocker.connector.tool_forbidden_authority"],
    }
  }

  const projection = projectConnectorEventToWorkspaceLane(event)
  if (!projection.allowedWritebackToolRefs.includes(request.toolRef)) {
    return {
      allowed: false,
      status: "denied",
      reasonRef: "reason.connector.tool_not_allowed",
      blockerRefs: ["blocker.connector.tool_not_allowed"],
    }
  }

  if (
    request.provider !== event.provider ||
    request.owner !== event.subject.owner ||
    request.repo !== event.subject.repo ||
    request.subjectKind !== event.subject.kind ||
    request.number !== event.subject.number
  ) {
    return {
      allowed: false,
      status: "denied",
      reasonRef: "reason.connector.writeback_subject_mismatch",
      blockerRefs: ["blocker.connector.writeback_subject_mismatch"],
    }
  }

  return {
    allowed: true,
    status: "allowed",
    reasonRef: "reason.connector.writeback.subject_bound",
    blockerRefs: [],
  }
}

export const connectorEventHasRawProviderMaterial = (
  value: ConnectorSourceVerifiedEvent,
): boolean => {
  return hasPrivateProviderMaterial(value)
}
