import { Schema as S } from "effect"

export const AgentRuntimeRunId = S.String
export type AgentRuntimeRunId = typeof AgentRuntimeRunId.Type

export const AgentDefinitionSchemaLiteral = "openagents.agent_definition.v1" as const

export const AgentDefinitionId = S.String
export type AgentDefinitionId = typeof AgentDefinitionId.Type

export const AgentRuntimeEventId = S.String
export type AgentRuntimeEventId = typeof AgentRuntimeEventId.Type

export const AgentRuntimeAdapterKind = S.Literals([
  "openagents_native",
  "claude_code",
  "codex",
  "opencode",
  "hermes",
  "hosted_container",
  "shc",
  "test_fixture",
])
export type AgentRuntimeAdapterKind = typeof AgentRuntimeAdapterKind.Type

export const agentRuntimeAdapterKinds: ReadonlyArray<AgentRuntimeAdapterKind> = [
  "openagents_native",
  "claude_code",
  "codex",
  "opencode",
  "hermes",
  "hosted_container",
  "shc",
  "test_fixture",
]

export const AgentDefinitionHarnessKind = S.Literals([
  "codex",
  "claude_code",
  "khala",
  "opencode",
  "hermes",
  "openagents_native",
  "hosted_container",
  "custom",
  "test_fixture",
])
export type AgentDefinitionHarnessKind = typeof AgentDefinitionHarnessKind.Type

export const agentDefinitionHarnessKinds: ReadonlyArray<AgentDefinitionHarnessKind> = [
  "codex",
  "claude_code",
  "khala",
  "opencode",
  "hermes",
  "openagents_native",
  "hosted_container",
  "custom",
  "test_fixture",
]

export const AgentRuntimeLoopKind = S.Literals([
  "native_model_loop",
  "external_agent_loop",
  "hosted_loop",
  "fixture_loop",
])
export type AgentRuntimeLoopKind = typeof AgentRuntimeLoopKind.Type

export const agentRuntimeLoopKinds: ReadonlyArray<AgentRuntimeLoopKind> = [
  "native_model_loop",
  "external_agent_loop",
  "hosted_loop",
  "fixture_loop",
]

export const AgentRuntimeVisibility = S.Literals(["public", "operator", "private"])
export type AgentRuntimeVisibility = typeof AgentRuntimeVisibility.Type

export const agentRuntimeVisibilities: ReadonlyArray<AgentRuntimeVisibility> = [
  "public",
  "operator",
  "private",
]

export const AgentRuntimeRedactionClass = S.Literals([
  "public_ref",
  "redacted_summary",
  "operator_summary",
  "private_ref",
])
export type AgentRuntimeRedactionClass = typeof AgentRuntimeRedactionClass.Type

export const agentRuntimeRedactionClasses: ReadonlyArray<AgentRuntimeRedactionClass> = [
  "public_ref",
  "redacted_summary",
  "operator_summary",
  "private_ref",
]

export const AgentRuntimeRedactionPolicy = S.Struct({
  policyRef: S.String,
  rawPromptAllowed: S.Boolean,
  rawShellLogAllowed: S.Boolean,
  providerPayloadAllowed: S.Boolean,
  localPathAllowed: S.Boolean,
  secretMaterialAllowed: S.Boolean,
})
export type AgentRuntimeRedactionPolicy = typeof AgentRuntimeRedactionPolicy.Type

export const AgentDefinitionNetworkPolicy = S.Literals([
  "none",
  "owner_scoped",
  "public_internet",
])
export type AgentDefinitionNetworkPolicy = typeof AgentDefinitionNetworkPolicy.Type

export const AgentDefinitionSecretPolicy = S.Literals([
  "none",
  "owner_scoped_refs_only",
])
export type AgentDefinitionSecretPolicy = typeof AgentDefinitionSecretPolicy.Type

export const AgentDefinitionToolRef = S.String
export type AgentDefinitionToolRef = typeof AgentDefinitionToolRef.Type

