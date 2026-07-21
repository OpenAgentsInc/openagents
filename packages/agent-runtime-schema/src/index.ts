import { Schema as S } from "effect";

export * from "./live-agent-graph.js";
export * from "./live-agent-graph-adapters.js";
export * from "./thread-control.js";
export * from "./thread-disclosure.js";
export * from "./thread-event-authority.js";
export * from "./thread-event-search.js";
export * from "./thread-export-artifact.js";

// AISDK-05 (#9151) public neutral model-call failure vocabulary.
export * from "./model-failure.js";

// AFS-00 frozen agent turn/route/provider/context/artifact/presentation contracts.
export * from "./turn.js";
export * from "./provider.js";
export * from "./context.js";
export * from "./artifact.js";
export * from "./route.js";
export * from "./presentation.js";

export const AgentRuntimeRunId = S.String;
export type AgentRuntimeRunId = typeof AgentRuntimeRunId.Type;

export const AgentDefinitionSchemaLiteral = "openagents.agent_definition.v1" as const;

export const AgentDefinitionId = S.String;
export type AgentDefinitionId = typeof AgentDefinitionId.Type;

export const AgentRuntimeEventId = S.String;
export type AgentRuntimeEventId = typeof AgentRuntimeEventId.Type;

export const AgentRuntimeAdapterKind = S.Literals([
  "openagents_native",
  "claude_code",
  "codex",
  "grok_cli",
  "cursor_cli",
  "agent_client_protocol",
  "opencode",
  "hermes",
  "hosted_container",
  "gcp",
  "test_fixture",
]);
export type AgentRuntimeAdapterKind = typeof AgentRuntimeAdapterKind.Type;

export const agentRuntimeAdapterKinds: ReadonlyArray<AgentRuntimeAdapterKind> = [
  "openagents_native",
  "claude_code",
  "codex",
  "grok_cli",
  "cursor_cli",
  "agent_client_protocol",
  "opencode",
  "hermes",
  "hosted_container",
  "gcp",
  "test_fixture",
];

export const AgentDefinitionHarnessKind = S.Literals([
  "codex",
  "claude_code",
  "grok_cli",
  "khala",
  "opencode",
  "hermes",
  "openagents_native",
  "hosted_container",
  "custom",
  "test_fixture",
]);
export type AgentDefinitionHarnessKind = typeof AgentDefinitionHarnessKind.Type;

export const agentDefinitionHarnessKinds: ReadonlyArray<AgentDefinitionHarnessKind> = [
  "codex",
  "claude_code",
  "grok_cli",
  "khala",
  "opencode",
  "hermes",
  "openagents_native",
  "hosted_container",
  "custom",
  "test_fixture",
];

export const AgentRuntimeLoopKind = S.Literals([
  "native_model_loop",
  "external_agent_loop",
  "hosted_loop",
  "fixture_loop",
]);
export type AgentRuntimeLoopKind = typeof AgentRuntimeLoopKind.Type;

export const agentRuntimeLoopKinds: ReadonlyArray<AgentRuntimeLoopKind> = [
  "native_model_loop",
  "external_agent_loop",
  "hosted_loop",
  "fixture_loop",
];

export const AgentRuntimeVisibility = S.Literals(["public", "operator", "private"]);
export type AgentRuntimeVisibility = typeof AgentRuntimeVisibility.Type;

export const agentRuntimeVisibilities: ReadonlyArray<AgentRuntimeVisibility> = [
  "public",
  "operator",
  "private",
];

export const AgentRuntimeRedactionClass = S.Literals([
  "public_ref",
  "redacted_summary",
  "operator_summary",
  "private_ref",
]);
export type AgentRuntimeRedactionClass = typeof AgentRuntimeRedactionClass.Type;

export const agentRuntimeRedactionClasses: ReadonlyArray<AgentRuntimeRedactionClass> = [
  "public_ref",
  "redacted_summary",
  "operator_summary",
  "private_ref",
];

export const AgentRuntimeRedactionPolicy = S.Struct({
  policyRef: S.String,
  rawPromptAllowed: S.Boolean,
  rawShellLogAllowed: S.Boolean,
  providerPayloadAllowed: S.Boolean,
  localPathAllowed: S.Boolean,
  secretMaterialAllowed: S.Boolean,
});
export type AgentRuntimeRedactionPolicy = typeof AgentRuntimeRedactionPolicy.Type;

export const AgentDefinitionNetworkPolicy = S.Literals(["none", "owner_scoped", "public_internet"]);
export type AgentDefinitionNetworkPolicy = typeof AgentDefinitionNetworkPolicy.Type;

export const AgentDefinitionSecretPolicy = S.Literals(["none", "owner_scoped_refs_only"]);
export type AgentDefinitionSecretPolicy = typeof AgentDefinitionSecretPolicy.Type;

export const AgentDefinitionToolRef = S.String;
export type AgentDefinitionToolRef = typeof AgentDefinitionToolRef.Type;

export const AgentDefinitionToolset = S.Struct({
  allow: S.Array(AgentDefinitionToolRef),
  deny: S.Array(AgentDefinitionToolRef),
  ask: S.Array(AgentDefinitionToolRef),
  networkPolicy: AgentDefinitionNetworkPolicy,
  secretPolicy: AgentDefinitionSecretPolicy,
});
export type AgentDefinitionToolset = typeof AgentDefinitionToolset.Type;

export const AgentDefinitionHarness = S.Struct({
  kind: AgentDefinitionHarnessKind,
  modelHint: S.optional(S.String),
  versionPin: S.optional(S.String),
});
export type AgentDefinitionHarness = typeof AgentDefinitionHarness.Type;

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
]);
export type AgentDefinitionInboundWebhookCondition =
  typeof AgentDefinitionInboundWebhookCondition.Type;

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
]);
export type AgentDefinitionTrigger = typeof AgentDefinitionTrigger.Type;

export const AgentDefinitionTriggerRecordSchemaLiteral =
  "openagents.agent_definition_trigger.v1" as const;

export const AgentDefinitionTriggerState = S.Literals(["enabled", "paused"]);
export type AgentDefinitionTriggerState = typeof AgentDefinitionTriggerState.Type;

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
});
export type AgentDefinitionTriggerRecord = typeof AgentDefinitionTriggerRecord.Type;

export const AgentDefinitionLane = S.Literals([
  "own_pylon",
  "cloud_workroom",
  "worker_only",
  "test_fixture",
]);
export type AgentDefinitionLane = typeof AgentDefinitionLane.Type;

export const AgentDefinitionBudget = S.Struct({
  maxRunSeconds: S.Number,
  maxRunsPerDay: S.Number,
  maxCreditsPerDay: S.optional(S.Number),
});
export type AgentDefinitionBudget = typeof AgentDefinitionBudget.Type;

export const AgentDefinitionEscalationChannel = S.Literals(["operator", "forum", "push", "email"]);
export type AgentDefinitionEscalationChannel = typeof AgentDefinitionEscalationChannel.Type;

export const AgentDefinitionAskPolicy = S.Struct({
  policyRef: S.String,
  mode: S.Literals(["operator_required", "deny_when_unavailable"]),
});
export type AgentDefinitionAskPolicy = typeof AgentDefinitionAskPolicy.Type;

export const AgentDefinitionEscalation = S.Struct({
  channel: AgentDefinitionEscalationChannel,
  askPolicy: AgentDefinitionAskPolicy,
});
export type AgentDefinitionEscalation = typeof AgentDefinitionEscalation.Type;

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
});
export type AgentDefinition = typeof AgentDefinition.Type;

export const AgentDefinitionToolAuthorityStatus = S.Literals([
  "allowed",
  "denied",
  "operator_escalation_required",
]);
export type AgentDefinitionToolAuthorityStatus = typeof AgentDefinitionToolAuthorityStatus.Type;

export type AgentDefinitionOperatorEscalation = {
  readonly escalationRef: string;
  readonly definitionId: AgentDefinitionId;
  readonly ownerRef: string;
  readonly toolRef: AgentDefinitionToolRef;
  readonly channel: AgentDefinitionEscalationChannel;
  readonly askPolicyRef: string;
  readonly reasonRef: string;
};

export type AgentDefinitionToolAuthorityDecision = {
  readonly status: AgentDefinitionToolAuthorityStatus;
  readonly allowed: boolean;
  readonly toolRef: AgentDefinitionToolRef;
  readonly definitionId: AgentDefinitionId;
  readonly reasonRef: string;
  readonly matchedPolicyRef?: string;
  readonly blockerRefs: ReadonlyArray<string>;
  readonly escalation?: AgentDefinitionOperatorEscalation;
};

export const AgentDefinitionToolRuntimePolicySchemaLiteral =
  "openagents.agent_definition_tool_runtime_policy.v1" as const;

export type AgentDefinitionCompiledToolRuntimePolicy = {
  readonly schema: typeof AgentDefinitionToolRuntimePolicySchemaLiteral;
  readonly definitionId: AgentDefinitionId;
  readonly ownerRef: string;
  readonly allow: ReadonlyArray<AgentDefinitionToolRef>;
  readonly ask: ReadonlyArray<AgentDefinitionToolRef>;
  readonly deny: ReadonlyArray<AgentDefinitionToolRef>;
  readonly networkPolicy: AgentDefinitionNetworkPolicy;
  readonly secretPolicy: AgentDefinitionSecretPolicy;
  readonly escalation: {
    readonly channel: AgentDefinitionEscalationChannel;
    readonly askPolicyRef: string;
  };
  readonly defaultDecision: "deny";
};

export const AgentRuntimeRunState = S.Literals([
  "pending",
  "running",
  "paused",
  "interrupted",
  "cancelled",
  "completed",
  "failed",
]);
export type AgentRuntimeRunState = typeof AgentRuntimeRunState.Type;

export const AgentRuntimeSafeRef = S.String;
export type AgentRuntimeSafeRef = typeof AgentRuntimeSafeRef.Type;

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
]);
export type AgentRuntimePart = typeof AgentRuntimePart.Type;

