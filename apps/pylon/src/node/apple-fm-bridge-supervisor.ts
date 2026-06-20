/**
 * Pure restart/backoff supervision policy for the local Apple FM Foundation
 * Models bridge helper.
 *
 * `discoverAppleFmBridgeHelper` (see ./apple-fm-bridge-helper.ts) only resolves
 * *where* the helper binary lives; nothing yet decides *when* to (re)launch it
 * or *when to stop trying* after repeated crashes. This module supplies that
 * decision layer as a deterministic, side-effect-free reducer so a launcher
 * (Pylon node host or a signed-installer helper supervisor) can drive an actual
 * `Bun.spawn`/`child_process` lifecycle on top of testable policy.
 *
 * It advances:
 *   blocker.product_promises.local_apple_fm_helper_supervision_missing
 *
 * It deliberately does NOT spawn processes, open sockets, or read the clock:
 * the caller injects `nowMs` on every event so the policy stays fully
 * reproducible in tests and emits no secrets, prompts, or local paths.
 */

export const APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS = {
  /** Maximum restart attempts allowed inside `restartWindowMs`. */
  maxRestartsInWindow: 5,
  /** Sliding window (ms) over which restart attempts are counted. */
  restartWindowMs: 60_000,
  /** First backoff delay (ms) before the initial restart. */
  baseBackoffMs: 500,
  /** Upper bound (ms) for any single backoff delay. */
  maxBackoffMs: 30_000,
  /** Exponential growth factor applied per consecutive restart. */
  backoffFactor: 2,
  /**
   * Uptime (ms) after which a healthy helper is considered "stable" and the
   * restart counter / backoff escalation reset, so transient crashes long ago
   * do not permanently penalize a currently-healthy helper.
   */
  stableUptimeResetMs: 120_000,
} as const

export type AppleFmBridgeSupervisorConfig = {
  readonly maxRestartsInWindow: number
  readonly restartWindowMs: number
  readonly baseBackoffMs: number
  readonly maxBackoffMs: number
  readonly backoffFactor: number
  readonly stableUptimeResetMs: number
}

export type AppleFmBridgeSupervisorPhase =
  | "idle"
  | "starting"
  | "running"
  | "backoff"
  | "given_up"

export type AppleFmBridgeSupervisorState = {
  readonly config: AppleFmBridgeSupervisorConfig
  readonly phase: AppleFmBridgeSupervisorPhase
  /** Epoch-ms timestamps of recent restart attempts, pruned to the window. */
  readonly restartTimestamps: ReadonlyArray<number>
  /** When the current process began running healthy, if any. */
  readonly runningSinceMs: number | null
  /** When the active backoff timer expires, if in backoff. */
  readonly backoffUntilMs: number | null
  /** Consecutive restart count used for backoff escalation. */
  readonly consecutiveRestarts: number
  /** Stable supervision blocker ref when the policy gives up; null otherwise. */
  readonly givenUpBlockerRef: string | null
}

export type AppleFmBridgeSupervisorEvent =
  | { readonly kind: "start_requested"; readonly nowMs: number }
  | { readonly kind: "process_started"; readonly nowMs: number }
  | { readonly kind: "health_ok"; readonly nowMs: number }
  | {
      readonly kind: "process_exited"
      readonly nowMs: number
      readonly exitCode: number | null
      readonly signal?: string | null
    }
  | { readonly kind: "tick"; readonly nowMs: number }

export type AppleFmBridgeSupervisorAction =
  | { readonly kind: "spawn" }
  | { readonly kind: "mark_running" }
  | {
      readonly kind: "schedule_restart"
      readonly delayMs: number
      readonly attempt: number
    }
  | { readonly kind: "give_up"; readonly blockerRef: string }
  | { readonly kind: "none" }

export type AppleFmBridgeSupervisorTransition = {
  readonly state: AppleFmBridgeSupervisorState
  readonly action: AppleFmBridgeSupervisorAction
}

export const APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER =
  "blocker.pylon.apple_fm.bridge_supervisor.crash_loop" as const

export function createAppleFmBridgeSupervisorState(
  overrides: Partial<AppleFmBridgeSupervisorConfig> = {},
): AppleFmBridgeSupervisorState {
  return {
    config: resolveConfig(overrides),
    phase: "idle",
    restartTimestamps: [],
    runningSinceMs: null,
    backoffUntilMs: null,
    consecutiveRestarts: 0,
    givenUpBlockerRef: null,
  }
}

/**
 * Deterministically fold one supervision event into the supervisor state and
 * emit the single side-effecting action the caller should perform.
 */
