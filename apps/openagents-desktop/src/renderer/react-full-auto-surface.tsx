/**
 * FA-UX-01 (#8974): the real React-rendered Full Auto launcher + read-only
 * run view. `WorkbenchShell` (./react-primitive-adapters.tsx) is the
 * production render path (see `mountReactWorkbench`); this component is its
 * dedicated "full-auto" `workspaceSurface`, mirroring how "settings" gets
 * `ReactSettingsSurface` -- a full override of `DesktopSurfaceManager`
 * (and therefore the ordinary chat composer) rather than a side panel next
 * to it. State/validation/derived-label logic stays in
 * `./full-auto-workspace.ts` (shared with the Effect Native projection used
 * by design-conformance tests); this file is presentation only.
 */
import { useEffect, type ReactElement } from "react"
import { ComponentValueBinding, IntentRef, type IntentError, type IntentReporter, type JsonPayload } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"

import { Button } from "#components/ui/button"
import { Badge } from "#components/ui/badge"
import { Input } from "#components/ui/input"
import { Textarea } from "#components/ui/textarea"
import type { DesktopShellState } from "./shell.ts"
import {
  fullAutoLauncherLaneOptions,
  fullAutoRunStatusLabel,
  validateFullAutoLauncherDraft,
  FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP,
  type FullAutoWorkspaceState,
} from "./full-auto-workspace.ts"

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload) as Effect.Effect<void, IntentError>).catch(() => undefined)
}

const RUN_STATE_BADGE_VARIANT: Readonly<Record<string, "default" | "secondary" | "outline" | "destructive">> = {
  draft: "outline",
  running: "default",
  pausing: "secondary",
  paused: "outline",
  retrying: "secondary",
  stalled: "destructive",
  completed: "default",
  failed: "destructive",
  stopped: "outline",
  cap_reached: "secondary",
}

const FullAutoLauncher = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const draft = state.fullAuto.launcher
  const validation = validateFullAutoLauncherDraft(draft)
  return <section className="oa-react-full-auto-launcher" aria-labelledby="react-full-auto-launcher-title">
    <header>
      <h1 id="react-full-auto-launcher-title">Full Auto</h1>
      <p>One durable, unattended run. Set the mission contract below, then Start — Full Auto keeps working, turn after turn, until it completes, stalls, or you Stop it.</p>
    </header>
    <div className="oa-react-full-auto-field">
      <label htmlFor="full-auto-launcher-title">Title</label>
      <Input
        id="full-auto-launcher-title"
        data-en-key="full-auto-launcher-title-field"
        placeholder="Run title"
        value={draft.title}
        disabled={draft.submitting}
        onChange={event => dispatch(report, "DesktopFullAutoLauncherTitleChanged", event.currentTarget.value)}
      />
    </div>
    <div className="oa-react-full-auto-field">
      <label htmlFor="full-auto-launcher-objective">Objective</label>
      <Textarea
        id="full-auto-launcher-objective"
        data-en-key="full-auto-launcher-objective-field"
        placeholder="What should this run accomplish?"
        value={draft.objective}
        disabled={draft.submitting}
        onChange={event => dispatch(report, "DesktopFullAutoLauncherObjectiveChanged", event.currentTarget.value)}
      />
    </div>
    <div className="oa-react-full-auto-field">
      <label htmlFor="full-auto-launcher-done-condition">Done condition</label>
      <Textarea
        id="full-auto-launcher-done-condition"
        data-en-key="full-auto-launcher-done-condition-field"
        placeholder="How will you know it's finished?"
        value={draft.doneCondition}
        disabled={draft.submitting}
        onChange={event => dispatch(report, "DesktopFullAutoLauncherDoneConditionChanged", event.currentTarget.value)}
      />
    </div>
    <div className="oa-react-full-auto-field">
      <label htmlFor="full-auto-launcher-workspace">Workspace</label>
      <Input
        id="full-auto-launcher-workspace"
        data-en-key="full-auto-launcher-workspace-field"
        placeholder="Workspace path"
        value={draft.workspaceRef}
        disabled={draft.submitting}
        onChange={event => dispatch(report, "DesktopFullAutoLauncherWorkspaceRefChanged", event.currentTarget.value)}
      />
    </div>
    <div className="oa-react-full-auto-row">
      <div className="oa-react-full-auto-field">
        <label htmlFor="full-auto-launcher-lane">Provider</label>
        <select
          id="full-auto-launcher-lane"
          data-en-key="full-auto-launcher-lane-field"
          className="oa-react-full-auto-select"
          value={draft.lane}
          disabled={draft.submitting}
          onChange={event => dispatch(report, "DesktopFullAutoLauncherLaneChanged", event.currentTarget.value)}
        >
          {fullAutoLauncherLaneOptions.map(option => <option key={option.value} value={option.value}>{option.label}</option>)}
        </select>
      </div>
      <div className="oa-react-full-auto-field">
        <label htmlFor="full-auto-launcher-turn-cap">{`Turn cap (default ${FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP})`}</label>
        <Input
          id="full-auto-launcher-turn-cap"
          data-en-key="full-auto-launcher-turn-cap-field"
          inputMode="numeric"
          placeholder={String(FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP)}
          value={draft.turnCapText}
          disabled={draft.submitting}
          onChange={event => dispatch(report, "DesktopFullAutoLauncherTurnCapChanged", event.currentTarget.value)}
        />
      </div>
    </div>
    {draft.error === null ? null : <p role="alert" className="oa-react-full-auto-error">{draft.error}</p>}
    <div className="oa-react-full-auto-actions">
      <Button
        data-en-key="full-auto-launcher-start"
        disabled={draft.submitting || !validation.ok}
        onClick={() => dispatch(report, "DesktopFullAutoLauncherStartRequested")}
      >
        {draft.submitting ? "Starting…" : "Start"}
      </Button>
      <Button
        variant="ghost"
        data-en-key="full-auto-launcher-cancel"
        disabled={draft.submitting}
        onClick={() => dispatch(report, "DesktopFullAutoLauncherCancelled")}
      >
        Cancel
      </Button>
    </div>
  </section>
}

