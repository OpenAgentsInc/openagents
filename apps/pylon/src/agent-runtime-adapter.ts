import { createHash } from "node:crypto"

import {
  assertAgentRuntimePublicEventSafe,
  type AgentRuntimeAdapterKind,
  type AgentRuntimeEvent,
  type AgentRuntimeEventId,
  type AgentRuntimeRunId,
} from "@openagentsinc/agent-runtime-schema"
import { Effect, Stream } from "effect"

import {
  agentRunnerForAdapterKind,
  type AgentRunnerExecutionOptions,
} from "./agent-runner-registry.js"
import { runOpencodeStream, type OpencodeStreamCallbacks } from "./opencode-run.js"
import type { PylonAssignmentLease } from "./assignment.js"
import type { PylonLocalState } from "./state.js"

export type AgentRuntimeRunRequest = {
  runId: AgentRuntimeRunId
  lease: PylonAssignmentLease
  now: Date
  state: PylonLocalState
  opencodePath?: string
  opencodePrompt?: string
}

export class AgentRuntimeAdapterError extends Error {
  readonly kind = "AgentRuntimeAdapterError"

  constructor(
    message: string,
    readonly adapterKind: AgentRuntimeAdapterKind,
    readonly blockerRefs: ReadonlyArray<string> = [],
  ) {
    super(message)
  }
}

export interface AgentRuntimeAdapter {
  readonly kind: AgentRuntimeAdapterKind
  canRun(request: AgentRuntimeRunRequest): Effect.Effect<boolean, AgentRuntimeAdapterError>
  start(request: AgentRuntimeRunRequest): Stream.Stream<AgentRuntimeEvent, AgentRuntimeAdapterError>
  cancel(runId: AgentRuntimeRunId): Effect.Effect<void, AgentRuntimeAdapterError>
}

type RuntimeCloseoutRecord = {
  artifactRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  buildRefs: ReadonlyArray<string>
  message: string
  previewRefs: ReadonlyArray<string>
  proofRefs: ReadonlyArray<string>
  resultRefs: ReadonlyArray<string>
  runRefs: ReadonlyArray<string>
  status: "accepted" | "rejected" | "timed-out"
  summaryRefs: ReadonlyArray<string>
  testRefs: ReadonlyArray<string>
}

