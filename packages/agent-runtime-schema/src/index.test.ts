import { describe, expect, test } from "bun:test"
import {
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
  agentRuntimeSurfaceStatusHasUnsafeMaterial,
  decodeAgentRuntimeEvent,
  decodeAgentRuntimeEventLog,
  decodeAgentRuntimeRun,
  projectAgentRuntimeSurfaceStatus,
} from "./index.js"
import { agentRuntimeFixtureEventLogs } from "./fixtures.js"

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
}

const baseEvent = {
  eventId: "event.public.schema_test.1",
  runId: "run.public.schema_test",
  sequence: 1,
  generatedAt: "2026-06-11T00:00:00.000Z",
  visibility: "public",
  redactionClass: "public_ref",
  refs: [],
  blockerRefs: [],
}

describe("@openagentsinc/agent-runtime-schema", () => {
  test("decodes every adapter kind, loop kind, redaction class, and visibility", () => {
    for (const adapterKind of agentRuntimeAdapterKinds) {
      expect(decodeAgentRuntimeRun({ ...baseRun, adapterKind })).toMatchObject({ adapterKind })
    }
    for (const loopKind of agentRuntimeLoopKinds) {
      expect(decodeAgentRuntimeRun({ ...baseRun, loopKind })).toMatchObject({ loopKind })
    }
    for (const redactionClass of agentRuntimeRedactionClasses) {
      expect(decodeAgentRuntimeEvent({ ...baseEvent, tag: "run.started", redactionClass }))
        .toMatchObject({ redactionClass })
    }
    for (const visibility of agentRuntimeVisibilities) {
      expect(decodeAgentRuntimeEvent({ ...baseEvent, tag: "run.started", visibility }))
        .toMatchObject({ visibility })
    }
  })

  test("decodes every RK1 event tag", () => {
    expect(agentRuntimeEventTags).toHaveLength(32)
    for (const tag of agentRuntimeEventTags) {
      expect(decodeAgentRuntimeEvent({ ...baseEvent, tag })).toMatchObject({ tag })
    }
  })

  test("decodes reusable fixture logs for every loop kind", () => {
    const decoded = agentRuntimeFixtureEventLogs.map((log) => decodeAgentRuntimeEventLog(log))
    expect(decoded.map((log) => log.run.loopKind).sort()).toEqual([
      "external_agent_loop",
      "fixture_loop",
      "hosted_loop",
      "native_model_loop",
    ])
    for (const log of decoded) {
      expect(log.events[0]?.tag).toBe("run.started")
      expect(log.events.at(-1)?.tag).toBe("run.completed")
      expect(assertAgentRuntimeEventLogSafe(log)).toBe(log)
    }
  })

  test("checks legal and illegal run lifecycle transitions", () => {
    expect(assertAgentRuntimeRunStateTransition("pending", "running")).toBe("running")
    expect(assertAgentRuntimeRunStateTransition("running", "paused")).toBe("paused")
    expect(assertAgentRuntimeRunStateTransition("paused", "running")).toBe("running")
    expect(assertAgentRuntimeRunStateTransition("running", "completed")).toBe("completed")
    expect(() => assertAgentRuntimeRunStateTransition("completed", "running")).toThrow(
      "Illegal AgentRuntimeRun state transition",
    )
    expect(() => assertAgentRuntimeRunStateTransition("cancelled", "completed")).toThrow(
      "Illegal AgentRuntimeRun state transition",
    )
  })

  test("rejects raw prompts, shell logs, provider payloads, secrets, and local paths in public events", () => {
    const unsafeEvents = [
      { ...baseEvent, tag: "model.text_delta", summary: "raw_prompt: fix this private repo" },
      { ...baseEvent, tag: "tool.failed", summary: "raw_shell_log: stack trace" },
      { ...baseEvent, tag: "external_agent.event", summary: "provider_payload included" },
      { ...baseEvent, tag: "external_agent.failed", summary: "secret sk-test" },
      { ...baseEvent, tag: "artifact.recorded", refs: ["/Users/example/private-source"] },
    ]
    for (const unsafeEvent of unsafeEvents) {
      const decoded = decodeAgentRuntimeEvent(unsafeEvent)
      expect(() => assertAgentRuntimePublicEventSafe(decoded)).toThrow(
        "Agent runtime public event contains raw/private material",
      )
    }
  })

  test("has no provider SDK or Vercel AI SDK fields in the durable schema shape", () => {
    const schemas = JSON.stringify([AgentRuntimeRun.ast, AgentRuntimeEvent.ast])
    expect(schemas).not.toContain("@anthropic-ai")
    expect(schemas).not.toContain("@openai/codex-sdk")
    expect(schemas).not.toContain("ai-sdk")
    expect(schemas).not.toContain("providerEvent")
    expect(schemas).not.toContain("sdkMessage")
  })

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
    })

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
    })
    expect(agentRuntimeSurfaceStatusHasUnsafeMaterial(row)).toBe(false)
  })

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
    })
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
    })

    expect(decodePylonLifecycleWireEventJson(JSON.stringify(assignmentEvent))).toEqual(assignmentEvent)
    expect(decodePylonLifecycleWireEventJson(JSON.stringify(workerEvent))).toEqual(workerEvent)
    expect(JSON.stringify(PylonLifecycleWireEventFromJsonString.ast)).toContain(
      "openagents.pylon.assignment_run_lifecycle_event.v0.1",
    )
  })

  test("rejects malformed Pylon lifecycle wire events", () => {
    expect(() =>
      decodePylonLifecycleWireEventJson(JSON.stringify({
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        event: "assignment_run.unknown",
        observedAt: "2026-07-01T00:00:00.000Z",
      })),
    ).toThrow()
    expect(() =>
      decodePylonLifecycleWireEventJson(JSON.stringify({
        schema: "openagents.pylon.khala_spawn_worker_event.v0.1",
        message: "assignment lifecycle event",
        observedAt: "2026-07-01T00:00:00.000Z",
        slotIndex: 0,
        state: "mystery",
      })),
    ).toThrow()
  })
})
