import { describe, expect, test } from "bun:test"
import type { KhalaRuntimeEvent } from "@openagentsinc/khala-sync"

import {
  buildHandoffPromptBody,
  handoffLaneLabel,
  handoffTargetLane,
  summarizeTurnEventsForHandoff
} from "../src/sync/khala-cross-agent-handoff-core"

const base = {
  causalityRefs: [] as ReadonlyArray<string>,
  observedAt: "2026-01-01T00:00:00Z",
  redactionClass: "private_ref" as const,
  schema: "openagents.khala_runtime_event.v1" as const,
  sequence: 0,
  source: { lane: "codex_app_server" as const },
  threadId: "t1",
  turnId: "turn1",
  visibility: "private" as const
}

describe("handoffTargetLane", () => {
  test("maps Codex to Claude", () => {
    expect(handoffTargetLane("codex_app_server")).toBe("claude_pylon")
  })

  test("maps Claude to Codex", () => {
    expect(handoffTargetLane("claude_pylon")).toBe("codex_app_server")
  })

  test("returns undefined for internal routing lanes with no user-facing counterpart", () => {
    expect(handoffTargetLane("khala_sync_mobile_control")).toBeUndefined()
    expect(handoffTargetLane("ai_sdk_core")).toBeUndefined()
    expect(handoffTargetLane("test_fixture")).toBeUndefined()
  })
})

describe("handoffLaneLabel", () => {
  test("gives friendly names to the two pickable lanes", () => {
    expect(handoffLaneLabel("codex_app_server")).toBe("Codex")
    expect(handoffLaneLabel("claude_pylon")).toBe("Claude")
  })

  test("falls back to the raw lane string for internal lanes", () => {
    expect(handoffLaneLabel("test_fixture")).toBe("test_fixture")
  })
})

describe("summarizeTurnEventsForHandoff", () => {
  test("joins merged text parts from the turn", () => {
    const events: ReadonlyArray<KhalaRuntimeEvent> = [
      { ...base, chunkId: "c1", eventId: "e1", kind: "text.delta", messageId: "m1", text: "Hel" },
      { ...base, chunkId: "c2", eventId: "e2", kind: "text.delta", messageId: "m1", text: "lo" }
    ]
    expect(summarizeTurnEventsForHandoff(events)).toBe("Hello")
  })

  test("includes completed and failed tool calls with their safe error message", () => {
    const events: ReadonlyArray<KhalaRuntimeEvent> = [
      {
        ...base,
        authority: {} as never,
        eventId: "e1",
        kind: "tool.call",
        toolCallId: "call1",
        toolName: "search"
      },
      {
        ...base,
        authority: {} as never,
        eventId: "e2",
        kind: "tool.result",
        resultRef: "ref1",
        toolCallId: "call1",
        toolName: "search"
      },
      {
        ...base,
        authority: {} as never,
        errorRef: "err1",
        eventId: "e3",
        kind: "tool.error",
        messageSafe: "workspace path not found",
        toolCallId: "call2",
        toolName: "workspaceRead"
      }
    ]
    const summary = summarizeTurnEventsForHandoff(events)
    expect(summary).toContain("- tool: search (completed)")
    expect(summary).toContain("- tool: workspaceRead (failed) — workspace path not found")
  })

  test("falls back to a bounded placeholder when a turn produced no readable parts", () => {
    const events: ReadonlyArray<KhalaRuntimeEvent> = [
      { ...base, eventId: "e1", finishReason: "stop", kind: "turn.finished" }
    ]
    expect(summarizeTurnEventsForHandoff(events)).toBe(
      "(no text response; turn completed with no readable output)"
    )
  })

  test("truncates an oversized summary with a visible marker instead of growing unbounded", () => {
    const hugeText = "x".repeat(7000)
    const events: ReadonlyArray<KhalaRuntimeEvent> = [
      { ...base, chunkId: "c1", eventId: "e1", kind: "text.delta", messageId: "m1", text: hugeText }
    ]
    const summary = summarizeTurnEventsForHandoff(events)
    expect(summary.length).toBeLessThan(hugeText.length)
    expect(summary.endsWith("[summary truncated]")).toBe(true)
  })
})

describe("buildHandoffPromptBody", () => {
  test("names both lanes and wraps the summary in a clear delimiter", () => {
    const body = buildHandoffPromptBody({
      sourceLane: "codex_app_server",
      summary: "Implemented the fix.",
      targetLane: "claude_pylon"
    })
    expect(body).toContain("Claude, please review")
    expect(body).toContain("Codex just completed")
    expect(body).toContain("Implemented the fix.")
  })
})