export function reduceAppleFmBridgeSupervisor(
  state: AppleFmBridgeSupervisorState,
  event: AppleFmBridgeSupervisorEvent,
): AppleFmBridgeSupervisorTransition {
  switch (event.kind) {
    case "start_requested":
      return onStartRequested(state)
    case "process_started":
      return onProcessStarted(state, event.nowMs)
    case "health_ok":
      return onHealthOk(state, event.nowMs)
    case "process_exited":
      return onProcessExited(state, event.nowMs)
    case "tick":
      return onTick(state, event.nowMs)
  }
}

function onStartRequested(
  state: AppleFmBridgeSupervisorState,
): AppleFmBridgeSupervisorTransition {
  // Already supervising or permanently stopped: do not double-spawn.
  if (state.phase === "starting" || state.phase === "running") {
    return noop(state)
  }
  if (state.phase === "given_up") {
    return noop(state)
  }
  return {
    state: { ...state, phase: "starting", backoffUntilMs: null },
    action: { kind: "spawn" },
  }
}

function onProcessStarted(
  state: AppleFmBridgeSupervisorState,
  nowMs: number,
): AppleFmBridgeSupervisorTransition {
  if (state.phase === "given_up") return noop(state)
  return {
    state: { ...state, phase: "running", runningSinceMs: nowMs, backoffUntilMs: null },
    action: { kind: "mark_running" },
  }
}

function onHealthOk(
  state: AppleFmBridgeSupervisorState,
  nowMs: number,
): AppleFmBridgeSupervisorTransition {
  if (state.phase === "given_up") return noop(state)
  const runningSinceMs = state.runningSinceMs ?? nowMs
  const stable =
    nowMs - runningSinceMs >= state.config.stableUptimeResetMs
  if (!stable) {
    return noop({ ...state, phase: "running", runningSinceMs })
  }
  // Healthy long enough: forgive past crashes so old failures do not
  // permanently cap future restarts or inflate backoff.
  return noop({
    ...state,
    phase: "running",
    runningSinceMs,
    restartTimestamps: [],
    consecutiveRestarts: 0,
  })
}

function onProcessExited(
  state: AppleFmBridgeSupervisorState,
  nowMs: number,
): AppleFmBridgeSupervisorTransition {
  if (state.phase === "given_up") return noop(state)

  const windowStart = nowMs - state.config.restartWindowMs
  const recent = state.restartTimestamps.filter((ts) => ts >= windowStart)
  const attemptsInWindow = recent.length + 1

  if (attemptsInWindow > state.config.maxRestartsInWindow) {
    return {
      state: {
        ...state,
        phase: "given_up",
        runningSinceMs: null,
        backoffUntilMs: null,
        restartTimestamps: recent,
        givenUpBlockerRef: APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
      },
      action: {
        kind: "give_up",
        blockerRef: APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
      },
    }
  }

  const consecutiveRestarts = state.consecutiveRestarts + 1
  const delayMs = backoffDelayMs(state.config, consecutiveRestarts)
  return {
    state: {
      ...state,
      phase: "backoff",
      runningSinceMs: null,
      restartTimestamps: [...recent, nowMs],
      consecutiveRestarts,
      backoffUntilMs: nowMs + delayMs,
    },
    action: { kind: "schedule_restart", delayMs, attempt: consecutiveRestarts },
  }
}

function onTick(
  state: AppleFmBridgeSupervisorState,
  nowMs: number,
): AppleFmBridgeSupervisorTransition {
  if (state.phase !== "backoff" || state.backoffUntilMs === null) {
    return noop(state)
  }
  if (nowMs < state.backoffUntilMs) {
    return noop(state)
  }
  return {
    state: { ...state, phase: "starting", backoffUntilMs: null },
    action: { kind: "spawn" },
  }
}

/**
 * Backoff for the Nth consecutive restart (1-indexed):
 * base * factor^(attempt - 1), clamped to `maxBackoffMs`.
 */
export function backoffDelayMs(
  config: AppleFmBridgeSupervisorConfig,
  attempt: number,
): number {
  const exponent = Math.max(0, attempt - 1)
  const raw = config.baseBackoffMs * Math.pow(config.backoffFactor, exponent)
  return Math.min(config.maxBackoffMs, Math.round(raw))
}

function resolveConfig(
  overrides: Partial<AppleFmBridgeSupervisorConfig>,
): AppleFmBridgeSupervisorConfig {
  return { ...APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS, ...overrides }
}

function noop(
  state: AppleFmBridgeSupervisorState,
): AppleFmBridgeSupervisorTransition {
  return { state, action: { kind: "none" } }
}
