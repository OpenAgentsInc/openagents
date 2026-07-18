import { Schema } from "effect"

import { fullAutoFailureBackoffMs } from "./full-auto-reconcile.ts"
import {
  isFullAutoRunTerminal,
  isLegalFullAutoRunTransition,
  settleFullAutoRunFromThreadState,
  type FullAutoRun,
  type FullAutoRunActor,
  type FullAutoRunRegistry,
  type FullAutoRunState,
  type FullAutoRunThreadSnapshot,
} from "./full-auto-run-registry.ts"

/**
 * FA-RUN-03 (#8971): the main-owned liveness/stall projection ProductSpec rev
 * 10 (FA-AC-42/47/48) commits to, layered on top of FA-RUN-01's (#8969)
 * `FullAutoRun` lifecycle and `settleFullAutoRunFromThreadState` exactly the
 * way that module's header comment reserves ("Full liveness/SLO
 * classification, retry-ETA, and owner-actionable recovery affordances are
 * FA-RUN-03 (#8971)'s job"). This module adds no new scheduler and never
 * writes state directly -- every mutating entry point (`settleFullAutoRunLiveness`)
 * routes through the SAME `applyFullAutoRunTransition` legal-edge machinery
 * FA-RUN-01 already owns, attributed to the `liveness_monitor` actor that
 * module already reserved for this purpose.
 *
 * Liveness is a run-level property distinct from a healthy long provider
 * turn (FA-AC-47): a run mid-turn for twenty minutes is not stalled; a run
 * whose terminal turn completed with nothing dispatched afterward for the
 * SLO window is, even though nothing "failed" in the FA-H5 sense. This is
 * exactly the 2026-07-17 incident shape the issue records -- an `enabled:
 * true` record that made no useful progress for roughly six hours with only
 * a generic failed-turn banner and the low-level `"That conversation no
 * longer exists."` string to go on.
 */

/**
 * FA-AC-47's terminal-turn-to-next-accepted-dispatch SLO. A run with no
 * active turn and no scheduled backoff retry that has not progressed for
 * longer than this window is Stalled rather than silently "still Running".
 * Chosen well above ordinary reconciliation-trigger latency (turn
 * completion normally re-triggers within milliseconds) and well below the
 * ~30 minute failure-backoff ceiling, so a genuinely healthy idle-but-armed
 * run is never misclassified.
 */
export const FULL_AUTO_LIVENESS_DISPATCH_SLO_MS = 5 * 60_000

/**
 * Grace period layered on top of a computed FA-H5 backoff retry ETA before a
 * still-pending retry is itself reclassified Stalled ("the retry that was
 * scheduled never actually happened"). Absorbs ordinary jitter in when the
 * next reconciliation trigger fires relative to the exact backoff deadline.
 */
export const FULL_AUTO_LIVENESS_RETRY_GRACE_MS = 2 * 60_000

/**
 * How stale a run's own `lastLivenessCheckAt` must be before a stall is
 * additionally attributed to the app itself not having run recently (sleep,
 * quit, or a crash) rather than a live app failing to progress. Every
 * liveness settle pass stamps this field, so a large gap here is direct
 * evidence the process was not ticking, not an inference from run content.
 */
export const FULL_AUTO_LIVENESS_APP_OFFLINE_GAP_MS = 30 * 60_000

/**
 * The bounded, distinct classifications the acceptance criteria require:
 * "Missing host thread, missing provider-native session, workspace
 * mismatch, auth/admission failure, stale lease, app offline, and unknown
 * error", plus `dispatch_overdue` for the exact silent-stall shape the
 * 2026-07-17 audit recorded -- a record that stayed enabled with no failure
 * ever recorded and nothing dispatched. That is deliberately its own bucket
 * rather than folded into `unknown_error`: the run's own durable state
 * proves nothing else (no failure, no missing thread, no workspace/auth
 * block) explains the gap, so calling it "unknown" would be less honest,
 * not more.
 */
export const FullAutoStallCauseSchema = Schema.Literals([
  "host_thread_missing",
  "provider_session_missing",
  "workspace_mismatch",
  "auth_admission_failure",
  "stale_lease",
  "app_offline",
  "dispatch_overdue",
  "unknown_error",
])
export type FullAutoStallCause = typeof FullAutoStallCauseSchema.Type

