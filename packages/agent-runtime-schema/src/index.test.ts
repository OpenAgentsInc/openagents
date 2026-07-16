import { describe, expect, test } from "vite-plus/test";
import {
  AgentDefinition,
  AgentRuntimeEvent,
  AgentRuntimeRun,
  PylonLifecycleWireEventFromJsonString,
  decodePylonLifecycleWireEventJson,
  encodePylonAssignmentRunLifecycleEvent,
  encodePylonKhalaSpawnWorkerEvent,
  agentRuntimeAdapterKinds,
  agentRuntimeEventTags,
  agentRuntimeLoopKinds,
  agentRuntimeRedactionClasses,
  agentRuntimeVisibilities,
  assertAgentRuntimeEventLogSafe,
  assertAgentRuntimePublicEventSafe,
  assertAgentRuntimeRunStateTransition,
  assertKhalaRuntimeControlIntentSafe,
  assertKhalaRuntimePublicEventSafe,
  agentRuntimeSurfaceStatusHasUnsafeMaterial,
  compileAgentDefinitionToolRuntimePolicy,
  decideAgentDefinitionCompiledToolAuthority,
  decideAgentDefinitionToolAuthority,
  decodeAgentDefinition,
  decodeAgentDefinitionTriggerRecord,
  decodeAgentRuntimeEvent,
  decodeAgentRuntimeEventLog,
  decodeAgentRuntimeRun,
  decodeKhalaRuntimeControlIntent,
  decodeKhalaRuntimeEvent,
  khalaRuntimeControlIntentKinds,
  khalaRuntimeEventFromAgentRuntimeEvent,
  khalaRuntimeEventFromAiSdkTextStreamPart,
  khalaRuntimeEventKinds,
  khalaRuntimeLanes,
  projectAgentRuntimeSurfaceStatus,
} from "./index.js";
import {
  agentDefinitionTriggerFixtures,
  allTriggerTypesAgentDefinitionFixture,
  agentRuntimeFixtureEventLogs,
  fulfillmentLoopAgentDefinitionFixture,
  khalaRuntimeControlIntentFixtures,
  khalaRuntimeEventFixtures,
  khalaRuntimeToolAuthorityFixture,
} from "./fixtures.js";

// Behavior contract oracle: background_agents.toolset.compiled_policy_enforced.v1

const baseRun = {
  runId: "run.public.schema_test",
  assignmentId: "assignment.public.schema_test",
  workspaceRef: "workspace.public.schema_test",
  adapterKind: "test_fixture",
  loopKind: "fixture_loop",
  sourceRefs: ["source.public.schema_test"],
  budgetRef: "budget.public.schema_test",
  usagePolicy: "usage.policy.public.schema_test",
  permissionPolicy: "permission.policy.public.schema_test",
  redactionPolicy: {
    policyRef: "redaction.policy.public.schema_test",
    rawPromptAllowed: false,
    rawShellLogAllowed: false,
    providerPayloadAllowed: false,
    localPathAllowed: false,
    secretMaterialAllowed: false,
  },
  visibility: "public",
  publicProjectionAllowed: true,
  state: "pending",
  createdAt: "2026-06-11T00:00:00.000Z",
  updatedAt: "2026-06-11T00:00:00.000Z",
  adapterSessionRefs: [],
};

const baseEvent = {
  eventId: "event.public.schema_test.1",
  runId: "run.public.schema_test",
  sequence: 1,
  generatedAt: "2026-06-11T00:00:00.000Z",
  visibility: "public",
  redactionClass: "public_ref",
  refs: [],
  blockerRefs: [],
};

const baseKhalaRuntimeEvent = {
  schema: "openagents.khala_runtime_event.v1",
  eventId: "event.public.schema_test.khala.1",
  turnId: "turn.public.schema_test.khala.1",
  threadId: "thread.public.schema_test.khala",
  sequence: 1,
  observedAt: "2026-06-11T00:00:00.000Z",
  source: {
    lane: "test_fixture",
    surface: "test_fixture",
  },
  visibility: "public",
  redactionClass: "public_ref",
  causalityRefs: [],
};