export const AgentRuntimeToolInvocation = S.Struct({
  invocationId: S.String,
  toolName: S.String,
  toolRef: S.String,
  inputRef: S.optional(S.String),
  outputRef: S.optional(S.String),
  status: S.Literals([
    "proposed",
    "approval_requested",
    "approved",
    "denied",
    "started",
    "completed",
    "failed",
  ]),
  summary: S.optional(S.String),
  blockerRefs: S.Array(S.String),
});
export type AgentRuntimeToolInvocation = typeof AgentRuntimeToolInvocation.Type;

export const AgentRuntimeExternalInvocation = S.Struct({
  invocationId: S.String,
  adapterKind: AgentRuntimeAdapterKind,
  sessionRef: S.optional(S.String),
  status: S.Literals(["started", "event", "artifact_recorded", "completed", "failed"]),
  summary: S.optional(S.String),
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
});
export type AgentRuntimeExternalInvocation = typeof AgentRuntimeExternalInvocation.Type;

export const AgentRuntimeArtifactRef = S.Struct({
  artifactRef: S.String,
  artifactKind: S.String,
  visibility: AgentRuntimeVisibility,
  digestRef: S.optional(S.String),
  summary: S.optional(S.String),
});
export type AgentRuntimeArtifactRef = typeof AgentRuntimeArtifactRef.Type;

export const AgentRuntimeUsageRecord = S.Struct({
  usageRef: S.String,
  providerRef: S.optional(S.String),
  modelRef: S.optional(S.String),
  inputTokens: S.optional(S.Number),
  outputTokens: S.optional(S.Number),
  totalTokens: S.optional(S.Number),
  costRef: S.optional(S.String),
});
export type AgentRuntimeUsageRecord = typeof AgentRuntimeUsageRecord.Type;

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
]);
export type AgentRuntimeEventTag = typeof AgentRuntimeEventTag.Type;

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
];

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
});
export type AgentRuntimeEvent = typeof AgentRuntimeEvent.Type;

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
});
export type AgentRuntimeRun = typeof AgentRuntimeRun.Type;

export const AgentRuntimeEventLog = S.Struct({
  run: AgentRuntimeRun,
  events: S.Array(AgentRuntimeEvent),
});
export type AgentRuntimeEventLog = typeof AgentRuntimeEventLog.Type;

export const KhalaRuntimeEventSchemaLiteral = "openagents.khala_runtime_event.v1" as const;
export const KhalaRuntimeControlIntentSchemaLiteral =
  "openagents.khala_runtime_control_intent.v1" as const;

const khalaRuntimeSafeRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/;

export const KhalaRuntimeSafeRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(khalaRuntimeSafeRefPattern),
);
export type KhalaRuntimeSafeRef = typeof KhalaRuntimeSafeRef.Type;

export const KhalaRuntimeThreadId = KhalaRuntimeSafeRef;
export type KhalaRuntimeThreadId = typeof KhalaRuntimeThreadId.Type;

export const KhalaRuntimeTurnId = KhalaRuntimeSafeRef;
export type KhalaRuntimeTurnId = typeof KhalaRuntimeTurnId.Type;

export const KhalaRuntimeMessageId = KhalaRuntimeSafeRef;
export type KhalaRuntimeMessageId = typeof KhalaRuntimeMessageId.Type;

export const KhalaRuntimeEventId = KhalaRuntimeSafeRef;
export type KhalaRuntimeEventId = typeof KhalaRuntimeEventId.Type;

export const KhalaRuntimeControlIntentId = KhalaRuntimeSafeRef;
export type KhalaRuntimeControlIntentId = typeof KhalaRuntimeControlIntentId.Type;

export const KhalaRuntimeToolCallId = KhalaRuntimeSafeRef;
export type KhalaRuntimeToolCallId = typeof KhalaRuntimeToolCallId.Type;

export const KhalaRuntimeStreamChunkId = KhalaRuntimeSafeRef;
export type KhalaRuntimeStreamChunkId = typeof KhalaRuntimeStreamChunkId.Type;

export const KhalaRuntimeChildAgentId = KhalaRuntimeSafeRef;
export type KhalaRuntimeChildAgentId = typeof KhalaRuntimeChildAgentId.Type;

export const KhalaRuntimeCausalityRef = KhalaRuntimeSafeRef;
export type KhalaRuntimeCausalityRef = typeof KhalaRuntimeCausalityRef.Type;

export const KhalaRuntimeLane = S.Literals([
  "codex_app_server",
  "claude_pylon",
  "ai_sdk_core",
  "ai_sdk_harness_sandbox",
  "khala_sync_mobile_control",
  "hosted_khala",
  "managed_cloud",
  "agent_client_protocol",
  "test_fixture",
]);
export type KhalaRuntimeLane = typeof KhalaRuntimeLane.Type;

export const khalaRuntimeLanes: ReadonlyArray<KhalaRuntimeLane> = [
  "codex_app_server",
  "claude_pylon",
  "ai_sdk_core",
  "ai_sdk_harness_sandbox",
  "khala_sync_mobile_control",
  "hosted_khala",
  "managed_cloud",
  "agent_client_protocol",
  "test_fixture",
];

export const KhalaRuntimeClientSurface = S.Literals([
  "desktop",
  "mobile",
  "web",
  "server",
  "cli",
  "test_fixture",
]);
export type KhalaRuntimeClientSurface = typeof KhalaRuntimeClientSurface.Type;

export const KhalaRuntimeFinishReason = S.Literals([
  "stop",
  "length",
  "tool-calls",
  "content-filter",
  "error",
  "cancelled",
  "interrupted",
  "unknown",
]);
export type KhalaRuntimeFinishReason = typeof KhalaRuntimeFinishReason.Type;

export const KhalaRuntimeSource = S.Struct({
  lane: KhalaRuntimeLane,
  adapterKind: S.optional(AgentRuntimeAdapterKind),
  surface: S.optional(KhalaRuntimeClientSurface),
  providerRef: S.optional(KhalaRuntimeSafeRef),
  modelRef: S.optional(KhalaRuntimeSafeRef),
  adapterSessionRef: S.optional(KhalaRuntimeSafeRef),
});
export type KhalaRuntimeSource = typeof KhalaRuntimeSource.Type;

export const KhalaRuntimeProviderMetadata = S.Struct({
  providerRef: S.optional(KhalaRuntimeSafeRef),
  modelRef: S.optional(KhalaRuntimeSafeRef),
  metadataRefs: S.Array(KhalaRuntimeSafeRef),
});
export type KhalaRuntimeProviderMetadata = typeof KhalaRuntimeProviderMetadata.Type;

export const KhalaRuntimeUsage = S.Struct({
  usageRef: KhalaRuntimeSafeRef,
  inputTokens: S.optional(S.Number),
  outputTokens: S.optional(S.Number),
  reasoningTokens: S.optional(S.Number),
  cacheReadInputTokens: S.optional(S.Number),
  cacheWriteInputTokens: S.optional(S.Number),
  totalTokens: S.optional(S.Number),
  costRef: S.optional(KhalaRuntimeSafeRef),
});
export type KhalaRuntimeUsage = typeof KhalaRuntimeUsage.Type;

export const KhalaRuntimeToolAuthority = S.Struct({
  authorityRef: KhalaRuntimeSafeRef,
  policyRef: KhalaRuntimeSafeRef,
  decisionRef: KhalaRuntimeSafeRef,
  toolRef: AgentDefinitionToolRef,
  status: AgentDefinitionToolAuthorityStatus,
  allowed: S.Boolean,
  blockerRefs: S.Array(KhalaRuntimeSafeRef),
});
export type KhalaRuntimeToolAuthority = typeof KhalaRuntimeToolAuthority.Type;

export const KhalaRuntimeFileChange = S.Struct({
  fileChangeRef: KhalaRuntimeSafeRef,
  pathRef: KhalaRuntimeSafeRef,
  op: S.Literals(["created", "modified", "deleted", "renamed"]),
  digestRef: S.optional(KhalaRuntimeSafeRef),
  previousPathRef: S.optional(KhalaRuntimeSafeRef),
});
export type KhalaRuntimeFileChange = typeof KhalaRuntimeFileChange.Type;

export const KhalaRuntimeWritebackStatus = S.Literals([
  "branch_pushed",
  "pull_request_opened",
  "pull_request_reused",
  "failed",
]);
export type KhalaRuntimeWritebackStatus = typeof KhalaRuntimeWritebackStatus.Type;

const KhalaRuntimeEventBase = {
  schema: S.Literal(KhalaRuntimeEventSchemaLiteral),
  eventId: KhalaRuntimeEventId,
  turnId: KhalaRuntimeTurnId,
  threadId: KhalaRuntimeThreadId,
  sequence: S.Number,
  observedAt: S.String,
  source: KhalaRuntimeSource,
  visibility: AgentRuntimeVisibility,
  redactionClass: AgentRuntimeRedactionClass,
  causalityRefs: S.Array(KhalaRuntimeCausalityRef),
  syncScopeRef: S.optional(KhalaRuntimeSafeRef),
} as const;

export const KhalaRuntimeEventKind = S.Literals([
  "turn.started",
  "turn.finished",
  "turn.interrupted",
  "step.started",
  "step.finished",
  "text.delta",
  "text.completed",
  "reasoning.delta",
  "reasoning.completed",
  "tool.input.delta",
  "tool.input.completed",
  "tool.call",
  "tool.result",
  "tool.error",
  "agent.child.started",
  "agent.child.progress",
  "agent.child.finished",
  "usage.recorded",
  "provider.metadata",
  "file.change",
  "writeback.recorded",
  "compaction.recorded",
  "raw.sidecar_ref",
]);
export type KhalaRuntimeEventKind = typeof KhalaRuntimeEventKind.Type;

export const khalaRuntimeEventKinds: ReadonlyArray<KhalaRuntimeEventKind> = [
  "turn.started",
  "turn.finished",
  "turn.interrupted",
  "step.started",
  "step.finished",
  "text.delta",
  "text.completed",
  "reasoning.delta",
  "reasoning.completed",
  "tool.input.delta",
  "tool.input.completed",
  "tool.call",
  "tool.result",
  "tool.error",
  "agent.child.started",
  "agent.child.progress",
  "agent.child.finished",
  "usage.recorded",
  "provider.metadata",
  "file.change",
  "writeback.recorded",
  "compaction.recorded",
  "raw.sidecar_ref",
];

