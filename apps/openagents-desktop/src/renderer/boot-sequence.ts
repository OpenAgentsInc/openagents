import { CODEX_CHIP_REASON_VERIFYING } from "../codex-local-contract.ts"
import type { DesktopShellState, HarnessLaneAvailability } from "./shell.ts"

/**
 * Boot Sequence (owner directive 2026-07-19). When the app opens it shows a
 * neutral, terminal-style scan of which coding agents/models are available —
 * Codex, Claude Code, Grok, Apple FM — so the user (and the system) knows which
 * harnesses can be used. This is a pure PROJECTION over the discovery state the
 * shell already tracks (`harnessLanes` for the built-in codex/fable transports,
 * `providerLaneCapabilities` for admitted ACP peers). It invents no authority:
 * an agent is "available" only when its lane reports it can actually run a turn.
 */

export type BootSequenceStatus = "checking" | "available" | "unavailable"

export type BootSequenceAgentLine = Readonly<{
  /** Stable row id. */
  id: string
  /** Human-facing agent name. */
  label: string
  status: BootSequenceStatus
  /** Model id, provider label, or the reason it is unavailable. */
  detail: string | null
}>

const laneStatus = (lane: HarnessLaneAvailability): BootSequenceStatus =>
  lane.reason === CODEX_CHIP_REASON_VERIFYING
    ? "checking"
    : lane.available
      ? "available"
      : "unavailable"

/**
 * The curated agent scan, in a stable display order. Real availability comes
 * from the discovery state; Apple FM is listed as a known target the desktop
 * does not yet detect (mobile-only bridge today).
 */
export const projectBootSequenceAgents = (
  state: DesktopShellState,
): ReadonlyArray<BootSequenceAgentLine> => {
  const laneFor = (ref: string) =>
    state.providerLaneCapabilities.find((lane) => lane.laneRef === ref)
  const codexLane = state.harnessLanes.codex
  const fableLane = state.harnessLanes.fable
  const codexCap = laneFor("codex-local")
  const fableCap = laneFor("fable-local")
  const grokCap = laneFor("acp:grok-cli")

  const codexStatus = laneStatus(codexLane)
  const fableStatus = laneStatus(fableLane)
  const grokStatus: BootSequenceStatus =
    grokCap === undefined ? "unavailable" : grokCap.admission === "admitted" ? "available" : "unavailable"

  return [
    {
      id: "codex",
      label: "Codex",
      status: codexStatus,
      detail:
        codexStatus === "available"
          ? (codexCap?.models[0] ?? codexCap?.displayName ?? "ready")
          : codexStatus === "checking"
            ? "verifying accounts…"
            : (codexLane.reason ?? "not detected"),
    },
    {
      id: "claude-code",
      label: "Claude Code",
      status: fableStatus,
      detail:
        fableStatus === "available"
          ? (fableCap?.models[0] ?? fableCap?.displayName ?? "ready")
          : fableStatus === "checking"
            ? "verifying accounts…"
            : (fableLane.reason ?? "not detected"),
    },
    {
      id: "grok",
      label: "Grok",
      status: grokStatus,
      detail: grokStatus === "available" ? (grokCap?.models[0] ?? grokCap?.displayName ?? "ready") : "not connected",
    },
    {
      id: "apple-fm",
      label: "Apple FM",
      status: "unavailable",
      detail: "not available on desktop",
    },
  ]
}

/** Count of agents that reported ready. */
export const bootSequenceReadyCount = (
  agents: ReadonlyArray<BootSequenceAgentLine>,
): number => agents.filter((agent) => agent.status === "available").length

/** True while any agent is still probing (drives the "scanning…" summary). */
export const bootSequenceScanning = (
  agents: ReadonlyArray<BootSequenceAgentLine>,
): boolean => agents.some((agent) => agent.status === "checking")
