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
  fullAutoLauncherLaneLabel,
  fullAutoLauncherLaneOptions,
  fullAutoRunStatusLabel,
  fullAutoTurnTimingLabel,
  validateFullAutoLauncherDraft,
  FULL_AUTO_LAUNCHER_DEFAULT_DONE_CONDITION,
  FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP,
  type FullAutoWorkspaceState,
} from "./full-auto-workspace.ts"
import { ConversationTimeline } from "./react-timeline.tsx"
import "./react-full-auto-surface.css"

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
      <p>Describe the outcome once. Codex and Claude keep moving it forward until it is done or needs you.</p>
    </header>
    <div className="oa-react-full-auto-field oa-react-full-auto-objective-field">
      <label htmlFor="full-auto-launcher-objective">What should Full Auto accomplish?</label>
      <Textarea
        id="full-auto-launcher-objective"
        data-en-key="full-auto-launcher-objective-field"
        rows={4}
        autoFocus
        placeholder="Implement the outcome, verify it in the real system, fix what breaks, and keep going until it works."
        value={draft.objective}
        disabled={draft.submitting}
        onChange={event => dispatch(report, "DesktopFullAutoLauncherObjectiveChanged", event.currentTarget.value)}
      />
    </div>
    <div className="oa-react-full-auto-defaults" aria-label="Full Auto defaults">
      <span>{draft.workspaceRef || "Resolving workspace…"}</span>
      <span>Codex → Claude</span>
      <span>{draft.turnCapText || FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP} turns</span>
    </div>
    <details className="oa-react-full-auto-advanced">
      <summary>Advanced</summary>
      <div className="oa-react-full-auto-advanced-grid">
        <div className="oa-react-full-auto-field">
          <label htmlFor="full-auto-launcher-title">Title (inferred when blank)</label>
          <Input id="full-auto-launcher-title" data-en-key="full-auto-launcher-title-field" placeholder="From the first line of the mission" value={draft.title} disabled={draft.submitting} onChange={event => dispatch(report, "DesktopFullAutoLauncherTitleChanged", event.currentTarget.value)} />
        </div>
        <div className="oa-react-full-auto-field oa-react-full-auto-wide-field">
          <label htmlFor="full-auto-launcher-done-condition">Done condition (inferred when blank)</label>
          <Textarea id="full-auto-launcher-done-condition" data-en-key="full-auto-launcher-done-condition-field" rows={3} placeholder={FULL_AUTO_LAUNCHER_DEFAULT_DONE_CONDITION} value={draft.doneCondition} disabled={draft.submitting} onChange={event => dispatch(report, "DesktopFullAutoLauncherDoneConditionChanged", event.currentTarget.value)} />
        </div>
        <div className="oa-react-full-auto-field oa-react-full-auto-wide-field">
          <label htmlFor="full-auto-launcher-workspace">Workspace</label>
          <Input id="full-auto-launcher-workspace" data-en-key="full-auto-launcher-workspace-field" placeholder="Workspace path" value={draft.workspaceRef} disabled={draft.submitting} onChange={event => dispatch(report, "DesktopFullAutoLauncherWorkspaceRefChanged", event.currentTarget.value)} />
        </div>
        <div className="oa-react-full-auto-field">
          <label htmlFor="full-auto-launcher-lane">Primary provider</label>
          <select id="full-auto-launcher-lane" data-en-key="full-auto-launcher-lane-field" className="oa-react-full-auto-select" value={draft.lane} disabled={draft.submitting} onChange={event => dispatch(report, "DesktopFullAutoLauncherLaneChanged", event.currentTarget.value)}>
            {fullAutoLauncherLaneOptions.map(option => <option key={option.value} value={option.value}>{option.label}</option>)}
          </select>
        </div>
        <div className="oa-react-full-auto-field">
          <label htmlFor="full-auto-launcher-model">Model</label>
          <Input id="full-auto-launcher-model" data-en-key="full-auto-launcher-model-field" placeholder="Provider default" value={draft.model} disabled={draft.submitting} onChange={event => dispatch(report, "DesktopFullAutoLauncherModelChanged", event.currentTarget.value)} />
        </div>
        <div className="oa-react-full-auto-field">
          <label htmlFor="full-auto-launcher-turn-cap">Turn cap</label>
          <Input id="full-auto-launcher-turn-cap" data-en-key="full-auto-launcher-turn-cap-field" inputMode="numeric" placeholder={String(FULL_AUTO_LAUNCHER_DEFAULT_TURN_CAP)} value={draft.turnCapText} disabled={draft.submitting} onChange={event => dispatch(report, "DesktopFullAutoLauncherTurnCapChanged", event.currentTarget.value)} />
        </div>
        <div className="oa-react-full-auto-field">
          <label htmlFor="full-auto-launcher-max-wall-clock">Wall clock (minutes)</label>
          <Input id="full-auto-launcher-max-wall-clock" data-en-key="full-auto-launcher-max-wall-clock-field" inputMode="numeric" placeholder="No limit" value={draft.maxWallClockMinutesText} disabled={draft.submitting} onChange={event => dispatch(report, "DesktopFullAutoLauncherMaxWallClockChanged", event.currentTarget.value)} />
        </div>
        <div className="oa-react-full-auto-field oa-react-full-auto-wide-field">
          <label htmlFor="full-auto-launcher-fallback-add">Provider rotation</label>
          <select id="full-auto-launcher-fallback-add" data-en-key="full-auto-launcher-fallback-add" className="oa-react-full-auto-select" value="" disabled={draft.submitting} aria-label="Add a fallback provider lane" onChange={event => dispatch(report, "DesktopFullAutoLauncherFallbackLaneAdded", event.currentTarget.value)}>
            <option value="">Add fallback provider…</option>
            {fullAutoLauncherLaneOptions.filter(option => option.value !== draft.lane && !draft.fallbackLanes.includes(option.value)).map(option => <option key={option.value} value={option.value}>{option.label}</option>)}
          </select>
          <div className="oa-react-full-auto-rotation-order" data-en-key="full-auto-launcher-rotation-order">
            {[draft.lane, ...draft.fallbackLanes].map((lane, index) => <span key={lane} data-rotation-lane={lane}>
              {index > 0 ? "→ " : ""}{fullAutoLauncherLaneLabel(lane)}
              {index === 0 ? null : <button type="button" data-en-key={`full-auto-launcher-fallback-remove-${lane}`} disabled={draft.submitting} aria-label={`Remove fallback lane ${fullAutoLauncherLaneLabel(lane)}`} onClick={() => dispatch(report, "DesktopFullAutoLauncherFallbackLaneRemoved", lane)}>×</button>}
            </span>)}
          </div>
        </div>
      </div>
    </details>
    {draft.error === null ? null : <p role="alert" className="oa-react-full-auto-error">{draft.error}</p>}
    <div className="oa-react-full-auto-actions">
      <Button
        data-en-key="full-auto-launcher-start"
        disabled={draft.submitting || !validation.ok}
        onClick={() => dispatch(report, "DesktopFullAutoLauncherStartRequested")}
      >
        {draft.submitting ? "Starting…" : "Start Full Auto"}
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