export const KhalaRuntimeEvent = S.Union([
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("turn.started"),
    controlIntentId: S.optional(KhalaRuntimeControlIntentId),
    userMessageId: S.optional(KhalaRuntimeMessageId),
    promptRef: S.optional(KhalaRuntimeSafeRef),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("turn.finished"),
    finishReason: KhalaRuntimeFinishReason,
    usage: S.optional(KhalaRuntimeUsage),
    providerMetadata: S.optional(KhalaRuntimeProviderMetadata),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("turn.interrupted"),
    controlIntentId: S.optional(KhalaRuntimeControlIntentId),
    reasonRef: S.optional(KhalaRuntimeSafeRef),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("step.started"),
    stepId: KhalaRuntimeSafeRef,
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("step.finished"),
    stepId: KhalaRuntimeSafeRef,
    finishReason: KhalaRuntimeFinishReason,
    usage: S.optional(KhalaRuntimeUsage),
    providerMetadata: S.optional(KhalaRuntimeProviderMetadata),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("text.delta"),
    messageId: KhalaRuntimeMessageId,
    chunkId: KhalaRuntimeStreamChunkId,
    text: S.String,
    providerMetadata: S.optional(KhalaRuntimeProviderMetadata),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("text.completed"),
    messageId: KhalaRuntimeMessageId,
    finalTextRef: S.optional(KhalaRuntimeSafeRef),
    providerMetadata: S.optional(KhalaRuntimeProviderMetadata),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("reasoning.delta"),
    messageId: KhalaRuntimeMessageId,
    chunkId: KhalaRuntimeStreamChunkId,
    text: S.String,
    providerMetadata: S.optional(KhalaRuntimeProviderMetadata),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("reasoning.completed"),
    messageId: KhalaRuntimeMessageId,
    summaryRef: S.optional(KhalaRuntimeSafeRef),
    providerMetadata: S.optional(KhalaRuntimeProviderMetadata),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("tool.input.delta"),
    toolCallId: KhalaRuntimeToolCallId,
    toolName: S.String,
    chunkId: KhalaRuntimeStreamChunkId,
    inputDelta: S.String,
    authority: KhalaRuntimeToolAuthority,
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("tool.input.completed"),
    toolCallId: KhalaRuntimeToolCallId,
    toolName: S.String,
    inputRef: S.optional(KhalaRuntimeSafeRef),
    authority: KhalaRuntimeToolAuthority,
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("tool.call"),
    toolCallId: KhalaRuntimeToolCallId,
    toolName: S.String,
    inputRef: S.optional(KhalaRuntimeSafeRef),
    authority: KhalaRuntimeToolAuthority,
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("tool.result"),
    toolCallId: KhalaRuntimeToolCallId,
    toolName: S.String,
    resultRef: KhalaRuntimeSafeRef,
    authority: KhalaRuntimeToolAuthority,
    providerExecuted: S.optional(S.Boolean),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("tool.error"),
    toolCallId: KhalaRuntimeToolCallId,
    toolName: S.String,
    errorRef: KhalaRuntimeSafeRef,
    messageSafe: S.String,
    authority: KhalaRuntimeToolAuthority,
    providerExecuted: S.optional(S.Boolean),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("agent.child.started"),
    childAgentId: KhalaRuntimeChildAgentId,
    childRunId: KhalaRuntimeSafeRef,
    parentAgentId: KhalaRuntimeSafeRef,
    taskRef: S.optional(KhalaRuntimeSafeRef),
    childKindRef: S.optional(KhalaRuntimeSafeRef),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("agent.child.progress"),
    childAgentId: KhalaRuntimeChildAgentId,
    childRunId: KhalaRuntimeSafeRef,
    parentAgentId: KhalaRuntimeSafeRef,
    taskRef: S.optional(KhalaRuntimeSafeRef),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("agent.child.finished"),
    childAgentId: KhalaRuntimeChildAgentId,
    childRunId: KhalaRuntimeSafeRef,
    parentAgentId: KhalaRuntimeSafeRef,
    taskRef: S.optional(KhalaRuntimeSafeRef),
    finishReason: KhalaRuntimeFinishReason,
    usage: S.optional(KhalaRuntimeUsage),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("usage.recorded"),
    usage: KhalaRuntimeUsage,
    providerMetadata: S.optional(KhalaRuntimeProviderMetadata),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("provider.metadata"),
    providerMetadata: KhalaRuntimeProviderMetadata,
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("file.change"),
    fileChange: KhalaRuntimeFileChange,
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("writeback.recorded"),
    writebackRef: KhalaRuntimeSafeRef,
    repositoryFullName: S.String,
    branch: S.String,
    branchUrl: S.String,
    status: KhalaRuntimeWritebackStatus,
    changedFileCount: S.optional(S.Number),
    pullRequestUrl: S.optional(S.String),
    pullRequestNumber: S.optional(S.Number),
    reasonRef: S.optional(KhalaRuntimeSafeRef),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("compaction.recorded"),
    beforeContextRef: KhalaRuntimeSafeRef,
    afterContextRef: KhalaRuntimeSafeRef,
    reasonRef: S.optional(KhalaRuntimeSafeRef),
  }),
  S.Struct({
    ...KhalaRuntimeEventBase,
    kind: S.Literal("raw.sidecar_ref"),
    rawEventRef: KhalaRuntimeSafeRef,
    rawEventKind: S.Literals([
      "ai_sdk_stream_part",
      "codex_sdk_event",
      "claude_sdk_event",
      "grok_acp_event",
      "cursor_acp_event",
      "agent_client_protocol_event",
      "provider_chunk",
      "other",
    ]),
  }),
]);
export type KhalaRuntimeEvent = typeof KhalaRuntimeEvent.Type;

export const KhalaRuntimeControlIntentKind = S.Literals([
  "thread.create",
  "thread.rename",
  "message.append",
  "turn.start",
  "turn.interrupt",
  "turn.continue",
  "turn.retry",
  "turn.close",
  "tool.approve",
  "tool.deny",
]);
export type KhalaRuntimeControlIntentKind = typeof KhalaRuntimeControlIntentKind.Type;

export const khalaRuntimeControlIntentKinds: ReadonlyArray<KhalaRuntimeControlIntentKind> = [
  "thread.create",
  "thread.rename",
  "message.append",
  "turn.start",
  "turn.interrupt",
  "turn.continue",
  "turn.retry",
  "turn.close",
  "tool.approve",
  "tool.deny",
];

export const KhalaRuntimeControlIntent = S.Struct({
  schema: S.Literal(KhalaRuntimeControlIntentSchemaLiteral),
  intentId: KhalaRuntimeControlIntentId,
  kind: KhalaRuntimeControlIntentKind,
  threadId: KhalaRuntimeThreadId,
  turnId: S.optional(KhalaRuntimeTurnId),
  messageId: S.optional(KhalaRuntimeMessageId),
  toolCallId: S.optional(KhalaRuntimeToolCallId),
  createdAt: S.String,
  /**
   * Immutable client-minted admission deadline. A Sync retry carries the
   * exact same value; once the server clock reaches it, the command is
   * durably projected as expired and must never reach a runtime adapter.
   */
  expiresAt: S.optional(S.String),
  origin: S.Struct({
    surface: KhalaRuntimeClientSurface,
    lane: KhalaRuntimeLane,
    deviceRef: S.optional(KhalaRuntimeSafeRef),
    userRef: S.optional(KhalaRuntimeSafeRef),
  }),
  target: S.Struct({
    lane: KhalaRuntimeLane,
    adapterKind: S.optional(AgentRuntimeAdapterKind),
    executionTargetId: S.optional(KhalaRuntimeSafeRef),
  }),
  visibility: S.Literals(["operator", "private"]),
  redactionClass: AgentRuntimeRedactionClass,
  idempotencyKey: KhalaRuntimeSafeRef,
  causalityRefs: S.Array(KhalaRuntimeCausalityRef),
  title: S.optional(S.String),
  body: S.optional(S.String),
  bodyRef: S.optional(KhalaRuntimeSafeRef),
  promptRef: S.optional(KhalaRuntimeSafeRef),
  reasonRef: S.optional(KhalaRuntimeSafeRef),
  authority: S.optional(KhalaRuntimeToolAuthority),
});
export type KhalaRuntimeControlIntent = typeof KhalaRuntimeControlIntent.Type;

export const decodeKhalaRuntimeEvent = S.decodeUnknownSync(KhalaRuntimeEvent);
export const decodeKhalaRuntimeControlIntent = S.decodeUnknownSync(KhalaRuntimeControlIntent);

// ---------------------------------------------------------------------------
// openagents.runtime_interaction.v1 — durable provider-neutral interaction.
// ---------------------------------------------------------------------------

export const RuntimeInteractionSchemaLiteral = "openagents.runtime_interaction.v1" as const;

export const RuntimeInteractionKind = S.Literals([
  "provider_question",
  "tool_approval",
  "plan_review",
]);
export type RuntimeInteractionKind = typeof RuntimeInteractionKind.Type;

export const RuntimeInteractionStatus = S.Literals(["pending", "resolved", "expired", "revoked"]);
export type RuntimeInteractionStatus = typeof RuntimeInteractionStatus.Type;

const RuntimeInteractionDisplayText = S.String.check(S.isMinLength(1), S.isMaxLength(2_000));

const RuntimeInteractionIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
);

export const RuntimeInteractionOption = S.Struct({
  optionRef: KhalaRuntimeSafeRef,
  label: S.String.check(S.isMinLength(1), S.isMaxLength(160)),
  description: S.optional(S.String.check(S.isMaxLength(500))),
});
export type RuntimeInteractionOption = typeof RuntimeInteractionOption.Type;