/** FA-AC-48: every Stalled run exposes at least one owner-actionable
 * affordance. `stop_only` marks a nonrecoverable, fail-closed cause (the
 * issue's "Nonrecoverable states fail closed and present one safe action"). */
export const FullAutoRecoveryActionSchema = Schema.Literals(["retry_now", "stop_only", "none"])
export type FullAutoRecoveryAction = typeof FullAutoRecoveryActionSchema.Type

/** Causes a retry cannot plausibly fix without owner intervention (a
 * mismatched/unbound workspace, a lane that failed admission, or a thread
 * record that no longer exists at all) fail closed to Stop-only. Every other
 * cause -- a missing provider session, a stale FA-H3 lease, a bare
 * reconciliation gap, or an unclassified error -- is offered "retry now"
 * because a fresh dispatch attempt is a safe, bounded, exactly-once action
 * (FA-H3 still governs it) that may simply succeed. */
const NONRECOVERABLE_STALL_CAUSES: ReadonlySet<FullAutoStallCause> = new Set([
  "host_thread_missing",
  "workspace_mismatch",
  "auth_admission_failure",
])

export const recoveryActionForCause = (cause: FullAutoStallCause | null): FullAutoRecoveryAction => {
  if (cause === null) return "none"
  return NONRECOVERABLE_STALL_CAUSES.has(cause) ? "stop_only" : "retry_now"
}

/**
 * Exact-match classification over the small, finite dispatch-failure reason
 * vocabulary this repository's own call sites produce (`main.ts`'s
 * `dispatch` wiring in `runFullAutoReconciliation`, and
 * `reconcileFullAutoThreads`'s in-flight guard). This is deterministic
 * lookup over a bounded set of internally-minted literal strings -- never
 * free-form user-intent parsing, and never a substitute for a typed reason
 * code should one land upstream later. An unrecognized reason honestly
 * classifies as `unknown_error` rather than guessing.
 */
export const classifyFullAutoDispatchFailureReason = (reason: string | null | undefined): FullAutoStallCause => {
  if (reason === null || reason === undefined || reason.length === 0) return "unknown_error"
  if (reason === "host_thread_missing") return "host_thread_missing"
  if (reason === "provider_session_missing") return "provider_session_missing"
  // Backward compatibility for rows persisted before #9001 added the typed
  // host failure. This display string has always come from the Desktop
  // ThreadStore check in provider-lane.ts, not from a provider session.
  if (reason === "That conversation no longer exists.") return "host_thread_missing"
  // reconcileFullAutoThreads's FA-H3 defense-in-depth guard: the durable
  // lease and the local-turn journal disagree about whether a turn is
  // already in flight.
  if (reason === "turn_already_in_flight") return "stale_lease"
  if (reason.startsWith("full_auto_lane_not_eligible:")) return "auth_admission_failure"
  if (reason === "workspace_mismatch" || reason === "workspace_unbound") return "workspace_mismatch"
  return "unknown_error"
}

export type FullAutoLivenessProjection = Readonly<{
  runRef: string
  /** What the run's lifecycle state honestly is right now, derived from
   * durable fields alone. `settleFullAutoRunLiveness` always returns a run
   * whose `.state` equals this (subject to the legal-transition graph);
   * read-only callers may use `classifyFullAutoRunLiveness` directly to
   * project without writing. */
  projectedState: FullAutoRunState
  /** Non-null only for `retrying`/`stalled`. */
  cause: FullAutoStallCause | null
  /** ISO timestamp of the next eligible retry, or null when none is
   * scheduled (healthy Running, or a Stalled run with no automatic retry). */
  nextRetryAt: string | null
  recoveryAction: FullAutoRecoveryAction
  /** Milliseconds since the run's last recorded progress; always >= 0. Feeds
   * the run report's liveness-gap signal (#8972) and control-API/sidebar
   * display without this module owning that persistence itself. */
  sinceLastProgressMs: number
}>

const parseTimeMs = (value: string | undefined): number | null => {
  if (value === undefined) return null
  const parsed = Date.parse(value)
  return Number.isNaN(parsed) ? null : parsed
}

