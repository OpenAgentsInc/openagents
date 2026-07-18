import {
  FULL_AUTO_CONTROL_CALLER,
  type FullAutoControlError,
  type FullAutoControlRun,
  type FullAutoControlRunHandoffRequest,
  type FullAutoControlRunStartRequest,
} from "./full-auto-control-contract.ts"
import type { FullAutoControlCapabilities } from "./full-auto-control-server.ts"
import { FULL_AUTO_DEFAULT_LANE } from "./full-auto-lane.ts"
import {
  retryFullAutoRunNow,
  settleFullAutoRunLiveness,
  type FullAutoLivenessProjection,
} from "./full-auto-liveness.ts"
import {
  buildProviderHandoffEnvelope,
  providerHandoffDispositionForEnvelope,
  type ProviderHandoffTransitionRecord,
} from "./full-auto-provider-handoff.ts"
import {
  deriveFullAutoRunReceipt,
  isFullAutoMetricsEnabled,
  type FullAutoRunReceipt,
  type FullAutoRunReport,
} from "./full-auto-run-report.ts"
import {
  type FullAutoRun,
  type FullAutoRunActor,
  type FullAutoRunThreadSnapshot,
} from "./full-auto-run-registry.ts"

/**
 * FA-UX-01 (#8974): the "main-owned transition service" the ProductSpec
 * requires the launcher/run-view UI to route through -- the SAME pure
 * action functions `full-auto-control-server.ts` (OpenAPI/CLI/MCP) uses,
 * hoisted out so a second caller (the Desktop renderer's own IPC bridge,
 * `full-auto-run-ipc-contract.ts` + main.ts's ipcMain handlers) can drive
 * the identical registry/liveness/report/handoff mutations without
 * duplicating any state-machine or side-effect logic. Every action here is
 * pure w.r.t. HTTP: it takes a `FullAutoRunActionContext` (capabilities +
 * now + actor + a human-readable callerLabel used only in reason/system-note
 * text) and returns a discriminated outcome instead of writing a response.
 *
 * `full-auto-control-server.ts` calls these with `actor: "control_api"`.
 * The Desktop UI's IPC handlers (main.ts) call the same functions with
 * `actor: "owner_ui"` -- the actor value `full-auto-run-registry.ts`
 * explicitly reserves for this Desktop UI.
 */
export type FullAutoRunActionContext = Readonly<{
  capabilities: FullAutoControlCapabilities
  now: () => Date
  actor: FullAutoRunActor
  /** Human-readable phrase used in `reason`/system-note text, e.g.
   * "the local control API (caller: control-api)" or "the desktop Full Auto UI". */
  callerLabel: string
}>

export type FullAutoRunActionOutcome<T = FullAutoControlRun> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ ok: false; status: number; error: FullAutoControlError }>

/** MOB-FA-02 (#8994): the thread-level `disabledBy` attribution for a given
 * run-level actor -- kept as one function so `pauseFullAutoRunAction` and
 * `stopFullAutoRunAction` never independently drift on how a mobile-
 * originated intent is attributed (previously any non-`owner_ui` actor,
 * including `mobile`, silently fell through to `control_api`). */
const disabledByForActor = (actor: FullAutoRunActor): "ui_toggle" | "control_api" | "mobile" => {
  if (actor === "owner_ui") return "ui_toggle"
  if (actor === "mobile") return "mobile"
  return "control_api"
}

const projectRun = (run: FullAutoRun, projection: FullAutoLivenessProjection): FullAutoControlRun => ({
  runRef: run.runRef,
  threadRef: run.threadRef ?? null,
  title: run.title,
  objective: run.objective,
  objectiveSource: run.objectiveSource,
  doneCondition: run.doneCondition,
  workspaceRef: run.workspaceRef ?? null,
  lane: run.profile?.lane ?? null,
  turnCap: run.turnCap,
  successfulAttempts: run.successfulAttempts,
  failedAttempts: run.failedAttempts,
  state: run.state,
  stateRevision: run.stateRevision,
  terminalReason: run.terminalReason ?? null,
  predecessorRunRef: run.predecessorRunRef ?? null,
  migratedFrom: run.migratedFrom ?? null,
  createdAt: run.createdAt,
  startedAt: run.startedAt ?? null,
  lastProgressAt: run.lastProgressAt ?? null,
  pausedAt: run.pausedAt ?? null,
  stoppedAt: run.stoppedAt ?? null,
  completedAt: run.completedAt ?? null,
  transitions: run.transitions,
  stallCause: projection.cause,
  nextRetryAt: projection.nextRetryAt,
  recoveryAction: projection.recoveryAction,
})

