import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { Effect, Stream } from "effect"

import { decodeAgentRuntimeEvent, type AgentRuntimeEvent } from "@openagentsinc/agent-runtime-schema"
import { replayAgentRuntimeEventLog, type AgentRuntimeRunRequest } from "../src/agent-runtime-adapter"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  OPENAGENTS_NATIVE_SUMMARY_TOOL_REF,
  OPENAGENTS_NATIVE_TASK_SCHEMA,
  OpenAgentsNativeBudgetStopLanguageModelLayer,
  OpenAgentsNativeTestToolkitLayer,
  createOpenAgentsNativeAgentRuntimeAdapter,
  openAgentsNativeTaskFrom,
} from "../src/openagents-native-runtime"
import { ensurePylonLocalState } from "../src/state"
import { Layer } from "effect"

const now = new Date("2026-06-11T13:00:00.000Z")

async function collect(stream: Stream.Stream<AgentRuntimeEvent>) {
  const chunk = await Effect.runPromise(Stream.runCollect(stream))
  return Array.from(chunk)
}

async function withNativeRequest<T>(
  openagentsNative: Record<string, unknown>,
  fn: (request: AgentRuntimeRunRequest) => Promise<T>,
) {
  const home = await mkdtemp(join(tmpdir(), "pylon-rk3-test-"))
  try {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    const state = await ensurePylonLocalState(summary)
    return await fn({
      runId: `run.public.rk3.${crypto.randomUUID()}`,
      lease: {
        schema: "openagents.pylon.assignment_lease.v0.3",
        assignmentRef: "assignment.public.rk3.test",
        leaseRef: "lease.public.rk3.test",
        goal: "goal.public.rk3.test",
        paymentMode: "no-spend",
        capabilityRefs: [],
        codingAssignment: { openagentsNative },
        expiresAt: new Date(now.getTime() + 60_000).toISOString(),
      },
      now,
      state,
    })
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("openagents native runtime adapter", () => {
  test("recognizes only the typed native task payload", () => {
    expect(openAgentsNativeTaskFrom({
      openagentsNative: {
        schema: OPENAGENTS_NATIVE_TASK_SCHEMA,
        allowedToolRefs: [OPENAGENTS_NATIVE_SUMMARY_TOOL_REF],
      },
    })).not.toBeNull()
    expect(openAgentsNativeTaskFrom({ openagentsNative: { schema: "wrong", allowedToolRefs: [] } })).toBeNull()
    expect(openAgentsNativeTaskFrom(undefined)).toBeNull()
  })

  test("runs the fixture contract under the test provider and toolkit layers", async () => {
    const adapter = createOpenAgentsNativeAgentRuntimeAdapter()
    await withNativeRequest({
      schema: OPENAGENTS_NATIVE_TASK_SCHEMA,
      allowedToolRefs: [OPENAGENTS_NATIVE_SUMMARY_TOOL_REF],
    }, async (request) => {
      expect(await Effect.runPromise(adapter.canRun(request))).toBe(true)
      const events = await collect(adapter.start(request))
      expect(events.map((event) => event.tag)).toEqual([
        "run.started",
        "model.stream_started",
        "model.reasoning_delta",
        "model.text_delta",
        "tool.call_proposed",
        "tool.approved",
        "tool.started",
        "tool.completed",
        "model.text_delta",
        "model.text_completed",
        "model.reasoning_completed",
        "run.completed",
      ])
      for (const runtimeEvent of events) {
        expect(decodeAgentRuntimeEvent(runtimeEvent)).toMatchObject({ runId: request.runId })
      }
      expect(replayAgentRuntimeEventLog(events)).toMatchObject({
        state: "completed",
        artifactRefs: ["artifact.public.openagents_native.fixture"],
      })
    })
  })

  test("denies tools by typed policy without executing the toolkit", async () => {
    const adapter = createOpenAgentsNativeAgentRuntimeAdapter()
    await withNativeRequest({
      schema: OPENAGENTS_NATIVE_TASK_SCHEMA,
      allowedToolRefs: [],
    }, async (request) => {
      const events = await collect(adapter.start(request))
      expect(events.map((event) => event.tag)).toContain("tool.denied")
      expect(events.map((event) => event.tag)).not.toContain("tool.started")
      expect(events.map((event) => event.tag)).not.toContain("tool.completed")
      expect(replayAgentRuntimeEventLog(events)).toMatchObject({
        state: "failed",
        blockerRefs: ["blocker.agent_runtime.openagents_native.tool_denied"],
      })
    })
  })

  test("budget stop interrupts and fails without hanging the stream", async () => {
    const adapter = createOpenAgentsNativeAgentRuntimeAdapter({
      layer: Layer.merge(OpenAgentsNativeBudgetStopLanguageModelLayer, OpenAgentsNativeTestToolkitLayer),
    })
    await withNativeRequest({
      schema: OPENAGENTS_NATIVE_TASK_SCHEMA,
      allowedToolRefs: [OPENAGENTS_NATIVE_SUMMARY_TOOL_REF],
      maxModelEvents: 1,
    }, async (request) => {
      const events = await collect(adapter.start(request))
      expect(events.map((event) => event.tag)).toEqual([
        "run.started",
        "model.stream_started",
        "model.reasoning_delta",
        "run.interrupted",
        "run.failed",
      ])
      expect(replayAgentRuntimeEventLog(events)).toMatchObject({
        state: "failed",
        blockerRefs: ["blocker.agent_runtime.openagents_native.budget_stop"],
      })
    })
  })

  test("cancel emits run.cancelled through the same adapter contract", async () => {
    const adapter = createOpenAgentsNativeAgentRuntimeAdapter()
    await withNativeRequest({
      schema: OPENAGENTS_NATIVE_TASK_SCHEMA,
      allowedToolRefs: [OPENAGENTS_NATIVE_SUMMARY_TOOL_REF],
    }, async (request) => {
      await Effect.runPromise(adapter.cancel(request.runId))
      const events = await collect(adapter.start(request))
      expect(events.map((event) => event.tag)).toEqual(["run.started", "run.cancelled"])
      expect(replayAgentRuntimeEventLog(events)).toMatchObject({ state: "cancelled" })
    })
  })
})
