import type {
  AgentDefinition,
  AgentDefinitionTrigger,
  AgentRuntimeAdapterKind,
  AgentRuntimeEvent,
  AgentRuntimeEventLog,
  AgentRuntimeLoopKind,
  AgentRuntimeRun,
  KhalaRuntimeControlIntent,
  KhalaRuntimeEvent,
  KhalaRuntimeToolAuthority,
} from "./index.js"

const at = "2026-06-11T00:00:00.000Z"

export const agentDefinitionTriggerFixtures: ReadonlyArray<AgentDefinitionTrigger> = [
  {
    kind: "cron",
    triggerRef: "trigger.public.fixture.cron.daily",
    expr: "0 14 * * *",
    tz: "UTC",
  },
  {
    kind: "inbound_webhook",
    triggerRef: "trigger.public.fixture.github.issue_opened",
    source: "github",
    conditions: [
      {
        kind: "event_type",
        equals: "issues.opened",
      },
      {
        kind: "json_path_equals",
        path: "$.repository.full_name",
        equals: "OpenAgentsInc/openagents",
      },
      {
        kind: "json_path_matches",
        path: "$.issue.title",
        pattern: "^BA-",
      },
      {
        kind: "json_path_in",
        path: "$.issue.labels[*].name",
        values: ["background-agents", "fable"],
      },
    ],
  },
  {
    kind: "inbox_match",
    triggerRef: "trigger.public.fixture.inbox.priority",
    classifierRef: "classifier.public.fixture.priority_inbox",
  },
  {
    kind: "manual",
    triggerRef: "trigger.public.fixture.manual.run_now",
  },
]

function run(input: {
  runId: string
  adapterKind: AgentRuntimeAdapterKind
  loopKind: AgentRuntimeLoopKind
}): AgentRuntimeRun {
  return {
    runId: input.runId,
    agentDefinitionId: `agent_definition.public.${input.runId}`,
    assignmentId: `assignment.public.${input.runId}`,
    workspaceRef: `workspace.public.${input.runId}`,
    adapterKind: input.adapterKind,
    loopKind: input.loopKind,
    sourceRefs: [`source.public.${input.adapterKind}`],
    budgetRef: `budget.public.${input.runId}`,
    usagePolicy: "usage.policy.public.runtime_kernel_fixture.v1",
    permissionPolicy: "permission.policy.public.runtime_kernel_fixture.v1",
    redactionPolicy: {
      policyRef: "redaction.policy.public.runtime_kernel_fixture.v1",
      rawPromptAllowed: false,
      rawShellLogAllowed: false,
      providerPayloadAllowed: false,
      localPathAllowed: false,
      secretMaterialAllowed: false,
    },
    visibility: "public",
    publicProjectionAllowed: true,
    state: "completed",
    createdAt: at,
    updatedAt: at,
    adapterSessionRefs: [`session.public.${input.runId}`],
  }
}

function event(
  runId: string,
  sequence: number,
  tag: AgentRuntimeEvent["tag"],
  input: Partial<AgentRuntimeEvent> = {},
): AgentRuntimeEvent {
  return {
    tag,
    eventId: `event.public.${runId}.${sequence}`,
    runId,
    sequence,
    generatedAt: at,
    visibility: "public",
    redactionClass: "public_ref",
    refs: [],
    blockerRefs: [],
    ...input,
  }
}

export const fulfillmentLoopAgentDefinitionFixture: AgentDefinition = {
  schema: "openagents.agent_definition.v1",
  id: "agent_definition.public.fulfillment_loop.daily_motion",
  ownerRef: "owner.public.fixture",
  name: "Daily Fulfillment Motion",
  slug: "daily-fulfillment-motion",
  goal: "Review the service promise state and produce one public-safe daily motion receipt.",
  harness: {
    kind: "codex",
    modelHint: "openagents/pylon-codex",
    versionPin: "fixture",
  },
  toolset: {
    allow: [
      "tool.openagents.promise.read",
      "tool.openagents.crm.read",
      "tool.openagents.receipt.write",
    ],
    deny: [
      "tool.openagents.payment.*",
      "tool.openagents.email.send",
    ],
    ask: [
      "tool.openagents.email.draft",
      "tool.openagents.stakeholder.page",
    ],
    networkPolicy: "owner_scoped",
    secretPolicy: "owner_scoped_refs_only",
  },
  triggers: [
    {
      kind: "cron",
      triggerRef: "trigger.public.fulfillment_loop.daily",
      expr: "0 14 * * *",
      tz: "UTC",
    },
  ],
  lane: "own_pylon",
  budget: {
    maxRunSeconds: 900,
    maxRunsPerDay: 1,
    maxCreditsPerDay: 0,
  },
  escalation: {
    channel: "operator",
    askPolicy: {
      policyRef: "policy.public.agent_definition.operator_required.v1",
      mode: "operator_required",
    },
  },
  sourceRefs: ["issue.public.github.OpenAgentsInc.openagents.8097"],
  createdAt: at,
  updatedAt: at,
}