const staleCheckCause = (run: FullAutoRun, now: Date): FullAutoStallCause => {
  const lastCheckMs = parseTimeMs(run.lastLivenessCheckAt)
  return lastCheckMs !== null && now.getTime() - lastCheckMs >= FULL_AUTO_LIVENESS_APP_OFFLINE_GAP_MS
    ? "app_offline"
    : "dispatch_overdue"
}

/**
 * Pure classification: given a run's durable fields, its bound thread-level
 * snapshot, and the current time, compute the liveness projection. Never
 * mutates anything -- `settleFullAutoRunLiveness` below is the only writer.
 */
export const classifyFullAutoRunLiveness = (input: Readonly<{
  run: FullAutoRun
  snapshot: FullAutoRunThreadSnapshot
  now: Date
}>): FullAutoLivenessProjection => {
  const { run, snapshot, now } = input
  const nowMs = now.getTime()
  const referenceMs = parseTimeMs(run.lastProgressAt) ?? parseTimeMs(run.startedAt) ?? parseTimeMs(run.createdAt) ??
    nowMs
  const sinceLastProgressMs = Math.max(0, nowMs - referenceMs)

  const finalize = (
    projectedState: FullAutoRunState,
    cause: FullAutoStallCause | null,
    nextRetryAt: string | null,
  ): FullAutoLivenessProjection => ({
    runRef: run.runRef,
    projectedState,
    cause,
    nextRetryAt,
    recoveryAction: projectedState === "stalled" ? recoveryActionForCause(cause) : "none",
    sinceLastProgressMs,
  })

  // Draft, Paused/Pausing, and every terminal state are outside this
  // classifier's authority -- FA-RUN-01's own transitions and
  // `settleFullAutoRunFromThreadState`'s turn-resolution sync already own
  // those edges exactly.
  if (
    run.state === "draft" || run.state === "paused" || run.state === "pausing" ||
    isFullAutoRunTerminal(run.state)
  ) {
    return finalize(run.state, null, null)
  }

  // FA-AC-42: a bound thread whose registry record has vanished entirely --
  // never silently reattach, never leave the run claiming Running.
  if (run.threadRef !== undefined && snapshot.threadRecord === null) {
    return finalize("stalled", "host_thread_missing", null)
  }

  const record = snapshot.threadRecord
  const failures = record?.consecutiveFailures ?? 0

  // AC-48: Stalled clears only through an explicit actor action (retry now)
  // or Stop -- never automatically, even if the underlying thread-level
  // record happens to look healthy again (for example an unrelated
  // reconciliation trigger dispatched this same thread successfully while
  // the run's own lifecycle state was still Stalled). Recompute an honest,
  // CURRENT cause for display without silently leaving Stalled.
  if (run.state === "stalled") {
    if (failures > 0) {
      return finalize("stalled", classifyFullAutoDispatchFailureReason(record?.blockedReason ?? null), null)
    }
    return finalize("stalled", staleCheckCause(run, now), null)
  }

  const lastFailureMs = parseTimeMs(record?.lastFailureAt)
  if (failures > 0 && lastFailureMs !== null) {
    const retryEtaMs = lastFailureMs + fullAutoFailureBackoffMs(failures)
    const cause = classifyFullAutoDispatchFailureReason(record?.blockedReason ?? null)
    if (nowMs < retryEtaMs + FULL_AUTO_LIVENESS_RETRY_GRACE_MS) {
      // Still within backoff, or within the grace period the next trigger
      // needs to actually fire -- Retrying with a real ETA, never generic
      // Running or a terminal-looking failure banner (AC #2).
      return finalize("retrying", cause, new Date(retryEtaMs).toISOString())
    }
    // Backoff plus grace elapsed with no fresh reconciliation attempt
    // recorded: the scheduled retry silently never happened.
    return finalize("stalled", cause, null)
  }

  if (snapshot.turnRunning) return finalize("running", null, null)

  if (sinceLastProgressMs <= FULL_AUTO_LIVENESS_DISPATCH_SLO_MS) {
    return finalize("running", null, null)
  }

  // AC-48's retry-now affordance transitions Stalled -> Retrying with no
  // FA-H5 failure/backoff state to anchor an ETA (the exact `dispatch_overdue`/
  // `app_offline`/`stale_lease` shape: nothing "failed", a fresh attempt was
  // just requested). Without this, the elapsed-SLO check above would
  // immediately re-classify Stalled on the very next evaluation, before the
  // reconciliation pass retry-now just scheduled has had a chance to run.
  // Give it the same bounded grace window a backoff-derived retry gets,
  // anchored to when Retrying was actually entered.
  if (run.state === "retrying") {
    const enteredRetryingAtMs = parseTimeMs(
      [...run.transitions].reverse().find(transition => transition.to === "retrying")?.at,
    )
    if (enteredRetryingAtMs !== null && nowMs - enteredRetryingAtMs < FULL_AUTO_LIVENESS_RETRY_GRACE_MS) {
      return finalize(
        "retrying",
        staleCheckCause(run, now),
        new Date(enteredRetryingAtMs + FULL_AUTO_LIVENESS_RETRY_GRACE_MS).toISOString(),
      )
    }
  }

  // No recorded failure, no turn in flight, and the SLO window since the
  // last terminal turn elapsed with nothing dispatched -- exactly the
  // 2026-07-17 incident shape (FA-AC-42/47).
  return finalize("stalled", staleCheckCause(run, now), null)
}

