/**
 * FA-UX-01 (#8974) unit coverage: launcher validation, the read-only run
 * view's state -> component-tree projection across every lifecycle state,
 * and the typed intent loop (launcher open/fields/start, run open/refresh/
 * pause/resume/stop/retry) run headlessly through the real registry --
 * mirrors ./fleet-workspace.test.ts's pattern.
 *
 * Oracle for behavior contracts:
 *   - openagents_desktop.full_auto_dedicated_launcher.v1
 *   - openagents_desktop.full_auto_read_only_run_view.v1
 *   - openagents_desktop.full_auto_play_pause_stop_lifecycle.v1
 */
import { describe, expect, test } from "vite-plus/test"
import { IntentRef, StaticPayload, resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  emptyFullAutoLauncherDraft,
  emptyFullAutoWorkspaceState,
  findActiveFullAutoRun,
  fullAutoLauncherView,
  fullAutoRunStatusLabel,
  fullAutoRunView,
  fullAutoTurnTimingLabel,
  formatFullAutoDuration,
  formatFullAutoRelativeTime,
  fullAutoWorkspaceIntents,
  fullAutoWorkspaceView,
  makeFullAutoWorkspaceHandlers,
  validateFullAutoLauncherDraft,
  type FullAutoCapableState,
  type FullAutoWorkspaceState,
} from "./full-auto-workspace.ts"
import type { FullAutoControlRun } from "../full-auto-control-contract.ts"
import { unavailableFullAutoRunRendererHost, type FullAutoRunRendererHost } from "../full-auto-run-ipc-contract.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

type AnyNode = Readonly<Record<string, unknown>>

const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: Array<AnyNode> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const item of value) walk(item)
      return
    }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag" || prop === "style" || prop === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}
const nodeByKey = (view: View, key: string): AnyNode | undefined =>
  collectNodes(view).find(node => node.key === key)

const baseRun = (overrides: Partial<FullAutoControlRun> = {}): FullAutoControlRun => ({
  runRef: "run-1",
  threadRef: "thread-1",
  title: "Ship the thing",
  objective: "Ship the thing end to end.",
  objectiveSource: "user",
  doneCondition: "pnpm run check is green on main.",
  workspaceRef: "/workspace/repo",
  lane: "codex-local",
  turnCap: 20,
  successfulAttempts: 2,
  failedAttempts: 0,
  state: "running",
  stateRevision: 3,
  terminalReason: null,
  predecessorRunRef: null,
  migratedFrom: null,
  createdAt: "2026-07-17T00:00:00.000Z",
  startedAt: "2026-07-17T00:00:01.000Z",
  lastProgressAt: "2026-07-17T00:05:00.000Z",
  pausedAt: null,
  stoppedAt: null,
  completedAt: null,
  transitions: [],
  stallCause: null,
  nextRetryAt: null,
  recoveryAction: "none",
  ...overrides,
})

