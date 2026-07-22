import { CODEX_CHIP_REASON_VERIFYING } from "../codex-local-contract.ts"
import type { ProviderLaneComposerProjection } from "../provider-lane-capabilities.ts"
import type { DesktopShellState, HarnessLaneAvailability } from "./shell.ts"

/**
 * Boot Sequence (owner directive 2026-07-19). When the app opens it shows a
 * neutral, terminal-style scan of which coding agents/models are available —
 * Codex, Claude Code, and every admitted Agent Client Protocol peer (Grok,
 * Cursor, and any newly-wired peer) — plus the on-device Apple FM model, so the
 * user (and the system) knows which harnesses can be used. This is a pure
 * PROJECTION over the discovery state the shell already tracks (`harnessLanes`
 * for the built-in codex/claude transports, `providerLaneCapabilities` for the
 * ACP peer lanes, `appleFmBoot` for the native Apple FM bridge). It invents no
 * authority: an agent is "available" only when its lane reports it can actually
 * run a turn.
 *
 * The ACP roster is DATA-DRIVEN (#9183): rather than a hardcoded four lines,
 * the scan enumerates the `acp:`-prefixed lanes the shell projects, so an
 * admitted Cursor (or a future admitted OpenCode/Goose/Pi) appears with honest
 * status the moment its lane is published — no per-peer edit here. Grok remains
 * the flagship ACP target that always shows a scan line, so the panel reads
 * "checking", never a premature "not connected", while discovery is still in
 * flight (owner directive 2026-07-20). Apple FM is listed as the on-device
 * model it is, deliberately NOT conflated with the ACP harness set.
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

/** An admitted ACP peer is available; anything else the lane reports is not. */
const acpLaneStatus = (lane: ProviderLaneComposerProjection): BootSequenceStatus =>
  lane.admission === "admitted" ? "available" : "unavailable"

/**
 * The scan label for an ACP peer lane, derived from the lane's own display
 * name so a newly-wired peer needs no edit here. The trusted peer CLIs carry a
 * `… CLI` / `… Agent CLI` suffix (`Grok CLI`, `Cursor Agent CLI`); the scan uses
 * the short product name (`Grok`, `Cursor`). Anything without that suffix is
 * shown verbatim.
 */
const acpLaneLabel = (lane: ProviderLaneComposerProjection): string =>
  lane.displayName.replace(/ (?:Agent )?CLI$/, "").trim() || lane.displayName

/** One boot scan line for an admitted/known ACP peer lane. */
const acpLaneLine = (lane: ProviderLaneComposerProjection): BootSequenceAgentLine => {
  const status = acpLaneStatus(lane)
  return {
    id: lane.provider,
    label: acpLaneLabel(lane),
    status,
    detail:
      status === "available"
        ? (lane.models[0] ?? lane.displayName ?? "ready")
        : (lane.reason ?? "not connected"),
  }
}

/**
 * The agent scan, in a stable display order: the two built-in transports
 * (Codex, Claude Code), then the ACP peer lanes, then the on-device Apple FM
 * model. Real availability comes from the discovery state. The ACP roster is
 * enumerated from `providerLaneCapabilities`, so every admitted peer — Grok,
 * Cursor, and any newly-wired OpenCode/Goose/Pi — appears automatically. Grok
 * is the flagship target and always shows a line (reading "checking" while the
 * scan is still in flight), matching the owner-directed pre-discovery posture.
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
  // the flagship Grok lane that has not arrived yet must read as CHECKING — its
  // lane capability lands on the same background refresh, so "not seen yet,
  // still scanning" is "checking", NOT a premature "not connected". Only a
  // settled scan with no admitted lane is honestly "not connected".
  const discoveryOngoing =
    codexStatus === "checking" || claudeStatus === "checking" || appleFmStatus === "checking"
  const grokStatus: BootSequenceStatus =
    grokCap !== undefined
      ? acpLaneStatus(grokCap)
      : discoveryOngoing ? "checking" : "unavailable"

  // Every OTHER admitted/known ACP peer lane the shell projects, in the order
  // it arrives. This is the data-driven roster (#9183): an admitted Cursor —
  // and any future admitted OpenCode/Goose/Pi — shows up here with honest
  // status, with no per-peer branch in this projection.
  const additionalAcpLines = state.providerLaneCapabilities
    .filter((lane) => lane.laneRef.startsWith("acp:") && lane.provider !== "grok")
    .map(acpLaneLine)

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
    ...additionalAcpLines,
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

  // IDR-07: surface the REAL status-only wallet mode from the app-side Spark
  // adapter's public projection. `status_only` reads as "status-only"; a missing
  // mode (adapter did not open) falls back to the neutral "ready".
  const walletModeLabel = identity?.walletMode === "status_only" ? "status-only" : "ready"
  const walletDetail =
    status === "available" && identity?.walletFingerprint != null
      ? `${identity.walletFingerprint} · ${walletModeLabel}`
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