const describeLivenessTransitionReason = (projection: FullAutoLivenessProjection): string => {
  if (projection.projectedState === "retrying") {
    return `liveness monitor: a recoverable dispatch failure is in backoff (cause: ${
      projection.cause ?? "unknown_error"
    }); next retry eligible at ${projection.nextRetryAt ?? "unknown"}`
  }
  if (projection.projectedState === "stalled") {
    return `liveness monitor: no continuation was accepted within the liveness SLO window (cause: ${
      projection.cause ?? "unknown_error"
    })`
  }
  return `liveness monitor: continuation dispatch resumed normally`
}

/**
 * The single mutating entry point. Callers pass the same `FullAutoRunRegistry`
 * and per-thread `FullAutoRunThreadSnapshot` shape `settleFullAutoRunFromThreadState`
 * already uses; this function:
 *
 *  1. Applies FA-RUN-01's existing fail-closed sync first (Pausing
 *     settlement, missing-thread orphan, and disabledBy-attributed
 *     cap/failure-limit/workspace-guard transitions) so the two settle
 *     functions never disagree about a state a prior pass already decided.
 *  2. Classifies liveness against the (possibly just-synced) run.
 *  3. Stamps `lastLivenessCheckAt` on every non-terminal, non-draft run it
 *     evaluates -- the durable "we actually checked this run at this time"
 *     signal `staleCheckCause` and future report/observability consumers
 *     rely on.
 *  4. Applies the classified transition ONLY when it differs from the
 *     current state and is a legal edge -- `classifyFullAutoRunLiveness`'s
 *     sticky-Stalled rule (above) means this never attempts an automatic
 *     exit from Stalled, so the legality check is defense in depth, not the
 *     primary guarantee.
 *
 * Every write is attributed to the `liveness_monitor` actor FA-RUN-01
 * already reserved for this module.
 */
export const settleFullAutoRunLiveness = (
  runRegistry: FullAutoRunRegistry,
  run: FullAutoRun,
  snapshot: FullAutoRunThreadSnapshot,
  now: () => Date = () => new Date(),
): Readonly<{ run: FullAutoRun; projection: FullAutoLivenessProjection }> => {
  const afterThreadSync = settleFullAutoRunFromThreadState(runRegistry, run, snapshot)
  const nowValue = now()

  if (isFullAutoRunTerminal(afterThreadSync.state) || afterThreadSync.state === "draft") {
    return {
      run: afterThreadSync,
      projection: classifyFullAutoRunLiveness({ run: afterThreadSync, snapshot, now: nowValue }),
    }
  }

  const touched = runRegistry.touchLiveness(afterThreadSync.runRef, nowValue.toISOString()) ?? afterThreadSync
  const projection = classifyFullAutoRunLiveness({ run: touched, snapshot, now: nowValue })

  if (
    projection.projectedState === touched.state ||
    !isLegalFullAutoRunTransition(touched.state, projection.projectedState)
  ) {
    return { run: touched, projection }
  }

  const result = runRegistry.transition(touched.runRef, {
    to: projection.projectedState,
    actor: "liveness_monitor",
    reason: describeLivenessTransitionReason(projection),
  })
  return { run: result.ok ? result.run : touched, projection }
}