export const AgentDefinitionToolset = S.Struct({
  allow: S.Array(AgentDefinitionToolRef),
  deny: S.Array(AgentDefinitionToolRef),
  ask: S.Array(AgentDefinitionToolRef),
  networkPolicy: AgentDefinitionNetworkPolicy,
  secretPolicy: AgentDefinitionSecretPolicy,
})
export type AgentDefinitionToolset = typeof AgentDefinitionToolset.Type

export const AgentDefinitionHarness = S.Struct({
  kind: AgentDefinitionHarnessKind,
  modelHint: S.optional(S.String),
  versionPin: S.optional(S.String),
})
export type AgentDefinitionHarness = typeof AgentDefinitionHarness.Type

export const AgentDefinitionInboundWebhookCondition = S.Union([
  S.Struct({
    kind: S.Literal("event_type"),
    equals: S.String,
  }),
  S.Struct({
    kind: S.Literal("json_path_equals"),
    path: S.String,
    equals: S.String,
  }),
  S.Struct({
    kind: S.Literal("json_path_matches"),
    path: S.String,
    pattern: S.String,
  }),
  S.Struct({
    kind: S.Literal("json_path_in"),
    path: S.String,
    values: S.Array(S.String),
  }),
])
export type AgentDefinitionInboundWebhookCondition =
  typeof AgentDefinitionInboundWebhookCondition.Type

export const AgentDefinitionTrigger = S.Union([
  S.Struct({
    kind: S.Literal("cron"),
    triggerRef: S.String,
    expr: S.String,
    tz: S.String,
  }),
  S.Struct({
    kind: S.Literal("inbound_webhook"),
    triggerRef: S.String,
    source: S.String,
    conditions: S.Array(AgentDefinitionInboundWebhookCondition),
  }),
  S.Struct({
    kind: S.Literal("inbox_match"),
    triggerRef: S.String,
    classifierRef: S.String,
  }),
  S.Struct({
    kind: S.Literal("manual"),
    triggerRef: S.String,
  }),
])
export type AgentDefinitionTrigger = typeof AgentDefinitionTrigger.Type

export const AgentDefinitionTriggerRecordSchemaLiteral =
  "openagents.agent_definition_trigger.v1" as const

export const AgentDefinitionTriggerState = S.Literals(["enabled", "paused"])
export type AgentDefinitionTriggerState = typeof AgentDefinitionTriggerState.Type

