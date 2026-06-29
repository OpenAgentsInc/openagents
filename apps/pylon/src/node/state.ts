// Pure, serializable domain state for the Pylon node (issue #4736).
//
// Nothing in this module may import Effect or any service module. These types
// and transitions are the seam between node services and external consumers:
// services compute next states here and the runtime publishes them. Every
// event type stays JSON-serializable so the same union is the control-server
// snapshot / SSE wire format (issue #4740).

export type PylonLogLevel = "error" | "info" | "verbose"

export type PylonLogEntry = {
  at: string
  level: PylonLogLevel
  message: string
  // Transient entries are session banners (ready line, identity, attach
  // notices): shown live but never persisted, so restored scrollback does
  // not accumulate one copy per launch.
  transient?: boolean
}

// Legacy cleanup: feed logs written before the transient flag existed are
// full of per-launch banners; drop them when restoring scrollback.
export function isSessionBannerMessage(message: string): boolean {
  return (
    message.startsWith("Pylon v0.3 ready.") ||
    message.startsWith("Pylon v0.3 dashboard active") ||
    message.startsWith("Pylon node-core running headless.") ||
    message.startsWith("[Identity] Pylon Nostr npub:") ||
    message.startsWith("Attaching to Pylon node") ||
    message.startsWith("Attached. Restored ") ||
    message.startsWith("Pylon v0.3 dashboard smoke complete.") ||
    message.startsWith("[Control] Attach API unavailable")
  )
}

export const maxLogEntries = 1000

export type WalletPaneState = {
  daemonOnline: boolean
  balanceSats: number | null
  readiness: string
}

export const initialWalletPaneState: WalletPaneState = {
  daemonOnline: false,
  balanceSats: null,
  readiness: "daemon-offline",
}

export type WalletStatusInput = {
  daemonOnline: boolean
  balanceSats: number | null
  readiness: string
}

export function walletPaneStateFromStatus(status: WalletStatusInput | null): WalletPaneState {
  if (!status) return initialWalletPaneState
  return {
    daemonOnline: status.daemonOnline,
    balanceSats: status.balanceSats,
    readiness: status.readiness,
  }
}

export function isWalletOnline(state: WalletPaneState): boolean {
  return state.daemonOnline && state.balanceSats !== null
}

export function walletTransitionMessage(
  previous: WalletPaneState,
  next: WalletPaneState,
): string | null {
  const wasOnline = isWalletOnline(previous)
  const nowOnline = isWalletOnline(next)
  if (!wasOnline && nowOnline) {
    return `[Wallet] Primary agent wallet connected. Readiness: ${next.readiness}.`
  }
  if (wasOnline && !nowOnline) {
    return "[Wallet] Primary agent wallet balance is unavailable. Operating in OFFLINE mode."
  }
  return null
}

export type TelemetryPaneState = {
  state: "IDLE" | "INVENTORY FRESH" | "INVENTORY BLOCKED" | "UNAVAILABLE"
  model: string
  vram: string
  psionic: string
}

export const initialTelemetryPaneState: TelemetryPaneState = {
  state: "IDLE",
  model: "-",
  vram: "-",
  psionic: "unknown",
}

export type TelemetryInventoryInput = {
  eligibleInventoryCount: number
  accelerator: { vramGb: number | null }
  backendHealth: ReadonlyArray<{ state: string; modelRef: string | null }>
}

export function telemetryPaneStateFromInventory(
  inventory: TelemetryInventoryInput | null,
  psionicPhase: string,
): TelemetryPaneState {
  if (!inventory) {
    return { state: "UNAVAILABLE", model: "inventory unavailable", vram: "--", psionic: psionicPhase }
  }
  const readyBackends = inventory.backendHealth.filter(
    (backend) => backend.state === "ready" || backend.state === "configured",
  )
  return {
    state: inventory.eligibleInventoryCount > 0 ? "INVENTORY FRESH" : "INVENTORY BLOCKED",
    model: readyBackends[0]?.modelRef ?? "None",
    vram: inventory.accelerator.vramGb === null ? "--" : `${inventory.accelerator.vramGb.toFixed(1)} GB`,
    psionic: psionicPhase,
  }
}

export type OperatorPaneState = {
  text: string
}

export const initialOperatorPaneState: OperatorPaneState = {
  text: " Operate: loading\n Inspect: loading\n Recovery: loading",
}

export type PylonEvent =
  | { type: "log"; at: string; level: PylonLogLevel; message: string }
  | { type: "wallet"; at: string; wallet: WalletPaneState }
  | { type: "telemetry"; at: string; telemetry: TelemetryPaneState }
  | { type: "operator"; at: string; text: string }

export function appendLogEntry(
  entries: ReadonlyArray<PylonLogEntry>,
  entry: PylonLogEntry,
  max: number = maxLogEntries,
): ReadonlyArray<PylonLogEntry> {
  const next = [...entries, entry]
  return next.length > max ? next.slice(next.length - max) : next
}

export function isLogEntryVisible(
  entry: Pick<PylonLogEntry, "level">,
  verbose: boolean,
): boolean {
  if (entry.level === "verbose") return verbose
  return true
}

// For service callbacks whose message content we don't control (e.g. the
// NIP-90 provider loop): failures stay visible, routine chatter is verbose.
export function classifyServiceLogLevel(message: string): PylonLogLevel {
  return /error|failed|crash/i.test(message) ? "error" : "verbose"
}

export function formatLogTimestamp(at: string): string {
  const date = new Date(at)
  if (Number.isNaN(date.getTime())) return "--:--:--"
  const pad = (value: number) => String(value).padStart(2, "0")
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}