export const RuntimeInteractionQuestion = S.Struct({
  questionRef: KhalaRuntimeSafeRef,
  displayText: RuntimeInteractionDisplayText,
  options: S.Array(RuntimeInteractionOption).check(S.isMaxLength(12)),
  multiSelect: S.Boolean,
});
export type RuntimeInteractionQuestion = typeof RuntimeInteractionQuestion.Type;

export const RuntimeInteractionPayload = S.Union([
  S.Struct({
    kind: S.Literal("provider_question"),
    displayTitle: S.String.check(S.isMinLength(1), S.isMaxLength(160)),
    questions: S.Array(RuntimeInteractionQuestion).check(S.isMinLength(1), S.isMaxLength(8)),
  }),
  S.Struct({
    kind: S.Literal("tool_approval"),
    displayText: RuntimeInteractionDisplayText,
    toolCallId: KhalaRuntimeToolCallId,
    toolName: S.String.check(S.isMinLength(1), S.isMaxLength(160)),
    authority: KhalaRuntimeToolAuthority,
  }),
  S.Struct({
    kind: S.Literal("plan_review"),
    displayText: RuntimeInteractionDisplayText,
    planRef: KhalaRuntimeSafeRef,
  }),
]);
export type RuntimeInteractionPayload = typeof RuntimeInteractionPayload.Type;

export const RuntimeInteractionQuestionAnswer = S.Struct({
  questionRef: KhalaRuntimeSafeRef,
  optionRefs: S.Array(KhalaRuntimeSafeRef).check(S.isMaxLength(12)),
  text: S.optional(S.String.check(S.isMaxLength(2_000))),
});
export type RuntimeInteractionQuestionAnswer = typeof RuntimeInteractionQuestionAnswer.Type;

export const RuntimeInteractionDecision = S.Union([
  S.Struct({
    kind: S.Literal("provider_question"),
    answers: S.Array(RuntimeInteractionQuestionAnswer).check(S.isMinLength(1), S.isMaxLength(8)),
  }),
  S.Struct({
    kind: S.Literal("tool_approval"),
    outcome: S.Literals(["approve", "deny"]),
  }),
  S.Struct({
    kind: S.Literal("plan_review"),
    outcome: S.Literals(["accept", "request_changes", "replan"]),
  }),
]);
export type RuntimeInteractionDecision = typeof RuntimeInteractionDecision.Type;

export const RuntimeInteractionDecisionEnvelope = S.Struct({
  decisionRef: KhalaRuntimeSafeRef,
  idempotencyKey: KhalaRuntimeSafeRef,
  decidedAt: RuntimeInteractionIsoTimestamp,
  surface: KhalaRuntimeClientSurface,
  decision: RuntimeInteractionDecision,
});
export type RuntimeInteractionDecisionEnvelope = typeof RuntimeInteractionDecisionEnvelope.Type;

export const RuntimeInteractionLifecycle = S.Union([
  S.Struct({ status: S.Literal("pending") }),
  S.Struct({
    status: S.Literal("resolved"),
    envelope: RuntimeInteractionDecisionEnvelope,
  }),
  S.Struct({
    status: S.Literal("expired"),
    terminalAt: RuntimeInteractionIsoTimestamp,
    reasonRef: KhalaRuntimeSafeRef,
  }),
  S.Struct({
    status: S.Literal("revoked"),
    terminalAt: RuntimeInteractionIsoTimestamp,
    reasonRef: KhalaRuntimeSafeRef,
  }),
]);
export type RuntimeInteractionLifecycle = typeof RuntimeInteractionLifecycle.Type;

export const RuntimeInteraction = S.Struct({
  schema: S.Literal(RuntimeInteractionSchemaLiteral),
  interactionRef: KhalaRuntimeSafeRef,
  threadId: KhalaRuntimeThreadId,
  turnId: KhalaRuntimeTurnId,
  requestedSequence: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  requestedAt: RuntimeInteractionIsoTimestamp,
  expiresAt: RuntimeInteractionIsoTimestamp,
  source: KhalaRuntimeSource,
  visibility: S.Literal("private"),
  redactionClass: S.Literal("private_ref"),
  causalityRefs: S.Array(KhalaRuntimeCausalityRef).check(S.isMaxLength(32)),
  payload: RuntimeInteractionPayload,
  lifecycle: RuntimeInteractionLifecycle,
});
export type RuntimeInteraction = typeof RuntimeInteraction.Type;

export const decodeRuntimeInteraction = S.decodeUnknownSync(RuntimeInteraction);
export const decodeRuntimeInteractionDecisionEnvelope = S.decodeUnknownSync(
  RuntimeInteractionDecisionEnvelope,
);

const questionDecisionIsValid = (
  payload: Extract<RuntimeInteractionPayload, { readonly kind: "provider_question" }>,
  decision: Extract<RuntimeInteractionDecision, { readonly kind: "provider_question" }>,
): boolean => {
  const answers = new Map(decision.answers.map((answer) => [answer.questionRef, answer]));
  if (answers.size !== decision.answers.length || answers.size !== payload.questions.length) {
    return false;
  }
  return payload.questions.every((question) => {
    const answer = answers.get(question.questionRef);
    if (answer === undefined) return false;
    const options = new Set(question.options.map((option) => option.optionRef));
    if (new Set(answer.optionRefs).size !== answer.optionRefs.length) return false;
    if (answer.optionRefs.some((optionRef) => !options.has(optionRef))) return false;
    if (!question.multiSelect && answer.optionRefs.length > 1) return false;
    return answer.optionRefs.length > 0 || (answer.text?.trim().length ?? 0) > 0;
  });
};

const interactionDecisionIsValid = (
  interaction: RuntimeInteraction,
  decision: RuntimeInteractionDecision,
): boolean => {
  if (interaction.payload.kind !== decision.kind) return false;
  return interaction.payload.kind === "provider_question" && decision.kind === "provider_question"
    ? questionDecisionIsValid(interaction.payload, decision)
    : true;
};

const sameRuntimeInteractionDecision = (
  left: RuntimeInteractionDecisionEnvelope,
  right: RuntimeInteractionDecisionEnvelope,
): boolean =>
  left.decisionRef === right.decisionRef &&
  left.idempotencyKey === right.idempotencyKey &&
  JSON.stringify(left.decision) === JSON.stringify(right.decision);

export type RuntimeInteractionDecisionResult =
  | Readonly<{
      state: "applied" | "duplicate" | "expired";
      interaction: RuntimeInteraction;
    }>
  | Readonly<{
      state: "conflict" | "invalid_decision" | "revoked";
      interaction: RuntimeInteraction;
    }>;

export const applyRuntimeInteractionDecision = (
  interaction: RuntimeInteraction,
  envelope: RuntimeInteractionDecisionEnvelope,
  serverNow: string,
): RuntimeInteractionDecisionResult => {
  if (interaction.lifecycle.status === "resolved") {
    return {
      state: sameRuntimeInteractionDecision(interaction.lifecycle.envelope, envelope)
        ? "duplicate"
        : "conflict",
      interaction,
    };
  }
  if (interaction.lifecycle.status === "expired") {
    return { state: "expired", interaction };
  }
  if (interaction.lifecycle.status === "revoked") {
    return { state: "revoked", interaction };
  }
  if (Date.parse(serverNow) >= Date.parse(interaction.expiresAt)) {
    return {
      state: "expired",
      interaction: {
        ...interaction,
        lifecycle: {
          status: "expired",
          terminalAt: serverNow,
          reasonRef: "reason.interaction_deadline_elapsed",
        },
      },
    };
  }
  if (!interactionDecisionIsValid(interaction, envelope.decision)) {
    return { state: "invalid_decision", interaction };
  }
  return {
    state: "applied",
    interaction: {
      ...interaction,
      lifecycle: { status: "resolved", envelope },
    },
  };
};

export const RuntimeInteractionProjection = S.Struct({
  schema: S.Literal("openagents.runtime_interaction_projection.v1"),
  interactionRef: KhalaRuntimeSafeRef,
  threadId: KhalaRuntimeThreadId,
  turnId: KhalaRuntimeTurnId,
  kind: RuntimeInteractionKind,
  status: RuntimeInteractionStatus,
  displayTitle: S.String.check(S.isMinLength(1), S.isMaxLength(160)),
  displayText: S.String.check(S.isMaxLength(2_000)),
  questions: S.Array(RuntimeInteractionQuestion).check(S.isMaxLength(8)),
  expiresAt: RuntimeInteractionIsoTimestamp,
  decisionRef: S.optional(KhalaRuntimeSafeRef),
});
export type RuntimeInteractionProjection = typeof RuntimeInteractionProjection.Type;

export const projectRuntimeInteraction = (
  interaction: RuntimeInteraction,
): RuntimeInteractionProjection => {
  const displayTitle =
    interaction.payload.kind === "provider_question"
      ? interaction.payload.displayTitle
      : interaction.payload.kind === "tool_approval"
        ? `Approve ${interaction.payload.toolName}`
        : "Review plan";
  const displayText =
    interaction.payload.kind === "provider_question"
      ? interaction.payload.questions.map((question) => question.displayText).join("\n")
      : interaction.payload.displayText;
  const questions =
    interaction.payload.kind === "provider_question" ? interaction.payload.questions : [];
  return {
    schema: "openagents.runtime_interaction_projection.v1",
    interactionRef: interaction.interactionRef,
    threadId: interaction.threadId,
    turnId: interaction.turnId,
    kind: interaction.payload.kind,
    status: interaction.lifecycle.status,
    displayTitle,
    displayText,
    questions,
    expiresAt: interaction.expiresAt,
    ...(interaction.lifecycle.status === "resolved"
      ? { decisionRef: interaction.lifecycle.envelope.decisionRef }
      : {}),
  };
};

// ---------------------------------------------------------------------------
// khala.chat_turn_event.v1 — the neutral, harness-agnostic chat turn event.
//
// This is the versioned, canonical turn-event contract shared across harnesses
// (codex | claude | grok | ...) and surfaces (desktop | mobile | sync | web).
// It is intentionally narrow: it carries only harness-neutral message and tool
// data so mobile/sync consumers can depend on it without pulling any desktop-
// specific types. Harness-specialized surfaces (e.g. Khala Code desktop) may
// project their richer per-item cards onto this contract, but the neutral shape
// here is the interoperable spine.
// ---------------------------------------------------------------------------

