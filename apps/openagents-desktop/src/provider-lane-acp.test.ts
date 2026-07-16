import type { AcpProjectionEvent } from "@openagentsinc/agent-client-runtime-bridge"
import { describe, expect, test } from "vite-plus/test"

import { acpProjectionEventToLaneEvent, makeAcpProviderLane } from "./provider-lane-acp.ts"

const event = (value: Record<string, unknown>): AcpProjectionEvent => value as AcpProjectionEvent

describe("ACP projection to provider-lane envelope", () => {
  test("maps the shared turn, text, reasoning, and tool vocabulary", () => {
    expect(acpProjectionEventToLaneEvent(event({ kind: "turn.started" }))).toEqual({ kind: "turn_started" })
    expect(acpProjectionEventToLaneEvent(event({ kind: "text.delta", text: "hello" }))).toEqual({
      kind: "text_delta",
      text: "hello",
    })
    expect(acpProjectionEventToLaneEvent(event({ kind: "reasoning.delta", text: "inspect" }))).toEqual({
      kind: "reasoning",
      text: "inspect",
    })
    expect(acpProjectionEventToLaneEvent(event({
      kind: "tool.call",
      toolName: "Read",
      toolCallId: "tool-1",
    }))).toEqual({ kind: "tool_use", toolName: "Read", summary: "", itemRef: "tool-1" })
    expect(acpProjectionEventToLaneEvent(event({
      kind: "tool.error",
      toolName: "Read",
      toolCallId: "tool-1",
      messageSafe: "not found",
    }))).toEqual({
      kind: "tool_result",
      toolName: "Read",
      ok: false,
      summary: "not found",
      itemRef: "tool-1",
    })
  })

  test("preserves a complete exact usage split and never invents missing fields", () => {
    expect(acpProjectionEventToLaneEvent(event({
      kind: "turn.finished",
      usage: {
        inputTokens: 10,
        cacheReadInputTokens: 4,
        outputTokens: 7,
        reasoningTokens: 2,
        totalTokens: 17,
      },
    }))).toEqual({
      kind: "turn_completed",
      totalTokens: 17,
      usage: {
        inputTokens: 10,
        cachedInputTokens: 4,
        outputTokens: 7,
        reasoningTokens: 2,
        totalTokens: 17,
      },
    })

    expect(acpProjectionEventToLaneEvent(event({
      kind: "turn.finished",
      usage: { inputTokens: 10, outputTokens: 7, totalTokens: 17 },
    }))).toEqual({ kind: "turn_completed", totalTokens: 17 })

    expect(acpProjectionEventToLaneEvent(event({
      kind: "usage.recorded",
      usage: { inputTokens: 10, totalTokens: 17 },
    }))).toEqual({ kind: "meter_updated", inputTokens: 10, totalTokens: 17 })
  })

  test("projects plans and degradation visibly while leaving sidecar facts off the transcript", () => {
    expect(acpProjectionEventToLaneEvent(event({
      kind: "plan-snapshot",
      stateRef: "state-1",
      safeSummary: "plan",
      snapshot: {
        entries: [
          { entryRef: "step-1", contentRef: "Inspect", status: "in_progress" },
          { entryRef: "step-2", contentRef: "Verify", status: "done" },
        ],
      },
    }))).toEqual({
      kind: "plan_updated",
      entries: [
        { step: "Inspect", status: "in_progress" },
        { step: "Verify", status: "pending" },
      ],
    })
    expect(acpProjectionEventToLaneEvent(event({
      kind: "degraded",
      stateRef: "state-2",
      safeSummary: "quarantined stale update",
      snapshot: {},
    }))).toEqual({ kind: "lane_notice", text: "quarantined stale update" })
    expect(acpProjectionEventToLaneEvent(event({ kind: "raw.sidecar_ref" }))).toBeNull()
  })

  test("the ACP bridge is a concrete provider-lane adapter, not only a vocabulary helper", async () => {
    const projected: unknown[] = []
    const lane = makeAcpProviderLane({
      laneRef: "grok-acp",
      graphLaneRef: "grok_acp",
      eventChannel: "openagents:grok-acp:event",
      capabilities: {
        laneRef: "grok-acp",
        provider: "grok",
        models: ["grok-code-fast"],
        features: {
          skills: false,
          planOnly: false,
          reasoningEffort: false,
          images: false,
          fullAuto: false,
          interrupt: true,
          queueFollowup: false,
          steerTurn: false,
          steerChild: false,
          answerQuestion: true,
        },
        recovery: "interrupt_on_restart",
      },
      driver: {
        runTurn: async input => {
          input.emit(event({ kind: "turn.started" }))
          input.emit(event({ kind: "text.delta", text: "from ACP" }))
          input.emit(event({ kind: "turn.finished" }))
          return { ok: true, text: "from ACP", totalTokens: null, providerSessionRef: "peer-session-1" }
        },
        interrupt: () => true,
      },
    })
    const result = await lane.runTurn({
      request: { threadRef: "thread-1", turnRef: "turn-1", message: "hello" },
      model: "grok-code-fast",
      context: null,
      history: [],
      message: "hello",
      background: false,
      emit: value => projected.push(value),
    })
    expect(result).toEqual({
      ok: true,
      text: "from ACP",
      totalTokens: null,
      providerSessionRef: "peer-session-1",
    })
    expect(projected).toEqual([
      { kind: "turn_started" },
      { kind: "text_delta", text: "from ACP" },
      { kind: "turn_completed", totalTokens: null },
    ])
    expect(lane.capabilities().models).toEqual(["grok-code-fast"])
  })
})