describe("validateFullAutoLauncherDraft (FA-AC-54)", () => {
  test("refuses an empty title, objective, done condition, or workspace", () => {
    const draft = emptyFullAutoLauncherDraft()
    expect(validateFullAutoLauncherDraft(draft)).toEqual({ ok: false, error: "Give this run a title." })
    expect(validateFullAutoLauncherDraft({ ...draft, title: "Run" })).toEqual({ ok: false, error: "Describe the objective." })
    expect(validateFullAutoLauncherDraft({ ...draft, title: "Run", objective: "Do it" }))
      .toEqual({ ok: false, error: "State an explicit done condition." })
    expect(validateFullAutoLauncherDraft({ ...draft, title: "Run", objective: "Do it", doneCondition: "Done" }))
      .toEqual({ ok: false, error: "Choose a workspace." })
  })

  test("refuses a non-integer or out-of-range turn cap", () => {
    const draft = { ...emptyFullAutoLauncherDraft(), title: "Run", objective: "Do it", doneCondition: "Done", workspaceRef: "/ws" }
    expect(validateFullAutoLauncherDraft({ ...draft, turnCapText: "0" }).ok).toBe(false)
    expect(validateFullAutoLauncherDraft({ ...draft, turnCapText: "1001" }).ok).toBe(false)
    expect(validateFullAutoLauncherDraft({ ...draft, turnCapText: "3.5" }).ok).toBe(false)
    expect(validateFullAutoLauncherDraft({ ...draft, turnCapText: "not a number" }).ok).toBe(false)
  })

  test("accepts a complete draft and derives the turn cap; blank turn cap text omits it", () => {
    const draft = { ...emptyFullAutoLauncherDraft(), title: "Run", objective: "Do it", doneCondition: "Done", workspaceRef: "/ws" }
    // FA-WIRE-01: the turn cap doubles as guardrails.maxTurns so the
    // thread-level cap follows the owner's chosen cap.
    expect(validateFullAutoLauncherDraft({ ...draft, turnCapText: "5" })).toEqual({
      ok: true, turnCap: 5, routingPolicy: undefined, guardrails: { maxTurns: 5 },
    })
    expect(validateFullAutoLauncherDraft({ ...draft, turnCapText: "" })).toEqual({
      ok: true, turnCap: undefined, routingPolicy: undefined, guardrails: undefined,
    })
  })

  test("FA-WIRE-01: ordered fallback lanes build the routing policy (primary first); duplicates refuse typed", () => {
    const draft = { ...emptyFullAutoLauncherDraft(), title: "Run", objective: "Do it", doneCondition: "Done", workspaceRef: "/ws", turnCapText: "" }
    const withFallback = validateFullAutoLauncherDraft({ ...draft, fallbackLanes: ["acp:grok-cli", "acp:cursor-agent"] })
    expect(withFallback).toEqual({
      ok: true,
      turnCap: undefined,
      routingPolicy: [{ lane: "codex-local" }, { lane: "acp:grok-cli" }, { lane: "acp:cursor-agent" }],
      guardrails: undefined,
    })
    const duplicate = validateFullAutoLauncherDraft({ ...draft, fallbackLanes: ["codex-local"] })
    expect(duplicate).toEqual({ ok: false, error: "Each lane can appear only once in the rotation order." })
    const overLong = validateFullAutoLauncherDraft({ ...draft, fallbackLanes: ["a", "b", "c", "d", "e", "f", "g", "h"] })
    expect(overLong.ok).toBe(false)
  })

  test("FA-WIRE-01: max wall clock minutes converts to maxWallClockMs; invalid values refuse typed", () => {
    const draft = { ...emptyFullAutoLauncherDraft(), title: "Run", objective: "Do it", doneCondition: "Done", workspaceRef: "/ws", turnCapText: "" }
    expect(validateFullAutoLauncherDraft({ ...draft, maxWallClockMinutesText: "90" })).toEqual({
      ok: true, turnCap: undefined, routingPolicy: undefined, guardrails: { maxWallClockMs: 90 * 60_000 },
    })
    expect(validateFullAutoLauncherDraft({ ...draft, maxWallClockMinutesText: "0" }).ok).toBe(false)
    expect(validateFullAutoLauncherDraft({ ...draft, maxWallClockMinutesText: "ten" }).ok).toBe(false)
  })
})

describe("FA-UX-02 (#8997): turn-row time formatting", () => {
  test("renders relative time + duration, never raw ISO concatenation", () => {
    const now = new Date("2026-07-18T02:29:11.000Z")
    expect(formatFullAutoDuration(5 * 60_000 + 7000)).toBe("5m 7s")
    expect(formatFullAutoDuration(2 * 3600_000 + 3 * 60_000)).toBe("2h 3m")
    expect(formatFullAutoRelativeTime("2026-07-18T02:23:59.000Z", now)).toBe("5m 12s ago")
    const label = fullAutoTurnTimingLabel(
      { createdAt: "2026-07-18T02:18:52.000Z", updatedAt: "2026-07-18T02:23:59.000Z" },
      now,
    )
    expect(label).toBe("5m 12s ago · 5m 7s")
    expect(label).not.toContain("2026-07-18T")
    // Unparseable timestamps degrade honestly instead of throwing.
    expect(fullAutoTurnTimingLabel({ createdAt: "nope", updatedAt: "also nope" })).toBe("also nope")
  })
})

