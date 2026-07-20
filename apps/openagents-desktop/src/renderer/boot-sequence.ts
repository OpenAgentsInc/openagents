import { CODEX_CHIP_REASON_VERIFYING } from "../codex-local-contract.ts"
import type { DesktopShellState, HarnessLaneAvailability } from "./shell.ts"

/**
 * Boot Sequence (owner directive 2026-07-19). When the app opens it shows a
 * neutral, terminal-style scan of which coding agents/models are available —
 * Codex, Claude Code, Grok, Apple FM — so the user (and the system) knows which
 * harnesses can be used. This is a pure PROJECTION over the discovery state the
 * shell already tracks (`harnessLanes` for the built-in codex/claude transports,
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
  /** Bounded reply from the on-open test inference (Apple FM only, when ready). */
  testInference?: string | null
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
  const claudeLane = state.harnessLanes.claude
  const codexCap = laneFor("codex-local")
  const claudeCap = laneFor("claude-local")
  const grokCap = laneFor("acp:grok-cli")

  const codexStatus = laneStatus(codexLane)
  const claudeStatus = laneStatus(claudeLane)
  const appleFmStatus: BootSequenceStatus = state.appleFmBoot?.status ?? "checking"
  // Owner directive 2026-07-20: while the slower discovery probes are still
  // running (codex/claude account verification, the Apple FM on-device probe),
  // an ACP peer whose capability has not arrived yet must read as CHECKING — its
  // lane capability lands on the same background refresh, so "not seen yet,
  // still scanning" is "checking", NOT a premature "not connected". Only a
  // settled scan with no admitted lane is honestly "not connected".
  const discoveryOngoing =
    codexStatus === "checking" || claudeStatus === "checking" || appleFmStatus === "checking"
  const grokStatus: BootSequenceStatus =
    grokCap !== undefined
      ? grokCap.admission === "admitted" ? "available" : "unavailable"
      : discoveryOngoing ? "checking" : "unavailable"

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
      status: claudeStatus,
      detail:
        claudeStatus === "available"
          ? (claudeCap?.models[0] ?? claudeCap?.displayName ?? "ready")
          : claudeStatus === "checking"
            ? "verifying accounts…"
            : (claudeLane.reason ?? "not detected"),
    },
    {
      id: "grok",
      label: "Grok",
      status: grokStatus,
      detail:
        grokStatus === "available"
          ? (grokCap?.models[0] ?? grokCap?.displayName ?? "ready")
          : grokStatus === "checking"
            ? "checking…"
            : "not connected",
    },
    {
      id: "apple-fm",
      label: "Apple FM",
      status: state.appleFmBoot?.status ?? "checking",
      detail:
        state.appleFmBoot === undefined
          ? "checking on-device model…"
          : state.appleFmBoot.detail,
      testInference: state.appleFmBoot?.testInference ?? null,
    },
  ]
}

/**
 * A Boot Sequence identity/wallet line (IDR-BS #9103). Same terminal-style shape
 * as an agent line, but sourced from the main-owned sovereign identity host —
 * public identifiers only.
 */
export type BootSequenceIdentityLine = Readonly<{
  id: string
  label: string
  status: BootSequenceStatus
  detail: string | null
}>

/** Truncate a long public identifier for display, e.g. `npub1abcd…wxyz`. */
const truncateIdentifier = (value: string): string =>
  value.length <= 20 ? value : `${value.slice(0, 12)}…${value.slice(-6)}`

/**
 * The sovereign identity scan lines for the Boot Sequence: the Nostr `npub` and
 * the Spark wallet public fingerprint. PUBLIC data only — never the mnemonic,
 * `nsec`, private key, or seed. `undefined` identity state reads as "checking".
 */
export const projectBootSequenceIdentity = (
  state: DesktopShellState,
): ReadonlyArray<BootSequenceIdentityLine> => {
  const identity = state.identityBoot
  const status: BootSequenceStatus = identity?.status ?? "checking"
  // Only a freshly minted identity is tagged (`new`). An existing identity
  // carries no adjective — the owner does not want "rehydrated" in the UI.
  const sourceLabel = identity?.source === "created" ? "new" : null

  const identityDetail =
    status === "available" && identity?.npub != null
      ? sourceLabel === null
        ? truncateIdentifier(identity.npub)
        : `${truncateIdentifier(identity.npub)} · ${sourceLabel}`
      : status === "checking"
        ? "deriving identity…"
        : "not detected"

  const walletDetail =
    status === "available" && identity?.walletFingerprint != null
      ? `${identity.walletFingerprint} · ready`
      : status === "checking"
        ? "deriving wallet…"
        : "not detected"

  return [
    { id: "identity", label: "Identity", status, detail: identityDetail },
    { id: "wallet", label: "Wallet", status, detail: walletDetail },
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