const threadSnapshot = (
  capabilities: FullAutoControlCapabilities,
  run: FullAutoRun,
): FullAutoRunThreadSnapshot => ({
  threadRecord: run.threadRef === undefined ? null : capabilities.registry.record(run.threadRef),
  turnRunning: run.threadRef !== undefined && capabilities.liveState(run.threadRef)?.state === "turn_running",
})

/** FA-RUN-03 (#8971): settles a run against current thread-level truth AND
 * the liveness/stall classifier before projecting it, so every read/mutation
 * response agrees with the persisted state (shared with the control server's
 * identically-named helper, see its doc comment for the full rationale). */
const settleRun = (
  capabilities: FullAutoControlCapabilities,
  run: FullAutoRun,
  now: () => Date,
): Readonly<{ run: FullAutoRun; projection: FullAutoLivenessProjection }> =>
  settleFullAutoRunLiveness(capabilities.runRegistry, run, threadSnapshot(capabilities, run), now)

const projectSettled = (
  capabilities: FullAutoControlCapabilities,
  run: FullAutoRun,
  now: () => Date,
): FullAutoControlRun => {
  const { run: settled, projection } = settleRun(capabilities, run, now)
  return projectRun(settled, projection)
}

/** FA-RUN-04 (#8972): the single sync point every run-touching action calls
 * after settling, feeding the freshly settled run/projection, a fresh
 * turn-journal read, and a fresh handoff-registry read into the report
 * store's incremental merge. */
const settleAndSyncReport = (
  capabilities: FullAutoControlCapabilities,
  run: FullAutoRun,
  now: () => Date,
): Readonly<{ run: FullAutoRun; projection: FullAutoLivenessProjection }> => {
  const settled = settleRun(capabilities, run, now)
  const turns = settled.run.threadRef === undefined ? [] : capabilities.listTurns(settled.run.threadRef)
  const handoffs = capabilities.providerHandoffRegistry?.list({ runRef: settled.run.runRef }) ?? []
  capabilities.reportStore.sync({
    run: settled.run,
    turns,
    handoffs,
    livenessProjection: settled.projection,
    // FA-RPT-01 (#8988): a fresh thread-record read sources the typed
    // failure history / rotation passthrough, and the local-only metrics
    // gate rides along (default ON via the env gate).
    threadRecord: settled.run.threadRef === undefined
      ? null
      : capabilities.registry.record(settled.run.threadRef),
    metricsEnabled: capabilities.metricsEnabled?.() ?? isFullAutoMetricsEnabled(process.env),
  })
  return settled
}

const settleSyncAndProject = (
  capabilities: FullAutoControlCapabilities,
  run: FullAutoRun,
  now: () => Date,
): FullAutoControlRun => {
  const { run: settled, projection } = settleAndSyncReport(capabilities, run, now)
  return projectRun(settled, projection)
}

const notFound = (): FullAutoRunActionOutcome<never> => ({
  ok: false,
  status: 404,
  error: { error: "not_found", message: "No Full Auto run exists for that runRef." },
})

/** Default callerLabel for control-API-driven callers -- identical phrase to
 * the text `full-auto-control-server.ts` produced before extraction. */
export const FULL_AUTO_CONTROL_CALLER_LABEL = `the local control API (caller: ${FULL_AUTO_CONTROL_CALLER})`

/** callerLabel for the Desktop UI's own IPC-driven actions (FA-UX-01). */
export const FULL_AUTO_OWNER_UI_CALLER_LABEL = "the desktop Full Auto UI"

export const listFullAutoRunsAction = (
  ctx: FullAutoRunActionContext,
): ReadonlyArray<FullAutoControlRun> =>
  ctx.capabilities.runRegistry.list().map(run => projectSettled(ctx.capabilities, run, ctx.now))

