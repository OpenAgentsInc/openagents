/**
 * Full Auto launcher + read-only run view (FA-UX-01, #8974).
 *
 * Replaces the composer-embedded Full Auto toggle with a dedicated left-rail
 * entry point and a dedicated read-only run view, per
 * specs/desktop/full-auto.product-spec.md rev 10. Pure Effect Native: state,
 * typed intents, and a `state -> View` projection, following the same
 * pattern as ./fleet-workspace.ts. Every mutation routes through
 * `../full-auto-run-ipc-contract.ts`'s renderer host, which is a thin,
 * schema-decoded IPC transport over the exact main-owned action functions
 * (`../full-auto-run-actions.ts`) the opt-in HTTP control server also uses --
 * one transition service, never a renderer-invented state machine.
 */
import {
  Badge,
  Button,
  Card,
  ComponentValueBinding,
  IntentRef,
  Select,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  defineIntent,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"

import {
  decodeFullAutoRunListResult,
  decodeFullAutoRunHandoffOutcome,
  decodeFullAutoRunOutcome,
  decodeFullAutoRunReportOutcome,
  unavailableFullAutoRunRendererHost,
  type FullAutoRunProjection,
  type FullAutoRunReportView,
  type FullAutoRunRendererHost,
} from "../full-auto-run-ipc-contract.ts"

export const FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP = 20
export const FULL_AUTO_LAUNCHER_DEFAULT_LANE = "codex-local"
export const FULL_AUTO_LAUNCHER_DEFAULT_DONE_CONDITION =
  "Complete the objective, run the relevant verification, and report the result or any concrete blocker."
/** FA-WIRE-01 (#8996): mirrors FULL_AUTO_ROUTING_POLICY_LIMIT in
 * full-auto-registry.ts (renderer-boundary duplicate; see the note in
 * full-auto-run-ipc-contract.ts for why the value is copied, not imported). */
export const FULL_AUTO_LAUNCHER_ROUTING_POLICY_LIMIT = 8

/** Fixed lane option set for v1 (mirrors the composer's admitted lanes).
 * Start still re-validates real L2 eligibility server-side -- an option
 * appearing here is not a promise of admission. This is the SAME admitted
 * Full-Auto-eligible lane projection the launcher's single-lane picker
 * already uses; the FA-WIRE-01 ordered fallback picker sources from it too. */
export const fullAutoLauncherLaneOptions: ReadonlyArray<Readonly<{ value: string; label: string }>> = [
  { value: "codex-local", label: "Codex" },
  { value: "claude-local", label: "Claude" },
  { value: "acp:grok-cli", label: "Grok CLI" },
  { value: "acp:cursor-agent", label: "Cursor Agent" },
]

export const fullAutoLauncherLaneLabel = (laneRef: string): string =>
  fullAutoLauncherLaneOptions.find(option => option.value === laneRef)?.label ?? laneRef

export const inferFullAutoRunTitle = (objective: string): string => {
  const firstLine = objective.trim().split(/\r?\n/, 1)[0]?.replace(/\s+/g, " ") ?? ""
  if (firstLine === "") return "Full Auto run"
  return firstLine.length <= 80 ? firstLine : `${firstLine.slice(0, 77).trimEnd()}…`
}

export type FullAutoLauncherDraft = Readonly<{
  title: string
  objective: string
  doneCondition: string
  workspaceRef: string
  lane: string
  /** Exact provider model, or blank for the lane default. */
  model: string
  /** FA-WIRE-01 (#8996): ordered fallback lanes AFTER the primary `lane`.
   * Non-empty means Start submits an ordered routingPolicy; empty keeps the
   * exact single-lane default behavior. */
  fallbackLanes: ReadonlyArray<string>
  turnCapText: string
  /** FA-WIRE-01 (#8996): optional wall-clock guardrail, in minutes (converted
   * to maxWallClockMs on submit). Blank = no wall-clock guardrail. */
  maxWallClockMinutesText: string
  submitting: boolean
  error: string | null
}>

export const emptyFullAutoLauncherDraft = (
  defaults?: Readonly<{ workspaceRef?: string }>,
): FullAutoLauncherDraft => ({
  title: "",
  objective: "",
  doneCondition: "",
  workspaceRef: defaults?.workspaceRef ?? "",
  lane: FULL_AUTO_LAUNCHER_DEFAULT_LANE,
  model: "",
  // The default path is Codex with Claude available for typed automatic
  // rotation. Main still re-validates both lanes before admitting the run.
  fallbackLanes: ["claude-local"],
  turnCapText: String(FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP),
  maxWallClockMinutesText: "",
  submitting: false,
  error: null,
})

export type FullAutoWorkspaceState = Readonly<{
  /** "launcher" shows the mission-contract form; "run" shows the read-only
   * run view for `activeRunRef`. Independent of the shell's own top-level
   * `workspace` field, which merely routes to this sub-state's view. */
  mode: "launcher" | "run"
  launcher: FullAutoLauncherDraft
  /** Last known `list()` projection -- sidebar rows and the launcher's
   * active-run-conflict short-circuit both read this cache. */
  runs: ReadonlyArray<FullAutoRunProjection>
  /** The SAME workspace value `startFullAutoRunAction` checks the launcher's
   * submitted `workspaceRef` against (from the last `list()` read) -- the
   * launcher pre-fills from this, never a possibly-divergent, unrelated
   * "current working directory" bridge. */
  resolvedWorkspaceRef: string | null
  activeRunRef: string | null
  activeReport: FullAutoRunReportView | null
  /** Surfaces a Pause/Resume/Stop/Retry failure without derailing the
   * pinned run header -- distinct from the launcher's own validation error. */
  actionError: string | null
}>

export const emptyFullAutoWorkspaceState = (): FullAutoWorkspaceState => ({
  mode: "launcher",
  launcher: emptyFullAutoLauncherDraft(),
  runs: [],
  resolvedWorkspaceRef: null,
  activeRunRef: null,
  activeReport: null,
  actionError: null,
})

/** Non-terminal, non-draft states used to order the monitor. Multiple rows
 * may be active concurrently; identity and controls are always runRef-scoped. */
const ACTIVE_RUN_STATES = new Set(["running", "pausing", "paused", "retrying", "stalled"])
export const activeFullAutoRuns = (
  runs: ReadonlyArray<FullAutoRunProjection>,
): ReadonlyArray<FullAutoRunProjection> => runs.filter(run => ACTIVE_RUN_STATES.has(run.state))

// ---------------------------------------------------------------------------
// Intents
// ---------------------------------------------------------------------------

export const DesktopFullAutoLauncherOpened = defineIntent("DesktopFullAutoLauncherOpened", Schema.Null)
export const DesktopFullAutoLauncherTitleChanged = defineIntent("DesktopFullAutoLauncherTitleChanged", Schema.String)
export const DesktopFullAutoLauncherObjectiveChanged = defineIntent("DesktopFullAutoLauncherObjectiveChanged", Schema.String)
export const DesktopFullAutoLauncherDoneConditionChanged = defineIntent("DesktopFullAutoLauncherDoneConditionChanged", Schema.String)
export const DesktopFullAutoLauncherWorkspaceRefChanged = defineIntent("DesktopFullAutoLauncherWorkspaceRefChanged", Schema.String)
export const DesktopFullAutoLauncherLaneChanged = defineIntent("DesktopFullAutoLauncherLaneChanged", Schema.String)
export const DesktopFullAutoLauncherModelChanged = defineIntent("DesktopFullAutoLauncherModelChanged", Schema.String)
// FA-WIRE-01 (#8996): ordered fallback-lane picker + wall-clock guardrail.
export const DesktopFullAutoLauncherFallbackLaneAdded = defineIntent("DesktopFullAutoLauncherFallbackLaneAdded", Schema.String)
export const DesktopFullAutoLauncherFallbackLaneRemoved = defineIntent("DesktopFullAutoLauncherFallbackLaneRemoved", Schema.String)
export const DesktopFullAutoLauncherMaxWallClockChanged = defineIntent("DesktopFullAutoLauncherMaxWallClockChanged", Schema.String)
export const DesktopFullAutoLauncherTurnCapChanged = defineIntent("DesktopFullAutoLauncherTurnCapChanged", Schema.String)
export const DesktopFullAutoLauncherCancelled = defineIntent("DesktopFullAutoLauncherCancelled", Schema.Null)
export const DesktopFullAutoLauncherStartRequested = defineIntent("DesktopFullAutoLauncherStartRequested", Schema.Null)
export const DesktopFullAutoRunOpened = defineIntent("DesktopFullAutoRunOpened", Schema.String)
export const DesktopFullAutoRunRefreshed = defineIntent("DesktopFullAutoRunRefreshed", Schema.Null)
export const DesktopFullAutoRunsListRefreshed = defineIntent("DesktopFullAutoRunsListRefreshed", Schema.Null)
export const DesktopFullAutoRunPauseRequested = defineIntent("DesktopFullAutoRunPauseRequested", Schema.Null)
export const DesktopFullAutoRunResumeRequested = defineIntent("DesktopFullAutoRunResumeRequested", Schema.Null)
export const DesktopFullAutoRunStopRequested = defineIntent("DesktopFullAutoRunStopRequested", Schema.Null)
export const DesktopFullAutoRunStopByRefRequested = defineIntent("DesktopFullAutoRunStopByRefRequested", Schema.String)
export const DesktopFullAutoRunRetryNowRequested = defineIntent("DesktopFullAutoRunRetryNowRequested", Schema.Null)
export const DesktopFullAutoRunHandoffRequested = defineIntent("DesktopFullAutoRunHandoffRequested", Schema.String)

export const fullAutoWorkspaceIntents = [
  DesktopFullAutoLauncherOpened,
  DesktopFullAutoLauncherTitleChanged,
  DesktopFullAutoLauncherObjectiveChanged,
  DesktopFullAutoLauncherDoneConditionChanged,
  DesktopFullAutoLauncherWorkspaceRefChanged,
  DesktopFullAutoLauncherLaneChanged,
  DesktopFullAutoLauncherModelChanged,
  DesktopFullAutoLauncherFallbackLaneAdded,
  DesktopFullAutoLauncherFallbackLaneRemoved,
  DesktopFullAutoLauncherMaxWallClockChanged,
  DesktopFullAutoLauncherTurnCapChanged,
  DesktopFullAutoLauncherCancelled,
  DesktopFullAutoLauncherStartRequested,
  DesktopFullAutoRunOpened,
  DesktopFullAutoRunRefreshed,
  DesktopFullAutoRunsListRefreshed,
  DesktopFullAutoRunPauseRequested,
  DesktopFullAutoRunResumeRequested,
  DesktopFullAutoRunStopRequested,
  DesktopFullAutoRunStopByRefRequested,
  DesktopFullAutoRunRetryNowRequested,
  DesktopFullAutoRunHandoffRequested,
] as const

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

export type FullAutoCapableState = Readonly<{ fullAuto: FullAutoWorkspaceState; workspace: string }>

const withFullAuto = <S extends FullAutoCapableState>(
  current: S,
  patch: Partial<FullAutoWorkspaceState>,
): S => ({ ...current, fullAuto: { ...current.fullAuto, ...patch } })

const upsertRun = (runs: ReadonlyArray<FullAutoRunProjection>, run: FullAutoRunProjection): ReadonlyArray<FullAutoRunProjection> =>
  runs.some(existing => existing.runRef === run.runRef)
    ? runs.map(existing => existing.runRef === run.runRef ? run : existing)
    : [...runs, run]

export type FullAutoLauncherValidation = Readonly<
  | {
      ok: true
      turnCap: number | undefined
      title: string
      doneCondition: string
      /** FA-WIRE-01 (#8996): the ordered policy Start submits (primary lane
       * first), or undefined for the exact single-lane default. */
      routingPolicy: ReadonlyArray<{ lane: string }> | undefined
      /** FA-WIRE-01 (#8996): the owner guardrails Start submits. The turn cap
       * doubles as guardrails.maxTurns so the thread-level cap follows the
       * owner's chosen cap instead of the built-in 20. */
      guardrails: Readonly<{ maxTurns?: number; maxWallClockMs?: number }> | undefined
    }
  | { ok: false; error: string }
>

/** Validates the compact one-click draft and resolves optional advanced
 * fields into the explicit mission contract `startFullAutoRunAction` requires.
 * Objective and workspace are the only required owner inputs; title and done
 * condition have deterministic, visible defaults and remain editable under
 * Advanced. Turn cap must be a 1-1000 integer when present --
 * plus the FA-WIRE-01 ordered-policy/guardrail fields (distinct bounded
 * fallback lanes, positive wall-clock minutes). Mirrors FA-AC-54's "Start is
 * enabled only once the mission contract is complete"; refusals are the exact
 * typed reasons the run setup view renders. */
export const validateFullAutoLauncherDraft = (
  draft: FullAutoLauncherDraft,
): FullAutoLauncherValidation => {
  if (draft.objective.trim() === "") return { ok: false, error: "Describe the objective." }
  if (draft.workspaceRef.trim() === "") return { ok: false, error: "Choose a workspace." }
  const trimmedCap = draft.turnCapText.trim()
  let turnCap: number | undefined
  if (trimmedCap !== "") {
    const parsed = Number(trimmedCap)
    if (!Number.isInteger(parsed) || parsed < 1 || parsed > 1000) {
      return { ok: false, error: "Turn cap must be a whole number between 1 and 1000." }
    }
    turnCap = parsed
  }
  // FA-WIRE-01: ordered fallback lanes -- duplicates refused (a silently
  // deduplicated policy would rotate differently than the one reviewed).
  const orderedLanes = [draft.lane, ...draft.fallbackLanes]
  if (new Set(orderedLanes).size !== orderedLanes.length) {
    return { ok: false, error: "Each lane can appear only once in the rotation order." }
  }
  if (orderedLanes.length > FULL_AUTO_LAUNCHER_ROUTING_POLICY_LIMIT) {
    return { ok: false, error: `At most ${FULL_AUTO_LAUNCHER_ROUTING_POLICY_LIMIT} lanes can be in the rotation order.` }
  }
  const trimmedWallClock = draft.maxWallClockMinutesText.trim()
  let maxWallClockMs: number | undefined
  if (trimmedWallClock !== "") {
    const minutes = Number(trimmedWallClock)
    if (!Number.isInteger(minutes) || minutes < 1) {
      return { ok: false, error: "Max wall clock must be a whole number of minutes (1 or more)." }
    }
    maxWallClockMs = minutes * 60_000
  }
  const guardrails = {
    ...(turnCap === undefined ? {} : { maxTurns: turnCap }),
    ...(maxWallClockMs === undefined ? {} : { maxWallClockMs }),
  }
  return {
    ok: true,
    title: draft.title.trim() === "" ? inferFullAutoRunTitle(draft.objective) : draft.title.trim(),
    doneCondition: draft.doneCondition.trim() === ""
      ? FULL_AUTO_LAUNCHER_DEFAULT_DONE_CONDITION
      : draft.doneCondition.trim(),
    turnCap,
    routingPolicy: draft.fallbackLanes.length === 0
      ? undefined
      : orderedLanes.map(lane => ({ lane })),
    guardrails: Object.keys(guardrails).length === 0 ? undefined : guardrails,
  }
}

export const makeFullAutoWorkspaceHandlers = <S extends FullAutoCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  host: FullAutoRunRendererHost = unavailableFullAutoRunRendererHost,
  selectWorkspace: (workspace: string) => Effect.Effect<void> = () => Effect.void,
  /**
   * FA-UX-02 (#8997): select the run's bound thread through the SAME
   * canonical thread-selection path ordinary chats use (the shell passes its
   * own commitLocalSession), so `state.notes` carries the bound thread's real
   * conversation and the run view can mount the canonical timeline component
   * -- never a parallel mini-renderer. The shell's selection sets the chat
   * workspace, so callers re-assert "full-auto" AFTER this settles.
   */
  selectThread: (threadRef: string) => Effect.Effect<void> = () => Effect.void,
) => {
  const selectRunThread = (runRef: string) => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const threadRef = current.fullAuto.runs.find(run => run.runRef === runRef)?.threadRef ?? null
    if (threadRef !== null) yield* selectThread(threadRef)
  })
  const refreshList = () => Effect.gen(function* () {
    const raw = yield* Effect.promise(() => host.list().catch(() => null))
    const decoded = decodeFullAutoRunListResult(raw)
    if (decoded === null) return
    yield* SubscriptionRef.update(state, current => withFullAuto(current, { runs: decoded.runs, resolvedWorkspaceRef: decoded.resolvedWorkspaceRef }))
  })

  const refreshActiveRun = () => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const runRef = current.fullAuto.activeRunRef
    if (runRef === null) return
    const rawRun = yield* Effect.promise(() => host.get(runRef).catch(() => null))
    const outcome = decodeFullAutoRunOutcome(rawRun)
    if (outcome?.ok === true) {
      yield* SubscriptionRef.update(state, next => withFullAuto(next, { runs: upsertRun(next.fullAuto.runs, outcome.value) }))
    }
    const rawReport = yield* Effect.promise(() => host.report(runRef).catch(() => null))
    const reportOutcome = decodeFullAutoRunReportOutcome(rawReport)
    if (reportOutcome?.ok === true) {
      yield* SubscriptionRef.update(state, next => withFullAuto(next, { activeReport: reportOutcome.value }))
    }
  })

  const runMutationFor = (
    runRef: string,
    call: (runRef: string) => Promise<unknown>,
  ) => Effect.gen(function* () {
    const raw = yield* Effect.promise(() => call(runRef).catch(() => null))
    const outcome = decodeFullAutoRunOutcome(raw)
    if (outcome === null) {
      yield* SubscriptionRef.update(state, next => withFullAuto(next, { actionError: "That action failed: an unexpected response was received." }))
      return
    }
    if (outcome.ok) {
      yield* SubscriptionRef.update(state, next => withFullAuto(next, {
        runs: upsertRun(next.fullAuto.runs, outcome.value),
        actionError: null,
      }))
      const current = yield* SubscriptionRef.get(state)
      if (current.fullAuto.activeRunRef === runRef) yield* refreshActiveRun()
      else yield* refreshList()
      return
    }
    yield* SubscriptionRef.update(state, next => withFullAuto(next, { actionError: outcome.error.message }))
  })

  const runMutation = (
    call: (runRef: string) => Promise<unknown>,
  ) => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const runRef = current.fullAuto.activeRunRef
    if (runRef === null) return
    yield* runMutationFor(runRef, call)
  })

  return {
    DesktopFullAutoLauncherOpened: () => Effect.gen(function* () {
      yield* refreshList()
      yield* SubscriptionRef.update(state, next => withFullAuto(next, {
        mode: "launcher",
        launcher: emptyFullAutoLauncherDraft({ workspaceRef: next.fullAuto.resolvedWorkspaceRef ?? undefined }),
        activeRunRef: null,
        activeReport: null,
        actionError: null,
      }))
      yield* selectWorkspace("full-auto")
    }),
    DesktopFullAutoLauncherTitleChanged: (value: string) =>
      SubscriptionRef.update(state, current => withFullAuto(current, { launcher: { ...current.fullAuto.launcher, title: value, error: null } })),
    DesktopFullAutoLauncherObjectiveChanged: (value: string) =>
      SubscriptionRef.update(state, current => withFullAuto(current, { launcher: { ...current.fullAuto.launcher, objective: value, error: null } })),
    DesktopFullAutoLauncherDoneConditionChanged: (value: string) =>
      SubscriptionRef.update(state, current => withFullAuto(current, { launcher: { ...current.fullAuto.launcher, doneCondition: value, error: null } })),
    DesktopFullAutoLauncherWorkspaceRefChanged: (value: string) =>
      SubscriptionRef.update(state, current => withFullAuto(current, { launcher: { ...current.fullAuto.launcher, workspaceRef: value, error: null } })),
    DesktopFullAutoLauncherLaneChanged: (value: string) =>
      SubscriptionRef.update(state, current => withFullAuto(current, {
        launcher: {
          ...current.fullAuto.launcher,
          lane: value,
          model: "",
          // The primary lane can never also be a fallback -- keep the ordered
          // list coherent when the primary changes. Codex and Claude remain
          // each other's default fallback when either is selected.
          fallbackLanes: (() => {
            const remaining = current.fullAuto.launcher.fallbackLanes.filter(lane => lane !== value)
            if (remaining.length > 0) return remaining
            if (value === "codex-local") return ["claude-local"]
            if (value === "claude-local") return ["codex-local"]
            return []
          })(),
          error: null,
        },
      })),
    DesktopFullAutoLauncherModelChanged: (value: string) =>
      SubscriptionRef.update(state, current => withFullAuto(current, {
        launcher: { ...current.fullAuto.launcher, model: value, error: null },
      })),
    // FA-WIRE-01 (#8996): append one fallback lane (order = priority).
    // Duplicates and the primary lane are no-ops here; validation renders the
    // typed reason if a duplicate ever reaches the draft another way.
    DesktopFullAutoLauncherFallbackLaneAdded: (value: string) =>
      SubscriptionRef.update(state, current => {
        const launcher = current.fullAuto.launcher
        if (value === "" || value === launcher.lane || launcher.fallbackLanes.includes(value)) return current
        return withFullAuto(current, {
          launcher: { ...launcher, fallbackLanes: [...launcher.fallbackLanes, value], error: null },
        })
      }),
    DesktopFullAutoLauncherFallbackLaneRemoved: (value: string) =>
      SubscriptionRef.update(state, current => withFullAuto(current, {
        launcher: {
          ...current.fullAuto.launcher,
          fallbackLanes: current.fullAuto.launcher.fallbackLanes.filter(lane => lane !== value),
          error: null,
        },
      })),
    DesktopFullAutoLauncherMaxWallClockChanged: (value: string) =>
      SubscriptionRef.update(state, current => withFullAuto(current, { launcher: { ...current.fullAuto.launcher, maxWallClockMinutesText: value, error: null } })),
    DesktopFullAutoLauncherTurnCapChanged: (value: string) =>
      SubscriptionRef.update(state, current => withFullAuto(current, { launcher: { ...current.fullAuto.launcher, turnCapText: value, error: null } })),
    DesktopFullAutoLauncherCancelled: () => Effect.gen(function* () {
      yield* SubscriptionRef.update(state, current => withFullAuto(current, { launcher: emptyFullAutoLauncherDraft() }))
      yield* selectWorkspace("chat")
    }),
    DesktopFullAutoLauncherStartRequested: () => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const draft = current.fullAuto.launcher
      if (draft.submitting) return
      const validation = validateFullAutoLauncherDraft(draft)
      if (!validation.ok) {
        yield* SubscriptionRef.update(state, next => withFullAuto(next, { launcher: { ...next.fullAuto.launcher, error: validation.error } }))
        return
      }
      yield* SubscriptionRef.update(state, next => withFullAuto(next, { launcher: { ...next.fullAuto.launcher, submitting: true, error: null } }))
      const raw = yield* Effect.promise(() => host.start({
        workspaceRef: draft.workspaceRef.trim(),
        title: validation.title,
        objective: draft.objective.trim(),
        doneCondition: validation.doneCondition,
        lane: draft.lane,
        ...(draft.model.trim() === "" ? {} : { model: draft.model.trim() }),
        ...(validation.turnCap === undefined ? {} : { turnCap: validation.turnCap }),
        // FA-WIRE-01 (#8996): the ordered policy + owner guardrails, validated
        // above and re-validated fail-closed main-side.
        ...(validation.routingPolicy === undefined ? {} : { routingPolicy: validation.routingPolicy }),
        ...(validation.guardrails === undefined ? {} : { guardrails: validation.guardrails }),
      }).catch(() => null))
      const outcome = decodeFullAutoRunOutcome(raw)
      if (outcome === null) {
        yield* SubscriptionRef.update(state, next => withFullAuto(next, {
          launcher: { ...next.fullAuto.launcher, submitting: false, error: "Start failed: an unexpected response was received." },
        }))
        return
      }
      if (!outcome.ok) {
        yield* SubscriptionRef.update(state, next => withFullAuto(next, {
          launcher: { ...next.fullAuto.launcher, submitting: false, error: outcome.error.message },
        }))
        return
      }
      yield* SubscriptionRef.update(state, next => withFullAuto(next, {
        mode: "run",
        activeRunRef: outcome.value.runRef,
        runs: upsertRun(next.fullAuto.runs, outcome.value),
        launcher: { ...next.fullAuto.launcher, submitting: false, error: null },
        actionError: null,
      }))
      yield* refreshActiveRun()
      // FA-UX-02 (#8997): hydrate the freshly minted bound thread through the
      // canonical selection path, then keep the full-auto surface in front.
      yield* selectRunThread(outcome.value.runRef)
      yield* selectWorkspace("full-auto")
    }),
    DesktopFullAutoRunOpened: (runRef: string) => Effect.gen(function* () {
      yield* SubscriptionRef.update(state, current => withFullAuto(current, { mode: "run", activeRunRef: runRef, actionError: null }))
      yield* refreshActiveRun()
      // FA-UX-02 (#8997): the canonical thread selection MUST settle before
      // the workspace re-asserts full-auto (the shell's selection path lands
      // in the chat workspace).
      yield* selectRunThread(runRef)
      yield* selectWorkspace("full-auto")
    }),
    DesktopFullAutoRunRefreshed: () => refreshActiveRun(),
    DesktopFullAutoRunsListRefreshed: () => refreshList(),
    DesktopFullAutoRunPauseRequested: () => runMutation(runRef => host.pause(runRef)),
    DesktopFullAutoRunResumeRequested: () => runMutation(runRef => host.resume(runRef)),
    DesktopFullAutoRunStopRequested: () => runMutation(runRef => host.stop(runRef)),
    DesktopFullAutoRunStopByRefRequested: (runRef: string) => runMutationFor(runRef, ref => host.stop(ref)),
    DesktopFullAutoRunRetryNowRequested: () => runMutation(runRef => host.retryNow(runRef)),
    DesktopFullAutoRunHandoffRequested: (targetLaneRef: string) => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const runRef = current.fullAuto.activeRunRef
      if (runRef === null) return
      const raw = yield* Effect.promise(() => host.handoff({
        runRef,
        targetLaneRef,
        reason: "Provider handoff requested from the dedicated Full Auto run view.",
      }).catch(() => null))
      const outcome = decodeFullAutoRunHandoffOutcome(raw)
      if (outcome === null) {
        yield* SubscriptionRef.update(state, next => withFullAuto(next, { actionError: "Provider handoff failed: an unexpected response was received." }))
        return
      }
      if (!outcome.ok) {
        yield* SubscriptionRef.update(state, next => withFullAuto(next, { actionError: outcome.error.message }))
        return
      }
      yield* SubscriptionRef.update(state, next => withFullAuto(next, {
        runs: upsertRun(next.fullAuto.runs, outcome.value.run),
        actionError: null,
      }))
      yield* refreshActiveRun()
    }),
  }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