export const allTriggerTypesAgentDefinitionFixture: AgentDefinition = {
  ...fulfillmentLoopAgentDefinitionFixture,
  id: "agent_definition.public.all_trigger_types",
  name: "All Trigger Types Fixture",
  slug: "all-trigger-types-fixture",
  triggers: agentDefinitionTriggerFixtures,
}

export const fixtureLoopEventLog: AgentRuntimeEventLog = {
  run: run({
    runId: "run.public.fixture_loop",
    adapterKind: "test_fixture",
    loopKind: "fixture_loop",
  }),
  events: [
    event("run.public.fixture_loop", 1, "run.started"),
    event("run.public.fixture_loop", 2, "run.input_accepted", {
      summary: "Fixture input accepted by ref.",
      refs: ["input.public.fixture_loop"],
    }),
    event("run.public.fixture_loop", 3, "step.started", { stepRef: "step.public.fixture_loop.1" }),
    event("run.public.fixture_loop", 4, "artifact.recorded", {
      artifact: {
        artifactRef: "artifact.public.fixture_loop.result",
        artifactKind: "summary",
        visibility: "public",
        digestRef: "digest.public.fixture_loop.result",
      },
    }),
    event("run.public.fixture_loop", 5, "run.completed"),
  ],
}

export const nativeModelLoopEventLog: AgentRuntimeEventLog = {
  run: run({
    runId: "run.public.native_model_loop",
    adapterKind: "openagents_native",
    loopKind: "native_model_loop",
  }),
  events: [
    event("run.public.native_model_loop", 1, "run.started"),
    event("run.public.native_model_loop", 2, "context.snapshot_created", {
      refs: ["context.public.native_model_loop.snapshot"],
    }),
    event("run.public.native_model_loop", 3, "model.stream_started", {
      stepRef: "step.public.native_model_loop.1",
    }),
    event("run.public.native_model_loop", 4, "model.text_delta", {
      part: { kind: "text", text: "public-safe delta" },
    }),
    event("run.public.native_model_loop", 5, "usage.recorded", {
      usage: {
        usageRef: "usage.public.native_model_loop",
        providerRef: "provider.public.test",
        modelRef: "model.public.test",
        inputTokens: 10,
        outputTokens: 4,
        totalTokens: 14,
      },
    }),
    event("run.public.native_model_loop", 6, "run.completed"),
  ],
}

export const externalAgentLoopEventLog: AgentRuntimeEventLog = {
  run: run({
    runId: "run.public.external_agent_loop",
    adapterKind: "codex",
    loopKind: "external_agent_loop",
  }),
  events: [
    event("run.public.external_agent_loop", 1, "run.started"),
    event("run.public.external_agent_loop", 2, "external_agent.started", {
      externalInvocation: {
        invocationId: "external.public.codex.1",
        adapterKind: "codex",
        sessionRef: "session.public.codex.1",
        status: "started",
        artifactRefs: [],
        blockerRefs: [],
      },
    }),
    event("run.public.external_agent_loop", 3, "external_agent.artifact_recorded", {
      externalInvocation: {
        invocationId: "external.public.codex.1",
        adapterKind: "codex",
        sessionRef: "session.public.codex.1",
        status: "artifact_recorded",
        artifactRefs: ["artifact.public.codex.closeout"],
        blockerRefs: [],
      },
    }),
    event("run.public.external_agent_loop", 4, "external_agent.completed", {
      externalInvocation: {
        invocationId: "external.public.codex.1",
        adapterKind: "codex",
        sessionRef: "session.public.codex.1",
        status: "completed",
        artifactRefs: ["artifact.public.codex.closeout"],
        blockerRefs: [],
      },
    }),
    event("run.public.external_agent_loop", 5, "run.completed"),
  ],
}

export const hostedLoopEventLog: AgentRuntimeEventLog = {
  run: run({
    runId: "run.public.hosted_loop",
    adapterKind: "hosted_container",
    loopKind: "hosted_loop",
  }),
  events: [
    event("run.public.hosted_loop", 1, "run.started"),
    event("run.public.hosted_loop", 2, "external_agent.started", {
      externalInvocation: {
        invocationId: "external.public.hosted.1",
        adapterKind: "hosted_container",
        sessionRef: "session.public.hosted.1",
        status: "started",
        artifactRefs: [],
        blockerRefs: [],
      },
    }),
    event("run.public.hosted_loop", 3, "run.completed"),
  ],
}

export const agentRuntimeFixtureEventLogs: ReadonlyArray<AgentRuntimeEventLog> = [
  fixtureLoopEventLog,
  nativeModelLoopEventLog,
  externalAgentLoopEventLog,
  hostedLoopEventLog,
]

export const khalaRuntimeToolAuthorityFixture: KhalaRuntimeToolAuthority = {
  authorityRef: "authority.public.fixture.allow_read",
  policyRef: "policy.public.fixture.tool_read",
  decisionRef: "decision.public.fixture.tool_read",
  toolRef: "tool.openagents.workspace.read",
  status: "allowed",
  allowed: true,
  blockerRefs: [],
}

