/**
 * Stateful driver that turns the pure Apple FM bridge supervision policy into
 * something a real launcher can run.
 *
 * `reduceAppleFmBridgeSupervisor` (see ./apple-fm-bridge-supervisor.ts) is a
 * deterministic, side-effect-free reducer: given a state + event it returns the
 * next state and the single action a caller should perform
 * (`spawn` / `mark_running` / `schedule_restart` / `give_up` / `none`). Nothing
 * yet holds that state across events, nor translates those actions into actual
 * effects, nor closes the backoff loop by re-feeding a `tick` when a restart
 * timer fires.
 *
 * This module supplies exactly that glue while keeping ALL real I/O injected:
 * the caller provides `spawn`, an optional `markRunning`/`giveUp`, a `now()`
 * clock, and a `timer` abstraction. A live launcher passes real
 * `Bun.spawn` + `setTimeout`; tests pass fakes. The driver itself never reads
 * the wall clock, never spawns a process, opens a socket, or emits prompts,
 * file contents, paths, tokens, URLs, or bearer material.
 *
 * It advances (does NOT clear):
 *   blocker.product_promises.local_apple_fm_helper_supervision_missing
 */

import {
  createAppleFmBridgeSupervisorState,
  reduceAppleFmBridgeSupervisor,
  type AppleFmBridgeSupervisorAction,
  type AppleFmBridgeSupervisorConfig,
  type AppleFmBridgeSupervisorEvent,
  type AppleFmBridgeSupervisorPhase,
  type AppleFmBridgeSupervisorState,
} from "./apple-fm-bridge-supervisor.js"
import {
  summarizeAppleFmBridgeSupervisor,
  type PylonAppleFmSupervisorStatus,
} from "./apple-fm-bridge-supervisor-status.js"

/** Opaque handle returned by the injected timer; the driver never inspects it. */
export type AppleFmBridgeSupervisorTimerHandle = unknown

/**
 * Minimal timer abstraction the driver uses to schedule the bounded backoff
 * restart. A live launcher backs this with `setTimeout`/`clearTimeout`; tests
 * back it with a controllable fake so backoff scheduling stays deterministic.
 */
export type AppleFmBridgeSupervisorTimer = {
  set(
    callback: () => void,
    delayMs: number,
  ): AppleFmBridgeSupervisorTimerHandle
  clear(handle: AppleFmBridgeSupervisorTimerHandle): void
}

export type AppleFmBridgeSupervisorDriverOptions = {
  /** Policy overrides; defaults come from APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS. */
  readonly config?: Partial<AppleFmBridgeSupervisorConfig>
  /** Perform the actual helper launch (e.g. Bun.spawn). Required. */
  readonly spawn: () => void
  /** Optional hook fired when the helper transitions to running. */
  readonly markRunning?: () => void
  /** Optional hook fired once supervision gives up after a crash loop. */
  readonly giveUp?: (blockerRef: string) => void
  /** Inject the clock so the driver stays reproducible (no wall-clock reads). */
  readonly now: () => number
  /** Schedule/cancel the backoff restart timer. */
  readonly timer: AppleFmBridgeSupervisorTimer
}

/**
 * A live, stateful handle around the supervision policy. The launcher feeds it
 * lifecycle signals (`requestStart`, `notifyStarted`, `notifyHealthy`,
 * `notifyExited`); the driver folds them through the reducer, performs the
 * emitted action via the injected effects, and re-feeds a `tick` when the
 * backoff timer fires so a restart actually happens.
 */
export type AppleFmBridgeSupervisorDriver = {
  /** Ask supervision to start the helper (no-op if already up / given up). */
  requestStart(): AppleFmBridgeSupervisorAction
  /** The helper process has started. */
  notifyStarted(): AppleFmBridgeSupervisorAction
  /** The helper reported a healthy heartbeat. */
  notifyHealthy(): AppleFmBridgeSupervisorAction
  /** The helper process exited (crash or clean stop). */
  notifyExited(
    exitCode: number | null,
    signal?: string | null,
  ): AppleFmBridgeSupervisorAction
  /** Current supervision phase. */
  phase(): AppleFmBridgeSupervisorPhase
  /** Immutable snapshot of the internal reducer state. */
  snapshot(): AppleFmBridgeSupervisorState
  /** Public-safe status summary for the apple_fm.status surface. */
  status(): PylonAppleFmSupervisorStatus
  /** Cancel any pending restart timer (e.g. on shutdown). */
  dispose(): void
}

export function createAppleFmBridgeSupervisorDriver(
  options: AppleFmBridgeSupervisorDriverOptions,
): AppleFmBridgeSupervisorDriver {
  let state = createAppleFmBridgeSupervisorState(options.config)
  let pendingTimer: AppleFmBridgeSupervisorTimerHandle | null = null

  function clearPendingTimer(): void {
    if (pendingTimer !== null) {
      options.timer.clear(pendingTimer)
      pendingTimer = null
    }
  }

  function dispatch(
    event: AppleFmBridgeSupervisorEvent,
  ): AppleFmBridgeSupervisorAction {
    const transition = reduceAppleFmBridgeSupervisor(state, event)
    state = transition.state
    const action = transition.action

    switch (action.kind) {
      case "spawn":
        // A fresh spawn supersedes any pending backoff timer.
        clearPendingTimer()
        options.spawn()
        break
      case "mark_running":
        clearPendingTimer()
        options.markRunning?.()
        break
      case "schedule_restart":
        // Replace any prior timer; when it fires, re-feed a tick so the
        // reducer can transition backoff -> starting and emit `spawn`.
        clearPendingTimer()
        pendingTimer = options.timer.set(() => {
          pendingTimer = null
          dispatch({ kind: "tick", nowMs: options.now() })
        }, action.delayMs)
        break
      case "give_up":
        clearPendingTimer()
        options.giveUp?.(action.blockerRef)
        break
      case "none":
        break
    }

    return action
  }

  return {
    requestStart() {
      return dispatch({ kind: "start_requested", nowMs: options.now() })
    },
    notifyStarted() {
      return dispatch({ kind: "process_started", nowMs: options.now() })
    },
    notifyHealthy() {
      return dispatch({ kind: "health_ok", nowMs: options.now() })
    },
    notifyExited(exitCode, signal) {
      return dispatch({
        kind: "process_exited",
        nowMs: options.now(),
        exitCode,
        signal: signal ?? null,
      })
    },
    phase() {
      return state.phase
    },
    snapshot() {
      return state
    },
    status() {
      return summarizeAppleFmBridgeSupervisor(state, options.now())
    },
    dispose() {
      clearPendingTimer()
    },
  }
}