/**
 * FA-UX-02 (#8997): the run view's conversation IS the bound thread's real
 * conversation, rendered by the SAME canonical `ConversationTimeline`
 * component every ordinary chat uses (read-only -- no composer per
 * CUT-DSK-06), never a parallel mini-renderer. The bound thread is selected
 * through the shell's canonical thread-selection path when the run view
 * opens (full-auto-workspace.ts `selectThread` wiring), so `state.notes`
 * carries its transcript and live updates flow through the normal channels.
 */
const FullAutoRunConversation = ({ state, run, report }: {
  readonly state: DesktopShellState
  readonly run: FullAutoWorkspaceState["runs"][number]
  readonly report: IntentReporter
}): ReactElement => {
  const boundThreadSelected = run.threadRef !== null && state.activeThreadId === run.threadRef
  if (!boundThreadSelected || state.notes.length === 0) {
    return <section className="oa-react-full-auto-conversation" aria-label="Run conversation">
      <p className="oa-react-full-auto-conversation-empty">No conversation yet. The first Full Auto turn will appear here.</p>
    </section>
  }
  const working = run.threadRef !== null &&
    state.fullAutoLiveByThread[run.threadRef]?.state === "turn_running"
  return <section className="oa-react-full-auto-conversation" aria-label="Run conversation">
    <ConversationTimeline
      page={null}
      notes={state.notes}
      loadingEdge={null}
      working={working}
      workingDirectory={state.workingDirectory}
      agentName={run.lane ?? "Codex"}
      report={report}
    />
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
            {/* FA-UX-02 (#8997): relative time + duration, never raw ISO. */}
            <span className="oa-react-full-auto-turn-time">{fullAutoTurnTimingLabel(turn)}</span>
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
  const handoffTargetLane = run.lane === "fable-local" ? "codex-local" : "fable-local"
  const handoffTargetLabel = fullAutoLauncherLaneLabel(handoffTargetLane)
  return <section className="oa-react-full-auto-run" data-full-auto-run-ref={run.runRef} data-full-auto-run-state={run.state} aria-labelledby="react-full-auto-run-title">
    <header className="oa-react-full-auto-run-header">
      <div className="oa-react-full-auto-run-heading">
        <h1 id="react-full-auto-run-title">{run.title}</h1>
        <Badge data-en-key="full-auto-run-state" variant={RUN_STATE_BADGE_VARIANT[run.state] ?? "outline"} aria-label={`Full Auto run state: ${fullAutoRunStatusLabel(run)}`}>
          {fullAutoRunStatusLabel(run)}
        </Badge>
      </div>
      <p className="oa-react-full-auto-run-meta">
        Workspace: {run.workspaceRef ?? "—"} · Provider: {run.lane ?? "—"} · Cap: {run.successfulAttempts + run.failedAttempts}/{run.turnCap}
      </p>
      <details className="oa-react-full-auto-mission-details">
        <summary>Mission details</summary>
        <p className="oa-react-full-auto-run-objective">{run.objective}</p>
        <p className="oa-react-full-auto-run-done-condition">Done when: {run.doneCondition}</p>
      </details>
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
        {!canResume ? null : <Button data-en-key="full-auto-run-handoff" variant="secondary" onClick={() => dispatch(report, "DesktopFullAutoRunHandoffRequested", handoffTargetLane)}>Switch to {handoffTargetLabel}</Button>}
        {!canStop ? null : <Button data-en-key="full-auto-run-stop" variant="destructive" onClick={() => dispatch(report, "DesktopFullAutoRunStopRequested")} aria-label="Stop this Full Auto run. This cannot be undone.">Stop</Button>}
        <Button data-en-key="full-auto-run-refresh" variant="ghost" onClick={() => dispatch(report, "DesktopFullAutoRunRefreshed")}>Refresh</Button>
      </div>
    </header>
    {/* FA-UX-02 (#8997): the canonical thread timeline first (the real
        conversation), then the compact per-turn outcome history. */}
    <FullAutoRunConversation state={state} run={run} report={report} />
    <FullAutoRunTranscript fullAuto={fullAuto} />
  </section>
}