describe("@openagentsinc/agent-runtime-schema", () => {
  test("decodes every adapter kind, loop kind, redaction class, and visibility", () => {
    for (const adapterKind of agentRuntimeAdapterKinds) {
      expect(decodeAgentRuntimeRun({ ...baseRun, adapterKind })).toMatchObject({ adapterKind });
    }
    for (const loopKind of agentRuntimeLoopKinds) {
      expect(decodeAgentRuntimeRun({ ...baseRun, loopKind })).toMatchObject({ loopKind });
    }
    for (const redactionClass of agentRuntimeRedactionClasses) {
      expect(
        decodeAgentRuntimeEvent({ ...baseEvent, tag: "run.started", redactionClass }),
      ).toMatchObject({ redactionClass });
    }
    for (const visibility of agentRuntimeVisibilities) {
      expect(
        decodeAgentRuntimeEvent({ ...baseEvent, tag: "run.started", visibility }),
      ).toMatchObject({ visibility });
    }
  });

  test("decodes agent_definition.v1 records and definition-backed runtime runs", () => {
    const definition = decodeAgentDefinition(fulfillmentLoopAgentDefinitionFixture);
    expect(definition).toMatchObject({
      schema: "openagents.agent_definition.v1",
      id: "agent_definition.public.fulfillment_loop.daily_motion",
      harness: { kind: "codex" },
      triggers: [
        {
          kind: "cron",
          triggerRef: "trigger.public.fulfillment_loop.daily",
          expr: "0 14 * * *",
          tz: "UTC",
        },
      ],
      lane: "own_pylon",
      escalation: {
        channel: "operator",
        askPolicy: { mode: "operator_required" },
      },
    });

    expect(
      decodeAgentRuntimeRun({
        ...baseRun,
        agentDefinitionId: definition.id,
        state: "running",
      }),
    ).toMatchObject({
      agentDefinitionId: definition.id,
      state: "running",
    });
  });

  test("decodes cron and inbound-webhook trigger records", () => {
    const definition = decodeAgentDefinition({
      ...fulfillmentLoopAgentDefinitionFixture,
      triggers: [
        {
          kind: "cron",
          triggerRef: "trigger.public.fulfillment_loop.hourly",
          expr: "15 * * * *",
          tz: "America/Chicago",
        },
        {
          kind: "inbound_webhook",
          triggerRef: "trigger.public.github.issue_opened",
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
      ],
    });

    expect(definition.triggers).toHaveLength(2);
    expect(definition.triggers[1]).toMatchObject({
      kind: "inbound_webhook",
      source: "github",
      conditions: [
        { kind: "event_type", equals: "issues.opened" },
        {
          kind: "json_path_equals",
          path: "$.repository.full_name",
          equals: "OpenAgentsInc/openagents",
        },
        { kind: "json_path_matches", path: "$.issue.title", pattern: "^BA-" },
        {
          kind: "json_path_in",
          path: "$.issue.labels[*].name",
          values: ["background-agents", "fable"],
        },
      ],
    });

    expect(
      decodeAgentDefinitionTriggerRecord({
        schema: "openagents.agent_definition_trigger.v1",
        triggerId: "agent_definition_trigger.public.hourly",
        ownerRef: definition.ownerRef,
        definitionId: definition.id,
        triggerRef: "trigger.public.fulfillment_loop.hourly",
        trigger: definition.triggers[0],
        state: "enabled",
        consecutiveFailures: 0,
        nextRunAt: "2026-07-03T16:15:00.000Z",
        createdAt: "2026-07-03T16:00:00.000Z",
        updatedAt: "2026-07-03T16:00:00.000Z",
      }),
    ).toMatchObject({
      schema: "openagents.agent_definition_trigger.v1",
      state: "enabled",
      consecutiveFailures: 0,
      nextRunAt: "2026-07-03T16:15:00.000Z",
    });
  });

  test("decodes reusable fixtures for every definition trigger type", () => {
    const definition = decodeAgentDefinition(allTriggerTypesAgentDefinitionFixture);

    expect(definition.triggers.map((trigger) => trigger.kind).sort()).toEqual([
      "cron",
      "inbound_webhook",
      "inbox_match",
      "manual",
    ]);
    expect(definition.triggers).toEqual(agentDefinitionTriggerFixtures);
    expect(definition.triggers.find((trigger) => trigger.kind === "inbox_match")).toMatchObject({
      classifierRef: "classifier.public.fixture.priority_inbox",
      triggerRef: "trigger.public.fixture.inbox.priority",
    });
    expect(definition.triggers.find((trigger) => trigger.kind === "manual")).toMatchObject({
      triggerRef: "trigger.public.fixture.manual.run_now",
    });
  });

  test("enforces definition toolsets with deny, ask escalation, allow, and deny-by-default", () => {
    const definition = decodeAgentDefinition(fulfillmentLoopAgentDefinitionFixture);
    const compiledPolicy = compileAgentDefinitionToolRuntimePolicy(definition);

    expect(compiledPolicy).toMatchObject({
      schema: "openagents.agent_definition_tool_runtime_policy.v1",
      definitionId: definition.id,
      ownerRef: definition.ownerRef,
      defaultDecision: "deny",
      networkPolicy: "owner_scoped",
      secretPolicy: "owner_scoped_refs_only",
      escalation: {
        askPolicyRef: "policy.public.agent_definition.operator_required.v1",
        channel: "operator",
      },
    });

    expect(
      decideAgentDefinitionToolAuthority({
        definition,
        toolRef: "tool.openagents.crm.read",
      }),
    ).toMatchObject({
      allowed: true,
      status: "allowed",
      reasonRef: "reason.agent_definition.tool_allowed",
      blockerRefs: [],
    });
    expect(
      decideAgentDefinitionCompiledToolAuthority({
        policy: compiledPolicy,
        toolRef: "tool.openagents.crm.read",
      }),
    ).toMatchObject({
      allowed: true,
      status: "allowed",
      matchedPolicyRef: "tool.openagents.crm.read",
    });

    expect(
      decideAgentDefinitionToolAuthority({
        definition,
        toolRef: "tool.openagents.payment.refund",
      }),
    ).toMatchObject({
      allowed: false,
      status: "denied",
      matchedPolicyRef: "tool.openagents.payment.*",
      blockerRefs: ["blocker.agent_definition.tool_denied"],
    });
    expect(
      decideAgentDefinitionCompiledToolAuthority({
        policy: compiledPolicy,
        toolRef: "tool.openagents.payment.refund",
      }),
    ).toMatchObject({
      allowed: false,
      status: "denied",
      matchedPolicyRef: "tool.openagents.payment.*",
      reasonRef: "reason.agent_definition.tool_denied",
    });

    const askDecision = decideAgentDefinitionToolAuthority({
      definition,
      toolRef: "tool.openagents.email.draft",
      invocationRef: "invocation.public.fixture.email_draft",
    });
    expect(askDecision).toMatchObject({
      allowed: false,
      status: "operator_escalation_required",
      blockerRefs: ["blocker.agent_definition.operator_escalation_required"],
      escalation: {
        definitionId: definition.id,
        ownerRef: definition.ownerRef,
        toolRef: "tool.openagents.email.draft",
        channel: "operator",
        askPolicyRef: "policy.public.agent_definition.operator_required.v1",
        reasonRef: "reason.agent_definition.ask_policy_hit",
      },
    });
    expect(askDecision.escalation?.escalationRef).toMatch(
      /^escalation\.operator\.agent_definition\.[a-f0-9]{8}$/,
    );
    const compiledAskDecision = decideAgentDefinitionCompiledToolAuthority({
      policy: compiledPolicy,
      toolRef: "tool.openagents.email.draft",
      invocationRef: "invocation.public.fixture.email_draft",
    });
    expect(compiledAskDecision).toMatchObject({
      allowed: false,
      status: "operator_escalation_required",
      escalation: {
        definitionId: definition.id,
        ownerRef: definition.ownerRef,
        askPolicyRef: "policy.public.agent_definition.operator_required.v1",
        channel: "operator",
      },
    });
    expect(compiledAskDecision.escalation?.escalationRef).toMatch(
      /^escalation\.operator\.agent_definition\.[a-f0-9]{8}$/,
    );

    expect(
      decideAgentDefinitionToolAuthority({
        definition,
        toolRef: "tool.openagents.github.write",
      }),
    ).toMatchObject({
      allowed: false,
      status: "denied",
      reasonRef: "reason.agent_definition.tool_not_in_allowlist",
      blockerRefs: ["blocker.agent_definition.tool_not_in_allowlist"],
    });
    expect(
      decideAgentDefinitionCompiledToolAuthority({
        policy: compiledPolicy,
        toolRef: "tool.openagents.github.write",
      }),
    ).toMatchObject({
      allowed: false,
      status: "denied",
      reasonRef: "reason.agent_definition.tool_not_in_allowlist",
    });
  });

  test("gives deny policy precedence over overlapping ask and allow entries", () => {
    const definition = decodeAgentDefinition({
      ...fulfillmentLoopAgentDefinitionFixture,
      toolset: {
        ...fulfillmentLoopAgentDefinitionFixture.toolset,
        allow: ["tool.openagents.email.draft"],
        ask: ["tool.openagents.email.draft"],
        deny: ["tool.openagents.email.*"],
      },
    });

    expect(
      decideAgentDefinitionToolAuthority({
        definition,
        toolRef: "tool.openagents.email.draft",
      }),
    ).toMatchObject({
      allowed: false,
      status: "denied",
      matchedPolicyRef: "tool.openagents.email.*",
      reasonRef: "reason.agent_definition.tool_denied",
      blockerRefs: ["blocker.agent_definition.tool_denied"],
    });
  });

  test("decodes every RK1 event tag", () => {
    expect(agentRuntimeEventTags).toHaveLength(32);
    for (const tag of agentRuntimeEventTags) {
      expect(decodeAgentRuntimeEvent({ ...baseEvent, tag })).toMatchObject({ tag });
    }
  });

  test("decodes reusable fixture logs for every loop kind", () => {
    const decoded = agentRuntimeFixtureEventLogs.map((log) => decodeAgentRuntimeEventLog(log));
    expect(decoded.map((log) => log.run.loopKind).sort()).toEqual([
      "external_agent_loop",
      "fixture_loop",
      "hosted_loop",
      "native_model_loop",
    ]);
    for (const log of decoded) {
      expect(log.events[0]?.tag).toBe("run.started");
      expect(log.events.at(-1)?.tag).toBe("run.completed");
      expect(assertAgentRuntimeEventLogSafe(log)).toBe(log);
    }
  });

  test("checks legal and illegal run lifecycle transitions", () => {
    expect(assertAgentRuntimeRunStateTransition("pending", "running")).toBe("running");
    expect(assertAgentRuntimeRunStateTransition("running", "paused")).toBe("paused");
    expect(assertAgentRuntimeRunStateTransition("paused", "running")).toBe("running");
    expect(assertAgentRuntimeRunStateTransition("running", "completed")).toBe("completed");
    expect(() => assertAgentRuntimeRunStateTransition("completed", "running")).toThrow(
      "Illegal AgentRuntimeRun state transition",
    );
    expect(() => assertAgentRuntimeRunStateTransition("cancelled", "completed")).toThrow(
      "Illegal AgentRuntimeRun state transition",
    );
  });

  test("rejects raw prompts, shell logs, provider payloads, secrets, and local paths in public events", () => {
    const unsafeEvents = [
      { ...baseEvent, tag: "model.text_delta", summary: "raw_prompt: fix this private repo" },
      { ...baseEvent, tag: "tool.failed", summary: "raw_shell_log: stack trace" },
      { ...baseEvent, tag: "external_agent.event", summary: "provider_payload included" },
      { ...baseEvent, tag: "external_agent.failed", summary: "secret sk-test" },
      { ...baseEvent, tag: "artifact.recorded", refs: ["/Users/example/private-source"] },
    ];
    for (const unsafeEvent of unsafeEvents) {
      const decoded = decodeAgentRuntimeEvent(unsafeEvent);
      expect(() => assertAgentRuntimePublicEventSafe(decoded)).toThrow(
        "Agent runtime public event contains raw/private material",
      );
    }
  });

  test("has no provider SDK or Vercel AI SDK fields in the durable schema shape", () => {
    const schemas = JSON.stringify([
      AgentRuntimeRun.ast,
      AgentRuntimeEvent.ast,
      AgentDefinition.ast,
    ]);
    expect(schemas).not.toContain("@anthropic-ai");
    expect(schemas).not.toContain("@openai/codex-sdk");
    expect(schemas).not.toContain("ai-sdk");
    expect(schemas).not.toContain("providerEvent");
    expect(schemas).not.toContain("sdkMessage");
  });

  test("decodes Khala runtime lanes, event kinds, control kinds, and golden fixtures", () => {
    expect(khalaRuntimeLanes).toEqual([
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
    expect(khalaRuntimeEventKinds).toHaveLength(23);
    expect(khalaRuntimeControlIntentKinds).toHaveLength(10);

    const decodedEvents = khalaRuntimeEventFixtures.map((event) => decodeKhalaRuntimeEvent(event));
    expect(decodedEvents.map((event) => event.kind)).toEqual([
      "text.delta",
      "tool.call",
      "writeback.recorded",
      "raw.sidecar_ref",
    ]);
    for (const event of decodedEvents) {
      expect(assertKhalaRuntimePublicEventSafe(event)).toBe(event);
    }

    const decodedIntents = khalaRuntimeControlIntentFixtures.map((intent) =>
      decodeKhalaRuntimeControlIntent(intent),
    );
    expect(decodedIntents.map((intent) => intent.kind)).toEqual(["message.append"]);
    for (const intent of decodedIntents) {
      expect(assertKhalaRuntimeControlIntentSafe(intent)).toBe(intent);
    }

    const accountTargetIntent = decodeKhalaRuntimeControlIntent({
      ...khalaRuntimeControlIntentFixtures[0],
      target: {
        executionTargetId: "codex:owner-account-ref-hash",
        lane: "codex_app_server",
      },
    });
    expect(accountTargetIntent.target).toEqual({
      executionTargetId: "codex:owner-account-ref-hash",
      lane: "codex_app_server",
    });

    const child = decodeKhalaRuntimeEvent({
      ...baseKhalaRuntimeEvent,
      eventId: "event.child.started.1",
      kind: "agent.child.started",
      childAgentId: "child.claude.task.1",
      childRunId: "run.child.claude.task.1",
      parentAgentId: "turn.public.schema_test.khala.1",
      taskRef: "task.claude.tool.1",
      childKindRef: "agent_kind.claude.general_purpose",
      description: "raw task description must not enter the contract",
    });
    expect(child).toMatchObject({
      kind: "agent.child.started",
      childAgentId: "child.claude.task.1",
      parentAgentId: "turn.public.schema_test.khala.1",
    });
    expect(JSON.stringify(child)).not.toContain("raw task description");
  });

  test("maps existing AgentRuntime events and AI SDK TextStreamPart fixtures into the Khala event schema", () => {
    const agentTextEvent = decodeAgentRuntimeEvent({
      ...baseEvent,
      tag: "model.text_delta",
      eventId: "event.public.schema_test.agent_text.1",
      sequence: 4,
      part: {
        kind: "text",
        text: "public-safe delta",
      },
    });

    const fromAgentRuntime = khalaRuntimeEventFromAgentRuntimeEvent({
      event: agentTextEvent,
      threadId: "thread.public.schema_test.shared",
      turnId: "turn.public.schema_test.shared.1",
      source: {
        lane: "codex_app_server",
        adapterKind: "codex",
        surface: "desktop",
      },
    });

    const fromAiSdk = khalaRuntimeEventFromAiSdkTextStreamPart({
      part: {
        type: "text-delta",
        id: "message.public.schema_test.shared.1",
        text: "public-safe delta",
      },
      eventId: "event.public.schema_test.ai_sdk_text.1",
      threadId: "thread.public.schema_test.shared",
      turnId: "turn.public.schema_test.shared.1",
      sequence: 4,
      observedAt: "2026-06-11T00:00:00.000Z",
      source: {
        lane: "ai_sdk_core",
        surface: "server",
      },
    });

    expect(fromAgentRuntime).toMatchObject({
      schema: "openagents.khala_runtime_event.v1",
      kind: "text.delta",
      text: "public-safe delta",
      threadId: "thread.public.schema_test.shared",
      turnId: "turn.public.schema_test.shared.1",
    });
    expect(fromAiSdk).toMatchObject({
      schema: "openagents.khala_runtime_event.v1",
      kind: "text.delta",
      text: "public-safe delta",
      threadId: "thread.public.schema_test.shared",
      turnId: "turn.public.schema_test.shared.1",
    });

    const raw = khalaRuntimeEventFromAiSdkTextStreamPart({
      part: {
        type: "raw",
        rawValue: {
          provider_payload: "never copied into public evidence",
        },
      },
      eventId: "event.private.schema_test.ai_sdk_raw.1",
      threadId: "thread.public.schema_test.shared",
      turnId: "turn.public.schema_test.shared.1",
      sequence: 5,
      observedAt: "2026-06-11T00:00:00.000Z",
      rawEventRef: "raw.private.schema_test.ai_sdk_raw.1",
    });

    expect(raw).toMatchObject({
      kind: "raw.sidecar_ref",
      visibility: "private",
      redactionClass: "private_ref",
      rawEventRef: "raw.private.schema_test.ai_sdk_raw.1",
    });
    expect(JSON.stringify(raw)).not.toContain("provider_payload");
  });

  test("requires authority on Khala runtime tool events before execution", () => {
    const toolEvent = decodeAgentRuntimeEvent({
      ...baseEvent,
      tag: "tool.started",
      eventId: "event.public.schema_test.tool.1",
      toolInvocation: {
        invocationId: "tool_call.public.schema_test.1",
        toolName: "workspaceRead",
        toolRef: "tool.openagents.workspace.read",
        inputRef: "input.private.schema_test.tool.1",
        status: "started",
        blockerRefs: [],
      },
    });

    expect(() =>
      khalaRuntimeEventFromAgentRuntimeEvent({
        event: toolEvent,
        threadId: "thread.public.schema_test.tools",
        turnId: "turn.public.schema_test.tools.1",
        source: {
          lane: "codex_app_server",
          adapterKind: "codex",
          surface: "desktop",
        },
      }),
    ).toThrow("Khala runtime tool event requires authority");

    expect(
      khalaRuntimeEventFromAgentRuntimeEvent({
        event: toolEvent,
        threadId: "thread.public.schema_test.tools",
        turnId: "turn.public.schema_test.tools.1",
        source: {
          lane: "codex_app_server",
          adapterKind: "codex",
          surface: "desktop",
        },
        authority: khalaRuntimeToolAuthorityFixture,
      }),
    ).toMatchObject({
      kind: "tool.call",
      authority: khalaRuntimeToolAuthorityFixture,
    });

    expect(() =>
      decodeKhalaRuntimeEvent({
        ...baseKhalaRuntimeEvent,
        kind: "tool.call",
        toolCallId: "tool_call.public.schema_test.2",
        toolName: "workspaceRead",
      }),
    ).toThrow();

    expect(() =>
      khalaRuntimeEventFromAiSdkTextStreamPart({
        part: {
          type: "tool-call",
          toolCallId: "tool_call.public.schema_test.3",
          toolName: "workspaceRead",
          input: {},
        },
        eventId: "event.public.schema_test.ai_sdk_tool.1",
        threadId: "thread.public.schema_test.tools",
        turnId: "turn.public.schema_test.tools.1",
        sequence: 2,
        observedAt: "2026-06-11T00:00:00.000Z",
      }),
    ).toThrow("Khala runtime tool event requires authority");
  });

  test("keeps Khala runtime public events and operator controls free of raw private material", () => {
    const unsafePublicEvent = decodeKhalaRuntimeEvent({
      ...baseKhalaRuntimeEvent,
      kind: "text.delta",
      messageId: "message.public.schema_test.unsafe.1",
      chunkId: "chunk.public.schema_test.unsafe.1",
      text: "raw_prompt /Users/example/private-source",
    });
    expect(() => assertKhalaRuntimePublicEventSafe(unsafePublicEvent)).toThrow(
      "Khala runtime public event contains raw/private material",
    );

    expect(() =>
      decodeKhalaRuntimeControlIntent({
        schema: "openagents.khala_runtime_control_intent.v1",
        intentId: "intent.public.schema_test.invalid.1",
        kind: "message.append",
        threadId: "thread.public.schema_test.khala",
        createdAt: "2026-06-11T00:00:00.000Z",
        origin: {
          surface: "mobile",
          lane: "khala_sync_mobile_control",
        },
        target: {
          lane: "codex_app_server",
          adapterKind: "codex",
        },
        visibility: "public",
        redactionClass: "public_ref",
        idempotencyKey: "idem.public.schema_test.invalid.1",
        causalityRefs: [],
      }),
    ).toThrow();

    const unsafeOperatorIntent = decodeKhalaRuntimeControlIntent({
      schema: "openagents.khala_runtime_control_intent.v1",
      intentId: "intent.operator.schema_test.unsafe.1",
      kind: "message.append",
      threadId: "thread.public.schema_test.khala",
      createdAt: "2026-06-11T00:00:00.000Z",
      origin: {
        surface: "mobile",
        lane: "khala_sync_mobile_control",
      },
      target: {
        lane: "codex_app_server",
        adapterKind: "codex",
      },
      visibility: "operator",
      redactionClass: "operator_summary",
      idempotencyKey: "idem.operator.schema_test.unsafe.1",
      causalityRefs: [],
      body: "raw_prompt should live behind promptRef",
    });
    expect(() => assertKhalaRuntimeControlIntentSafe(unsafeOperatorIntent)).toThrow(
      "Khala runtime operator control intent contains raw/private material",
    );
  });

  test("projects one public-safe status row for workroom and TUI surfaces", () => {
    const row = projectAgentRuntimeSurfaceStatus({
      runId: "run.public.schema_test",
      state: "failed",
      generatedAt: "2026-06-11T00:00:00.000Z",
      eventCount: 7,
      artifactRefs: ["artifact.public.schema_test.patch"],
      blockerRefs: ["blocker.agent_runtime.test_fixture.failed"],
      latestEventId: "event.public.schema_test.7",
      staleness: {
        maxStalenessSeconds: 0,
        transitionRefs: ["agent_runtime_event_ingested"],
      },
    });

    expect(row).toMatchObject({
      runId: "run.public.schema_test",
      status: "failed",
      label: "Failed",
      eventCount: 7,
      freshness: {
        generatedAt: "2026-06-11T00:00:00.000Z",
        maxStalenessSeconds: 0,
        transitionRefs: ["agent_runtime_event_ingested"],
      },
      verificationRefs: ["artifact.public.schema_test.patch"],
      reviewActionRefs: ["review.public.agent_runtime.blocker.agent_runtime.test_fixture.failed"],
    });
    expect(agentRuntimeSurfaceStatusHasUnsafeMaterial(row)).toBe(false);
  });

  test("round-trips Pylon lifecycle wire events through shared JSON string schemas", () => {
    const assignmentEvent = encodePylonAssignmentRunLifecycleEvent({
      schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
      event: "assignment_run.runtime_progress",
      observedAt: "2026-07-01T00:00:00.000Z",
      assignmentRef: "assignment.public.schema_test",
      leaseRef: "lease.public.schema_test",
      elapsedMs: 1200,
      phase: "runtime_active",
      tokenCountKind: "estimated",
      tokensSoFar: 42,
    });
    const workerEvent = encodePylonKhalaSpawnWorkerEvent({
      schema: "openagents.pylon.khala_spawn_worker_event.v0.1",
      assignmentEvent: "assignment_run.completed",
      assignmentRef: "assignment.public.schema_test",
      leaseRef: "lease.public.schema_test",
      message: "assignment lifecycle event",
      observedAt: "2026-07-01T00:00:01.000Z",
      slotIndex: 0,
      state: "accepted",
      status: "accepted",
    });

    expect(decodePylonLifecycleWireEventJson(JSON.stringify(assignmentEvent))).toEqual(
      assignmentEvent,
    );
    expect(decodePylonLifecycleWireEventJson(JSON.stringify(workerEvent))).toEqual(workerEvent);
    expect(JSON.stringify(PylonLifecycleWireEventFromJsonString.ast)).toContain(
      "openagents.pylon.assignment_run_lifecycle_event.v0.1",
    );
  });

  test("rejects malformed Pylon lifecycle wire events", () => {
    expect(() =>
      decodePylonLifecycleWireEventJson(
        JSON.stringify({
          schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
          event: "assignment_run.unknown",
          observedAt: "2026-07-01T00:00:00.000Z",
        }),
      ),
    ).toThrow();
    expect(() =>
      decodePylonLifecycleWireEventJson(
        JSON.stringify({
          schema: "openagents.pylon.khala_spawn_worker_event.v0.1",
          message: "assignment lifecycle event",
          observedAt: "2026-07-01T00:00:00.000Z",
          slotIndex: 0,
          state: "mystery",
        }),
      ),
    ).toThrow();
  });
});
