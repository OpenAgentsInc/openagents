import { Effect } from "effect"
import {
  HISTORY_RECALL_TOOL_NAME,
  HISTORY_RECALL_TURN_POLICY_CAPABILITY,
  makeInMemoryEventLogStore,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract"
import {
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
} from "@openagentsinc/agent-harness-contract"
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema"
import { describe, expect, test } from "vite-plus/test"

import {
  desktopAdmittedHostTools,
  desktopHostToolCapabilitiesForTurn,
  dispatchDesktopHistoryRecall,
  historyRecallToolsForTurn,
  isHistoryRecallHostTool,
  makeDesktopHistoryRecall,
  type HistoryRecallHostSources,
} from "./history-recall-host.ts"
import {
  formatHistoryRecallCitedSpans,
  projectHistoryRecallToolCard,
} from "./renderer/history-recall-card.ts"

// Keep the renderer constant aligned with the harness registration without
// importing the harness package from a pure-renderer test path beyond this
// main-side suite.

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" }
const THREAD_ID = "thread.desktop-hr"
const PLANTED = "DECISION: ship RLM-03 host tool"

const scriptTurn = (
  turnId: string,
  words: ReadonlyArray<string>,
): Array<HarnessStreamEvent> => {
  const events: Array<HarnessStreamEvent> = []
  let seq = 0
  events.push(
    buildTurnStarted({
      turnId,
      threadId: THREAD_ID,
      sequence: seq++,
      source: SOURCE,
      observedAt: "2026-07-21T09:00:00.000Z",
    }),
  )
  for (const word of words) {
    events.push(
      buildTextDelta({
        turnId,
        threadId: THREAD_ID,
        sequence: seq++,
        source: SOURCE,
        observedAt: "2026-07-21T09:00:01.000Z",
        messageId: `msg.${turnId}`,
        text: word,
      }),
    )
  }
  events.push(
    buildTurnFinished({
      turnId,
      threadId: THREAD_ID,
      sequence: seq,
      source: SOURCE,
      observedAt: "2026-07-21T09:00:02.000Z",
      finishReason: "stop",
    }),
  )
  return events
}

const makeSources = async (): Promise<HistoryRecallHostSources> => {
  const eventLog = makeInMemoryEventLogStore()
  const turnIds = ["turn.1", "turn.2"]
  for (const events of [
    scriptTurn("turn.1", ["hello", "there"]),
    scriptTurn("turn.2", [PLANTED, "confirmed"]),
  ]) {
    for (const event of events) {
      await Effect.runPromise(eventLog.append(event))
    }
  }
  return {
    eventLog,
    turnIdsForThread: (threadId) => (threadId === THREAD_ID ? turnIds : []),
    builtAt: () => "2026-07-21T12:00:00.000Z",
    source: SOURCE,
  }
}

describe("desktop history_recall host (RLM-03)", () => {
  test("registers history_recall in admitted host tools and turn-policy capabilities", () => {
    expect(desktopAdmittedHostTools.map((t) => t.name)).toEqual([
      HISTORY_RECALL_TOOL_NAME,
    ])
    expect(historyRecallToolsForTurn()[0]?.name).toBe(HISTORY_RECALL_TOOL_NAME)
    expect(desktopHostToolCapabilitiesForTurn()).toContain(
      HISTORY_RECALL_TURN_POLICY_CAPABILITY,
    )
    expect(isHistoryRecallHostTool(HISTORY_RECALL_TOOL_NAME)).toBe(true)
    expect(isHistoryRecallHostTool("Bash")).toBe(false)
  })

  test("round-trips through the dispatcher, neutral log, and cited-span projection", async () => {
    const sources = await makeSources()
    const dispatched = await Effect.runPromise(
      dispatchDesktopHistoryRecall(sources, {
        call: {
          toolCallId: "toolcall.desktop.1",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: {
            scope: { _tag: "Thread", threadId: THREAD_ID },
            question: { _tag: "Grep", pattern: "DECISION: ship" },
            caps: { maxSpans: 5 },
          },
        },
        turnId: "turn.active",
        threadId: THREAD_ID,
        sequence: 20,
        observedAt: "2026-07-21T13:00:00.000Z",
      }),
    )

    expect(dispatched.result.isError).toBeUndefined()
    expect(dispatched.answer).not.toBeNull()
    expect(dispatched.citedSpans.length).toBeGreaterThan(0)
    expect(dispatched.citedSpans[0]?.excerpt).toContain("DECISION")
    expect(dispatched.summary).toContain("history_recall")
    expect(dispatched.summary).toContain("span")

    // Neutral stream re-entry
    expect(dispatched.neutralEvents.map((e) => e.kind)).toEqual([
      "tool.call",
      "tool.result",
    ])
    const call = dispatched.neutralEvents[0]!
    const result = dispatched.neutralEvents[1]!
    if (call.kind === "tool.call") {
      expect(call.toolName).toBe(HISTORY_RECALL_TOOL_NAME)
      expect(call.sequence).toBe(20)
    }
    if (result.kind === "tool.result") {
      expect(result.providerExecuted).toBe(false)
      expect(result.sequence).toBe(21)
    }

    // Append to the durable log and prove replay sees the re-entry.
    for (const event of dispatched.neutralEvents) {
      await Effect.runPromise(sources.eventLog.append(event))
    }
    const replayed = await Effect.runPromise(
      sources.eventLog.read({ turnId: "turn.active", fromCursor: -1 }),
    )
    expect(replayed.map((e) => e.kind)).toEqual(["tool.call", "tool.result"])

    // Renderer row with cited spans
    const card = projectHistoryRecallToolCard({
      toolCallId: "toolcall.desktop.1",
      phase: "ok",
      summary: dispatched.summary,
      citedSpans: dispatched.citedSpans,
    })
    expect(card.toolName).toBe(HISTORY_RECALL_TOOL_NAME)
    expect(card.status).toBe("ok")
    expect(card.citedSpans.length).toBeGreaterThan(0)
    expect(card.citedSpansLine).toContain("turn.2")
    expect(formatHistoryRecallCitedSpans(dispatched.citedSpans)).toContain("#")
  })

  test("corpus provider is used by the desktop HistoryRecall layer", async () => {
    const sources = await makeSources()
    const recall = makeDesktopHistoryRecall(sources)
    const response = await Effect.runPromise(
      recall.recall({
        corpus: {
          _tag: "Scope",
          scope: { _tag: "Thread", threadId: THREAD_ID },
        },
        question: { _tag: "Grep", pattern: "hello" },
      }),
    )
    expect(response.answers.length).toBeGreaterThan(0)
    expect(response.cost.modelCalls).toBe(0)
  })

  test("invalid tool input fails closed as isError with tool.error re-entry", async () => {
    const sources = await makeSources()
    const dispatched = await Effect.runPromise(
      dispatchDesktopHistoryRecall(sources, {
        call: {
          toolCallId: "toolcall.desktop.bad",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: { garbage: true },
        },
        turnId: "turn.bad",
        threadId: THREAD_ID,
        sequence: 0,
      }),
    )
    expect(dispatched.result.isError).toBe(true)
    expect(dispatched.answer).toBeNull()
    expect(dispatched.citedSpans).toEqual([])
    expect(dispatched.neutralEvents.map((e) => e.kind)).toEqual([
      "tool.call",
      "tool.error",
    ])
  })
})