export const KhalaChatTurnEventSchemaLiteral = "khala.chat_turn_event.v1" as const;

export const KhalaChatTurnEventMessageRole = S.Literals(["user", "assistant", "system", "tool"]);
export type KhalaChatTurnEventMessageRole = typeof KhalaChatTurnEventMessageRole.Type;

export const KhalaChatTurnEventMessage = S.Struct({
  id: S.String,
  role: KhalaChatTurnEventMessageRole,
  body: S.String,
});
export type KhalaChatTurnEventMessage = typeof KhalaChatTurnEventMessage.Type;

// Neutral tool event: harness-agnostic envelope. `payload` is opaque so each
// harness can carry its own tool-event body without leaking a harness-specific
// shape into the shared contract.
export const KhalaChatTurnEventToolEvent = S.Struct({
  eventId: S.String,
  invocationId: S.optional(S.String),
  kind: S.String,
  sessionId: S.String,
  payload: S.Unknown,
});
export type KhalaChatTurnEventToolEvent = typeof KhalaChatTurnEventToolEvent.Type;

export const KhalaChatTurnEventKind = S.Literals([
  "thread_ready",
  "message_start",
  "message_delta",
  "message_replace",
  "message_done",
  "tool_event",
]);
export type KhalaChatTurnEventKind = typeof KhalaChatTurnEventKind.Type;

export const khalaChatTurnEventKinds: ReadonlyArray<KhalaChatTurnEventKind> = [
  "thread_ready",
  "message_start",
  "message_delta",
  "message_replace",
  "message_done",
  "tool_event",
];

export const KhalaChatTurnEventV1 = S.Union([
  S.Struct({
    type: S.Literal("thread_ready"),
    threadId: S.String,
    turnId: S.String,
  }),
  S.Struct({
    type: S.Literal("message_start"),
    turnId: S.String,
    message: KhalaChatTurnEventMessage,
  }),
  S.Struct({
    type: S.Literal("message_delta"),
    turnId: S.String,
    messageId: S.String,
    delta: S.String,
  }),
  S.Struct({
    type: S.Literal("message_replace"),
    turnId: S.String,
    message: KhalaChatTurnEventMessage,
  }),
  S.Struct({
    type: S.Literal("message_done"),
    turnId: S.String,
    messageId: S.String,
  }),
  S.Struct({
    type: S.Literal("tool_event"),
    turnId: S.String,
    event: KhalaChatTurnEventToolEvent,
  }),
]);
export type KhalaChatTurnEventV1 = typeof KhalaChatTurnEventV1.Type;

export const decodeKhalaChatTurnEventV1 = S.decodeUnknownSync(KhalaChatTurnEventV1);

export type KhalaRuntimeAiSdkUsage = {
  readonly inputTokens?: number | undefined;
  readonly outputTokens?: number | undefined;
  readonly totalTokens?: number | undefined;
  readonly inputTokenDetails?:
    | {
        readonly cacheReadTokens?: number | undefined;
        readonly cacheWriteTokens?: number | undefined;
      }
    | undefined;
  readonly outputTokenDetails?:
    | {
        readonly reasoningTokens?: number | undefined;
      }
    | undefined;
};

export type KhalaRuntimeAiSdkTextStreamPart =
  | { readonly type: "start" }
  | {
      readonly type: "start-step";
      readonly request?: unknown;
      readonly warnings?: ReadonlyArray<unknown>;
    }
  | { readonly type: "text-start"; readonly id: string; readonly providerMetadata?: unknown }
  | {
      readonly type: "text-delta";
      readonly id: string;
      readonly text: string;
      readonly providerMetadata?: unknown;
    }
  | { readonly type: "text-end"; readonly id: string; readonly providerMetadata?: unknown }
  | { readonly type: "reasoning-start"; readonly id: string; readonly providerMetadata?: unknown }
  | {
      readonly type: "reasoning-delta";
      readonly id: string;
      readonly text: string;
      readonly providerMetadata?: unknown;
    }
  | { readonly type: "reasoning-end"; readonly id: string; readonly providerMetadata?: unknown }
  | {
      readonly type: "tool-input-start";
      readonly id: string;
      readonly toolName: string;
      readonly providerMetadata?: unknown;
    }
  | {
      readonly type: "tool-input-delta";
      readonly id: string;
      readonly delta: string;
      readonly providerMetadata?: unknown;
    }
  | { readonly type: "tool-input-end"; readonly id: string; readonly providerMetadata?: unknown }
  | {
      readonly type: "tool-call";
      readonly toolCallId: string;
      readonly toolName: string;
      readonly input?: unknown;
      readonly providerExecuted?: boolean | undefined;
      readonly providerMetadata?: unknown;
    }
  | {
      readonly type: "tool-result";
      readonly toolCallId: string;
      readonly toolName: string;
      readonly output?: unknown;
      readonly providerExecuted?: boolean | undefined;
      readonly providerMetadata?: unknown;
    }
  | {
      readonly type: "tool-error";
      readonly toolCallId: string;
      readonly toolName: string;
      readonly error?: unknown;
      readonly providerExecuted?: boolean | undefined;
      readonly providerMetadata?: unknown;
    }
  | { readonly type: "tool-output-denied"; readonly toolCallId: string; readonly toolName: string }
  | {
      readonly type: "tool-approval-request";
      readonly toolCallId: string;
      readonly toolName: string;
    }
  | {
      readonly type: "tool-approval-response";
      readonly toolCallId: string;
      readonly toolName: string;
    }
  | {
      readonly type: "finish-step";
      readonly finishReason: string;
      readonly usage?: KhalaRuntimeAiSdkUsage | undefined;
      readonly providerMetadata?: unknown;
    }
  | {
      readonly type: "finish";
      readonly finishReason: string;
      readonly totalUsage?: KhalaRuntimeAiSdkUsage | undefined;
    }
  | { readonly type: "abort"; readonly reason?: string | undefined }
  | { readonly type: "error"; readonly error?: unknown }
  | { readonly type: "raw"; readonly rawValue: unknown }
  | { readonly type: "custom"; readonly kind: string; readonly providerMetadata?: unknown }
  | { readonly type: "source"; readonly [key: string]: unknown }
  | { readonly type: "file"; readonly [key: string]: unknown }
  | { readonly type: "reasoning-file"; readonly [key: string]: unknown };