const RUN_STATE_LABEL: Readonly<Record<FullAutoRunProjection["state"], string>> = {
  draft: "Draft",
  running: "Running",
  pausing: "Pausing",
  paused: "Paused",
  retrying: "Retrying",
  stalled: "Stalled",
  completed: "Completed",
  failed: "Failed",
  stopped: "Stopped",
  cap_reached: "Cap reached",
}

const RUN_STATE_TONE: Readonly<Record<FullAutoRunProjection["state"], "neutral" | "success" | "warn" | "danger">> = {
  draft: "neutral",
  running: "success",
  pausing: "warn",
  paused: "neutral",
  retrying: "warn",
  stalled: "danger",
  completed: "success",
  failed: "danger",
  stopped: "neutral",
  cap_reached: "warn",
}

/** Sidebar-facing status label -- the accessible text FA-AC-57 requires
 * alongside (never in place of) the restrained state indicator. */
export const fullAutoRunStatusLabel = (run: FullAutoRunProjection): string => RUN_STATE_LABEL[run.state]

// ---------------------------------------------------------------------------
// FA-UX-02 (#8997): turn-row time formatting -- provider chip + disposition +
// relative time + duration ("completed · 5m 12s ago · 5m 7s"), never raw ISO
// concatenation. Pure and exported so both the Effect Native projection and
// the React surface share one formatter (and the oracle test can pin it).
// ---------------------------------------------------------------------------