export const startFullAutoRunAction = (
  ctx: FullAutoRunActionContext,
  body: FullAutoControlRunStartRequest,
): FullAutoRunActionOutcome => {
  const { capabilities, now, actor, callerLabel } = ctx
  const resolvedWorkspaceRef = capabilities.resolveWorkspaceRef()
  if (body.workspaceRef !== resolvedWorkspaceRef) {
    return {
      ok: false,
      status: 409,
      error: {
        error: "workspace_mismatch",
        message: "The named workspace does not match the currently resolved workspace; no run was started.",
        expectedWorkspaceRef: body.workspaceRef,
        resolvedWorkspaceRef,
      },
    }
  }
  const lane = body.lane ?? FULL_AUTO_DEFAULT_LANE
  if (!(capabilities.isLaneEligible?.(lane) ?? lane === FULL_AUTO_DEFAULT_LANE)) {
    return {
      ok: false,
      status: 409,
      error: {
        error: "lane_not_eligible",
        message: `Provider lane ${lane} is not admitted for Full Auto background turns.`,
      },
    }
  }
  if (body.model !== undefined && capabilities.isModelEligible?.(lane, body.model) !== true) {
    return {
      ok: false,
      status: 409,
      error: {
        error: "model_not_eligible",
        message: `Model ${body.model} is not admitted for provider lane ${lane}.`,
      },
    }
  }
  // FA-AC-39: check BEFORE minting anything -- a refusal must leave no side
  // effect behind, never a half-started thread.
  const existingActive = capabilities.runRegistry.activeRun()
  if (existingActive !== null) {
    return {
      ok: false,
      status: 409,
      error: {
        error: "active_run_conflict",
        message: "A Full Auto run is already active for this Desktop profile.",
        activeRunRef: existingActive.runRef,
      },
    }
  }
  const startedThreadRef = capabilities.createThread(body.title, lane)
  const profile = { lane, ...(body.model === undefined ? {} : { model: body.model }) }
  capabilities.registry.set(startedThreadRef, true, { workspaceRef: resolvedWorkspaceRef, profile })
  const result = capabilities.runRegistry.startNew({
    title: body.title,
    objective: body.objective,
    doneCondition: body.doneCondition,
    objectiveSource: actor === "owner_ui" ? "user" : "control_caller",
    workspaceRef: resolvedWorkspaceRef,
    profile,
    ...(body.turnCap === undefined ? {} : { turnCap: body.turnCap }),
    threadRef: startedThreadRef,
    actor,
    reason: `started via ${callerLabel}`,
  })
  if (!result.ok) {
    return {
      ok: false,
      status: 409,
      error: {
        error: result.reason === "active_run_conflict" ? "active_run_conflict" : "invalid_request",
        message: "A Full Auto run could not be started.",
        ...(result.reason === "active_run_conflict" ? { activeRunRef: result.activeRunRef } : {}),
      },
    }
  }
  capabilities.appendSystemNote(startedThreadRef, `Full Auto run started via ${callerLabel}.`)
  void capabilities.triggerReconciliation().catch(() => {})
  return { ok: true, value: settleSyncAndProject(capabilities, result.run, now) }
}

export const getFullAutoRunAction = (
  ctx: FullAutoRunActionContext,
  runRef: string,
): FullAutoRunActionOutcome => {
  const run = ctx.capabilities.runRegistry.get(runRef)
  if (run === null) return notFound()
  return { ok: true, value: projectSettled(ctx.capabilities, run, ctx.now) }
}