describe("findActiveFullAutoRun (FA-AC-39 concurrency slot)", () => {
  test("finds the single non-terminal, non-draft run and ignores terminal/draft rows", () => {
    const draft = baseRun({ runRef: "run-draft", state: "draft" })
    const completed = baseRun({ runRef: "run-done", state: "completed" })
    const active = baseRun({ runRef: "run-active", state: "paused" })
    expect(findActiveFullAutoRun([draft, completed, active])).toEqual(active)
    expect(findActiveFullAutoRun([draft, completed])).toBeNull()
  })
})

describe("fullAutoRunView (FA-AC-55): explicit lifecycle state across every named state", () => {
  const statesAndLabels: ReadonlyArray<readonly [FullAutoControlRun["state"], string]> = [
    ["draft", "Draft"],
    ["running", "Running"],
    ["pausing", "Pausing"],
    ["paused", "Paused"],
    ["retrying", "Retrying"],
    ["stalled", "Stalled"],
    ["completed", "Completed"],
    ["failed", "Failed"],
    ["stopped", "Stopped"],
    ["cap_reached", "Cap reached"],
  ]
  for (const [state, label] of statesAndLabels) {
    test(`renders the explicit "${label}" state for state="${state}", never a generic banner`, () => {
      const run = baseRun({ state })
      expect(fullAutoRunStatusLabel(run)).toBe(label)
      const workspaceState: FullAutoWorkspaceState = { ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: run.runRef, runs: [run] }
      const view = fullAutoRunView(workspaceState)
      const badge = nodeByKey(view, "full-auto-run-state")
      expect(badge?.label).toBe(label)
    })
  }

  test("Pause is primary control while Running; Stop is present and distinct", () => {
    const view = fullAutoRunView({ ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs: [baseRun({ state: "running" })] })
    expect(nodeByKey(view, "full-auto-run-pause")).not.toBeUndefined()
    expect(nodeByKey(view, "full-auto-run-resume")).toBeUndefined()
    expect(nodeByKey(view, "full-auto-run-stop")).not.toBeUndefined()
  })

  test("Resume is primary control while Paused; Pause is absent", () => {
    const view = fullAutoRunView({ ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs: [baseRun({ state: "paused" })] })
    expect(nodeByKey(view, "full-auto-run-resume")).not.toBeUndefined()
    expect(nodeByKey(view, "full-auto-run-pause")).toBeUndefined()
    expect(nodeByKey(view, "full-auto-run-stop")).not.toBeUndefined()
  })

  test("Retry now appears only when stalled with a recoverable cause", () => {
    const recoverable = fullAutoRunView({ ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs: [baseRun({ state: "stalled", recoveryAction: "retry_now", stallCause: "dispatch_overdue" })] })
    expect(nodeByKey(recoverable, "full-auto-run-retry-now")).not.toBeUndefined()
    const notRecoverable = fullAutoRunView({ ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs: [baseRun({ state: "stalled", recoveryAction: "stop_only", stallCause: "workspace_mismatch" })] })
    expect(nodeByKey(notRecoverable, "full-auto-run-retry-now")).toBeUndefined()
    expect(nodeByKey(notRecoverable, "full-auto-run-stall-copy")?.content).toContain("workspace mismatch")
  })

  test("Stop is absent once a run reaches a terminal state; Pause/Resume/Retry are also absent", () => {
    for (const state of ["completed", "failed", "stopped", "cap_reached"] as const) {
      const view = fullAutoRunView({ ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs: [baseRun({ state })] })
      expect(nodeByKey(view, "full-auto-run-stop")).toBeUndefined()
      expect(nodeByKey(view, "full-auto-run-pause")).toBeUndefined()
      expect(nodeByKey(view, "full-auto-run-resume")).toBeUndefined()
      expect(nodeByKey(view, "full-auto-run-retry-now")).toBeUndefined()
    }
  })

  test("pins objective and done condition at the top of the run view", () => {
    const view = fullAutoRunView({ ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs: [baseRun()] })
    expect(nodeByKey(view, "full-auto-run-objective")?.content).toBe("Ship the thing end to end.")
    expect(nodeByKey(view, "full-auto-run-done-condition")?.content).toContain("pnpm run check is green on main.")
  })

  test("fullAutoWorkspaceView selects the run view only once mode is run AND a run is active", () => {
    const runs = [baseRun()]
    expect(nodeByKey(fullAutoWorkspaceView({ ...emptyFullAutoWorkspaceState(), mode: "launcher", runs }), "full-auto-run-mission-contract")).toBeUndefined()
    expect(nodeByKey(fullAutoWorkspaceView({ ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: null, runs }), "full-auto-run-mission-contract")).toBeUndefined()
    expect(nodeByKey(fullAutoWorkspaceView({ ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs }), "full-auto-run-mission-contract")).not.toBeUndefined()
  })
})

