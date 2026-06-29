import type {
  AgentRuntimeAdapterKind,
  AgentRuntimeEvent,
  AgentRuntimeEventLog,
  AgentRuntimeLoopKind,
  AgentRuntimeRun,
} from "./index.js"

const at = "2026-06-11T00:00:00.000Z"

function run(input: {
  runId: string
  adapterKind: AgentRuntimeAdapterKind
  loopKind: AgentRuntimeLoopKind
}): AgentRuntimeRun {
  return {
    runId: input.runId,
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
