import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  agentRuntimeSurfaceStatusHasUnsafeMaterial,
  projectAgentRuntimeSurfaceStatus,
  type AgentRuntimeEvent,
} from "@openagentsinc/agent-runtime-schema"
import { Effect, Layer, Stream } from "effect"

import {
  MemoryAgentRuntimeEventRepository,
  ingestAgentRuntimeEvents,
  projectAgentRuntimeWorkroomStatus,
  projectPublicAgentRuntimeRun,
} from "../../openagents.com/workers/api/src/agent-runtime-kernel"
import {
  createHermesReservedAgentRuntimeAdapter,
  createTestFixtureAgentRuntimeAdapter,
  type AgentRuntimeAdapter,
  type AgentRuntimeRunRequest,
} from "../src/agent-runtime-adapter"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  OPENAGENTS_NATIVE_SUMMARY_TOOL_REF,
  OPENAGENTS_NATIVE_TASK_SCHEMA,
  OpenAgentsNativeBudgetStopLanguageModelLayer,
  OpenAgentsNativeTestToolkitLayer,
  createOpenAgentsNativeAgentRuntimeAdapter,
} from "../src/openagents-native-runtime"
import { ensurePylonLocalState } from "../src/state"

const now = new Date("2026-06-11T15:30:00.000Z")

async function collect(stream: Stream.Stream<AgentRuntimeEvent>) {
  const chunk = await Effect.runPromise(Stream.runCollect(stream))
  return Array.from(chunk)
}

async function withRequest<T>(
  name: string,
  codingAssignment: Record<string, unknown>,
  fn: (request: AgentRuntimeRunRequest) => Promise<T>,
) {
  const home = await mkdtemp(join(tmpdir(), "pylon-rk5-test-"))
  try {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    const state = await ensurePylonLocalState(summary)
    return await fn({
      runId: `run.public.rk5.${name}`,
      lease: {
        schema: "openagents.pylon.assignment_lease.v0.3",
        assignmentRef: `assignment.public.rk5.${name}`,
        leaseRef: `lease.public.rk5.${name}`,
        goal: `goal.public.rk5.${name}`,
        paymentMode: "no-spend",
        capabilityRefs: [],
        codingAssignment,
        expiresAt: new Date(now.getTime() + 60_000).toISOString(),
      },
      now,
      state,
    })
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

async function projectBothSurfaces(
  adapter: AgentRuntimeAdapter,
  request: AgentRuntimeRunRequest,
) {
  const events = await collect(adapter.start(request))
  const repository = new MemoryAgentRuntimeEventRepository()
  await ingestAgentRuntimeEvents(repository, events)
  const projection = await projectPublicAgentRuntimeRun(repository, request.runId, now.toISOString())
  const workroomRow = projectAgentRuntimeWorkroomStatus(projection)

  // The surface-status projection (shared schema package) must agree with the
  // workroom row, and stay public-safe. This used to be cross-checked against
  // the now-deleted TUI store; the projection function is the real authority.
  const surfaceRow = projectAgentRuntimeSurfaceStatus(projection)

  expect(surfaceRow).toEqual(workroomRow)
  expect(agentRuntimeSurfaceStatusHasUnsafeMaterial(workroomRow)).toBe(false)
  return { events, projection, row: workroomRow }
}

describe("RK5 agent runtime status surfaces", () => {
  test("cancellation produces coherent workroom and TUI projection rows", async () => {
    const adapter = createTestFixtureAgentRuntimeAdapter()
    await withRequest("cancel", {}, async request => {
      await Effect.runPromise(adapter.cancel(request.runId))
      const { events, row } = await projectBothSurfaces(adapter, request)

      expect(events.map(event => event.tag)).toEqual(["run.started", "run.cancelled"])
      expect(row.status).toBe("cancelled")
      expect(row.blockerRefs).toContain("blocker.agent_runtime.test_fixture.cancelled")
      expect(row.freshness).toMatchObject({ maxStalenessSeconds: 0 })
    })
  })

  test("tool denial carries decision refs and no side effect events", async () => {
    const adapter = createOpenAgentsNativeAgentRuntimeAdapter()
    await withRequest("tool_denied", {
      openagentsNative: {
        schema: OPENAGENTS_NATIVE_TASK_SCHEMA,
        allowedToolRefs: [],
      },
    }, async request => {
      const { events, row } = await projectBothSurfaces(adapter, request)
      const denied = events.find(event => event.tag === "tool.denied")

      expect(denied?.toolInvocation).toMatchObject({
        invocationId: "tool.public.openagents_native.fixture_summary.1",
        toolRef: OPENAGENTS_NATIVE_SUMMARY_TOOL_REF,
        status: "denied",
        blockerRefs: ["blocker.agent_runtime.openagents_native.tool_denied"],
      })
      expect(events.map(event => event.tag)).not.toContain("tool.started")
      expect(events.map(event => event.tag)).not.toContain("tool.completed")
      expect(row.status).toBe("failed")
      expect(row.reviewActionRefs).toContain(
        "review.public.agent_runtime.blocker.agent_runtime.openagents_native.tool_denied",
      )
    })
  })

  test("budget stop interrupts before failing and stays public-safe", async () => {
    const adapter = createOpenAgentsNativeAgentRuntimeAdapter({
      layer: Layer.merge(OpenAgentsNativeBudgetStopLanguageModelLayer, OpenAgentsNativeTestToolkitLayer),
    })
    await withRequest("budget_stop", {
      openagentsNative: {
        schema: OPENAGENTS_NATIVE_TASK_SCHEMA,
        allowedToolRefs: [OPENAGENTS_NATIVE_SUMMARY_TOOL_REF],
        maxModelEvents: 1,
      },
    }, async request => {
      const { events, row } = await projectBothSurfaces(adapter, request)

      expect(events.map(event => event.tag)).toContain("run.interrupted")
      expect(events.at(-1)?.tag).toBe("run.failed")
      expect(row.status).toBe("failed")
      expect(row.blockerRefs).toContain("blocker.agent_runtime.openagents_native.budget_stop")
    })
  })

  test("adapter failure projects as failed without adapter transcript parsing", async () => {
    const adapter = createHermesReservedAgentRuntimeAdapter()
    await withRequest("adapter_failure", {}, async request => {
      expect(await Effect.runPromise(adapter.canRun(request))).toBe(false)
      const { events, row } = await projectBothSurfaces(adapter, request)

      expect(events.map(event => event.tag)).toEqual([
        "run.started",
        "external_agent.failed",
        "run.failed",
      ])
      expect(row.status).toBe("failed")
      expect(row.blockerRefs).toContain("blocker.agent_runtime.hermes.unsupported_assignment")
      expect(row.eventCount).toBe(3)
    })
  })
})