const FULL_AUTO_ACTIVE_STATES = new Set(["running", "pausing", "paused", "retrying", "stalled"])

const FullAutoRunMonitor = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const sorted = [...state.fullAuto.runs].sort((left, right) => {
    const activeDifference = Number(FULL_AUTO_ACTIVE_STATES.has(right.state)) - Number(FULL_AUTO_ACTIVE_STATES.has(left.state))
    return activeDifference !== 0 ? activeDifference : right.createdAt.localeCompare(left.createdAt)
  })
  const activeCount = sorted.filter(run => FULL_AUTO_ACTIVE_STATES.has(run.state)).length
  const visible = sorted.filter((run, index) => FULL_AUTO_ACTIVE_STATES.has(run.state) || index < activeCount + 6)
  return <aside className="oa-react-full-auto-monitor" aria-labelledby="full-auto-monitor-title">
    <header>
      <div>
        <h2 id="full-auto-monitor-title">Runs</h2>
        <p>{activeCount === 0 ? "No active runs" : `${activeCount} active`}</p>
      </div>
      <Button size="sm" data-en-key="full-auto-new-run" onClick={() => dispatch(report, "DesktopFullAutoLauncherOpened")}>New run</Button>
    </header>
    <div className="oa-react-full-auto-run-list" aria-live="polite">
      {visible.length === 0 ? <p className="oa-react-full-auto-run-list-empty">Your Full Auto runs will appear here.</p> : visible.map(run => {
        const active = FULL_AUTO_ACTIVE_STATES.has(run.state)
        const selected = state.fullAuto.activeRunRef === run.runRef
        return <article key={run.runRef} className="oa-react-full-auto-run-row" data-active={active ? "true" : "false"} data-selected={selected ? "true" : "false"}>
          <button type="button" className="oa-react-full-auto-run-open" aria-label={`Open ${run.title}`} onClick={() => dispatch(report, "DesktopFullAutoRunOpened", run.runRef)}>
            <span className="oa-react-full-auto-run-row-title">{run.title}</span>
            <span className="oa-react-full-auto-run-row-meta">
              <Badge variant={RUN_STATE_BADGE_VARIANT[run.state] ?? "outline"}>{fullAutoRunStatusLabel(run)}</Badge>
              <span>{run.successfulAttempts + run.failedAttempts}/{run.turnCap}</span>
            </span>
          </button>
          {!active ? null : <Button
            size="sm"
            variant="ghost"
            data-en-key={`full-auto-run-stop-${run.runRef}`}
            aria-label={`Stop ${run.title}. This cannot be undone.`}
            onClick={() => dispatch(report, "DesktopFullAutoRunStopByRefRequested", run.runRef)}
          >Stop</Button>}
        </article>
      })}
    </div>
    <Button variant="ghost" size="sm" data-en-key="full-auto-monitor-refresh" onClick={() => dispatch(report, "DesktopFullAutoRunsListRefreshed")}>Refresh runs</Button>
  </aside>
}

export const ReactFullAutoSurface = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  useEffect(() => {
    dispatch(report, "DesktopFullAutoRunsListRefreshed")
    const timer = setInterval(() => dispatch(report, "DesktopFullAutoRunsListRefreshed"), FULL_AUTO_RUN_POLL_INTERVAL_MS)
    return () => clearInterval(timer)
  }, [report])
  return <div className="oa-react-full-auto-surface">
    <main className="oa-react-full-auto-main">
      {state.fullAuto.mode === "run" && state.fullAuto.activeRunRef !== null
        ? <FullAutoRunView state={state} report={report} />
        : <FullAutoLauncher state={state} report={report} />}
    </main>
    <FullAutoRunMonitor state={state} report={report} />
  </div>
}