export const khalaRuntimeAiSdkTextDeltaEventFixture: KhalaRuntimeEvent = {
  schema: "openagents.khala_runtime_event.v1",
  eventId: "event.public.fixture.ai_sdk.text_delta.1",
  turnId: "turn.public.fixture.ai_sdk.1",
  threadId: "thread.public.fixture.ai_sdk",
  sequence: 1,
  observedAt: at,
  source: {
    lane: "ai_sdk_core",
    surface: "server",
    providerRef: "provider.public.fixture",
    modelRef: "model.public.fixture",
  },
  visibility: "public",
  redactionClass: "public_ref",
  causalityRefs: [],
  kind: "text.delta",
  messageId: "message.public.fixture.ai_sdk.1",
  chunkId: "chunk.public.fixture.ai_sdk.1",
  text: "public-safe delta",
}

export const khalaRuntimeCodexToolCallEventFixture: KhalaRuntimeEvent = {
  schema: "openagents.khala_runtime_event.v1",
  eventId: "event.public.fixture.codex.tool_call.1",
  turnId: "turn.public.fixture.codex.1",
  threadId: "thread.public.fixture.codex",
  sequence: 2,
  observedAt: at,
  source: {
    lane: "codex_app_server",
    adapterKind: "codex",
    surface: "desktop",
    adapterSessionRef: "session.public.fixture.codex.1",
  },
  visibility: "public",
  redactionClass: "public_ref",
  causalityRefs: ["event.public.fixture.codex.text.1"],
  kind: "tool.call",
  toolCallId: "tool_call.public.fixture.codex.1",
  toolName: "workspaceRead",
  inputRef: "input.private.fixture.codex.tool_call.1",
  authority: khalaRuntimeToolAuthorityFixture,
}

export const khalaRuntimeRawSidecarEventFixture: KhalaRuntimeEvent = {
  schema: "openagents.khala_runtime_event.v1",
  eventId: "event.private.fixture.ai_sdk.raw.1",
  turnId: "turn.public.fixture.ai_sdk.1",
  threadId: "thread.public.fixture.ai_sdk",
  sequence: 3,
  observedAt: at,
  source: {
    lane: "ai_sdk_core",
    surface: "server",
  },
  visibility: "private",
  redactionClass: "private_ref",
  causalityRefs: [],
  kind: "raw.sidecar_ref",
  rawEventRef: "raw.private.fixture.ai_sdk.1",
  rawEventKind: "ai_sdk_stream_part",
}

export const khalaRuntimeWritebackRecordedEventFixture: KhalaRuntimeEvent = {
  schema: "openagents.khala_runtime_event.v1",
  eventId: "event.public.fixture.writeback.1",
  turnId: "turn.public.fixture.ai_sdk.1",
  threadId: "thread.public.fixture.ai_sdk",
  sequence: 4,
  observedAt: at,
  source: {
    lane: "codex_app_server",
    adapterKind: "codex",
    surface: "server",
  },
  visibility: "private",
  redactionClass: "private_ref",
  causalityRefs: ["event.public.fixture.codex.tool_call.1"],
  kind: "writeback.recorded",
  writebackRef: "writeback.public.fixture.pr_8477",
  repositoryFullName: "OpenAgentsInc/openagents",
  branch: "pylon/assignment-issue-8477",
  branchUrl: "https://github.com/OpenAgentsInc/openagents/tree/pylon/assignment-issue-8477",
  pullRequestUrl: "https://github.com/OpenAgentsInc/openagents/pull/8477",
  pullRequestNumber: 8477,
  changedFileCount: 3,
  status: "pull_request_opened",
}

export const khalaRuntimeMobileMessageAppendIntentFixture: KhalaRuntimeControlIntent = {
  schema: "openagents.khala_runtime_control_intent.v1",
  intentId: "intent.private.fixture.mobile.message_append.1",
  kind: "message.append",
  threadId: "thread.public.fixture.ai_sdk",
  messageId: "message.private.fixture.mobile.1",
  createdAt: at,
  origin: {
    surface: "mobile",
    lane: "khala_sync_mobile_control",
    deviceRef: "device.private.fixture.mobile.1",
    userRef: "user.private.fixture.operator.1",
  },
  target: {
    lane: "codex_app_server",
    adapterKind: "codex",
  },
  visibility: "private",
  redactionClass: "private_ref",
  idempotencyKey: "idem.private.fixture.mobile.message_append.1",
  causalityRefs: [],
  bodyRef: "message_body.private.fixture.mobile.1",
  promptRef: "prompt.private.fixture.mobile.1",
}

export const khalaRuntimeEventFixtures: ReadonlyArray<KhalaRuntimeEvent> = [
  khalaRuntimeAiSdkTextDeltaEventFixture,
  khalaRuntimeCodexToolCallEventFixture,
  khalaRuntimeWritebackRecordedEventFixture,
  khalaRuntimeRawSidecarEventFixture,
]

export const khalaRuntimeControlIntentFixtures: ReadonlyArray<KhalaRuntimeControlIntent> = [
  khalaRuntimeMobileMessageAppendIntentFixture,
]
