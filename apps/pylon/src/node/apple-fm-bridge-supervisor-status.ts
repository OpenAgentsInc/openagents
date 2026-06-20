/**
 * Public-safe projection of the local Apple FM bridge supervisor phase.
 *
 * The pure supervision policy in ./apple-fm-bridge-supervisor.ts decides *when*
 * to (re)launch the Foundation Models bridge helper and *when to give up*, but
 * its raw state (`AppleFmBridgeSupervisorState`) is an internal reducer shape.
 * For the `apple_fm.status` surface and Autopilot Desktop to *observe* whether
 * supervision is healthy, recovering, or has stopped, that internal state needs
 * a stable, redacted summary that carries no prompts, file contents, paths,
 * tokens, URLs, or bearer material — only coarse supervision health.
 *
 * This module supplies that projection deterministically: the caller injects
 * `nowMs` so backoff-remaining is computed without reading the clock, keeping
 * the summary reproducible in tests.
 *
 * It advances (does not clear):
 *   blocker.product_promises.local_apple_fm_helper_supervision_missing
 */

import {
  APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
  type AppleFmBridgeSupervisorPhase,
  type AppleFmBridgeSupervisorState,
} from "./apple-fm-bridge-supervisor.js"

export const PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA =
  "openagents.pylon.apple_fm.supervisor.v0.1" as const

/**
 * Coarse, operator-facing health of the bridge supervisor:
 * - `running`: a helper process is up (may still be warming to healthy).
 * - `starting`: a spawn has been requested / a backoff timer just fired.
 * - `recovering`: the helper crashed and a bounded backoff restart is pending.
 * - `stopped`: supervision gave up after a crash loop; manual action needed.
 * - `idle`: supervision has not been asked to start yet.
 */
export type AppleFmBridgeSupervisorHealth =
  | "idle"
  | "starting"
  | "running"
  | "recovering"
  | "stopped"

export type PylonAppleFmSupervisorStatus = {
  readonly schema: typeof PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA
  readonly kind: "pylon_apple_fm_supervisor_status"
  readonly health: AppleFmBridgeSupervisorHealth
  readonly phase: AppleFmBridgeSupervisorPhase
  /** True while supervision is actively managing the helper (not idle/stopped). */
  readonly supervised: boolean
  /** Consecutive restarts driving the current backoff escalation. */
  readonly consecutiveRestarts: number
  /** Restart attempts still counted inside the sliding restart window. */
  readonly restartsInWindow: number
  /** Remaining backoff (ms) before the next restart, or null when not waiting. */
  readonly backoffRemainingMs: number | null
  /** Stable blocker ref(s) the surface should expose; empty when healthy. */
  readonly blockerRefs: ReadonlyArray<string>
  /** Marker that this summary intentionally carries no sensitive content. */
  readonly contentRedacted: true
}

function healthFromPhase(
  phase: AppleFmBridgeSupervisorPhase,
): AppleFmBridgeSupervisorHealth {
  switch (phase) {
    case "idle":
      return "idle"
    case "starting":
      return "starting"
    case "running":
      return "running"
    case "backoff":
      return "recovering"
    case "given_up":
      return "stopped"
  }
}

/**
 * Project the internal supervisor state into a public-safe status summary.
 *
 * `nowMs` is used only to (a) compute remaining backoff and (b) prune restart
 * attempts that have aged out of the sliding window, so the reported
 * `restartsInWindow` matches what the policy would currently enforce.
 */
export function summarizeAppleFmBridgeSupervisor(
  state: AppleFmBridgeSupervisorState,
  nowMs: number,
): PylonAppleFmSupervisorStatus {
  const windowStart = nowMs - state.config.restartWindowMs
  const restartsInWindow = state.restartTimestamps.filter(
    (ts) => ts >= windowStart,
  ).length

  const backoffRemainingMs =
    state.phase === "backoff" && state.backoffUntilMs !== null
      ? Math.max(0, state.backoffUntilMs - nowMs)
      : null

  const blockerRefs: string[] = []
  if (state.phase === "given_up") {
    blockerRefs.push(
      state.givenUpBlockerRef ?? APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
    )
  }

  return {
    schema: PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA,
    kind: "pylon_apple_fm_supervisor_status",
    health: healthFromPhase(state.phase),
    phase: state.phase,
    supervised: state.phase !== "idle" && state.phase !== "given_up",
    consecutiveRestarts: state.consecutiveRestarts,
    restartsInWindow,
    backoffRemainingMs,
    blockerRefs,
    contentRedacted: true,
  }
}