describe("fullAutoLauncherView (FA-AC-54)", () => {
  test("Start is disabled until the mission contract is complete", () => {
    const view = fullAutoLauncherView(emptyFullAutoWorkspaceState())
    const start = nodeByKey(view, "full-auto-launcher-start") as { disabled?: boolean }
    expect(start.disabled).toBe(true)
  })

  test("Start is enabled once title/objective/done-condition/workspace are set", () => {
    const complete = { ...emptyFullAutoWorkspaceState(), launcher: { ...emptyFullAutoWorkspaceState().launcher, title: "Run", objective: "Do it", doneCondition: "Done", workspaceRef: "/ws" } }
    const view = fullAutoLauncherView(complete)
    const start = nodeByKey(view, "full-auto-launcher-start") as { disabled?: boolean }
    expect(start.disabled).toBe(false)
  })
})

type Harness = FullAutoCapableState

const makeHarness = (
  hostOverrides: Partial<FullAutoRunRendererHost> = {},
  initial: Partial<Harness> = {},
  selectedThreads: Array<string> = [],
) => {
  const host: FullAutoRunRendererHost = { ...unavailableFullAutoRunRendererHost, ...hostOverrides }
  return Effect.gen(function* () {
    const state = yield* SubscriptionRef.make<Harness>({ workspace: "chat", fullAuto: emptyFullAutoWorkspaceState(), ...initial })
    const registry = yield* makeIntentRegistry(
      fullAutoWorkspaceIntents,
      makeFullAutoWorkspaceHandlers(
        state,
        host,
        workspace => SubscriptionRef.update(state, current => ({ ...current, workspace })),
        threadRef => Effect.sync(() => { selectedThreads.push(threadRef) }),
      ),
    )
    return { state, registry }
  })
}