export function khalaRuntimeEventFromAgentRuntimeEvent(input: {
  readonly event: AgentRuntimeEvent;
  readonly threadId: string;
  readonly turnId: string;
  readonly source: KhalaRuntimeSource;
  readonly authority?: KhalaRuntimeToolAuthority;
}): KhalaRuntimeEvent {
  const event = input.event;
  const base = khalaRuntimeBaseFromAgentRuntimeEvent(input);

  switch (event.tag) {
    case "run.started":
    case "run.input_accepted":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "turn.started",
        ...(event.refs[0] === undefined
          ? {}
          : { promptRef: khalaRuntimeSafeRef(event.refs[0], "prompt.private.agent_runtime") }),
      });

    case "run.completed":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "turn.finished",
        finishReason: "stop",
        ...(event.usage === undefined
          ? {}
          : { usage: khalaRuntimeUsageFromAgentRuntimeUsage(event.usage) }),
      });

    case "run.failed":
      return decodeKhalaRuntimeEvent({ ...base, kind: "turn.finished", finishReason: "error" });

    case "run.cancelled":
      return decodeKhalaRuntimeEvent({ ...base, kind: "turn.finished", finishReason: "cancelled" });

    case "run.interrupted":
    case "run.paused":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "turn.interrupted",
        ...(event.blockerRefs[0] === undefined
          ? {}
          : {
              reasonRef: khalaRuntimeSafeRef(event.blockerRefs[0], "reason.private.agent_runtime"),
            }),
      });

    case "step.started":
    case "model.stream_started":
    case "external_agent.started":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "step.started",
        stepId: khalaRuntimeStepIdFromAgentEvent(event),
      });

    case "step.completed":
    case "external_agent.completed":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "step.finished",
        stepId: khalaRuntimeStepIdFromAgentEvent(event),
        finishReason: "stop",
      });

    case "step.failed":
    case "external_agent.failed":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "step.finished",
        stepId: khalaRuntimeStepIdFromAgentEvent(event),
        finishReason: "error",
      });

    case "model.text_delta":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "text.delta",
        messageId: khalaRuntimeMessageIdFromAgentEvent(event),
        chunkId: khalaRuntimeChunkIdFromAgentEvent(event),
        text: event.part?.kind === "text" ? event.part.text : (event.summary ?? ""),
      });

    case "model.text_completed":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "text.completed",
        messageId: khalaRuntimeMessageIdFromAgentEvent(event),
        ...(event.refs[0] === undefined
          ? {}
          : { finalTextRef: khalaRuntimeSafeRef(event.refs[0], "text.private.agent_runtime") }),
      });

    case "model.reasoning_delta":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "reasoning.delta",
        messageId: khalaRuntimeMessageIdFromAgentEvent(event),
        chunkId: khalaRuntimeChunkIdFromAgentEvent(event),
        text: event.part?.kind === "reasoning" ? event.part.summary : (event.summary ?? ""),
      });

    case "model.reasoning_completed":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "reasoning.completed",
        messageId: khalaRuntimeMessageIdFromAgentEvent(event),
        ...(event.refs[0] === undefined
          ? {}
          : { summaryRef: khalaRuntimeSafeRef(event.refs[0], "reasoning.private.agent_runtime") }),
      });

    case "tool.input_delta":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.input.delta",
        toolCallId: khalaRuntimeToolCallIdFromAgentEvent(event),
        toolName: event.toolInvocation?.toolName ?? "unknown",
        chunkId: khalaRuntimeChunkIdFromAgentEvent(event),
        inputDelta: event.part?.kind === "text" ? event.part.text : (event.summary ?? ""),
        authority: khalaRuntimeRequireToolAuthority(input.authority),
      });

    case "tool.input_completed":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.input.completed",
        toolCallId: khalaRuntimeToolCallIdFromAgentEvent(event),
        toolName: event.toolInvocation?.toolName ?? "unknown",
        ...(event.toolInvocation?.inputRef === undefined
          ? {}
          : {
              inputRef: khalaRuntimeSafeRef(
                event.toolInvocation.inputRef,
                "input.private.agent_runtime",
              ),
            }),
        authority: khalaRuntimeRequireToolAuthority(input.authority),
      });

    case "tool.completed":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.result",
        toolCallId: khalaRuntimeToolCallIdFromAgentEvent(event),
        toolName: event.toolInvocation?.toolName ?? "unknown",
        resultRef: khalaRuntimeSafeRef(
          event.toolInvocation?.outputRef ?? event.refs[0] ?? `result.private.${event.eventId}`,
          "result.private.agent_runtime",
        ),
        authority: khalaRuntimeRequireToolAuthority(input.authority),
      });

    case "tool.failed":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.error",
        toolCallId: khalaRuntimeToolCallIdFromAgentEvent(event),
        toolName: event.toolInvocation?.toolName ?? "unknown",
        errorRef: khalaRuntimeSafeRef(
          event.blockerRefs[0] ?? event.refs[0] ?? `error.private.${event.eventId}`,
          "error.private.agent_runtime",
        ),
        messageSafe: event.summary ?? "tool failed",
        authority: khalaRuntimeRequireToolAuthority(input.authority),
      });

    case "tool.call_proposed":
    case "tool.approval_requested":
    case "tool.approved":
    case "tool.denied":
    case "tool.started":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.call",
        toolCallId: khalaRuntimeToolCallIdFromAgentEvent(event),
        toolName: event.toolInvocation?.toolName ?? "unknown",
        ...(event.toolInvocation?.inputRef === undefined
          ? {}
          : {
              inputRef: khalaRuntimeSafeRef(
                event.toolInvocation.inputRef,
                "input.private.agent_runtime",
              ),
            }),
        authority: khalaRuntimeRequireToolAuthority(input.authority),
      });

    case "usage.recorded":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "usage.recorded",
        usage: khalaRuntimeUsageFromAgentRuntimeUsage(event.usage),
        ...(event.usage === undefined
          ? {}
          : { providerMetadata: khalaRuntimeProviderMetadataFromAgentUsage(event.usage) }),
      });

    case "artifact.recorded":
      return event.artifact?.artifactKind === "file_change"
        ? decodeKhalaRuntimeEvent({
            ...base,
            kind: "file.change",
            fileChange: {
              fileChangeRef: khalaRuntimeSafeRef(
                event.artifact.artifactRef,
                "file_change.private.agent_runtime",
              ),
              pathRef: khalaRuntimeSafeRef(
                event.artifact.digestRef ?? event.artifact.artifactRef,
                "path.private.agent_runtime",
              ),
              op: "modified",
              ...(event.artifact.digestRef === undefined
                ? {}
                : {
                    digestRef: khalaRuntimeSafeRef(
                      event.artifact.digestRef,
                      "digest.private.agent_runtime",
                    ),
                  }),
            },
          })
        : khalaRuntimeRawSidecarEvent(base, event, "other");

    case "context.snapshot_created":
    case "external_agent.event":
    case "external_agent.artifact_recorded":
      return khalaRuntimeRawSidecarEvent(
        base,
        event,
        event.tag.startsWith("external_agent") ? "codex_sdk_event" : "other",
      );
  }
}

export function khalaRuntimeEventFromAiSdkTextStreamPart(input: {
  readonly part: KhalaRuntimeAiSdkTextStreamPart;
  readonly eventId: string;
  readonly threadId: string;
  readonly turnId: string;
  readonly sequence: number;
  readonly observedAt: string;
  readonly source?: KhalaRuntimeSource;
  readonly messageId?: string;
  readonly stepId?: string;
  readonly authority?: KhalaRuntimeToolAuthority;
  readonly rawEventRef?: string;
}): KhalaRuntimeEvent {
  const base = {
    schema: KhalaRuntimeEventSchemaLiteral,
    eventId: khalaRuntimeSafeRef(input.eventId, "event.private.ai_sdk"),
    turnId: khalaRuntimeSafeRef(input.turnId, "turn.private.ai_sdk"),
    threadId: khalaRuntimeSafeRef(input.threadId, "thread.private.ai_sdk"),
    sequence: input.sequence,
    observedAt: input.observedAt,
    source: input.source ?? { lane: "ai_sdk_core", surface: "server" },
    visibility: "public",
    redactionClass: "public_ref",
    causalityRefs: [],
  };

  switch (input.part.type) {
    case "start":
      return decodeKhalaRuntimeEvent({ ...base, kind: "turn.started" });

    case "start-step":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "step.started",
        stepId: khalaRuntimeSafeRef(input.stepId ?? `step.${input.eventId}`, "step.private.ai_sdk"),
      });

    case "finish-step":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "step.finished",
        stepId: khalaRuntimeSafeRef(input.stepId ?? `step.${input.eventId}`, "step.private.ai_sdk"),
        finishReason: khalaRuntimeFinishReason(input.part.finishReason),
        ...(input.part.usage === undefined
          ? {}
          : { usage: khalaRuntimeUsageFromAiSdkUsage(input.part.usage, input.eventId) }),
        ...(input.part.providerMetadata === undefined
          ? {}
          : { providerMetadata: khalaRuntimeSidecarProviderMetadata(input.eventId, input.source) }),
      });

    case "finish":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "turn.finished",
        finishReason: khalaRuntimeFinishReason(input.part.finishReason),
        ...(input.part.totalUsage === undefined
          ? {}
          : { usage: khalaRuntimeUsageFromAiSdkUsage(input.part.totalUsage, input.eventId) }),
      });

    case "abort":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "turn.interrupted",
        ...(input.part.reason === undefined
          ? {}
          : { reasonRef: khalaRuntimeSafeRef(input.part.reason, "reason.private.ai_sdk") }),
      });

    case "text-delta":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "text.delta",
        messageId: khalaRuntimeSafeRef(input.messageId ?? input.part.id, "message.private.ai_sdk"),
        chunkId: khalaRuntimeSafeRef(`chunk.${input.eventId}`, "chunk.private.ai_sdk"),
        text: input.part.text,
        ...(input.part.providerMetadata === undefined
          ? {}
          : { providerMetadata: khalaRuntimeSidecarProviderMetadata(input.eventId, input.source) }),
      });

    case "text-end":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "text.completed",
        messageId: khalaRuntimeSafeRef(input.messageId ?? input.part.id, "message.private.ai_sdk"),
        ...(input.part.providerMetadata === undefined
          ? {}
          : { providerMetadata: khalaRuntimeSidecarProviderMetadata(input.eventId, input.source) }),
      });

    case "reasoning-delta":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "reasoning.delta",
        messageId: khalaRuntimeSafeRef(input.messageId ?? input.part.id, "message.private.ai_sdk"),
        chunkId: khalaRuntimeSafeRef(`chunk.${input.eventId}`, "chunk.private.ai_sdk"),
        text: input.part.text,
        ...(input.part.providerMetadata === undefined
          ? {}
          : { providerMetadata: khalaRuntimeSidecarProviderMetadata(input.eventId, input.source) }),
      });

    case "reasoning-end":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "reasoning.completed",
        messageId: khalaRuntimeSafeRef(input.messageId ?? input.part.id, "message.private.ai_sdk"),
        ...(input.part.providerMetadata === undefined
          ? {}
          : { providerMetadata: khalaRuntimeSidecarProviderMetadata(input.eventId, input.source) }),
      });

    case "tool-input-delta":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.input.delta",
        toolCallId: khalaRuntimeSafeRef(input.part.id, "tool_call.private.ai_sdk"),
        toolName: "unknown",
        chunkId: khalaRuntimeSafeRef(`chunk.${input.eventId}`, "chunk.private.ai_sdk"),
        inputDelta: input.part.delta,
        authority: khalaRuntimeRequireToolAuthority(input.authority),
      });

    case "tool-input-end":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.input.completed",
        toolCallId: khalaRuntimeSafeRef(input.part.id, "tool_call.private.ai_sdk"),
        toolName: "unknown",
        authority: khalaRuntimeRequireToolAuthority(input.authority),
      });

    case "tool-input-start":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.call",
        toolCallId: khalaRuntimeSafeRef(input.part.id, "tool_call.private.ai_sdk"),
        toolName: input.part.toolName,
        authority: khalaRuntimeRequireToolAuthority(input.authority),
      });

    case "tool-call":
    case "tool-approval-request":
    case "tool-approval-response":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.call",
        toolCallId: khalaRuntimeSafeRef(input.part.toolCallId, "tool_call.private.ai_sdk"),
        toolName: input.part.toolName,
        inputRef: khalaRuntimeSafeRef(`input.${input.eventId}`, "input.private.ai_sdk"),
        authority: khalaRuntimeRequireToolAuthority(input.authority),
        ...(input.part.type === "tool-call" && input.part.providerExecuted !== undefined
          ? { providerExecuted: input.part.providerExecuted }
          : {}),
      });

    case "tool-result":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.result",
        toolCallId: khalaRuntimeSafeRef(input.part.toolCallId, "tool_call.private.ai_sdk"),
        toolName: input.part.toolName,
        resultRef: khalaRuntimeSafeRef(`result.${input.eventId}`, "result.private.ai_sdk"),
        authority: khalaRuntimeRequireToolAuthority(input.authority),
        ...(input.part.providerExecuted === undefined
          ? {}
          : { providerExecuted: input.part.providerExecuted }),
      });

    case "tool-error":
    case "tool-output-denied":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "tool.error",
        toolCallId: khalaRuntimeSafeRef(input.part.toolCallId, "tool_call.private.ai_sdk"),
        toolName: input.part.toolName,
        errorRef: khalaRuntimeSafeRef(`error.${input.eventId}`, "error.private.ai_sdk"),
        messageSafe: input.part.type === "tool-output-denied" ? "tool output denied" : "tool error",
        authority: khalaRuntimeRequireToolAuthority(input.authority),
        ...(input.part.type === "tool-error" && input.part.providerExecuted !== undefined
          ? { providerExecuted: input.part.providerExecuted }
          : {}),
      });

    case "error":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "turn.finished",
        finishReason: "error",
      });

    case "text-start":
    case "reasoning-start":
    case "custom":
    case "source":
    case "file":
    case "reasoning-file":
    case "raw":
      return decodeKhalaRuntimeEvent({
        ...base,
        kind: "raw.sidecar_ref",
        rawEventRef: khalaRuntimeSafeRef(
          input.rawEventRef ?? `raw.${input.eventId}`,
          input.part.type === "raw" ? "raw.private.ai_sdk" : "event.private.ai_sdk",
        ),
        rawEventKind: "ai_sdk_stream_part",
        visibility: "private",
        redactionClass: "private_ref",
      });
  }
}

