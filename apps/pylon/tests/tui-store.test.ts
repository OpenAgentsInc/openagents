import { beforeEach, describe, expect, test } from "bun:test"
import {
  appendChatFeedItem,
  appendFeedText,
  appendRuntimeLogEntry,
  agentRuntimeStatusRows,
  collapseFeedLine,
  computeFeedWindow,
  feedLineCount,
  maxFeedLineChars,
  maxFeedLines,
  resetViewState,
  scrollFeedBy,
  feedScrollOffset,
  registerFeedViewport,
  setAgentRuntimeSurfaceProjections,
  setWalletState,
  streamingTails,
  visibleFeedLines,
  walletState,
} from "../src/tui/store"

const entry = (level: "error" | "info" | "verbose", message: string) => ({
  at: new Date(0).toISOString(),
  level,
  message,
})

describe("tui view store (virtualized feed)", () => {
  beforeEach(() => {
    resetViewState()
    registerFeedViewport(() => 10)
  })

  test("verbose log entries are filtered out unless verbose mode is on", () => {
    appendRuntimeLogEntry(entry("verbose", "chatter"), false)
    appendRuntimeLogEntry(entry("info", "ready"), false)
    appendRuntimeLogEntry(entry("error", "boom"), false)
    expect(feedLineCount()).toBe(2)
    const lines = visibleFeedLines(0, 10)
    expect(lines[0]?.text).toContain("ready")
    expect(lines[1]?.tone).toBe("logError")

    resetViewState()
    appendRuntimeLogEntry(entry("verbose", "chatter"), true)
    expect(feedLineCount()).toBe(1)
  })

  test("line buffer is capped at maxFeedLines with flat growth", () => {
    for (let i = 0; i < maxFeedLines + 50; i += 1) {
      appendFeedText("log", `line ${i}`)
    }
    expect(feedLineCount()).toBe(maxFeedLines)
    const tail = visibleFeedLines(0, 1)
    expect(tail[0]?.text).toBe(`line ${maxFeedLines + 49}`)
  })

  test("100k appends stay fast and the visible window stays viewport-sized", () => {
    const startedAt = performance.now()
    for (let i = 0; i < 100_000; i += 1) {
      appendFeedText("log", `entry ${i}`)
    }
    const elapsed = performance.now() - startedAt
    expect(elapsed).toBeLessThan(5_000)
    expect(visibleFeedLines(0, 40).length).toBe(40)
    expect(visibleFeedLines(50_000, 40).length).toBe(40)
  })

  test("window math clamps offsets and slices from the bottom", () => {
    expect(computeFeedWindow(100, 0, 10)).toEqual({ start: 90, end: 100 })
    expect(computeFeedWindow(100, 95, 10)).toEqual({ start: 0, end: 10 })
    expect(computeFeedWindow(5, 0, 10)).toEqual({ start: 0, end: 5 })
  })

  test("long lines are collapsed with an elision marker", () => {
    const long = "x".repeat(maxFeedLineChars + 120)
    expect(collapseFeedLine(long)).toContain("...(+120 chars)")
    appendFeedText("log", long)
    expect(visibleFeedLines(0, 5)[0]?.text.length).toBeLessThan(maxFeedLineChars + 40)
  })

  test("scrollFeedBy moves the window and sticky-bottom is offset zero", () => {
    for (let i = 0; i < 100; i += 1) appendFeedText("log", `l${i}`)
    expect(feedScrollOffset()).toBe(0)
    scrollFeedBy(-5)
    expect(feedScrollOffset()).toBe(5)
    scrollFeedBy(1, "content")
    expect(feedScrollOffset()).toBe(0)
    scrollFeedBy(-1, "content")
    expect(feedScrollOffset()).toBe(90)
  })

  test("streaming chat items update a live tail and flatten on finish", () => {
    const handle = appendChatFeedItem("**OpenCode**: thinking", { streaming: true })
    expect(streamingTails.length).toBe(1)
    handle.update("**OpenCode**: partial answer")
    expect(streamingTails[0]?.markdown).toContain("partial answer")
    const linesBefore = feedLineCount()
    handle.finish()
    expect(streamingTails.length).toBe(0)
    expect(feedLineCount()).toBeGreaterThan(linesBefore)
    const tail = visibleFeedLines(0, 3)
    expect(tail.some((line) => line.text.includes("OpenCode: partial answer"))).toBe(true)
  })

  test("non-streaming chat items append directly as flattened lines", () => {
    appendChatFeedItem("**User**: hello\nworld")
    expect(feedLineCount()).toBe(2)
    expect(visibleFeedLines(0, 5)[0]?.text).toBe("User: hello")
  })

  test("wallet signal reflects the latest set", () => {
    setWalletState({ daemonOnline: true, balanceSats: 7, readiness: "receive-ready" })
    expect(walletState().balanceSats).toBe(7)
  })

  test("agent runtime rows are derived from kernel projections only", () => {
    setAgentRuntimeSurfaceProjections([
      {
        runId: "run.public.tui.rk5",
        state: "cancelled",
        generatedAt: "2026-06-11T15:00:00.000Z",
        eventCount: 2,
        artifactRefs: [],
        blockerRefs: ["blocker.agent_runtime.test_fixture.cancelled"],
        staleness: {
          maxStalenessSeconds: 0,
          transitionRefs: ["agent_runtime_event_ingested"],
        },
      },
    ])

    expect(agentRuntimeStatusRows()).toEqual([
      {
        runId: "run.public.tui.rk5",
        status: "cancelled",
        label: "Cancelled",
        generatedAt: "2026-06-11T15:00:00.000Z",
        eventCount: 2,
        artifactRefs: [],
        blockerRefs: ["blocker.agent_runtime.test_fixture.cancelled"],
        freshness: {
          generatedAt: "2026-06-11T15:00:00.000Z",
          maxStalenessSeconds: 0,
          transitionRefs: ["agent_runtime_event_ingested"],
        },
        verificationRefs: [],
        reviewActionRefs: ["review.public.agent_runtime.blocker.agent_runtime.test_fixture.cancelled"],
      },
    ])
  })
})