describe("Full Auto intent loop (FA-UX-01 #8974)", () => {
  test("opening the launcher with no active run resets the draft to defaults and switches to the full-auto workspace", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { state, registry } = yield* makeHarness({ list: async () => ({ runs: [], resolvedWorkspaceRef: "/resolved/workspace" }) })
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherOpened", StaticPayload(null))))
      const next = yield* SubscriptionRef.get(state)
      expect(next.fullAuto.mode).toBe("launcher")
      expect(next.fullAuto.launcher.workspaceRef).toBe("/resolved/workspace")
      expect(next.workspace).toBe("full-auto")
    }))
  })

  test("opening the launcher with an already-active run redirects straight to the run view (FA-AC-39)", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const active = baseRun({ runRef: "run-active", state: "running" })
      const { state, registry } = yield* makeHarness({
        list: async () => ({ runs: [active], resolvedWorkspaceRef: active.workspaceRef }),
        get: async () => ({ ok: true, value: active }),
        report: async () => ({ ok: true, value: { turns: [], providerTransitions: [] } }),
      })
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherOpened", StaticPayload(null))))
      const next = yield* SubscriptionRef.get(state)
      expect(next.fullAuto.mode).toBe("run")
      expect(next.fullAuto.activeRunRef).toBe("run-active")
      expect(next.workspace).toBe("full-auto")
    }))
  })

  test("field-change intents update exactly the named draft field", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { state, registry } = yield* makeHarness()
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherTitleChanged", StaticPayload("My run"))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherObjectiveChanged", StaticPayload("Objective"))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherDoneConditionChanged", StaticPayload("Done"))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherWorkspaceRefChanged", StaticPayload("/ws"))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherTurnCapChanged", StaticPayload("5"))))
      const draft = (yield* SubscriptionRef.get(state)).fullAuto.launcher
      expect(draft).toMatchObject({ title: "My run", objective: "Objective", doneCondition: "Done", workspaceRef: "/ws", turnCapText: "5" })
    }))
  })

  test("Start refuses an incomplete draft locally without calling the host", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      let called = false
      const { state, registry } = yield* makeHarness({ start: async () => { called = true; return { ok: true, value: baseRun() } } })
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherStartRequested", StaticPayload(null))))
      expect(called).toBe(false)
      const draft = (yield* SubscriptionRef.get(state)).fullAuto.launcher
      expect(draft.error).not.toBeNull()
    }))
  })

  test("Start on a valid draft calls host.start and switches into the run view on success", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const started = baseRun({ runRef: "run-new", state: "running" })
      const { state, registry } = yield* makeHarness({
        start: async () => ({ ok: true, value: started }),
        get: async () => ({ ok: true, value: started }),
        report: async () => ({ ok: true, value: { turns: [], providerTransitions: [] } }),
      }, {
        fullAuto: {
          ...emptyFullAutoWorkspaceState(),
          launcher: { ...emptyFullAutoWorkspaceState().launcher, title: "Run", objective: "Do it", doneCondition: "Done", workspaceRef: "/ws" },
        },
      })
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherStartRequested", StaticPayload(null))))
      const next = yield* SubscriptionRef.get(state)
      expect(next.fullAuto.mode).toBe("run")
      expect(next.fullAuto.activeRunRef).toBe("run-new")
      expect(next.fullAuto.launcher.error).toBeNull()
    }))
  })

  test("Start surfaces an active_run_conflict refusal as a launcher error, never a silent no-op", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { state, registry } = yield* makeHarness({
        start: async () => ({ ok: false, status: 409, error: { error: "active_run_conflict", message: "A Full Auto run is already active for this Desktop profile." } }),
      }, {
        fullAuto: {
          ...emptyFullAutoWorkspaceState(),
          launcher: { ...emptyFullAutoWorkspaceState().launcher, title: "Run", objective: "Do it", doneCondition: "Done", workspaceRef: "/ws" },
        },
      })
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherStartRequested", StaticPayload(null))))
      const draft = (yield* SubscriptionRef.get(state)).fullAuto.launcher
      expect(draft.error).toBe("A Full Auto run is already active for this Desktop profile.")
      expect(draft.submitting).toBe(false)
    }))
  })

  test("Pause/Resume/Stop/RetryNow each call the matching host method against the active runRef and refresh the cache", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const calls: Array<string> = []
      const run = baseRun({ runRef: "run-1", state: "paused" })
      const { state, registry } = yield* makeHarness({
        pause: async runRef => { calls.push(`pause:${runRef}`); return { ok: true, value: { ...run, state: "paused" } } },
        resume: async runRef => { calls.push(`resume:${runRef}`); return { ok: true, value: { ...run, state: "running" } } },
        stop: async runRef => { calls.push(`stop:${runRef}`); return { ok: true, value: { ...run, state: "stopped" } } },
        retryNow: async runRef => { calls.push(`retryNow:${runRef}`); return { ok: true, value: { ...run, state: "retrying" } } },
        get: async () => ({ ok: true, value: run }),
        report: async () => ({ ok: true, value: { turns: [], providerTransitions: [] } }),
      }, { fullAuto: { ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs: [run] } })
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoRunPauseRequested", StaticPayload(null))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoRunResumeRequested", StaticPayload(null))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoRunStopRequested", StaticPayload(null))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoRunRetryNowRequested", StaticPayload(null))))
      expect(calls).toEqual(["pause:run-1", "resume:run-1", "stop:run-1", "retryNow:run-1"])
      const next = yield* SubscriptionRef.get(state)
      expect(next.fullAuto.actionError).toBeNull()
    }))
  })

  test("FA-WIRE-01: fallback add/remove/wall-clock intents update the draft; Start submits the ordered policy + guardrails", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const startRequests: Array<Record<string, unknown>> = []
      const started = baseRun({ runRef: "run-policy", state: "running" })
      const { state, registry } = yield* makeHarness({
        start: async request => {
          startRequests.push(request as unknown as Record<string, unknown>)
          return { ok: true, value: started }
        },
        get: async () => ({ ok: true, value: started }),
        report: async () => ({ ok: true, value: { turns: [], providerTransitions: [] } }),
      }, {
        fullAuto: {
          ...emptyFullAutoWorkspaceState(),
          launcher: { ...emptyFullAutoWorkspaceState().launcher, title: "Run", objective: "Do it", doneCondition: "Done", workspaceRef: "/ws", turnCapText: "5" },
        },
      })
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherFallbackLaneAdded", StaticPayload("acp:grok-cli"))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherFallbackLaneAdded", StaticPayload("acp:cursor-agent"))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherFallbackLaneRemoved", StaticPayload("acp:cursor-agent"))))
      // Duplicates and the primary lane are no-ops.
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherFallbackLaneAdded", StaticPayload("acp:grok-cli"))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherFallbackLaneAdded", StaticPayload("codex-local"))))
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherMaxWallClockChanged", StaticPayload("120"))))
      const draft = (yield* SubscriptionRef.get(state)).fullAuto.launcher
      expect(draft.fallbackLanes).toEqual(["acp:grok-cli"])
      expect(draft.maxWallClockMinutesText).toBe("120")
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoLauncherStartRequested", StaticPayload(null))))
      expect(startRequests).toHaveLength(1)
      expect(startRequests[0]).toMatchObject({
        routingPolicy: [{ lane: "codex-local" }, { lane: "acp:grok-cli" }],
        guardrails: { maxTurns: 5, maxWallClockMs: 120 * 60_000 },
        turnCap: 5,
      })
    }))
  })

  test("FA-UX-02 (#8997): opening a run selects its bound thread through the injected canonical selection path, then re-asserts the full-auto workspace", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const run = baseRun({ runRef: "run-1", threadRef: "thread-1", state: "running" })
      const selectedThreads: Array<string> = []
      const { state, registry } = yield* makeHarness({
        get: async () => ({ ok: true, value: run }),
        report: async () => ({ ok: true, value: { turns: [], providerTransitions: [] } }),
      }, { fullAuto: { ...emptyFullAutoWorkspaceState(), runs: [run] } }, selectedThreads)
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoRunOpened", StaticPayload("run-1"))))
      expect(selectedThreads).toEqual(["thread-1"])
      const next = yield* SubscriptionRef.get(state)
      expect(next.workspace).toBe("full-auto")
      expect(next.fullAuto.mode).toBe("run")
    }))
  })

  test("a refused mutation surfaces its message as actionError without throwing", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const run = baseRun({ runRef: "run-1", state: "running" })
      const { state, registry } = yield* makeHarness({
        pause: async () => ({ ok: false, status: 409, error: { error: "illegal_transition", message: "Pause is not legal from state stopped." } }),
      }, { fullAuto: { ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs: [run] } })
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopFullAutoRunPauseRequested", StaticPayload(null))))
      const next = yield* SubscriptionRef.get(state)
      expect(next.fullAuto.actionError).toBe("Pause is not legal from state stopped.")
    }))
  })
})