export const pauseFullAutoRunAction = (
  ctx: FullAutoRunActionContext,
  runRef: string,
): FullAutoRunActionOutcome => {
  const { capabilities, now, actor, callerLabel } = ctx
  const run = capabilities.runRegistry.get(runRef)
  if (run === null) return notFound()
  const turnRunning = run.threadRef !== undefined && capabilities.liveState(run.threadRef)?.state === "turn_running"
  const to = turnRunning ? "pausing" : "paused"
  const result = capabilities.runRegistry.transition(runRef, {
    to,
    actor,
    reason: `Pause requested via ${callerLabel}.`,
  })
  if (!result.ok) {
    if (result.reason === "not_found") return notFound()
    return {
      ok: false,
      status: 409,
      error: {
        error: "illegal_transition",
        message: `Pause is not legal from state ${result.from}.`,
        fromState: result.from,
        toState: result.to,
      },
    }
  }
  // Pause immediately prevents any new dispatch, whether or not a turn is
  // currently in flight -- disable the thread-level gate right now rather
  // than waiting for the turn to resolve. Unlike Stop, Pause drains the
  // already-admitted turn normally. This preserves its accepted evidence and
  // makes the Pausing -> Paused handoff boundary deterministic; Stop remains
  // the explicit interrupting action.
  if (run.threadRef !== undefined) {
    capabilities.registry.set(run.threadRef, false, { disabledBy: disabledByForActor(actor) })
    capabilities.appendSystemNote(run.threadRef, `Full Auto run paused via ${callerLabel}.`)
  }
  return { ok: true, value: settleSyncAndProject(capabilities, result.run, now) }
}

export const resumeFullAutoRunAction = (
  ctx: FullAutoRunActionContext,
  runRef: string,
): FullAutoRunActionOutcome => {
  const { capabilities, now, actor, callerLabel } = ctx
  const run = capabilities.runRegistry.get(runRef)
  if (run === null) return notFound()
  if (run.state !== "paused") {
    return {
      ok: false,
      status: 409,
      error: {
        error: "illegal_transition",
        message: `Resume is legal only from paused (current state: ${run.state}).`,
        fromState: run.state,
        toState: "running",
      },
    }
  }
  // FA-AC-44: revalidate workspace admission before dispatching again -- a
  // mismatch is a refusal, never a redirect or a silent state change: the
  // run stays exactly Paused.
  const resolvedWorkspaceRef = capabilities.resolveWorkspaceRef()
  if (run.workspaceRef !== undefined && run.workspaceRef !== resolvedWorkspaceRef) {
    return {
      ok: false,
      status: 409,
      error: {
        error: "workspace_mismatch",
        message: "The run's granted workspace no longer matches the currently resolved workspace; Resume refused and the run remains Paused.",
        expectedWorkspaceRef: run.workspaceRef,
        resolvedWorkspaceRef,
      },
    }
  }
  const lane = run.profile?.lane ?? FULL_AUTO_DEFAULT_LANE
  if (!(capabilities.isLaneEligible?.(lane) ?? lane === FULL_AUTO_DEFAULT_LANE)) {
    return {
      ok: false,
      status: 409,
      error: {
        error: "lane_not_eligible",
        message: `Provider lane ${lane} is not admitted for Full Auto background turns.`,
      },
    }
  }
  const model = run.profile?.model
  if (model !== undefined && capabilities.isModelEligible?.(lane, model) !== true) {
    return {
      ok: false,
      status: 409,
      error: {
        error: "model_not_eligible",
        message: `Model ${model} is not admitted for provider lane ${lane}; Resume refused and the run remains Paused.`,
      },
    }
  }
  const result = capabilities.runRegistry.transition(runRef, {
    to: "running",
    actor,
    reason: `Resume requested via ${callerLabel}.`,
  })
  if (!result.ok) {
    return {
      ok: false,
      status: 409,
      error: {
        error: "illegal_transition",
        message: "Resume is no longer legal for this run.",
        fromState: run.state,
        toState: "running",
      },
    }
  }
  if (run.threadRef !== undefined) {
    // FA-AC-15/FA-AC-44: re-enable through the exact same exactly-once
    // dispatch path every other Full Auto trigger already uses.
    capabilities.registry.set(run.threadRef, true, { workspaceRef: resolvedWorkspaceRef, profile: run.profile })
    capabilities.appendSystemNote(run.threadRef, `Full Auto run resumed via ${callerLabel}.`)
  }
  void capabilities.triggerReconciliation().catch(() => {})
  return { ok: true, value: settleSyncAndProject(capabilities, result.run, now) }
}