const FullAutoRunTranscript = ({ fullAuto }: { readonly fullAuto: FullAutoWorkspaceState }): ReactElement => {
  const turns = fullAuto.activeReport?.turns ?? []
  return <section className="oa-react-full-auto-transcript" aria-labelledby="react-full-auto-transcript-title">
    <h2 id="react-full-auto-transcript-title">Turns</h2>
    {turns.length === 0
      ? <p>No turns recorded yet.</p>
      : <ol className="oa-react-full-auto-turn-list">
          {turns.map(turn => <li key={turn.turnRef} className="oa-react-full-auto-turn" data-turn-lane={turn.lane}>
            <Badge variant="outline">{turn.lane}</Badge>
            <span className="oa-react-full-auto-turn-summary">{turn.outcomeSummary}</span>
            <span className="oa-react-full-auto-turn-time">{turn.createdAt} → {turn.updatedAt}</span>
          </li>)}
        </ol>}
    {(fullAuto.activeReport?.providerTransitions.length ?? 0) === 0 ? null : <>
      <h3>Provider transitions</h3>
      <ul className="oa-react-full-auto-transition-list">
        {fullAuto.activeReport!.providerTransitions.map(transition => <li key={transition.handoffRef}>
          {transition.from} → {transition.to} ({transition.disposition}{transition.truncated ? ", truncated" : ""}) — {transition.reason}
        </li>)}
      </ul>
    </>}
  </section>
}

/** Live-state polling interval (FA-AC-47/48): the run's true lifecycle state
 * can move (turn completion, cap reached, stall detection) with no owner
 * action at all, so the read-only view must not rely solely on mutation
 * responses to stay current -- it re-reads the same `get`/`report` main
 * already settles on every call. Stops once the run reaches a terminal
 * state; a fresh run/navigation restarts it via the effect's dependency. */
const FULL_AUTO_RUN_POLL_INTERVAL_MS = 3000
const FULL_AUTO_RUN_TERMINAL_STATES: ReadonlyArray<string> = ["completed", "failed", "stopped", "cap_reached"]

