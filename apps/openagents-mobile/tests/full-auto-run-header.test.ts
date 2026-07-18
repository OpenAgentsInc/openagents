import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"

import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import { buildHomeProgram, fullAutoRunHeaderForState, renderContentView } from "../src/screens/home-core"
import type { FullAutoRunMobileProjection } from "../src/full-auto/full-auto-run-projection"
import type { FullAutoRunControlDispatchOutcome } from "../src/full-auto/full-auto-run-control-intent"

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
  runRef: "run.full-auto.header-test-0001",
  threadRef: activeThread.threadRef,
  objective: "Ship the mobile Full Auto live thread fast-follow.",
  doneCondition: "Mobile shows the live thread and header.",
  lifecycleState: "running",
  workspaceLabel: "openagents (fixture)",
  startedAt: now,
  updatedAt: now,
  lastTransition: { actor: "owner_ui", at: now },
  laneRef: "codex-local",
  accountRef: null,
  turnCap: 20,
  successfulAttempts: 3,
  failedAttempts: 0,
  rotationCount: 0,
  receiptSummary: null,
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
    expect(header).toMatchObject({
      runRef: runningProjection.runRef,
      lifecycleLabel: "Running",
      objective: runningProjection.objective,
      workspaceLabel: runningProjection.workspaceLabel,
      // MOB-FA-02 (#8994): Pause is legal from "running"; Stop is legal
      // from any non-terminal, non-draft state (including "running").
      control: { availableActions: ["pause", "stop"], pendingAction: null, lastOutcomeLabel: null },
      receipt: null,
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

  test("run not yet bound to a thread (threadRef: null): renders no header even on a signed-in thread view", () => {
    // Real #8981 shape: threadRef is nullable when Desktop has created the
    // run but not yet bound it to a khala-sync thread. null must never
    // spuriously "match" an active thread that also happens to be unset.
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: { ...runningProjection, threadRef: null } },
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

describe("MOB-FA-02 (#8994): remote Pause/Resume/Stop control", () => {
  test("availableActions reflects exact Desktop legality per lifecycle state", () => {
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: { ...runningProjection, lifecycleState: "paused" } },
    })
    expect(fullAutoRunHeaderForState(program.initialState)?.control.availableActions).toEqual(["resume", "stop"])
  })

  test("dispatching Pause shows a pending state, then applies the durable outcome and folds the resulting lifecycle into the header", async () => {
    let dispatchCalls: Array<Readonly<{ runRef: string; action: string }>> = []
    let resolveDispatch: ((outcome: FullAutoRunControlDispatchOutcome) => void) | null = null
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: runningProjection },
      fullAutoControl: async input => {
        dispatchCalls = [...dispatchCalls, input]
        return new Promise(resolve => { resolveDispatch = resolve })
      },
    })
    program.fullAuto.dispatchControl(runningProjection.runRef, "pause")
    await Effect.runPromise(settle)
    const pending = await Effect.runPromise(lastState(program))
    expect(fullAutoRunHeaderForState(pending)?.control.pendingAction).toBe("pause")
    expect(dispatchCalls).toEqual([{ runRef: runningProjection.runRef, action: "pause" }])

    resolveDispatch!({ state: "applied", resultLifecycleState: "paused" })
    await Effect.runPromise(settle)
    const applied = await Effect.runPromise(lastState(program))
    const header = fullAutoRunHeaderForState(applied)
    expect(header?.control.pendingAction).toBeNull()
    expect(header?.control.lastOutcomeLabel).toBe("Done.")
    // A receipted `applied` outcome is durable confirmed truth -- the
    // header's lifecycle updates immediately, not just on the next poll.
    expect(header?.lifecycleLabel).toBe("Paused")
  })

  test("a rejected outcome renders an honest status line and never mutates the displayed lifecycle", async () => {
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: runningProjection },
      fullAutoControl: async () => ({ state: "rejected", reason: "illegal_transition" }),
    })
    program.fullAuto.dispatchControl(runningProjection.runRef, "pause")
    await Effect.runPromise(settle)
    const rejected = await Effect.runPromise(lastState(program))
    const header = fullAutoRunHeaderForState(rejected)
    expect(header?.control.pendingAction).toBeNull()
    expect(header?.control.lastOutcomeLabel).toBe("Couldn't complete: illegal transition.")
    expect(header?.lifecycleLabel).toBe("Running")
  })

  test("a second dispatch while one is already pending is a no-op (never a double-dispatch)", async () => {
    let dispatchCount = 0
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: runningProjection },
      fullAutoControl: async () => {
        dispatchCount += 1
        return new Promise(() => undefined)
      },
    })
    program.fullAuto.dispatchControl(runningProjection.runRef, "pause")
    await Effect.runPromise(settle)
    program.fullAuto.dispatchControl(runningProjection.runRef, "pause")
    await Effect.runPromise(settle)
    expect(dispatchCount).toBe(1)
  })

  test("without fullAutoControl configured, dispatchControl is a safe no-op (no button would render, but a stray call never throws)", async () => {
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: runningProjection },
    })
    expect(() => program.fullAuto.dispatchControl(runningProjection.runRef, "pause")).not.toThrow()
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(fullAutoRunHeaderForState(state)?.control.pendingAction).toBeNull()
  })

  test("a terminal, fresh run keeps the header visible and renders the bounded run-report summary", () => {
    const terminalProjection: FullAutoRunMobileProjection = {
      ...runningProjection,
      lifecycleState: "completed",
      updatedAt: now,
      receiptSummary: {
        schema: "full_auto_run.mobile_receipt.v1",
        runRef: runningProjection.runRef,
        objectiveDigest: "a".repeat(64),
        doneConditionDigest: "b".repeat(64),
        workspaceRefDigest: null,
        state: "completed",
        turnCap: 20,
        successfulAttempts: 9,
        failedAttempts: 1,
        providerIdentities: ["codex-local"],
        providerTransitionCount: 0,
        providerTransitionDispositions: [],
        livenessGapCount: 0,
        recoveryActionsUsed: [],
        verifiedRefCount: 0,
        claimedRefCount: 0,
        progressDisposition: "unknown",
        usageKnown: false,
        reportRevision: 2,
        createdAt: now,
      },
    }
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: terminalProjection },
    })
    const header = fullAutoRunHeaderForState(program.initialState)
    expect(header).toMatchObject({
      lifecycleLabel: "Completed",
      control: { availableActions: [] },
      receipt: { successfulAttempts: 9, failedAttempts: 1, providerIdentities: ["codex-local"] },
    })
    const view = JSON.stringify(renderContentView({ ...program.initialState, surfaceMode: "khala" }))
    expect(view).toContain("khala-full-auto-run-receipt")
  })

  test("a terminal, STALE run renders no header at all (the freshness window still applies)", () => {
    const staleTerminalProjection: FullAutoRunMobileProjection = {
      ...runningProjection,
      lifecycleState: "completed",
      updatedAt: "2020-01-01T00:00:00.000Z",
    }
    const program = buildHomeProgram({
      conversation: selection,
      fullAutoRun: { state: "active", projection: staleTerminalProjection },
    })
    expect(fullAutoRunHeaderForState(program.initialState)).toBeNull()
  })
})
