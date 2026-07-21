import { Effect } from "effect"
import {
  HISTORY_RECALL_TOOL_NAME,
  makeInMemoryEventLogStore,
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract"
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema"
import { describe, expect, test } from "vite-plus/test"

import {
  DESKTOP_HISTORY_ADDRESS_SCHEMA_ID,
  DESKTOP_RLM_STRATEGY_REF,
  decodeHistoryCursorAddress,
  desktopHistoryCorpusInputForScope,
  encodeHistoryCursorAddress,
  makeDesktopHistoryCorpusSource,
  resolveDesktopHistoryCorpus,
} from "./desktop-history-corpus-source.ts"
import {
  citedSpansFromRlmResult,
  makeDesktopRlmToolHandler,
  runDesktopRlmDeterministicGrep,
  type HistoryRecallHostSources,
} from "./history-recall-host.ts"

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" }
const THREAD_ID = "thread.desktop-rlm"
const PLANTED = "DECISION: ship OPENRLM-SDK corpus source"

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

const makeSources = async (
  authorize: (threadId: string) => boolean = () => true,
): Promise<HistoryRecallHostSources> => {
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
    authorizeThread: authorize,
  }
}

describe("DesktopHistoryCorpusSource (OPENRLM-SDK)", () => {
  test("pins strategy ref and history address schema", () => {
    expect(DESKTOP_RLM_STRATEGY_REF).toBe("openagents.desktop.rlm.history.v1")
    expect(DESKTOP_HISTORY_ADDRESS_SCHEMA_ID).toContain("history_cursor")
    const addr = encodeHistoryCursorAddress({ turnId: "t", sequence: 3 })
    expect(decodeHistoryCursorAddress(addr)).toEqual({ turnId: "t", sequence: 3 })
  })

  test("resolves authorized thread corpus to Rlm handle with digests", async () => {
    const sources = await makeSources()
    const handle = await Effect.runPromise(
      resolveDesktopHistoryCorpus(
        {
          eventLog: sources.eventLog,
          turnIdsForThread: sources.turnIdsForThread,
          builtAt: sources.builtAt,
          authorizeThread: sources.authorizeThread ?? (() => true),
        },
        { _tag: "Thread", threadId: THREAD_ID },
      ),
    )
    expect(handle.identity.contentDigest.length).toBe(64)
    expect(handle.manifest.coverage.entryCount).toBeGreaterThan(0)
    expect(handle.manifest.coverage.note).toContain("seven core kinds")
    const entries = await Effect.runPromise(handle.materializeAll())
    expect(entries.some((e) => (e.text ?? "").includes("DECISION"))).toBe(true)
  })

  test("refuses unauthorized thread before store materialization", async () => {
    const sources = await makeSources(() => false)
    const source = makeDesktopHistoryCorpusSource({
      eventLog: sources.eventLog,
      turnIdsForThread: sources.turnIdsForThread,
      builtAt: sources.builtAt,
      authorizeThread: () => false,
    })
    const outcome = await Effect.runPromise(
      source
        .resolve(desktopHistoryCorpusInputForScope({ _tag: "Thread", threadId: THREAD_ID }))
        .pipe(
          Effect.match({
            onFailure: (err) => ({ ok: false as const, err }),
            onSuccess: (handle) => ({ ok: true as const, handle }),
          }),
        ),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.err.detailSafe).toContain("not authorized")
    }
  })

  test("Rlm deterministic Grep finds planted text with zero model calls", async () => {
    const sources = await makeSources()
    const terminal = await Effect.runPromise(
      runDesktopRlmDeterministicGrep(sources, {
        scope: { _tag: "Thread", threadId: THREAD_ID },
        pattern: "DECISION: ship",
        runRef: "run.desktop.rlm.1",
      }),
    )
    expect(terminal._tag).toBe("Completed")
    if (terminal._tag === "Completed") {
      expect(terminal.usage.modelCalls).toBe(0)
      expect(terminal.output._tag).toBe("DeterministicFindings")
      const spans = citedSpansFromRlmResult(terminal)
      expect(spans.length).toBeGreaterThan(0)
      expect(spans[0]?.excerpt).toContain("DECISION")
      expect(spans[0]?.turnId).toBe("turn.2")
    }
  })

  test("makeDesktopRlmToolHandler exposes strategy pin and no artifact sink", async () => {
    const sources = await makeSources()
    const { makeDesktopRlmDeterministic } = await import("./history-recall-host.ts")
    const rlm = await Effect.runPromise(makeDesktopRlmDeterministic(sources))
    const tool = makeDesktopRlmToolHandler(rlm)
    expect(tool.name).toBe("rlm")
    expect(tool.strategyRef).toBe(DESKTOP_RLM_STRATEGY_REF)
    expect(tool.rootLimits.maxInlineOutputBytes).toBe(16_384)
    expect(HISTORY_RECALL_TOOL_NAME).toBe("history_recall")
  })
})
