import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"

import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import { buildHomeProgram, fullAutoRunHeaderForState, renderContentView } from "../src/screens/home-core"
import type { FullAutoRunMobileProjection } from "../src/full-auto/full-auto-run-projection"

// `fullAutoRunHeaderForState` freshness-checks against real `Date.now()`
// (openagents #8982), so fixtures here must be freshly "now", not a
// hardcoded past instant — otherwise the freshness check itself makes an
// intentionally-active fixture look stale.
const now = new Date().toISOString()

const activeThread: MobileConversationThread = {
  threadRef: "thread.full-auto.header.1",
  title: "Full Auto run",
  status: "active",
  messageCount: 0,
  lastMessageAt: null,
  updatedAt: now,
  version: 1,
  messages: [],
}

const host: MobileConversationHost = {
  listThreads: async () => [activeThread],
  newThread: async () => ({ ok: true, thread: activeThread }),
  openThread: async () => activeThread,
  sendMessage: async () => ({ ok: true, thread: activeThread }),
}

const selection: Extract<MobileConversationSelection, { mode: "sync" }> = {
  mode: "sync",
  host,
  threads: [activeThread],
  archivedThreads: [],
  activeThread,
}

const runningProjection: FullAutoRunMobileProjection = {
  schema: "full_auto_run.mobile_projection.v1",
  runRef: "full_auto_run.header.test.0001",
  threadRef: activeThread.threadRef,
  objective: "Ship the mobile Full Auto live thread fast-follow.",
  doneCondition: "Mobile shows the live thread and header.",
  lifecycleState: "running",
  workspaceLabel: "openagents (fixture)",
  startedAt: now,
  updatedAt: now,
}

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("contract openagents_mobile.full_auto_run_header.v1 (openagents #8982)", () => {
  test("no active run: renders no header and preserves existing default rendering", () => {
    const program = buildHomeProgram({ conversation: selection })
    expect(fullAutoRunHeaderForState(program.initialState)).toBeNull()
    const view = JSON.stringify(renderContentView({ ...program.initialState, surfaceMode: "khala" }))
    expect(view).not.toContain("khala-full-auto-run-header")
    expect(view).toContain('"_tag":"Transcript"')
  })

  test("active run on the active thread: renders the lifecycle badge and objective above the transcript", () => {
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: runningProjection },
    })
    const header = fullAutoRunHeaderForState(program.initialState)
    expect(header).toEqual({
      lifecycleLabel: "Running",
      objective: runningProjection.objective,
      workspaceLabel: runningProjection.workspaceLabel,
    })
    const view = JSON.stringify(renderContentView({ ...program.initialState, surfaceMode: "khala" }))
    expect(view).toContain("khala-full-auto-run-header")
    expect(view).toContain("Running")
    expect(view).toContain(runningProjection.objective)
    // The header renders above the transcript, not in place of it.
    expect(view).toContain('"_tag":"Transcript"')
  })

  test("projection for a different thread than the active one: renders no header", () => {
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: {
        state: "active",
        projection: { ...runningProjection, threadRef: "thread.some-other-thread" },
      },
    })
    expect(fullAutoRunHeaderForState(program.initialState)).toBeNull()
  })

  test("stale projection (past the freshness window): renders no header even though lifecycle looks active", () => {
    const staleProjection: FullAutoRunMobileProjection = {
      ...runningProjection,
      updatedAt: "2020-01-01T00:00:00.000Z",
    }
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: staleProjection },
    })
    expect(fullAutoRunHeaderForState(program.initialState)).toBeNull()
  })

  test("live update without restart: setProjection pushes a lifecycle transition into rendered state", async () => {
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: runningProjection },
    })
    expect(fullAutoRunHeaderForState(program.initialState)?.lifecycleLabel).toBe("Running")

    program.fullAuto.setProjection({
      state: "active",
      projection: { ...runningProjection, lifecycleState: "paused", updatedAt: now },
    })
    await Effect.runPromise(settle)
    const updated = await Effect.runPromise(lastState(program))
    expect(fullAutoRunHeaderForState(updated)?.lifecycleLabel).toBe("Paused")
    const view = JSON.stringify(renderContentView({ ...updated, surfaceMode: "khala" }))
    expect(view).toContain("Paused")

    // A later poll reporting no active run clears the header — no restart
    // required, and the surface falls back to its unchanged default shape.
    program.fullAuto.setProjection({ state: "none" })
    await Effect.runPromise(settle)
    const cleared = await Effect.runPromise(lastState(program))
    expect(fullAutoRunHeaderForState(cleared)).toBeNull()
  })
})