export const AgentDefinitionTriggerRecord = S.Struct({
  schema: S.Literal(AgentDefinitionTriggerRecordSchemaLiteral),
  triggerId: S.String,
  ownerRef: S.String,
  definitionId: AgentDefinitionId,
  triggerRef: S.String,
  trigger: AgentDefinitionTrigger,
  state: AgentDefinitionTriggerState,
  consecutiveFailures: S.Number,
  nextRunAt: S.optional(S.String),
  pausedAt: S.optional(S.String),
  pauseReason: S.optional(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type AgentDefinitionTriggerRecord =
  typeof AgentDefinitionTriggerRecord.Type

export const AgentDefinitionLane = S.Literals([
  "own_pylon",
  "cloud_workroom",
  "worker_only",
  "test_fixture",
])
export type AgentDefinitionLane = typeof AgentDefinitionLane.Type

export const AgentDefinitionBudget = S.Struct({
  maxRunSeconds: S.Number,
  maxRunsPerDay: S.Number,
  maxCreditsPerDay: S.optional(S.Number),
})
export type AgentDefinitionBudget = typeof AgentDefinitionBudget.Type

export const AgentDefinitionEscalationChannel = S.Literals([
  "operator",
  "forum",
  "push",
  "email",
])
export type AgentDefinitionEscalationChannel =
  typeof AgentDefinitionEscalationChannel.Type

export const AgentDefinitionAskPolicy = S.Struct({
  policyRef: S.String,
  mode: S.Literals(["operator_required", "deny_when_unavailable"]),
})
export type AgentDefinitionAskPolicy = typeof AgentDefinitionAskPolicy.Type

export const AgentDefinitionEscalation = S.Struct({
  channel: AgentDefinitionEscalationChannel,
  askPolicy: AgentDefinitionAskPolicy,
})
export type AgentDefinitionEscalation = typeof AgentDefinitionEscalation.Type

export const AgentDefinition = S.Struct({
  schema: S.Literal(AgentDefinitionSchemaLiteral),
  id: AgentDefinitionId,
  ownerRef: S.String,
  name: S.String,
  slug: S.String,
  goal: S.String,
  harness: AgentDefinitionHarness,
  toolset: AgentDefinitionToolset,
  triggers: S.Array(AgentDefinitionTrigger),
  lane: AgentDefinitionLane,
  budget: AgentDefinitionBudget,
  escalation: AgentDefinitionEscalation,
  sourceRefs: S.Array(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type AgentDefinition = typeof AgentDefinition.Type

export const AgentDefinitionToolAuthorityStatus = S.Literals([
  "allowed",
  "denied",
  "operator_escalation_required",
])
export type AgentDefinitionToolAuthorityStatus =
  typeof AgentDefinitionToolAuthorityStatus.Type

export type AgentDefinitionOperatorEscalation = {
  readonly escalationRef: string
  readonly definitionId: AgentDefinitionId
  readonly ownerRef: string
  readonly toolRef: AgentDefinitionToolRef
  readonly channel: AgentDefinitionEscalationChannel
  readonly askPolicyRef: string
  readonly reasonRef: string
}

export type AgentDefinitionToolAuthorityDecision = {
  readonly status: AgentDefinitionToolAuthorityStatus
  readonly allowed: boolean
  readonly toolRef: AgentDefinitionToolRef
  readonly definitionId: AgentDefinitionId
  readonly reasonRef: string
  readonly matchedPolicyRef?: string
  readonly blockerRefs: ReadonlyArray<string>
  readonly escalation?: AgentDefinitionOperatorEscalation
}

export const AgentDefinitionToolRuntimePolicySchemaLiteral =
  "openagents.agent_definition_tool_runtime_policy.v1" as const

export type AgentDefinitionCompiledToolRuntimePolicy = {
  readonly schema: typeof AgentDefinitionToolRuntimePolicySchemaLiteral
  readonly definitionId: AgentDefinitionId
  readonly ownerRef: string
  readonly allow: ReadonlyArray<AgentDefinitionToolRef>
  readonly ask: ReadonlyArray<AgentDefinitionToolRef>
  readonly deny: ReadonlyArray<AgentDefinitionToolRef>
  readonly networkPolicy: AgentDefinitionNetworkPolicy
  readonly secretPolicy: AgentDefinitionSecretPolicy
  readonly escalation: {
    readonly channel: AgentDefinitionEscalationChannel
    readonly askPolicyRef: string
  }
  readonly defaultDecision: "deny"
}

export const AgentRuntimeRunState = S.Literals([
  "pending",
  "running",
  "paused",
  "interrupted",
  "cancelled",
  "completed",
  "failed",
])
export type AgentRuntimeRunState = typeof AgentRuntimeRunState.Type

export const AgentRuntimeSafeRef = S.String
export type AgentRuntimeSafeRef = typeof AgentRuntimeSafeRef.Type

export const AgentRuntimePart = S.Union([
  S.Struct({
    kind: S.Literal("text"),
    text: S.String,
  }),
  S.Struct({
    kind: S.Literal("reasoning"),
    summary: S.String,
  }),
  S.Struct({
    kind: S.Literal("ref"),
    ref: AgentRuntimeSafeRef,
    label: S.optional(S.String),
  }),
])
export type AgentRuntimePart = typeof AgentRuntimePart.Type

export const AgentRuntimeToolInvocation = S.Struct({
  invocationId: S.String,
  toolName: S.String,
  toolRef: S.String,
  inputRef: S.optional(S.String),
  outputRef: S.optional(S.String),
  status: S.Literals(["proposed", "approval_requested", "approved", "denied", "started", "completed", "failed"]),
  summary: S.optional(S.String),
  blockerRefs: S.Array(S.String),
})
export type AgentRuntimeToolInvocation = typeof AgentRuntimeToolInvocation.Type

export const AgentRuntimeExternalInvocation = S.Struct({
  invocationId: S.String,
  adapterKind: AgentRuntimeAdapterKind,
  sessionRef: S.optional(S.String),
  status: S.Literals(["started", "event", "artifact_recorded", "completed", "failed"]),
  summary: S.optional(S.String),
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
})
export type AgentRuntimeExternalInvocation = typeof AgentRuntimeExternalInvocation.Type

export const AgentRuntimeArtifactRef = S.Struct({
  artifactRef: S.String,
  artifactKind: S.String,
  visibility: AgentRuntimeVisibility,
  digestRef: S.optional(S.String),
  summary: S.optional(S.String),
})
export type AgentRuntimeArtifactRef = typeof AgentRuntimeArtifactRef.Type

export const AgentRuntimeUsageRecord = S.Struct({
  usageRef: S.String,
  providerRef: S.optional(S.String),
  modelRef: S.optional(S.String),
  inputTokens: S.optional(S.Number),
  outputTokens: S.optional(S.Number),
  totalTokens: S.optional(S.Number),
  costRef: S.optional(S.String),
})
export type AgentRuntimeUsageRecord = typeof AgentRuntimeUsageRecord.Type

export const AgentRuntimeEventTag = S.Literals([
  "run.started",
  "run.input_accepted",
  "context.snapshot_created",
  "step.started",
  "model.stream_started",
  "model.text_delta",
  "model.text_completed",
  "model.reasoning_delta",
  "model.reasoning_completed",
  "tool.call_proposed",
  "tool.input_delta",
  "tool.input_completed",
  "tool.approval_requested",
  "tool.approved",
  "tool.denied",
  "tool.started",
  "tool.completed",
  "tool.failed",
  "external_agent.started",
  "external_agent.event",
  "external_agent.artifact_recorded",
  "external_agent.completed",
  "external_agent.failed",
  "artifact.recorded",
  "usage.recorded",
  "step.completed",
  "step.failed",
  "run.paused",
  "run.interrupted",
  "run.cancelled",
  "run.completed",
  "run.failed",
])
export type AgentRuntimeEventTag = typeof AgentRuntimeEventTag.Type

export const agentRuntimeEventTags: ReadonlyArray<AgentRuntimeEventTag> = [
  "run.started",
  "run.input_accepted",
  "context.snapshot_created",
  "step.started",
  "model.stream_started",
  "model.text_delta",
  "model.text_completed",
  "model.reasoning_delta",
  "model.reasoning_completed",
  "tool.call_proposed",
  "tool.input_delta",
  "tool.input_completed",
  "tool.approval_requested",
  "tool.approved",
  "tool.denied",
  "tool.started",
  "tool.completed",
  "tool.failed",
  "external_agent.started",
  "external_agent.event",
  "external_agent.artifact_recorded",
  "external_agent.completed",
  "external_agent.failed",
  "artifact.recorded",
  "usage.recorded",
  "step.completed",
  "step.failed",
  "run.paused",
  "run.interrupted",
  "run.cancelled",
  "run.completed",
  "run.failed",
]

export const AgentRuntimeEvent = S.Struct({
  tag: AgentRuntimeEventTag,
  eventId: AgentRuntimeEventId,
  runId: AgentRuntimeRunId,
  sequence: S.Number,
  generatedAt: S.String,
  visibility: AgentRuntimeVisibility,
  redactionClass: AgentRuntimeRedactionClass,
  stepRef: S.optional(S.String),
  part: S.optional(AgentRuntimePart),
  toolInvocation: S.optional(AgentRuntimeToolInvocation),
  externalInvocation: S.optional(AgentRuntimeExternalInvocation),
  artifact: S.optional(AgentRuntimeArtifactRef),
  usage: S.optional(AgentRuntimeUsageRecord),
  summary: S.optional(S.String),
  refs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
})
export type AgentRuntimeEvent = typeof AgentRuntimeEvent.Type

export const AgentRuntimeRun = S.Struct({
  runId: AgentRuntimeRunId,
  agentDefinitionId: S.optional(AgentDefinitionId),
  assignmentId: S.optional(S.String),
  workOrderId: S.optional(S.String),
  workspaceRef: S.String,
  adapterKind: AgentRuntimeAdapterKind,
  loopKind: AgentRuntimeLoopKind,
  sourceRefs: S.Array(S.String),
  budgetRef: S.optional(S.String),
  usagePolicy: S.optional(S.String),
  permissionPolicy: S.String,
  redactionPolicy: AgentRuntimeRedactionPolicy,
  visibility: AgentRuntimeVisibility,
  publicProjectionAllowed: S.Boolean,
  state: AgentRuntimeRunState,
  createdAt: S.String,
  updatedAt: S.String,
  adapterSessionRefs: S.Array(S.String),
})
export type AgentRuntimeRun = typeof AgentRuntimeRun.Type

export const AgentRuntimeEventLog = S.Struct({
  run: AgentRuntimeRun,
  events: S.Array(AgentRuntimeEvent),
})
export type AgentRuntimeEventLog = typeof AgentRuntimeEventLog.Type

export type AgentRuntimeSurfaceProjection = {
  readonly runId: AgentRuntimeRunId
  readonly state: Exclude<AgentRuntimeRunState, "pending">
  readonly generatedAt: string
  readonly eventCount: number
  readonly artifactRefs: ReadonlyArray<string>
  readonly blockerRefs: ReadonlyArray<string>
  readonly latestEventId?: string
  readonly staleness?: {
    readonly maxStalenessSeconds?: number
    readonly rebuildsOn?: ReadonlyArray<string>
    readonly transitionRefs?: ReadonlyArray<string>
  }
}

export type AgentRuntimeSurfaceStatus = "running" | "attention" | "completed" | "failed" | "cancelled"

export type AgentRuntimeSurfaceStatusRow = {
  readonly runId: AgentRuntimeRunId
  readonly status: AgentRuntimeSurfaceStatus
  readonly label: string
  readonly generatedAt: string
  readonly eventCount: number
  readonly artifactRefs: ReadonlyArray<string>
  readonly blockerRefs: ReadonlyArray<string>
  readonly freshness: {
    readonly generatedAt: string
    readonly maxStalenessSeconds?: number
    readonly transitionRefs: ReadonlyArray<string>
  }
  readonly verificationRefs: ReadonlyArray<string>
  readonly reviewActionRefs: ReadonlyArray<string>
}

export const decodeAgentRuntimeRun = S.decodeUnknownSync(AgentRuntimeRun)
export const decodeAgentRuntimeEvent = S.decodeUnknownSync(AgentRuntimeEvent)
export const decodeAgentRuntimeEventLog = S.decodeUnknownSync(AgentRuntimeEventLog)
export const decodeAgentDefinition = S.decodeUnknownSync(AgentDefinition)
export const decodeAgentDefinitionTriggerRecord =
  S.decodeUnknownSync(AgentDefinitionTriggerRecord)

export const PylonAssignmentRunLifecycleEventSchemaLiteral =
  "openagents.pylon.assignment_run_lifecycle_event.v0.1" as const

export const PylonKhalaSpawnWorkerEventSchemaLiteral =
  "openagents.pylon.khala_spawn_worker_event.v0.1" as const

export const PylonAssignmentStatus = S.Literals([
  "offered",
  "accepted",
  "running",
  "closed",
  "rejected",
  "cancelled",
  "timed-out",
  "stale",
])
export type PylonAssignmentStatus = typeof PylonAssignmentStatus.Type

export const PylonAssignmentProgressStatus = S.Literals([
  "accepted",
  "running",
  "artifact-ready",
  "proof-ready",
  "closeout-submitted",
])
export type PylonAssignmentProgressStatus = typeof PylonAssignmentProgressStatus.Type

export const PylonCodexAgentRuntimePhase = S.String
export type PylonCodexAgentRuntimePhase = typeof PylonCodexAgentRuntimePhase.Type

export const PylonAssignmentRunLifecycleEventName = S.Literals([
  "assignment_run.poll_complete",
  "assignment_run.accepted",
  "assignment_run.runtime_started",
  "assignment_run.runtime_progress",
  "assignment_run.runtime_failed",
  "assignment_run.progress_submitted",
  "assignment_run.artifacts_submitted",
  "assignment_run.closeout_submitted",
  "assignment_run.completed",
  "assignment_run.no_assignment",
])
export type PylonAssignmentRunLifecycleEventName = typeof PylonAssignmentRunLifecycleEventName.Type

export const PylonAssignmentRunLifecycleEvent = S.Struct({
  schema: S.Literal(PylonAssignmentRunLifecycleEventSchemaLiteral),
  event: PylonAssignmentRunLifecycleEventName,
  observedAt: S.String,
  assignmentRef: S.optional(S.String),
  leaseRef: S.optional(S.String),
  leaseCount: S.optional(S.Number),
  candidateCount: S.optional(S.Number),
  status: S.optional(S.Union([PylonAssignmentStatus, PylonAssignmentProgressStatus])),
  statusRef: S.optional(S.String),
  progressRef: S.optional(S.String),
  artifactRef: S.optional(S.String),
  closeoutRef: S.optional(S.String),
  accountRefHash: S.optional(S.String),
  elapsedMs: S.optional(S.Number),
  phase: S.optional(S.Union([S.Literal("runtime_active"), PylonCodexAgentRuntimePhase])),
  tokensSoFar: S.optional(S.Number),
  tokenCountKind: S.optional(S.Literals(["exact", "estimated"])),
  lastProgressEvent: S.optional(S.Union([PylonAssignmentRunLifecycleEventName, S.String])),
  blockerRefs: S.optional(S.Array(S.String)),
})
export type PylonAssignmentRunLifecycleEvent = typeof PylonAssignmentRunLifecycleEvent.Type

export const PylonKhalaSpawnWorkerState = S.Literals([
  "queued",
  "requesting",
  "assignment_created",
  "running",
  "closeout_submitted",
  "proof_checked",
  "accepted",
  "rejected",
  "failed",
  "cancelled",
])
export type PylonKhalaSpawnWorkerState = typeof PylonKhalaSpawnWorkerState.Type

export const PylonKhalaSpawnWorkerEvent = S.Struct({
  schema: S.Literal(PylonKhalaSpawnWorkerEventSchemaLiteral),
  assignmentEvent: S.optional(PylonAssignmentRunLifecycleEventName),
  assignmentRef: S.optional(S.String),
  closeoutRef: S.optional(S.String),
  leaseRef: S.optional(S.String),
  message: S.String,
  observedAt: S.String,
  slotIndex: S.Number,
  state: PylonKhalaSpawnWorkerState,
  status: S.optional(S.String),
})
export type PylonKhalaSpawnWorkerEvent = typeof PylonKhalaSpawnWorkerEvent.Type

export const PylonLifecycleWireEvent = S.Union([
  PylonAssignmentRunLifecycleEvent,
  PylonKhalaSpawnWorkerEvent,
])
export type PylonLifecycleWireEvent = typeof PylonLifecycleWireEvent.Type

export const PylonLifecycleWireEventFromJsonString = S.fromJsonString(PylonLifecycleWireEvent)

export const decodePylonAssignmentRunLifecycleEvent =
  S.decodeUnknownSync(PylonAssignmentRunLifecycleEvent)
export const encodePylonAssignmentRunLifecycleEvent =
  S.encodeUnknownSync(PylonAssignmentRunLifecycleEvent)
export const decodePylonKhalaSpawnWorkerEvent = S.decodeUnknownSync(PylonKhalaSpawnWorkerEvent)
export const encodePylonKhalaSpawnWorkerEvent = S.encodeUnknownSync(PylonKhalaSpawnWorkerEvent)
export const decodePylonLifecycleWireEvent = S.decodeUnknownSync(PylonLifecycleWireEvent)
export const decodePylonLifecycleWireEventJson =
  S.decodeUnknownSync(PylonLifecycleWireEventFromJsonString)

const unsafePublicMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|local[_-]?path|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|email|invoice|log|payment|payload|prompt|provider|record|repo|runner|run[_-]?log|shell|source|state|target|text|trace|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i

export function agentRuntimePublicEventHasUnsafeMaterial(event: AgentRuntimeEvent): boolean {
  return event.visibility === "public" && unsafePublicMaterialPattern.test(JSON.stringify(event))
}

export function assertAgentRuntimePublicEventSafe(event: AgentRuntimeEvent): AgentRuntimeEvent {
  if (agentRuntimePublicEventHasUnsafeMaterial(event)) {
    throw new Error("Agent runtime public event contains raw/private material")
  }
  return event
}

export function assertAgentRuntimeEventLogSafe(log: AgentRuntimeEventLog): AgentRuntimeEventLog {
  for (const event of log.events) {
    assertAgentRuntimePublicEventSafe(event)
  }
  return log
}

export function projectAgentRuntimeSurfaceStatus(
  projection: AgentRuntimeSurfaceProjection,
): AgentRuntimeSurfaceStatusRow {
  const transitionRefs = projection.staleness?.transitionRefs ?? projection.staleness?.rebuildsOn ?? []
  const status: AgentRuntimeSurfaceStatus =
    projection.state === "completed"
      ? "completed"
      : projection.state === "cancelled"
        ? "cancelled"
        : projection.state === "failed"
          ? "failed"
          : projection.state === "paused" || projection.state === "interrupted"
            ? "attention"
            : "running"

  const label =
    status === "completed"
      ? "Completed"
      : status === "cancelled"
        ? "Cancelled"
        : status === "failed"
          ? "Failed"
          : status === "attention"
            ? "Needs attention"
            : "Running"

  return {
    runId: projection.runId,
    status,
    label,
    generatedAt: projection.generatedAt,
    eventCount: projection.eventCount,
    artifactRefs: [...projection.artifactRefs],
    blockerRefs: [...projection.blockerRefs],
    freshness: {
      generatedAt: projection.generatedAt,
      ...(projection.staleness?.maxStalenessSeconds === undefined
        ? {}
        : { maxStalenessSeconds: projection.staleness.maxStalenessSeconds }),
      transitionRefs: [...transitionRefs],
    },
    verificationRefs: projection.artifactRefs.filter(ref =>
      /^(artifact|proof|result|test)\.public\./.test(ref),
    ),
    reviewActionRefs: projection.blockerRefs.map(ref => `review.public.agent_runtime.${ref}`),
  }
}

export function agentRuntimeSurfaceStatusHasUnsafeMaterial(row: AgentRuntimeSurfaceStatusRow): boolean {
  return unsafePublicMaterialPattern.test(JSON.stringify(row))
}

export function decideAgentDefinitionToolAuthority(input: {
  readonly definition: AgentDefinition
  readonly toolRef: AgentDefinitionToolRef
  readonly invocationRef?: string
}): AgentDefinitionToolAuthorityDecision {
  return decideAgentDefinitionCompiledToolAuthority({
    policy: compileAgentDefinitionToolRuntimePolicy(input.definition),
    toolRef: input.toolRef,
    ...(input.invocationRef === undefined ? {} : { invocationRef: input.invocationRef }),
  })
}

export function compileAgentDefinitionToolRuntimePolicy(
  definition: AgentDefinition,
): AgentDefinitionCompiledToolRuntimePolicy {
  return {
    schema: AgentDefinitionToolRuntimePolicySchemaLiteral,
    definitionId: definition.id,
    ownerRef: definition.ownerRef,
    allow: [...definition.toolset.allow],
    ask: [...definition.toolset.ask],
    deny: [...definition.toolset.deny],
    networkPolicy: definition.toolset.networkPolicy,
    secretPolicy: definition.toolset.secretPolicy,
    escalation: {
      channel: definition.escalation.channel,
      askPolicyRef: definition.escalation.askPolicy.policyRef,
    },
    defaultDecision: "deny",
  }
}

export function decideAgentDefinitionCompiledToolAuthority(input: {
  readonly policy: AgentDefinitionCompiledToolRuntimePolicy
  readonly toolRef: AgentDefinitionToolRef
  readonly invocationRef?: string
}): AgentDefinitionToolAuthorityDecision {
  const toolRef = input.toolRef
  const policy = input.policy

  const deniedBy = firstMatchingToolPolicy(policy.deny, toolRef)
  if (deniedBy !== undefined) {
    return {
      status: "denied",
      allowed: false,
      toolRef,
      definitionId: policy.definitionId,
      matchedPolicyRef: deniedBy,
      reasonRef: "reason.agent_definition.tool_denied",
      blockerRefs: ["blocker.agent_definition.tool_denied"],
    }
  }

  const askBy = firstMatchingToolPolicy(policy.ask, toolRef)
  if (askBy !== undefined) {
    const escalationRef = stableAgentDefinitionRef("escalation.operator.agent_definition", [
      policy.definitionId,
      input.invocationRef ?? toolRef,
      askBy,
    ])
    return {
      status: "operator_escalation_required",
      allowed: false,
      toolRef,
      definitionId: policy.definitionId,
      matchedPolicyRef: askBy,
      reasonRef: "reason.agent_definition.tool_requires_operator",
      blockerRefs: ["blocker.agent_definition.operator_escalation_required"],
      escalation: {
        escalationRef,
        definitionId: policy.definitionId,
        ownerRef: policy.ownerRef,
        toolRef,
        channel: policy.escalation.channel,
        askPolicyRef: policy.escalation.askPolicyRef,
        reasonRef: "reason.agent_definition.ask_policy_hit",
      },
    }
  }

  const allowedBy = firstMatchingToolPolicy(policy.allow, toolRef)
  if (allowedBy !== undefined) {
    return {
      status: "allowed",
      allowed: true,
      toolRef,
      definitionId: policy.definitionId,
      matchedPolicyRef: allowedBy,
      reasonRef: "reason.agent_definition.tool_allowed",
      blockerRefs: [],
    }
  }

  return {
    status: "denied",
    allowed: false,
    toolRef,
    definitionId: policy.definitionId,
    reasonRef: "reason.agent_definition.tool_not_in_allowlist",
    blockerRefs: ["blocker.agent_definition.tool_not_in_allowlist"],
  }
}

function firstMatchingToolPolicy(
  policyRefs: ReadonlyArray<AgentDefinitionToolRef>,
  toolRef: AgentDefinitionToolRef,
): AgentDefinitionToolRef | undefined {
  return policyRefs.find((policyRef) => toolRefMatchesPolicyRef(policyRef, toolRef))
}

function toolRefMatchesPolicyRef(
  policyRef: AgentDefinitionToolRef,
  toolRef: AgentDefinitionToolRef,
): boolean {
  if (policyRef === toolRef) return true
  if (!policyRef.endsWith(".*")) return false
  const prefix = policyRef.slice(0, -1)
  return toolRef.startsWith(prefix)
}

function stableAgentDefinitionRef(prefix: string, parts: ReadonlyArray<string>): string {
  let hash = 2166136261
  for (const char of parts.join("\u001f")) {
    hash ^= char.charCodeAt(0)
    hash = Math.imul(hash, 16777619)
  }
  return `${prefix}.${(hash >>> 0).toString(16).padStart(8, "0")}`
}

const terminalRunStates: ReadonlySet<AgentRuntimeRunState> = new Set([
  "cancelled",
  "completed",
  "failed",
])

const legalRunStateTransitions: ReadonlyMap<AgentRuntimeRunState, ReadonlySet<AgentRuntimeRunState>> = new Map([
  ["pending", new Set(["running", "cancelled", "failed"])],
  ["running", new Set(["paused", "interrupted", "cancelled", "completed", "failed"])],
  ["paused", new Set(["running", "cancelled", "failed"])],
  ["interrupted", new Set(["running", "cancelled", "failed"])],
  ["cancelled", new Set()],
  ["completed", new Set()],
  ["failed", new Set()],
])

export function agentRuntimeRunStateTransitionIsLegal(
  from: AgentRuntimeRunState,
  to: AgentRuntimeRunState,
): boolean {
  if (from === to && !terminalRunStates.has(from)) {
    return true
  }
  return legalRunStateTransitions.get(from)?.has(to) ?? false
}

export function assertAgentRuntimeRunStateTransition(
  from: AgentRuntimeRunState,
  to: AgentRuntimeRunState,
): AgentRuntimeRunState {
  if (!agentRuntimeRunStateTransitionIsLegal(from, to)) {
    throw new Error(`Illegal AgentRuntimeRun state transition: ${from} -> ${to}`)
  }
  return to
}