export function khalaRuntimePublicEventHasUnsafeMaterial(event: KhalaRuntimeEvent): boolean {
  return event.visibility === "public" && unsafePublicMaterialPattern.test(JSON.stringify(event));
}

export function assertKhalaRuntimePublicEventSafe(event: KhalaRuntimeEvent): KhalaRuntimeEvent {
  if (khalaRuntimePublicEventHasUnsafeMaterial(event)) {
    throw new Error("Khala runtime public event contains raw/private material");
  }
  return event;
}

export function assertKhalaRuntimeControlIntentSafe(
  intent: KhalaRuntimeControlIntent,
): KhalaRuntimeControlIntent {
  if (
    intent.visibility === "operator" &&
    unsafePublicMaterialPattern.test(JSON.stringify(intent))
  ) {
    throw new Error("Khala runtime operator control intent contains raw/private material");
  }
  return intent;
}

function khalaRuntimeBaseFromAgentRuntimeEvent(input: {
  readonly event: AgentRuntimeEvent;
  readonly threadId: string;
  readonly turnId: string;
  readonly source: KhalaRuntimeSource;
}): Record<string, unknown> {
  return {
    schema: KhalaRuntimeEventSchemaLiteral,
    eventId: khalaRuntimeSafeRef(input.event.eventId, "event.private.agent_runtime"),
    turnId: khalaRuntimeSafeRef(input.turnId, "turn.private.agent_runtime"),
    threadId: khalaRuntimeSafeRef(input.threadId, "thread.private.agent_runtime"),
    sequence: input.event.sequence,
    observedAt: input.event.generatedAt,
    source: input.source,
    visibility: input.event.visibility,
    redactionClass: input.event.redactionClass,
    causalityRefs: input.event.refs.map((ref, index) =>
      khalaRuntimeSafeRef(ref, `cause.private.agent_runtime.${index}`),
    ),
  };
}

function khalaRuntimeUsageFromAgentRuntimeUsage(
  usage: AgentRuntimeUsageRecord | undefined,
): Record<string, unknown> {
  if (usage === undefined) {
    return {
      usageRef: "usage.private.agent_runtime.missing",
    };
  }

  return {
    usageRef: khalaRuntimeSafeRef(usage.usageRef, "usage.private.agent_runtime"),
    ...(usage.inputTokens === undefined ? {} : { inputTokens: usage.inputTokens }),
    ...(usage.outputTokens === undefined ? {} : { outputTokens: usage.outputTokens }),
    ...(usage.totalTokens === undefined ? {} : { totalTokens: usage.totalTokens }),
    ...(usage.costRef === undefined
      ? {}
      : { costRef: khalaRuntimeSafeRef(usage.costRef, "cost.private.agent_runtime") }),
  };
}

function khalaRuntimeProviderMetadataFromAgentUsage(
  usage: AgentRuntimeUsageRecord,
): Record<string, unknown> {
  return {
    ...(usage.providerRef === undefined
      ? {}
      : { providerRef: khalaRuntimeSafeRef(usage.providerRef, "provider.private.agent_runtime") }),
    ...(usage.modelRef === undefined
      ? {}
      : { modelRef: khalaRuntimeSafeRef(usage.modelRef, "model.private.agent_runtime") }),
    metadataRefs: [],
  };
}

function khalaRuntimeUsageFromAiSdkUsage(
  usage: KhalaRuntimeAiSdkUsage,
  eventId: string,
): Record<string, unknown> {
  return {
    usageRef: khalaRuntimeSafeRef(`usage.${eventId}`, "usage.private.ai_sdk"),
    ...(usage.inputTokens === undefined ? {} : { inputTokens: usage.inputTokens }),
    ...(usage.outputTokens === undefined ? {} : { outputTokens: usage.outputTokens }),
    ...(usage.outputTokenDetails?.reasoningTokens === undefined
      ? {}
      : { reasoningTokens: usage.outputTokenDetails.reasoningTokens }),
    ...(usage.inputTokenDetails?.cacheReadTokens === undefined
      ? {}
      : { cacheReadInputTokens: usage.inputTokenDetails.cacheReadTokens }),
    ...(usage.inputTokenDetails?.cacheWriteTokens === undefined
      ? {}
      : { cacheWriteInputTokens: usage.inputTokenDetails.cacheWriteTokens }),
    ...(usage.totalTokens === undefined ? {} : { totalTokens: usage.totalTokens }),
  };
}

function khalaRuntimeSidecarProviderMetadata(
  eventId: string,
  source: KhalaRuntimeSource | undefined,
): Record<string, unknown> {
  return {
    ...(source?.providerRef === undefined ? {} : { providerRef: source.providerRef }),
    ...(source?.modelRef === undefined ? {} : { modelRef: source.modelRef }),
    metadataRefs: [khalaRuntimeSafeRef(`metadata.${eventId}`, "metadata.private.ai_sdk")],
  };
}

function khalaRuntimeStepIdFromAgentEvent(event: AgentRuntimeEvent): string {
  return khalaRuntimeSafeRef(
    event.stepRef ?? event.externalInvocation?.invocationId ?? `step.${event.eventId}`,
    "step.private.agent_runtime",
  );
}

function khalaRuntimeMessageIdFromAgentEvent(event: AgentRuntimeEvent): string {
  return khalaRuntimeSafeRef(
    event.stepRef ?? `message.${event.runId}`,
    "message.private.agent_runtime",
  );
}

function khalaRuntimeChunkIdFromAgentEvent(event: AgentRuntimeEvent): string {
  return khalaRuntimeSafeRef(`chunk.${event.eventId}`, "chunk.private.agent_runtime");
}

function khalaRuntimeToolCallIdFromAgentEvent(event: AgentRuntimeEvent): string {
  return khalaRuntimeSafeRef(
    event.toolInvocation?.invocationId ?? `tool_call.${event.eventId}`,
    "tool_call.private.agent_runtime",
  );
}

function khalaRuntimeRequireToolAuthority(
  authority: KhalaRuntimeToolAuthority | undefined,
): KhalaRuntimeToolAuthority {
  if (authority === undefined) {
    throw new Error("Khala runtime tool event requires authority");
  }
  return authority;
}

function khalaRuntimeRawSidecarEvent(
  base: Record<string, unknown>,
  event: AgentRuntimeEvent,
  rawEventKind:
    | "ai_sdk_stream_part"
    | "codex_sdk_event"
    | "claude_sdk_event"
    | "grok_acp_event"
    | "provider_chunk"
    | "other",
): KhalaRuntimeEvent {
  return decodeKhalaRuntimeEvent({
    ...base,
    kind: "raw.sidecar_ref",
    rawEventRef: khalaRuntimeSafeRef(
      event.externalInvocation?.sessionRef ?? event.refs[0] ?? `raw.${event.eventId}`,
      "raw.private.agent_runtime",
    ),
    rawEventKind,
    visibility: "private",
    redactionClass: "private_ref",
  });
}

function khalaRuntimeFinishReason(reason: string | undefined): KhalaRuntimeFinishReason {
  switch (reason) {
    case "stop":
    case "length":
    case "tool-calls":
    case "content-filter":
    case "error":
    case "cancelled":
    case "interrupted":
      return reason;
    case "abort":
      return "cancelled";
    default:
      return "unknown";
  }
}

function khalaRuntimeSafeRef(value: string, fallbackPrefix: string): string {
  if (value.length <= 256 && khalaRuntimeSafeRefPattern.test(value)) {
    return value;
  }
  return stableAgentDefinitionRef(fallbackPrefix, [value]);
}

export type AgentRuntimeSurfaceProjection = {
  readonly runId: AgentRuntimeRunId;
  readonly state: Exclude<AgentRuntimeRunState, "pending">;
  readonly generatedAt: string;
  readonly eventCount: number;
  readonly artifactRefs: ReadonlyArray<string>;
  readonly blockerRefs: ReadonlyArray<string>;
  readonly latestEventId?: string;
  readonly staleness?: {
    readonly maxStalenessSeconds?: number;
    readonly rebuildsOn?: ReadonlyArray<string>;
    readonly transitionRefs?: ReadonlyArray<string>;
  };
};

export type AgentRuntimeSurfaceStatus =
  | "running"
  | "attention"
  | "completed"
  | "failed"
  | "cancelled";

export type AgentRuntimeSurfaceStatusRow = {
  readonly runId: AgentRuntimeRunId;
  readonly status: AgentRuntimeSurfaceStatus;
  readonly label: string;
  readonly generatedAt: string;
  readonly eventCount: number;
  readonly artifactRefs: ReadonlyArray<string>;
  readonly blockerRefs: ReadonlyArray<string>;
  readonly freshness: {
    readonly generatedAt: string;
    readonly maxStalenessSeconds?: number;
    readonly transitionRefs: ReadonlyArray<string>;
  };
  readonly verificationRefs: ReadonlyArray<string>;
  readonly reviewActionRefs: ReadonlyArray<string>;
};