export type AgentRuntimeReplayProjection = {
  runId: AgentRuntimeRunId
  state: "running" | "paused" | "interrupted" | "cancelled" | "completed" | "failed"
  artifactRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  externalStatus: "idle" | "started" | "event" | "artifact_recorded" | "completed" | "failed"
  generatedAt: string
  eventCount: number
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function eventId(runId: AgentRuntimeRunId, sequence: number): AgentRuntimeEventId {
  return `event.public.${stableRef("agent_runtime", `${runId}:${sequence}`).slice("agent_runtime.".length)}`
}

function event(
  request: AgentRuntimeRunRequest,
  sequence: number,
  tag: AgentRuntimeEvent["tag"],
  input: Partial<AgentRuntimeEvent> = {},
): AgentRuntimeEvent {
  return assertAgentRuntimePublicEventSafe({
    tag,
    eventId: eventId(request.runId, sequence),
    runId: request.runId,
    sequence,
    generatedAt: request.now.toISOString(),
    visibility: "public",
    redactionClass: "public_ref",
    refs: [],
    blockerRefs: [],
    ...input,
  })
}

function externalInvocation(
  adapterKind: AgentRuntimeAdapterKind,
  request: AgentRuntimeRunRequest,
  status: NonNullable<AgentRuntimeEvent["externalInvocation"]>["status"],
  closeout?: RuntimeCloseoutRecord,
) {
  return {
    invocationId: stableRef("external.public.agent_runtime", `${adapterKind}:${request.lease.leaseRef}`),
    adapterKind,
    sessionRef: closeout?.runRefs[1],
    status,
    summary: closeout?.summaryRefs[0],
    artifactRefs: [...(closeout?.artifactRefs ?? [])],
    blockerRefs: [...(closeout?.blockerRefs ?? [])],
  }
}

function closeoutToEvents(
  adapterKind: AgentRuntimeAdapterKind,
  request: AgentRuntimeRunRequest,
  closeout: RuntimeCloseoutRecord | null,
): ReadonlyArray<AgentRuntimeEvent> {
  if (closeout === null) {
    return [
      event(request, 1, "run.started"),
      event(request, 2, "external_agent.failed", {
        blockerRefs: [`blocker.agent_runtime.${adapterKind}.unsupported_assignment`],
        externalInvocation: {
          invocationId: stableRef("external.public.agent_runtime", `${adapterKind}:${request.lease.leaseRef}`),
          adapterKind,
          status: "failed",
          artifactRefs: [],
          blockerRefs: [`blocker.agent_runtime.${adapterKind}.unsupported_assignment`],
        },
      }),
      event(request, 3, "run.failed", {
        blockerRefs: [`blocker.agent_runtime.${adapterKind}.unsupported_assignment`],
      }),
    ]
  }

  const terminalEvent =
    closeout.status === "accepted"
      ? event(request, 5, "run.completed", { refs: [...closeout.resultRefs] })
      : event(request, 5, "run.failed", { blockerRefs: [...closeout.blockerRefs], refs: [...closeout.resultRefs] })

  return [
    event(request, 1, "run.started"),
    event(request, 2, "external_agent.started", {
      externalInvocation: externalInvocation(adapterKind, request, "started", closeout),
      refs: [...closeout.runRefs],
    }),
    event(request, 3, "external_agent.artifact_recorded", {
      artifact: closeout.artifactRefs[0] === undefined
        ? undefined
        : {
            artifactRef: closeout.artifactRefs[0],
            artifactKind: "closeout",
            visibility: "public",
            digestRef: closeout.proofRefs[0],
            summary: closeout.summaryRefs[0],
          },
      externalInvocation: externalInvocation(adapterKind, request, "artifact_recorded", closeout),
      refs: [...closeout.proofRefs, ...closeout.buildRefs, ...closeout.previewRefs],
    }),
    event(request, 4, closeout.status === "accepted" ? "external_agent.completed" : "external_agent.failed", {
      blockerRefs: [...closeout.blockerRefs],
      externalInvocation: externalInvocation(
        adapterKind,
        request,
        closeout.status === "accepted" ? "completed" : "failed",
        closeout,
      ),
      refs: [...closeout.resultRefs, ...closeout.testRefs],
      summary: closeout.message,
    }),
    terminalEvent,
  ]
}

function cancelledEvents(adapterKind: AgentRuntimeAdapterKind, request: AgentRuntimeRunRequest) {
  const blockerRef = `blocker.agent_runtime.${adapterKind}.cancelled`
  return [
    event(request, 1, "run.started"),
    event(request, 2, "run.cancelled", {
      blockerRefs: [blockerRef],
      refs: [stableRef("cancel.public.agent_runtime", request.runId)],
    }),
  ]
}

function makeAdapter(input: {
  kind: AgentRuntimeAdapterKind
  canRun: (request: AgentRuntimeRunRequest) => boolean
  start: (request: AgentRuntimeRunRequest) => Promise<ReadonlyArray<AgentRuntimeEvent>>
}): AgentRuntimeAdapter {
  const cancelledRunIds = new Set<AgentRuntimeRunId>()

  return {
    kind: input.kind,
    canRun: (request) => Effect.succeed(input.canRun(request)),
    start: (request) =>
      Stream.unwrap(
        Effect.promise(async () => {
          if (cancelledRunIds.has(request.runId)) {
            return Stream.fromIterable(cancelledEvents(input.kind, request))
          }
          return Stream.fromIterable(await input.start(request))
        }),
      ),
    cancel: (runId) =>
      Effect.sync(() => {
        cancelledRunIds.add(runId)
      }),
  }
}

export function createClaudeCodeAgentRuntimeAdapter(
  options: AgentRunnerExecutionOptions = {},
): AgentRuntimeAdapter {
  return createRegisteredAgentRuntimeAdapter("claude_code", options)
}

export function createCodexAgentRuntimeAdapter(
  options: AgentRunnerExecutionOptions = {},
): AgentRuntimeAdapter {
  return createRegisteredAgentRuntimeAdapter("codex", options)
}

function createRegisteredAgentRuntimeAdapter(
  adapterKind: AgentRuntimeAdapterKind,
  options: AgentRunnerExecutionOptions,
): AgentRuntimeAdapter {
  const runner = agentRunnerForAdapterKind(adapterKind)
  if (runner === null) {
    throw new AgentRuntimeAdapterError(
      `No registered agent runner for ${adapterKind}.`,
      adapterKind,
      [`blocker.agent_runtime.${adapterKind}.runner_unregistered`],
    )
  }
  return makeAdapter({
    kind: runner.adapterKind,
    canRun: (request) => runner.canRunAssignment(request.lease.codingAssignment),
    start: async (request) =>
      closeoutToEvents(
        runner.adapterKind,
        request,
        await runner.execute(request.state, request.lease, request.now, options),
      ),
  })
}

export function createOpenCodeAgentRuntimeAdapter(input: {
  callbacks?: OpencodeStreamCallbacks
  model?: string
} = {}): AgentRuntimeAdapter {
  return makeAdapter({
    kind: "opencode",
    canRun: (request) => typeof request.opencodePath === "string" && typeof request.opencodePrompt === "string",
    start: async (request) => {
      if (request.opencodePath === undefined || request.opencodePrompt === undefined) {
        return closeoutToEvents("opencode", request, null)
      }
      const result = await runOpencodeStream(
        request.opencodePath,
        request.opencodePrompt,
        input.callbacks,
        input.model,
      )
      const closeout: RuntimeCloseoutRecord = {
        artifactRefs: [stableRef("artifact.pylon.opencode.response", result.text)],
        blockerRefs: [],
        buildRefs: [stableRef("build.pylon.opencode.bytes", String(result.byteCount))],
        message: `OpenCode completed with ${result.eventCount} event(s).`,
        previewRefs: [],
        proofRefs: [stableRef("proof.pylon.opencode.usage", `${result.cost}:${result.tokens}`)],
        resultRefs: [stableRef("result.public.pylon.opencode", result.text)],
        runRefs: [stableRef("run.pylon.opencode", request.lease.leaseRef)],
        status: "accepted",
        summaryRefs: ["summary.public.pylon.opencode.completed"],
        testRefs: [],
      }
      return closeoutToEvents("opencode", request, closeout)
    },
  })
}

export function createTestFixtureAgentRuntimeAdapter(): AgentRuntimeAdapter {
  return makeAdapter({
    kind: "test_fixture",
    canRun: () => true,
    start: async (request) =>
      closeoutToEvents("test_fixture", request, {
        artifactRefs: [stableRef("artifact.pylon.test_fixture", request.lease.leaseRef)],
        blockerRefs: [],
        buildRefs: [stableRef("build.pylon.test_fixture", request.lease.leaseRef)],
        message: "Deterministic test fixture completed.",
        previewRefs: [],
        proofRefs: [stableRef("proof.pylon.test_fixture", request.lease.leaseRef)],
        resultRefs: ["result.public.pylon.test_fixture.completed"],
        runRefs: [stableRef("run.pylon.test_fixture", request.lease.leaseRef)],
        status: "accepted",
        summaryRefs: ["summary.public.pylon.test_fixture.completed"],
        testRefs: ["test.public.pylon.test_fixture.completed"],
      }),
  })
}

export function createHermesReservedAgentRuntimeAdapter(): AgentRuntimeAdapter {
  return makeAdapter({
    kind: "hermes",
    canRun: () => false,
    start: async (request) => closeoutToEvents("hermes", request, null),
  })
}

export function agentRuntimeToolDeniedEvent(input: {
  runId: AgentRuntimeRunId
  generatedAt: string
  invocationId: string
  sequence: number
  toolName: string
  toolRef: string
  blockerRefs: ReadonlyArray<string>
}): AgentRuntimeEvent {
  return assertAgentRuntimePublicEventSafe({
    tag: "tool.denied",
    eventId: eventId(input.runId, input.sequence),
    runId: input.runId,
    sequence: input.sequence,
    generatedAt: input.generatedAt,
    visibility: "public",
    redactionClass: "public_ref",
    refs: [],
    blockerRefs: [...input.blockerRefs],
    toolInvocation: {
      invocationId: input.invocationId,
      toolName: input.toolName,
      toolRef: input.toolRef,
      status: "denied",
      blockerRefs: [...input.blockerRefs],
    },
  })
}

export function replayAgentRuntimeEventLog(events: ReadonlyArray<AgentRuntimeEvent>): AgentRuntimeReplayProjection {
  const runId = events[0]?.runId ?? "run.public.empty"
  const artifactRefs = new Set<string>()
  const blockerRefs = new Set<string>()
  let state: AgentRuntimeReplayProjection["state"] = "running"
  let externalStatus: AgentRuntimeReplayProjection["externalStatus"] = "idle"
  let generatedAt = events[0]?.generatedAt ?? new Date(0).toISOString()

  for (const runtimeEvent of events) {
    generatedAt = runtimeEvent.generatedAt
    for (const blockerRef of runtimeEvent.blockerRefs) {
      blockerRefs.add(blockerRef)
    }
    if (runtimeEvent.artifact?.artifactRef !== undefined) {
      artifactRefs.add(runtimeEvent.artifact.artifactRef)
    }
    for (const artifactRef of runtimeEvent.externalInvocation?.artifactRefs ?? []) {
      artifactRefs.add(artifactRef)
    }
    if (runtimeEvent.externalInvocation?.status !== undefined) {
      externalStatus = runtimeEvent.externalInvocation.status
    }
    if (runtimeEvent.tag === "run.paused") state = "paused"
    if (runtimeEvent.tag === "run.interrupted") state = "interrupted"
    if (runtimeEvent.tag === "run.cancelled") state = "cancelled"
    if (runtimeEvent.tag === "run.completed") state = "completed"
    if (runtimeEvent.tag === "run.failed") state = "failed"
  }

  return {
    runId,
    state,
    artifactRefs: [...artifactRefs],
    blockerRefs: [...blockerRefs],
    externalStatus,
    generatedAt,
    eventCount: events.length,
  }
}