const FullAutoRunView = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const fullAuto = state.fullAuto
  const run = fullAuto.runs.find(candidate => candidate.runRef === fullAuto.activeRunRef) ?? null
  const runRef = run?.runRef ?? null
  const runState = run?.state ?? null
  useEffect(() => {
    if (runRef === null || runState === null || FULL_AUTO_RUN_TERMINAL_STATES.includes(runState)) return
    const timer = setInterval(() => dispatch(report, "DesktopFullAutoRunRefreshed"), FULL_AUTO_RUN_POLL_INTERVAL_MS)
    return () => clearInterval(timer)
  }, [report, runRef, runState])
  if (run === null) {
    return <section className="oa-react-full-auto-run" aria-live="polite"><p>This Full Auto run could not be found.</p></section>
  }
  const canPause = run.state === "running"
  const canResume = run.state === "paused"
  const canRetryNow = run.state === "stalled" && run.recoveryAction === "retry_now"
  const canStop = !FULL_AUTO_RUN_TERMINAL_STATES.includes(run.state)
  return <section className="oa-react-full-auto-run" data-full-auto-run-ref={run.runRef} data-full-auto-run-state={run.state} aria-labelledby="react-full-auto-run-title">
    <header className="oa-react-full-auto-run-header">
      <div className="oa-react-full-auto-run-heading">
        <h1 id="react-full-auto-run-title">{run.title}</h1>
        <Badge data-en-key="full-auto-run-state" variant={RUN_STATE_BADGE_VARIANT[run.state] ?? "outline"} aria-label={`Full Auto run state: ${fullAutoRunStatusLabel(run)}`}>
          {fullAutoRunStatusLabel(run)}
        </Badge>
      </div>
      <p className="oa-react-full-auto-run-objective">{run.objective}</p>
      <p className="oa-react-full-auto-run-done-condition">Done when: {run.doneCondition}</p>
      <p className="oa-react-full-auto-run-meta">
        Workspace: {run.workspaceRef ?? "—"} · Provider: {run.lane ?? "—"} · Cap: {run.successfulAttempts + run.failedAttempts}/{run.turnCap}
      </p>
      {run.state !== "stalled" ? null : <p role="alert" className="oa-react-full-auto-stall-copy">
        {run.stallCause === null
          ? "This run has stalled."
          : `Stalled: ${run.stallCause.replaceAll("_", " ")}.${run.nextRetryAt !== null ? ` Next retry ${run.nextRetryAt}.` : ""}`}
      </p>}
      {run.terminalReason === null ? null : <p className="oa-react-full-auto-terminal-reason">{run.terminalReason}</p>}
      {fullAuto.actionError === null ? null : <p role="alert" className="oa-react-full-auto-error">{fullAuto.actionError}</p>}
      <div className="oa-react-full-auto-run-controls">
        {!canPause ? null : <Button data-en-key="full-auto-run-pause" variant="secondary" onClick={() => dispatch(report, "DesktopFullAutoRunPauseRequested")}>Pause</Button>}
        {!canResume ? null : <Button data-en-key="full-auto-run-resume" onClick={() => dispatch(report, "DesktopFullAutoRunResumeRequested")}>Resume</Button>}
        {!canRetryNow ? null : <Button data-en-key="full-auto-run-retry-now" variant="secondary" onClick={() => dispatch(report, "DesktopFullAutoRunRetryNowRequested")}>Retry now</Button>}
        {!canStop ? null : <Button data-en-key="full-auto-run-stop" variant="destructive" onClick={() => dispatch(report, "DesktopFullAutoRunStopRequested")} aria-label="Stop this Full Auto run. This cannot be undone.">Stop</Button>}
        <Button data-en-key="full-auto-run-refresh" variant="ghost" onClick={() => dispatch(report, "DesktopFullAutoRunRefreshed")}>Refresh</Button>
      </div>
    </header>
    <FullAutoRunTranscript fullAuto={fullAuto} />
  </section>
}

export const ReactFullAutoSurface = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement =>
  state.fullAuto.mode === "run" && state.fullAuto.activeRunRef !== null
    ? <FullAutoRunView state={state} report={report} />
    : <FullAutoLauncher state={state} report={report} />