export const retryFullAutoRunNowAction = (
  ctx: FullAutoRunActionContext,
  runRef: string,
): FullAutoRunActionOutcome => {
  const { capabilities, now, actor, callerLabel } = ctx
  const run = capabilities.runRegistry.get(runRef)
  if (run === null) return notFound()
  const { run: settled } = settleRun(capabilities, run, now)
  const result = retryFullAutoRunNow(
    capabilities.runRegistry,
    settled,
    threadSnapshot(capabilities, settled),
    { actor },
    now,
  )
  if (!result.ok) {
    if (result.reason === "not_stalled") {
      return {
        ok: false,
        status: 409,
        error: {
          error: "illegal_transition",
          message: `Retry now is legal only from Stalled (current state: ${result.state}).`,
          fromState: result.state,
          toState: "retrying",
        },
      }
    }
    return {
      ok: false,
      status: 409,
      error: {
        error: "not_recoverable",
        message: `This run's current stall cause (${result.cause ?? "unknown_error"}) cannot be resolved by retrying; Stop is the safe action.`,
        stallCause: result.cause ?? undefined,
      },
    }
  }
  if (settled.threadRef !== undefined) {
    capabilities.appendSystemNote(settled.threadRef, `Full Auto run retry requested via ${callerLabel}.`)
  }
  void capabilities.triggerReconciliation().catch(() => {})
  return { ok: true, value: settleSyncAndProject(capabilities, result.run, now) }
}

export const stopFullAutoRunAction = (
  ctx: FullAutoRunActionContext,
  runRef: string,
): FullAutoRunActionOutcome => {
  const { capabilities, now, actor, callerLabel } = ctx
  const run = capabilities.runRegistry.get(runRef)
  if (run === null) return notFound()
  const turnRunning = run.threadRef !== undefined && capabilities.liveState(run.threadRef)?.state === "turn_running"
  const result = capabilities.runRegistry.transition(runRef, {
    to: "stopped",
    actor,
    reason: `Stop requested via ${callerLabel}.`,
  })
  if (!result.ok) {
    if (result.reason === "not_found") return notFound()
    return {
      ok: false,
      status: 409,
      error: {
        error: "illegal_transition",
        message: `Stop is not legal from state ${result.from} (the run is already terminal).`,
        fromState: result.from,
        toState: result.to,
      },
    }
  }
  if (run.threadRef !== undefined) {
    capabilities.registry.set(run.threadRef, false, { disabledBy: disabledByForActor(actor) })
    if (turnRunning) capabilities.interruptLiveTurn?.(run.threadRef)
    capabilities.appendSystemNote(run.threadRef, `Full Auto run stopped via ${callerLabel}.`)
  }
  return { ok: true, value: settleSyncAndProject(capabilities, result.run, now) }
}

