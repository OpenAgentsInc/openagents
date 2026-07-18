import type {
  FullAutoRunClientLifecycleState,
  FullAutoRunClientProjection,
  FullAutoRunClientRunProjection,
} from "@openagentsinc/khala-sync"

/**
 * Mobile-local helpers over the real, landed `FullAutoRun` mobile projection
 * (openagents #8981, FA-RUN-05 — `packages/khala-sync/src/full-auto-run-client-projection.ts`,
 * schema id `full_auto_run.mobile_projection.v1`). #8982 consumes the real
 * shape directly rather than a parallel local schema; this file only adds
 * mobile-specific derived predicates (active/fresh, display truncation).
 */
export type FullAutoRunMobileProjection = FullAutoRunClientRunProjection
export type FullAutoRunLifecycleState = FullAutoRunClientLifecycleState
export type { FullAutoRunClientProjection }

export const FullAutoRunLifecycleStateLabel: Readonly<Record<FullAutoRunLifecycleState, string>> = {
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

/**
 * A run is still "live" work worth prioritizing/displaying. `draft` (not yet
 * started) and the terminal states (`completed`/`failed`/`stopped`/
 * `cap_reached`) are excluded — those are not an in-progress run mobile
 * should auto-navigate to or show a live header for.
 */
export const isFullAutoRunLifecycleActive = (state: FullAutoRunLifecycleState): boolean =>
  state === "running" || state === "pausing" || state === "paused" ||
  state === "retrying" || state === "stalled"

/** MOB-FA-02 (#8994): the run has ended -- never transitions again. Used to
 * gate the terminal run-report surface: a fresh terminal projection still
 * renders the header (to show the report) even though it is no longer
 * "active" work. */
export const isFullAutoRunLifecycleTerminal = (state: FullAutoRunLifecycleState): boolean =>
  state === "completed" || state === "failed" || state === "stopped" || state === "cap_reached"

/** Mobile treats an active-looking run as stale once its projection has not
 * been refreshed for this long, matching the freshness posture used by other
 * live projections (e.g. Pylon heartbeat freshness — Desktop republishes on
 * a 60s heartbeat while Running per #8981) rather than trusting a possibly-
 * abandoned `lifecycleState` forever. */
export const FULL_AUTO_RUN_STALE_AFTER_MS = 10 * 60 * 1000

export const isFullAutoRunProjectionFresh = (
  projection: FullAutoRunMobileProjection,
  nowMs: number,
  staleAfterMs: number = FULL_AUTO_RUN_STALE_AFTER_MS,
): boolean => {
  const updatedAtMs = Date.parse(projection.updatedAt)
  if (!Number.isFinite(updatedAtMs)) return false
  return nowMs - updatedAtMs < staleAfterMs
}

/** The single predicate thread-selection and header display both use to
 * decide whether a fetched run still counts as "an active run". */
export const isFullAutoRunProjectionActive = (
  projection: FullAutoRunMobileProjection,
  nowMs: number = Date.now(),
  staleAfterMs?: number,
): boolean =>
  isFullAutoRunLifecycleActive(projection.lifecycleState) &&
  isFullAutoRunProjectionFresh(projection, nowMs, staleAfterMs)

export type FullAutoRunProjectionResult =
  | Readonly<{ state: "active"; projection: FullAutoRunMobileProjection }>
  | Readonly<{ state: "none" }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "unavailable" }>

/** Truncate the objective for the compact mobile header per #8982 ("the
 * objective (or a truncated version)"). */
export const truncateFullAutoRunObjective = (objective: string, maxLength = 96): string =>
  objective.length <= maxLength ? objective : `${objective.slice(0, maxLength - 1).trimEnd()}…`
