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
  Text,
  TextField,
  defineIntent,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"

import {
  decodeFullAutoRunListResult,
  decodeFullAutoRunOutcome,
  decodeFullAutoRunReportOutcome,
  unavailableFullAutoRunRendererHost,
  type FullAutoRunProjection,
  type FullAutoRunReportView,
  type FullAutoRunRendererHost,
} from "../full-auto-run-ipc-contract.ts"

export const FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP = 20
export const FULL_AUTO_LAUNCHER_DEFAULT_LANE = "codex-local"

/** Fixed lane option set for v1 (mirrors the composer's admitted lanes).
 * Start still re-validates real L2 eligibility server-side -- an option
 * appearing here is not a promise of admission. */
export const fullAutoLauncherLaneOptions: ReadonlyArray<Readonly<{ value: string; label: string }>> = [
  { value: "codex-local", label: "Codex" },
  { value: "acp:grok-cli", label: "Grok CLI" },
  { value: "acp:cursor-agent", label: "Cursor Agent" },
]

export type FullAutoLauncherDraft = Readonly<{
  title: string
  objective: string
  doneCondition: string
  workspaceRef: string
  lane: string
  turnCapText: string
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
  turnCapText: String(FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP),
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

/** Non-terminal, non-draft states -- the v1 "one active run" concurrency
 * slot (mirrors `full-auto-run-registry.ts`'s own definition). */
const ACTIVE_RUN_STATES = new Set(["running", "pausing", "paused", "retrying", "stalled"])
export const findActiveFullAutoRun = (
  runs: ReadonlyArray<FullAutoRunProjection>,
): FullAutoRunProjection | null => runs.find(run => ACTIVE_RUN_STATES.has(run.state)) ?? null

// ---------------------------------------------------------------------------
// Intents
// ---------------------------------------------------------------------------

export const DesktopFullAutoLauncherOpened = defineIntent("DesktopFullAutoLauncherOpened", Schema.Null)
export const DesktopFullAutoLauncherTitleChanged = defineIntent("DesktopFullAutoLauncherTitleChanged", Schema.String)
export const DesktopFullAutoLauncherObjectiveChanged = defineIntent("DesktopFullAutoLauncherObjectiveChanged", Schema.String)
export const DesktopFullAutoLauncherDoneConditionChanged = defineIntent("DesktopFullAutoLauncherDoneConditionChanged", Schema.String)
export const DesktopFullAutoLauncherWorkspaceRefChanged = defineIntent("DesktopFullAutoLauncherWorkspaceRefChanged", Schema.String)
export const DesktopFullAutoLauncherLaneChanged = defineIntent("DesktopFullAutoLauncherLaneChanged", Schema.String)
export const DesktopFullAutoLauncherTurnCapChanged = defineIntent("DesktopFullAutoLauncherTurnCapChanged", Schema.String)
export const DesktopFullAutoLauncherCancelled = defineIntent("DesktopFullAutoLauncherCancelled", Schema.Null)
export const DesktopFullAutoLauncherStartRequested = defineIntent("DesktopFullAutoLauncherStartRequested", Schema.Null)
export const DesktopFullAutoRunOpened = defineIntent("DesktopFullAutoRunOpened", Schema.String)
export const DesktopFullAutoRunRefreshed = defineIntent("DesktopFullAutoRunRefreshed", Schema.Null)
export const DesktopFullAutoRunsListRefreshed = defineIntent("DesktopFullAutoRunsListRefreshed", Schema.Null)
export const DesktopFullAutoRunPauseRequested = defineIntent("DesktopFullAutoRunPauseRequested", Schema.Null)
export const DesktopFullAutoRunResumeRequested = defineIntent("DesktopFullAutoRunResumeRequested", Schema.Null)
export const DesktopFullAutoRunStopRequested = defineIntent("DesktopFullAutoRunStopRequested", Schema.Null)
export const DesktopFullAutoRunRetryNowRequested = defineIntent("DesktopFullAutoRunRetryNowRequested", Schema.Null)

export const fullAutoWorkspaceIntents = [
  DesktopFullAutoLauncherOpened,
  DesktopFullAutoLauncherTitleChanged,
  DesktopFullAutoLauncherObjectiveChanged,
  DesktopFullAutoLauncherDoneConditionChanged,
  DesktopFullAutoLauncherWorkspaceRefChanged,
  DesktopFullAutoLauncherLaneChanged,
  DesktopFullAutoLauncherTurnCapChanged,
  DesktopFullAutoLauncherCancelled,
  DesktopFullAutoLauncherStartRequested,
  DesktopFullAutoRunOpened,
  DesktopFullAutoRunRefreshed,
  DesktopFullAutoRunsListRefreshed,
  DesktopFullAutoRunPauseRequested,
  DesktopFullAutoRunResumeRequested,
  DesktopFullAutoRunStopRequested,
  DesktopFullAutoRunRetryNowRequested,
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

/** Validates the draft against exactly the fields `full-auto-run-actions.ts`'
 * `startFullAutoRunAction` requires -- title/objective/doneCondition
 * non-empty, workspaceRef non-empty, turnCap a 1-1000 integer when present.
 * Mirrors FA-AC-54's "Start is enabled only once the mission contract is
 * complete". */
export const validateFullAutoLauncherDraft = (
  draft: FullAutoLauncherDraft,
): Readonly<{ ok: true; turnCap: number | undefined } | { ok: false; error: string }> => {
  if (draft.title.trim() === "") return { ok: false, error: "Give this run a title." }
  if (draft.objective.trim() === "") return { ok: false, error: "Describe the objective." }
  if (draft.doneCondition.trim() === "") return { ok: false, error: "State an explicit done condition." }
  if (draft.workspaceRef.trim() === "") return { ok: false, error: "Choose a workspace." }
  const trimmedCap = draft.turnCapText.trim()
  if (trimmedCap === "") return { ok: true, turnCap: undefined }
  const parsed = Number(trimmedCap)
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > 1000) {
    return { ok: false, error: "Turn cap must be a whole number between 1 and 1000." }
  }
  return { ok: true, turnCap: parsed }
}

export const makeFullAutoWorkspaceHandlers = <S extends FullAutoCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  host: FullAutoRunRendererHost = unavailableFullAutoRunRendererHost,
  selectWorkspace: (workspace: string) => Effect.Effect<void> = () => Effect.void,
) => {
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

  const runMutation = (
    call: (runRef: string) => Promise<unknown>,
  ) => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const runRef = current.fullAuto.activeRunRef
    if (runRef === null) return
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
      yield* refreshActiveRun()
      return
    }
    yield* SubscriptionRef.update(state, next => withFullAuto(next, { actionError: outcome.error.message }))
  })

  return {
    DesktopFullAutoLauncherOpened: () => Effect.gen(function* () {
      yield* refreshList()
      const afterList = yield* SubscriptionRef.get(state)
      const active = findActiveFullAutoRun(afterList.fullAuto.runs)
      if (active !== null) {
        // FA-AC-39/54: at most one active run per profile. Route straight to
        // the existing run instead of presenting a launcher that will just
        // refuse with active_run_conflict.
        yield* SubscriptionRef.update(state, next => withFullAuto(next, { mode: "run", activeRunRef: active.runRef }))
        yield* refreshActiveRun()
        yield* selectWorkspace("full-auto")
        return
      }
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
      SubscriptionRef.update(state, current => withFullAuto(current, { launcher: { ...current.fullAuto.launcher, lane: value, error: null } })),
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
        title: draft.title.trim(),
        objective: draft.objective.trim(),
        doneCondition: draft.doneCondition.trim(),
        lane: draft.lane,
        ...(validation.turnCap === undefined ? {} : { turnCap: validation.turnCap }),
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
    }),
    DesktopFullAutoRunOpened: (runRef: string) => Effect.gen(function* () {
      yield* SubscriptionRef.update(state, current => withFullAuto(current, { mode: "run", activeRunRef: runRef, actionError: null }))
      yield* selectWorkspace("full-auto")
      yield* refreshActiveRun()
    }),
    DesktopFullAutoRunRefreshed: () => refreshActiveRun(),
    DesktopFullAutoRunsListRefreshed: () => refreshList(),
    DesktopFullAutoRunPauseRequested: () => runMutation(runRef => host.pause(runRef)),
    DesktopFullAutoRunResumeRequested: () => runMutation(runRef => host.resume(runRef)),
    DesktopFullAutoRunStopRequested: () => runMutation(runRef => host.stop(runRef)),
    DesktopFullAutoRunRetryNowRequested: () => runMutation(runRef => host.retryNow(runRef)),
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
                  content: `${turn.createdAt} → ${turn.updatedAt}`,
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
