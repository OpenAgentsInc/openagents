import type {
  KhalaCodeDesktopFleetRunProjection,
  KhalaCodeDesktopKhalaSyncFleetStateResult,
} from "../shared/rpc"

/**
 * Khala Sync → Fleet screen view-model mapping (KS-6.2, #8303).
 *
 * Behavior contracts bound here (see
 * clients/khala-code-desktop/src/contracts/ux-contracts.ts):
 *
 * - khala_code.fleet.khala_sync_indicator_truthful.v1 — the freshness
 *   indicator reflects server truth: "Live" is rendered ONLY when the sync
 *   session's live socket is open (`phase === "live"`); every other phase
 *   renders an explicit syncing/reconnecting/re-sync state. No fake
 *   freshness, ever.
 * - khala_code.fleet.khala_sync_must_refetch_recovers.v1 — `must_refetch`
 *   maps to a visible "Resyncing" state (the session auto re-bootstraps);
 *   it never maps to an empty/hidden indicator that would strand the
 *   screen without explanation.
 */

export type KhalaSyncFleetIndicator = Readonly<{
  /** True ONLY for an open live socket. */
  live: boolean
  label: string
  tone: "live" | "syncing" | "degraded" | "disabled"
  phase: KhalaCodeDesktopKhalaSyncFleetStateResult["phase"]
}>

export const khalaSyncFleetIndicator = (
  state: KhalaCodeDesktopKhalaSyncFleetStateResult,
): KhalaSyncFleetIndicator => {
  if (!state.enabled) {
    return { live: false, label: "Khala Sync off", tone: "disabled", phase: state.phase }
  }
  if (state.authState === "missing") {
    return {
      live: false,
      label: "Khala Sync: connect OpenAgents",
      tone: "degraded",
      phase: state.phase,
    }
  }
  switch (state.phase) {
    case "live":
      return { live: true, label: "Khala Sync: Live", tone: "live", phase: state.phase }
    case "bootstrapping":
      return {
        live: false,
        label: "Khala Sync: Bootstrapping…",
        tone: "syncing",
        phase: state.phase,
      }
    case "catching_up":
      return {
        live: false,
        label: "Khala Sync: Catching up…",
        tone: "syncing",
        phase: state.phase,
      }
    case "must_refetch":
      return {
        live: false,
        label: `Khala Sync: Resyncing${state.reason === null ? "" : ` (${state.reason})`}…`,
        tone: "syncing",
        phase: state.phase,
      }
    case "denied":
      // Fail-closed scope auth (KS-7.1): the server refused this scope.
      // Honest terminal state — never shown as syncing or live.
      return {
        live: false,
        label: "Khala Sync: Access denied",
        tone: "degraded",
        phase: state.phase,
      }
    case "idle":
    case "disabled":
      return {
        live: false,
        label: "Khala Sync: Reconnecting…",
        tone: "degraded",
        phase: state.phase,
      }
  }
}

/**
 * Merge the synced `fleet_run` entity over the locally-known run projection.
 * Server truth wins for lifecycle status, desired slots, counters, and
 * timestamps; fields the public-safe sync entity does not carry (work
 * source, refill policy, pylon ref) keep their local values.
 */
export const mergeKhalaSyncActiveFleetRun = (
  local: KhalaCodeDesktopFleetRunProjection,
  sync: KhalaCodeDesktopKhalaSyncFleetStateResult,
): KhalaCodeDesktopFleetRunProjection => {
  const run = sync.run
  if (!sync.enabled || run === null || run.runId !== local.runRef) return local
  return {
    ...local,
    counters: { ...run.counters },
    startedAt: run.startedAt,
    state: run.status,
    targetConcurrency: run.desiredSlots,
    updatedAt: run.updatedAt,
    workerKind: run.workerKind,
  }
}