export const decodeAgentRuntimeRun = S.decodeUnknownSync(AgentRuntimeRun);
export const decodeAgentRuntimeEvent = S.decodeUnknownSync(AgentRuntimeEvent);
export const decodeAgentRuntimeEventLog = S.decodeUnknownSync(AgentRuntimeEventLog);
export const decodeAgentDefinition = S.decodeUnknownSync(AgentDefinition);
export const decodeAgentDefinitionTriggerRecord = S.decodeUnknownSync(AgentDefinitionTriggerRecord);

export const PylonAssignmentRunLifecycleEventSchemaLiteral =
  "openagents.pylon.assignment_run_lifecycle_event.v0.1" as const;

export const PylonKhalaSpawnWorkerEventSchemaLiteral =
  "openagents.pylon.khala_spawn_worker_event.v0.1" as const;

export const PylonAssignmentStatus = S.Literals([
  "offered",
  "accepted",
  "running",
  "closed",
  "rejected",
  "cancelled",
  "timed-out",
  "stale",
]);
export type PylonAssignmentStatus = typeof PylonAssignmentStatus.Type;

export const PylonAssignmentProgressStatus = S.Literals([
  "accepted",
  "running",
  "artifact-ready",
  "proof-ready",
  "closeout-submitted",
]);
export type PylonAssignmentProgressStatus = typeof PylonAssignmentProgressStatus.Type;

export const PylonCodexAgentRuntimePhase = S.String;
export type PylonCodexAgentRuntimePhase = typeof PylonCodexAgentRuntimePhase.Type;

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
]);
export type PylonAssignmentRunLifecycleEventName = typeof PylonAssignmentRunLifecycleEventName.Type;

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
});
export type PylonAssignmentRunLifecycleEvent = typeof PylonAssignmentRunLifecycleEvent.Type;

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
]);
export type PylonKhalaSpawnWorkerState = typeof PylonKhalaSpawnWorkerState.Type;

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
});
export type PylonKhalaSpawnWorkerEvent = typeof PylonKhalaSpawnWorkerEvent.Type;

export const PylonLifecycleWireEvent = S.Union([
  PylonAssignmentRunLifecycleEvent,
  PylonKhalaSpawnWorkerEvent,
]);
export type PylonLifecycleWireEvent = typeof PylonLifecycleWireEvent.Type;

export const PylonLifecycleWireEventFromJsonString = S.fromJsonString(PylonLifecycleWireEvent);

export const decodePylonAssignmentRunLifecycleEvent = S.decodeUnknownSync(
  PylonAssignmentRunLifecycleEvent,
);
export const encodePylonAssignmentRunLifecycleEvent = S.encodeUnknownSync(
  PylonAssignmentRunLifecycleEvent,
);
export const decodePylonKhalaSpawnWorkerEvent = S.decodeUnknownSync(PylonKhalaSpawnWorkerEvent);
export const encodePylonKhalaSpawnWorkerEvent = S.encodeUnknownSync(PylonKhalaSpawnWorkerEvent);
export const decodePylonLifecycleWireEvent = S.decodeUnknownSync(PylonLifecycleWireEvent);
export const decodePylonLifecycleWireEventJson = S.decodeUnknownSync(
  PylonLifecycleWireEventFromJsonString,
);

const unsafePublicMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|local[_-]?path|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|email|invoice|log|payment|payload|prompt|provider|record|repo|runner|run[_-]?log|shell|source|state|target|text|trace|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i;

export function agentRuntimePublicEventHasUnsafeMaterial(event: AgentRuntimeEvent): boolean {
  return event.visibility === "public" && unsafePublicMaterialPattern.test(JSON.stringify(event));
}

export function assertAgentRuntimePublicEventSafe(event: AgentRuntimeEvent): AgentRuntimeEvent {
  if (agentRuntimePublicEventHasUnsafeMaterial(event)) {
    throw new Error("Agent runtime public event contains raw/private material");
  }
  return event;
}

export function assertAgentRuntimeEventLogSafe(log: AgentRuntimeEventLog): AgentRuntimeEventLog {
  for (const event of log.events) {
    assertAgentRuntimePublicEventSafe(event);
  }
  return log;
}

export function projectAgentRuntimeSurfaceStatus(
  projection: AgentRuntimeSurfaceProjection,
): AgentRuntimeSurfaceStatusRow {
  const transitionRefs =
    projection.staleness?.transitionRefs ?? projection.staleness?.rebuildsOn ?? [];
  const status: AgentRuntimeSurfaceStatus =
    projection.state === "completed"
      ? "completed"
      : projection.state === "cancelled"
        ? "cancelled"
        : projection.state === "failed"
          ? "failed"
          : projection.state === "paused" || projection.state === "interrupted"
            ? "attention"
            : "running";

  const label =
    status === "completed"
      ? "Completed"
      : status === "cancelled"
        ? "Cancelled"
        : status === "failed"
          ? "Failed"
          : status === "attention"
            ? "Needs attention"
            : "Running";

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
    verificationRefs: projection.artifactRefs.filter((ref) =>
      /^(artifact|proof|result|test)\.public\./.test(ref),
    ),
    reviewActionRefs: projection.blockerRefs.map((ref) => `review.public.agent_runtime.${ref}`),
  };
}

export function agentRuntimeSurfaceStatusHasUnsafeMaterial(
  row: AgentRuntimeSurfaceStatusRow,
): boolean {
  return unsafePublicMaterialPattern.test(JSON.stringify(row));
}

export function decideAgentDefinitionToolAuthority(input: {
  readonly definition: AgentDefinition;
  readonly toolRef: AgentDefinitionToolRef;
  readonly invocationRef?: string;
}): AgentDefinitionToolAuthorityDecision {
  return decideAgentDefinitionCompiledToolAuthority({
    policy: compileAgentDefinitionToolRuntimePolicy(input.definition),
    toolRef: input.toolRef,
    ...(input.invocationRef === undefined ? {} : { invocationRef: input.invocationRef }),
  });
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
  };
}

export function decideAgentDefinitionCompiledToolAuthority(input: {
  readonly policy: AgentDefinitionCompiledToolRuntimePolicy;
  readonly toolRef: AgentDefinitionToolRef;
  readonly invocationRef?: string;
}): AgentDefinitionToolAuthorityDecision {
  const toolRef = input.toolRef;
  const policy = input.policy;

  const deniedBy = firstMatchingToolPolicy(policy.deny, toolRef);
  if (deniedBy !== undefined) {
    return {
      status: "denied",
      allowed: false,
      toolRef,
      definitionId: policy.definitionId,
      matchedPolicyRef: deniedBy,
      reasonRef: "reason.agent_definition.tool_denied",
      blockerRefs: ["blocker.agent_definition.tool_denied"],
    };
  }

  const askBy = firstMatchingToolPolicy(policy.ask, toolRef);
  if (askBy !== undefined) {
    const escalationRef = stableAgentDefinitionRef("escalation.operator.agent_definition", [
      policy.definitionId,
      input.invocationRef ?? toolRef,
      askBy,
    ]);
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
    };
  }

  const allowedBy = firstMatchingToolPolicy(policy.allow, toolRef);
  if (allowedBy !== undefined) {
    return {
      status: "allowed",
      allowed: true,
      toolRef,
      definitionId: policy.definitionId,
      matchedPolicyRef: allowedBy,
      reasonRef: "reason.agent_definition.tool_allowed",
      blockerRefs: [],
    };
  }

  return {
    status: "denied",
    allowed: false,
    toolRef,
    definitionId: policy.definitionId,
    reasonRef: "reason.agent_definition.tool_not_in_allowlist",
    blockerRefs: ["blocker.agent_definition.tool_not_in_allowlist"],
  };
}

function firstMatchingToolPolicy(
  policyRefs: ReadonlyArray<AgentDefinitionToolRef>,
  toolRef: AgentDefinitionToolRef,
): AgentDefinitionToolRef | undefined {
  return policyRefs.find((policyRef) => toolRefMatchesPolicyRef(policyRef, toolRef));
}

function toolRefMatchesPolicyRef(
  policyRef: AgentDefinitionToolRef,
  toolRef: AgentDefinitionToolRef,
): boolean {
  if (policyRef === toolRef) return true;
  if (!policyRef.endsWith(".*")) return false;
  const prefix = policyRef.slice(0, -1);
  return toolRef.startsWith(prefix);
}

function stableAgentDefinitionRef(prefix: string, parts: ReadonlyArray<string>): string {
  let hash = 2166136261;
  for (const char of parts.join("\u001f")) {
    hash ^= char.charCodeAt(0);
    hash = Math.imul(hash, 16777619);
  }
  return `${prefix}.${(hash >>> 0).toString(16).padStart(8, "0")}`;
}

const terminalRunStates: ReadonlySet<AgentRuntimeRunState> = new Set([
  "cancelled",
  "completed",
  "failed",
]);

const legalRunStateTransitions: ReadonlyMap<
  AgentRuntimeRunState,
  ReadonlySet<AgentRuntimeRunState>
> = new Map([
  ["pending", new Set(["running", "cancelled", "failed"])],
  ["running", new Set(["paused", "interrupted", "cancelled", "completed", "failed"])],
  ["paused", new Set(["running", "cancelled", "failed"])],
  ["interrupted", new Set(["running", "cancelled", "failed"])],
  ["cancelled", new Set()],
  ["completed", new Set()],
  ["failed", new Set()],
]);

export function agentRuntimeRunStateTransitionIsLegal(
  from: AgentRuntimeRunState,
  to: AgentRuntimeRunState,
): boolean {
  if (from === to && !terminalRunStates.has(from)) {
    return true;
  }
  return legalRunStateTransitions.get(from)?.has(to) ?? false;
}

export function assertAgentRuntimeRunStateTransition(
  from: AgentRuntimeRunState,
  to: AgentRuntimeRunState,
): AgentRuntimeRunState {
  if (!agentRuntimeRunStateTransitionIsLegal(from, to)) {
    throw new Error(`Illegal AgentRuntimeRun state transition: ${from} -> ${to}`);
  }
  return to;
}