/**
 * FA-AC-48's owner-actionable "retry now" affordance. Legal only from
 * Stalled, and only when the run's CURRENT (freshly classified) cause is
 * plausibly recoverable -- a fail-closed cause refuses here so the control
 * surface can present the one safe action (Stop) instead of a retry that is
 * guaranteed to repeat the same nonrecoverable failure.
 */
export type FullAutoRunRetryNowResult =
  | Readonly<{ ok: true; run: FullAutoRun }>
  | Readonly<{ ok: false; reason: "not_stalled"; state: FullAutoRunState }>
  | Readonly<{ ok: false; reason: "not_recoverable"; cause: FullAutoStallCause | null }>

export const retryFullAutoRunNow = (
  runRegistry: FullAutoRunRegistry,
  run: FullAutoRun,
  snapshot: FullAutoRunThreadSnapshot,
  input: Readonly<{ actor: FullAutoRunActor }>,
  now: () => Date = () => new Date(),
): FullAutoRunRetryNowResult => {
  if (run.state !== "stalled") return { ok: false, reason: "not_stalled", state: run.state }
  const projection = classifyFullAutoRunLiveness({ run, snapshot, now: now() })
  if (projection.recoveryAction !== "retry_now") return { ok: false, reason: "not_recoverable", cause: projection.cause }
  const result = runRegistry.transition(run.runRef, {
    to: "retrying",
    actor: input.actor,
    reason: `retry now requested (cause was: ${projection.cause ?? "unknown_error"})`,
  })
  if (!result.ok) return { ok: false, reason: "not_stalled", state: run.state }
  return { ok: true, run: result.run }
}

// -----------------------------------------------------------------------
// Attention signal (issue's "Attention signals" section): a dedup'd,
// permission-gated, redacted notification decision. Pure and testable --
// the actual OS notification call (Electron `Notification`) is main-owned
// wiring outside this module, which is IPC/control-API surface, not the
// dedicated sidebar UI (#8974).
// -----------------------------------------------------------------------

/** Only Retrying and Stalled are attention-worthy; Paused/Stopped/Completed/
 * Cap-reached are owner intent or an expected terminal outcome, never a
 * silent-stall signal. */
export const FULL_AUTO_LIVENESS_NOTIFIABLE_STATES: ReadonlySet<FullAutoRunState> = new Set(["retrying", "stalled"])

export type FullAutoLivenessNotifyDecision = Readonly<{
  /** False when the state is genuinely notifiable and new but permission was
   * denied -- the caller still updates its dedup key so a denied permission
   * does not cause a re-decision storm on every subsequent tick. */
  notify: boolean
  dedupKey: string
  /** Names the run and state only -- never objective/doneCondition/transcript
   * text (the issue's explicit redaction requirement). */
  title: string
  body: string
}>

export const decideFullAutoLivenessNotification = (input: Readonly<{
  runRef: string
  runTitle: string
  projectedState: FullAutoRunState
  cause: FullAutoStallCause | null
  /** The dedup key most recently notified (or attempted) for this run, or
   * null if none yet. */
  previousDedupKey: string | null
  /** Collapsed permission signal -- main.ts computes this from the existing
   * `notifications.taskCompletion` (and, optionally, `onlyWhenUnfocused`)
   * desktop preference; this module has no Electron dependency. */
  permissionGranted: boolean
}>): FullAutoLivenessNotifyDecision | null => {
  if (!FULL_AUTO_LIVENESS_NOTIFIABLE_STATES.has(input.projectedState)) return null
  const dedupKey = `${input.runRef}:${input.projectedState}:${input.cause ?? "none"}`
  if (dedupKey === input.previousDedupKey) return null
  // Redaction: title/body are built exclusively from the bounded run title
  // (never objective/doneCondition) and the typed state -- structurally
  // incapable of carrying transcript or objective content.
  const stateLabel = input.projectedState === "stalled" ? "stalled" : "retrying"
  return {
    notify: input.permissionGranted,
    dedupKey,
    title: `Full Auto ${stateLabel}`,
    body: `"${input.runTitle}" is ${stateLabel}.`,
  }
}