export const formatFullAutoDuration = (ms: number): string => {
  if (!Number.isFinite(ms) || ms < 0) return ""
  const totalSeconds = Math.round(ms / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  if (hours > 0) return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`
  if (minutes > 0) return seconds > 0 ? `${minutes}m ${seconds}s` : `${minutes}m`
  return `${seconds}s`
}

export const formatFullAutoRelativeTime = (iso: string, now: Date = new Date()): string => {
  const timestamp = Date.parse(iso)
  if (!Number.isFinite(timestamp)) return iso
  const elapsed = now.getTime() - timestamp
  if (elapsed < 1000) return "just now"
  return `${formatFullAutoDuration(elapsed)} ago`
}

/** "5m 12s ago · 5m 7s" -- relative end time plus the turn's own duration. */
export const fullAutoTurnTimingLabel = (
  turn: Readonly<{ createdAt: string; updatedAt: string }>,
  now: Date = new Date(),
): string => {
  const started = Date.parse(turn.createdAt)
  const ended = Date.parse(turn.updatedAt)
  const relative = formatFullAutoRelativeTime(turn.updatedAt, now)
  if (!Number.isFinite(started) || !Number.isFinite(ended) || ended < started) return relative
  const duration = formatFullAutoDuration(ended - started)
  return duration === "" ? relative : `${relative} · ${duration}`
}

export const fullAutoLauncherView = (fullAuto: FullAutoWorkspaceState): View => {
  const draft = fullAuto.launcher
  const validation = validateFullAutoLauncherDraft(draft)
  return Stack(
    {
      key: "workspace-full-auto-launcher",
      direction: "column",
      gap: "3",
      style: { width: "full", maxWidth: "2xl", flex: 1, minHeight: 0, paddingTop: "2" },
    },
    [
      Stack({ key: "full-auto-launcher-heading", direction: "row", gap: "2", align: "center" }, [
        Text({ key: "full-auto-launcher-title", content: "Full Auto", variant: "heading", color: "textPrimary" }),
      ]),
      Text({
        key: "full-auto-launcher-copy",
        content: "One durable, unattended run. Set the mission contract below, then Start -- Full Auto keeps working, turn after turn, until it completes, stalls, or you Stop it.",
        variant: "body",
        color: "textMuted",
      }),
      TextField({
        key: "full-auto-launcher-title-field",
        value: draft.title,
        placeholder: "Run title",
        disabled: draft.submitting,
        a11y: { label: "Full Auto run title" },
        onChange: IntentRef("DesktopFullAutoLauncherTitleChanged", ComponentValueBinding()),
        style: { width: "full" },
      }),
      TextField({
        key: "full-auto-launcher-objective-field",
        value: draft.objective,
        multiline: true,
        placeholder: "Objective -- what should this run accomplish?",
        disabled: draft.submitting,
        a11y: { label: "Full Auto objective" },
        onChange: IntentRef("DesktopFullAutoLauncherObjectiveChanged", ComponentValueBinding()),
        style: { width: "full" },
      }),
      TextField({
        key: "full-auto-launcher-done-condition-field",
        value: draft.doneCondition,
        multiline: true,
        placeholder: "Done condition -- how will you know it's finished?",
        disabled: draft.submitting,
        a11y: { label: "Full Auto done condition" },
        onChange: IntentRef("DesktopFullAutoLauncherDoneConditionChanged", ComponentValueBinding()),
        style: { width: "full" },
      }),
      TextField({
        key: "full-auto-launcher-workspace-field",
        value: draft.workspaceRef,
        placeholder: "Workspace path",
        disabled: draft.submitting,
        a11y: { label: "Full Auto workspace" },
        onChange: IntentRef("DesktopFullAutoLauncherWorkspaceRefChanged", ComponentValueBinding()),
        style: { width: "full" },
      }),
      Stack({ key: "full-auto-launcher-row", direction: "row", gap: "2", align: "center" }, [
        Select({
          key: "full-auto-launcher-lane-field",
          value: draft.lane,
          options: fullAutoLauncherLaneOptions.map(option => ({ value: option.value, label: option.label })),
          disabled: draft.submitting,
          a11y: { label: "Full Auto provider lane" },
          onChange: IntentRef("DesktopFullAutoLauncherLaneChanged", ComponentValueBinding()),
        }),
        TextField({
          key: "full-auto-launcher-model-field",
          value: draft.model,
          placeholder: "lane default",
          disabled: draft.submitting,
          a11y: { label: "Full Auto provider model (optional exact model id)" },
          onChange: IntentRef("DesktopFullAutoLauncherModelChanged", ComponentValueBinding()),
          style: { width: "sm" },
        }),
        TextField({
          key: "full-auto-launcher-turn-cap-field",
          value: draft.turnCapText,
          placeholder: String(FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP),
          disabled: draft.submitting,
          a11y: { label: `Full Auto turn cap (default ${FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP})` },
          onChange: IntentRef("DesktopFullAutoLauncherTurnCapChanged", ComponentValueBinding()),
          style: { width: "3xs" },
        }),
        Text({
          key: "full-auto-launcher-turn-cap-caption",
          content: `Turn cap (default ${FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP})`,
          variant: "caption",
          color: "textMuted",
        }),
      ]),
      // FA-WIRE-01 (#8996): the ordered fallback-lane picker over the same
      // admitted Full-Auto-eligible lane options as the primary picker, plus
      // the optional wall-clock guardrail. Order = rotation priority.
      Stack({ key: "full-auto-launcher-routing-row", direction: "row", gap: "2", align: "center" }, [
        Select({
          key: "full-auto-launcher-fallback-add",
          value: "",
          options: [
            { value: "", label: "Add fallback lane…" },
            ...fullAutoLauncherLaneOptions
              .filter(option => option.value !== draft.lane && !draft.fallbackLanes.includes(option.value))
              .map(option => ({ value: option.value, label: option.label })),
          ],
          disabled: draft.submitting,
          a11y: { label: "Add a fallback provider lane (order is rotation priority)" },
          onChange: IntentRef("DesktopFullAutoLauncherFallbackLaneAdded", ComponentValueBinding()),
        }),
        TextField({
          key: "full-auto-launcher-max-wall-clock-field",
          value: draft.maxWallClockMinutesText,
          placeholder: "no limit",
          disabled: draft.submitting,
          a11y: { label: "Max wall clock in minutes (optional guardrail)" },
          onChange: IntentRef("DesktopFullAutoLauncherMaxWallClockChanged", ComponentValueBinding()),
          style: { width: "3xs" },
        }),
        Text({
          key: "full-auto-launcher-max-wall-clock-caption",
          content: "Max wall clock (minutes, optional)",
          variant: "caption",
          color: "textMuted",
        }),
      ]),
      ...(draft.fallbackLanes.length === 0 ? [] : [
        Stack({ key: "full-auto-launcher-fallback-list", direction: "column", gap: "1" }, [
          Text({
            key: "full-auto-launcher-fallback-caption",
            content: "Rotation order on account exhaustion / rate limit / provider error:",
            variant: "caption",
            color: "textMuted",
          }),
          ...[draft.lane, ...draft.fallbackLanes].map((lane, index) =>
            Stack({ key: `full-auto-launcher-fallback-${lane}`, direction: "row", gap: "2", align: "center" }, [
              Text({
                key: `full-auto-launcher-fallback-${lane}-label`,
                content: `${index + 1}. ${fullAutoLauncherLaneLabel(lane)}`,
                variant: "body",
                color: "textPrimary",
              }),
              ...(index === 0
                ? [Text({ key: `full-auto-launcher-fallback-${lane}-primary`, content: "primary", variant: "caption", color: "textMuted" })]
                : [Button({
                    key: `full-auto-launcher-fallback-${lane}-remove`,
                    label: "Remove",
                    variant: "ghost",
                    disabled: draft.submitting,
                    onPress: IntentRef("DesktopFullAutoLauncherFallbackLaneRemoved", StaticPayload(lane)),
                    a11y: { label: `Remove fallback lane ${fullAutoLauncherLaneLabel(lane)}` },
                  })]),
            ])),
        ]),
      ]),
      ...(draft.error !== null
        ? [Text({ key: "full-auto-launcher-error", content: draft.error, variant: "caption", color: "danger" })]
        : []),
      Stack({ key: "full-auto-launcher-actions", direction: "row", gap: "2", align: "center" }, [
        Button({
          key: "full-auto-launcher-start",
          label: draft.submitting ? "Starting…" : "Start",
          variant: "primary",
          disabled: draft.submitting || !validation.ok,
          onPress: IntentRef("DesktopFullAutoLauncherStartRequested"),
          a11y: { label: "Start this Full Auto run" },
        }),
        Button({
          key: "full-auto-launcher-cancel",
          label: "Cancel",
          variant: "ghost",
          disabled: draft.submitting,
          onPress: IntentRef("DesktopFullAutoLauncherCancelled"),
          a11y: { label: "Cancel starting a Full Auto run" },
        }),
      ]),
    ],
  )
}

export const fullAutoRunView = (fullAuto: FullAutoWorkspaceState): View => {
  const run = fullAuto.runs.find(candidate => candidate.runRef === fullAuto.activeRunRef) ?? null
  if (run === null) {
    return Stack({ key: "workspace-full-auto-run", direction: "column", gap: "2", style: { width: "full", flex: 1, minHeight: 0, paddingTop: "2" } }, [
      Text({ key: "full-auto-run-missing", content: "This Full Auto run could not be found.", variant: "body", color: "textMuted" }),
    ])
  }
  const canPause = run.state === "running"
  const canResume = run.state === "paused"
  const canRetryNow = run.state === "stalled" && run.recoveryAction === "retry_now"
  const canStop = !["completed", "failed", "stopped", "cap_reached"].includes(run.state)
  const handoffTargetLane = run.lane === "claude-local" ? "codex-local" : "claude-local"
  const handoffTargetLabel = fullAutoLauncherLaneLabel(handoffTargetLane)
  return Stack(
    {
      key: "workspace-full-auto-run",
      direction: "column",
      gap: "3",
      style: { width: "full", flex: 1, minHeight: 0, paddingTop: "2", paddingRight: "4" },
    },
    [
      Card({ key: "full-auto-run-mission-contract", padding: "3", radius: "md", style: { width: "full", borderColor: "borderSubtle", borderWidth: 1 } }, [
        Stack({ key: "full-auto-run-heading", direction: "row", gap: "2", align: "center" }, [
          Text({ key: "full-auto-run-title", content: run.title, variant: "heading", color: "textPrimary" }),
          Spacer({ key: "full-auto-run-heading-fill", flex: true }),
          Badge({
            key: "full-auto-run-state",
            label: fullAutoRunStatusLabel(run),
            tone: RUN_STATE_TONE[run.state],
            a11y: { label: `Full Auto run state: ${fullAutoRunStatusLabel(run)}` },
          }),
        ]),
        Text({ key: "full-auto-run-objective", content: run.objective, variant: "body", color: "textPrimary" }),
        Text({ key: "full-auto-run-done-condition", content: `Done when: ${run.doneCondition}`, variant: "caption", color: "textMuted" }),
        Text({
          key: "full-auto-run-meta",
          content: `Workspace: ${run.workspaceRef ?? "—"} · Provider: ${run.lane ?? "—"} · Cap: ${run.successfulAttempts + run.failedAttempts}/${run.turnCap}`,
          variant: "caption",
          color: "textMuted",
        }),
        ...(run.state === "stalled"
          ? [Text({
              key: "full-auto-run-stall-copy",
              content: run.stallCause === null
                ? "This run has stalled."
                : `Stalled: ${run.stallCause.replace(/_/g, " ")}.${run.nextRetryAt !== null ? ` Next retry ${run.nextRetryAt}.` : ""}`,
              variant: "caption",
              color: "danger",
            })]
          : []),
        ...(run.terminalReason !== null
          ? [Text({ key: "full-auto-run-terminal-reason", content: run.terminalReason, variant: "caption", color: "textMuted" })]
          : []),
        ...(fullAuto.actionError !== null
          ? [Text({ key: "full-auto-run-action-error", content: fullAuto.actionError, variant: "caption", color: "danger" })]
          : []),
        Stack({ key: "full-auto-run-controls", direction: "row", gap: "2", align: "center" }, [
          ...(canPause
            ? [Button({ key: "full-auto-run-pause", label: "Pause", variant: "secondary", onPress: IntentRef("DesktopFullAutoRunPauseRequested"), a11y: { label: "Pause this Full Auto run" } })]
            : []),
          ...(canResume
            ? [Button({ key: "full-auto-run-resume", label: "Resume", variant: "primary", onPress: IntentRef("DesktopFullAutoRunResumeRequested"), a11y: { label: "Resume this Full Auto run" } })]
            : []),
          ...(canRetryNow
            ? [Button({ key: "full-auto-run-retry-now", label: "Retry now", variant: "secondary", onPress: IntentRef("DesktopFullAutoRunRetryNowRequested"), a11y: { label: "Retry this stalled Full Auto run now" } })]
            : []),
          ...(canResume
            ? [Button({
                key: "full-auto-run-handoff",
                label: `Switch to ${handoffTargetLabel}`,
                variant: "secondary",
                onPress: IntentRef("DesktopFullAutoRunHandoffRequested", StaticPayload(handoffTargetLane)),
                a11y: { label: `Switch this paused Full Auto run to ${handoffTargetLabel}` },
              })]
            : []),
          ...(canStop
            ? [Button({
                key: "full-auto-run-stop",
                label: "Stop",
                variant: "secondary",
                style: { color: "danger" },
                onPress: IntentRef("DesktopFullAutoRunStopRequested"),
                a11y: { label: "Stop this Full Auto run. This cannot be undone." },
              })]
            : []),
          Spacer({ key: "full-auto-run-controls-fill", flex: true }),
          Button({ key: "full-auto-run-refresh", label: "Refresh", variant: "ghost", onPress: IntentRef("DesktopFullAutoRunRefreshed"), a11y: { label: "Refresh this run's state" } }),
        ]),
      ]),
      Card({ key: "full-auto-run-transcript", padding: "3", radius: "md", style: { width: "full", borderColor: "borderSubtle", borderWidth: 1 } }, [
        Text({ key: "full-auto-run-transcript-title", content: "Turns", variant: "title", color: "textPrimary" }),
        ...(fullAuto.activeReport === null || fullAuto.activeReport.turns.length === 0
          ? [Text({ key: "full-auto-run-transcript-empty", content: "No turns recorded yet.", variant: "body", color: "textMuted" })]
          : fullAuto.activeReport.turns.map((turn, index) =>
              Stack({ key: `full-auto-run-turn-${index}`, direction: "row", gap: "2", align: "center" }, [
                Badge({
                  key: `full-auto-run-turn-${index}-lane`,
                  label: turn.lane,
                  tone: "neutral",
                  a11y: { label: `Turn provider: ${turn.lane}` },
                }),
                Text({ key: `full-auto-run-turn-${index}-summary`, content: turn.outcomeSummary, variant: "body", color: "textPrimary" }),
                Spacer({ key: `full-auto-run-turn-${index}-fill`, flex: true }),
                Text({
                  key: `full-auto-run-turn-${index}-time`,
                  // FA-UX-02 (#8997): relative time + duration, never raw ISO
                  // concatenation.
                  content: fullAutoTurnTimingLabel(turn),
                  variant: "caption",
                  color: "textMuted",
                }),
              ]),
            )),
      ]),
      ...(fullAuto.activeReport !== null && fullAuto.activeReport.providerTransitions.length > 0
        ? [Card({ key: "full-auto-run-provider-transitions", padding: "3", radius: "md", style: { width: "full", borderColor: "borderSubtle", borderWidth: 1 } }, [
            Text({ key: "full-auto-run-provider-transitions-title", content: "Provider transitions", variant: "title", color: "textPrimary" }),
            ...fullAuto.activeReport.providerTransitions.map((transition, index) =>
              Text({
                key: `full-auto-run-provider-transition-${index}`,
                content: `${transition.from} → ${transition.to} (${transition.disposition}${transition.truncated ? ", truncated" : ""}) — ${transition.reason}`,
                variant: "caption",
                color: "textMuted",
              }),
            ),
          ])]
        : []),
    ],
  )
}

/** Whichever sub-view the "full-auto" workspace should render right now. */
export const fullAutoWorkspaceView = (fullAuto: FullAutoWorkspaceState): View =>
  fullAuto.mode === "run" && fullAuto.activeRunRef !== null ? fullAutoRunView(fullAuto) : fullAutoLauncherView(fullAuto)