export const handoffFullAutoRunAction = async (
  ctx: FullAutoRunActionContext,
  runRef: string,
  body: FullAutoControlRunHandoffRequest,
): Promise<FullAutoRunActionOutcome<Readonly<{ run: FullAutoControlRun; transition: ProviderHandoffTransitionRecord }>>> => {
  const { capabilities, now, actor, callerLabel } = ctx
  const run = capabilities.runRegistry.get(runRef)
  if (run === null) return notFound()
  // FA-AC-58: a manual provider switch is legal only while paused -- the
  // exact same state gate Resume enforces, so a switch can never race an
  // active dispatch.
  if (run.state !== "paused") {
    return {
      ok: false,
      status: 409,
      error: {
        error: "illegal_transition",
        message: `A provider handoff is legal only while paused (current state: ${run.state}).`,
        fromState: run.state,
        toState: run.state,
      },
    }
  }
  if (capabilities.providerLaneRegistry === undefined || capabilities.providerHandoffRegistry === undefined) {
    return {
      ok: false,
      status: 409,
      error: { error: "handoff_refused", message: "Provider handoff is not available on this server instance." },
    }
  }
  const sourceLaneRef = run.profile?.lane ?? FULL_AUTO_DEFAULT_LANE
  const targetLaneRef = body.targetLaneRef
  if (body.model !== undefined && capabilities.isModelEligible?.(targetLaneRef, body.model) !== true) {
    return {
      ok: false,
      status: 409,
      error: {
        error: "model_not_eligible",
        message: `Model ${body.model} is not admitted for provider lane ${targetLaneRef}.`,
      },
    }
  }
  const reason = body.reason ?? `Provider handoff requested via ${callerLabel}.`
  const thread = run.threadRef === undefined ? null : (capabilities.getThread?.(run.threadRef) ?? null)
  // FA-AC-59: re-check target admission/auth/capability eligibility through
  // the exact same gate the existing interactive manual-switch path uses --
  // a refusal leaves the run's lane/profile untouched (rollback).
  const switchResult = capabilities.providerLaneRegistry.switchThread({
    threadRef: run.threadRef ?? runRef,
    laneRef: targetLaneRef,
    lanes: await (capabilities.listLanes?.() ?? Promise.resolve([])),
    thread,
    requiredCapabilities: ["fullAuto"],
  })
  if (!switchResult.ok) {
    const refusedAt = now().toISOString()
    capabilities.providerHandoffRegistry.record({
      runRef: run.runRef,
      ...(run.threadRef === undefined ? {} : { threadRef: run.threadRef }),
      from: sourceLaneRef,
      to: targetLaneRef,
      actor,
      at: refusedAt,
      reason,
      disposition: "refused",
      truncated: false,
      refusalReason: switchResult.reason,
    })
    // The refusal itself is a receipted fact (rollback, never a silent
    // omission) -- fold it into the report even though the run's own
    // lifecycle/profile is unchanged.
    settleAndSyncReport(capabilities, run, now)
    return {
      ok: false,
      status: 409,
      error: {
        error: "handoff_refused",
        message: switchResult.message,
        handoffRefusalReason: switchResult.reason,
      },
    }
  }
  const at = now().toISOString()
  const envelope = buildProviderHandoffEnvelope({
    run,
    sourceLaneRef,
    targetLaneRef,
    thread,
    reason,
    actor,
    at,
  })
  const disposition = providerHandoffDispositionForEnvelope(envelope)
  const transitionRecord = capabilities.providerHandoffRegistry.record({
    runRef: run.runRef,
    ...(run.threadRef === undefined ? {} : { threadRef: run.threadRef }),
    from: sourceLaneRef,
    to: targetLaneRef,
    actor,
    at,
    reason,
    disposition,
    truncated: envelope.contextTruncated,
    envelopeSchema: envelope.schema,
  })
  const { model: _sourceModel, ...sourceProfileWithoutModel } = run.profile ?? {}
  const rebound = capabilities.runRegistry.rebindProfile(runRef, {
    ...sourceProfileWithoutModel,
    lane: targetLaneRef,
    ...(body.model === undefined ? {} : { model: body.model }),
  })
  if (rebound === null) return notFound()
  capabilities.appendSystemNote(
    run.threadRef ?? runRef,
    `Provider handoff: ${sourceLaneRef} → ${targetLaneRef} (${disposition}). Reason: ${reason} (caller: ${callerLabel}).`,
  )
  return {
    ok: true,
    value: { run: settleSyncAndProject(capabilities, rebound, now), transition: transitionRecord },
  }
}

export const getFullAutoRunReportAction = (
  ctx: FullAutoRunActionContext,
  runRef: string,
): FullAutoRunActionOutcome<FullAutoRunReport> => {
  const { capabilities, now } = ctx
  const run = capabilities.runRegistry.get(runRef)
  if (run === null) return notFound()
  const { run: settled } = settleAndSyncReport(capabilities, run, now)
  const report = capabilities.reportStore.get(settled.runRef)
  if (report === null) {
    return {
      ok: false,
      status: 404,
      error: { error: "not_found", message: "No Full Auto run report exists for that runRef." },
    }
  }
  return { ok: true, value: report }
}

export const getFullAutoRunReceiptAction = (
  ctx: FullAutoRunActionContext,
  runRef: string,
): FullAutoRunActionOutcome<FullAutoRunReceipt> => {
  const reportOutcome = getFullAutoRunReportAction(ctx, runRef)
  if (!reportOutcome.ok) return reportOutcome
  return { ok: true, value: deriveFullAutoRunReceipt(reportOutcome.value, ctx.now) }
}
