import type { NodeLaunchStatus, NodeStateMessage } from "./rpc.js"

// HUD H7 (#5504): the PURE projection from live desktop state to the small
// status/meters HUD overlay's element states. It maps the real model signals —
// the node launch lifecycle, the running-session count, and the read-only MDK
// wallet balance — onto the H2 three-effect HUD kit vocabulary
// (`createHudStatusLight` status + `createHudMeter` 0..1 value). Honesty is the
// whole point: when a signal is absent we emit an explicit `offline`/`unknown`
// state and a `0`/null value rather than fabricating a reading.
//
// This module is intentionally framework-free and DOM-free so it is unit-tested
// directly (model → element states), and so the desktop view, the three-effect
// scene, and the tests all agree on the same projection.
//
// Public-safe: counts + the launch-status word + a sat balance only — never any
// session/objective refs, tokens, addresses, or seeds (those never reach the
// webview anyway; this is a read projection over what already crossed).

// The HUD kit's status palette keys we drive a status light with. Mirrors
// `HudStatus` in `@openagentsinc/three-effect/core` (we keep our own copy of the
// keys we use so this pure module has no Three.js import).
export type HudStatusTone =
  | "success"
  | "info"
  | "warning"
  | "error"
  | "neutral"

// One status-light element: a tone (LED color) + an honest one-line label and
// whether it should pulse (we pulse the node light only while it is converging
// or in a recoverable failure, mirroring the kit's `pulseHz` attention cue).
export type HudStatusLightState = Readonly<{
  id: string
  label: string
  tone: HudStatusTone
  pulse: boolean
}>

// One meter element: an honest label, a 0..1 fill for the gauge, and the raw
// human-readable value text. `known: false` means the underlying signal has not
// been observed yet — the meter renders empty with an "unknown" value text and
// the caller must NOT present it as a real reading.
export type HudMeterState = Readonly<{
  id: string
  label: string
  value: number
  valueText: string
  known: boolean
}>

export type HudStatusProjection = Readonly<{
  // The node online/heartbeat LED.
  nodeLight: HudStatusLightState
  // Active (running) coding sessions, as a small gauge + count.
  sessionsMeter: HudMeterState
  // Read-only MDK wallet balance, as a gauge + sat count.
  balanceMeter: HudMeterState
}>

export type HudStatusInput = Readonly<{
  // The honest node launch lifecycle (model.nodeLaunchStatus). Null before the
  // first status arrives — treated as "connecting…" (an unknown/neutral light),
  // never as online. Typed as `string` too because the desktop model stores the
  // status as a plain string; an unrecognized value degrades to "connecting".
  nodeLaunchStatus: NodeLaunchStatus | string | null
  // The live node-state projection, or null when the node has not reported yet.
  node: NodeStateMessage | null
}>

const clamp01 = (value: number): number =>
  Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0))

// The reference fill cap for the sessions gauge. The runtime runs a small,
// bounded number of concurrent coding sessions; we normalize the running count
// against this so a busy node fills the gauge without ever overflowing it. The
// label always carries the exact integer count, so the gauge is decorative
// magnitude only — never a fabricated "out of N" claim.
export const HUD_SESSIONS_GAUGE_CAP = 6

// A soft reference balance (in sats) the balance gauge fills against. This is a
// presentational magnitude only; the label always shows the exact sat balance.
// Picked so an early earner's balance reads as a partial bar rather than pinning
// empty or full. Not a target, cap, or threshold.
export const HUD_BALANCE_GAUGE_REFERENCE_SATS = 100_000

// --- node online / heartbeat → status light --------------------------------
//
// launching/null → still converging (a pulsing neutral/info light, "connecting"
// or "starting"); online/adopted → success (steady green); failed/unavailable →
// error/warning (the honest "offline" state). We NEVER show success without an
// observed online/adopted status.
// Accepts `string | null` (the model stores the status as a plain string) so an
// unrecognized value degrades to the honest neutral "connecting" rather than an
// undefined element — we never invent an "online".
const nodeLightState = (
  status: NodeLaunchStatus | string | null,
): HudStatusLightState => {
  switch (status) {
    case "online":
      return { id: "node", label: "node online", tone: "success", pulse: false }
    case "adopted":
      return {
        id: "node",
        label: "node online (adopted)",
        tone: "success",
        pulse: false,
      }
    case "launching":
      return {
        id: "node",
        label: "node starting…",
        tone: "info",
        pulse: true,
      }
    case "failed":
      return { id: "node", label: "node offline", tone: "error", pulse: true }
    case "unavailable":
      return {
        id: "node",
        label: "node unavailable",
        tone: "warning",
        pulse: false,
      }
    default:
      // No status observed yet (null) or an unrecognized value: an honest
      // neutral "connecting", never online.
      return {
        id: "node",
        label: "connecting…",
        tone: "neutral",
        pulse: true,
      }
  }
}

// --- running sessions → meter -----------------------------------------------
//
// "Active sessions" = sessions whose state is `running`. When the node has not
// reported (`node === null`) the count is unknown (empty gauge, "unknown" text).
const sessionsMeterState = (node: NodeStateMessage | null): HudMeterState => {
  if (node === null) {
    return {
      id: "sessions",
      label: "active sessions",
      value: 0,
      valueText: "unknown",
      known: false,
    }
  }
  const running = node.sessions.filter(
    (session) => session.state === "running",
  ).length
  return {
    id: "sessions",
    label: "active sessions",
    value: clamp01(running / HUD_SESSIONS_GAUGE_CAP),
    valueText: `${running}`,
    known: true,
  }
}

// --- wallet balance → meter -------------------------------------------------
//
// The read-only MDK wallet balance (sats). Unknown when the node has not
// reported a wallet, or the wallet is unconfigured / reports a null balance —
// all of which render as an honest "unknown" empty gauge rather than "0 sats".
const balanceMeterState = (node: NodeStateMessage | null): HudMeterState => {
  const balance = node?.wallet?.balanceSats
  if (
    node === null ||
    node.wallet === null ||
    node.wallet === undefined ||
    balance === null ||
    balance === undefined ||
    !Number.isFinite(balance)
  ) {
    return {
      id: "balance",
      label: "wallet balance",
      value: 0,
      valueText: "unknown",
      known: false,
    }
  }
  const sats = Math.max(0, Math.floor(balance))
  return {
    id: "balance",
    label: "wallet balance",
    value: clamp01(sats / HUD_BALANCE_GAUGE_REFERENCE_SATS),
    valueText: `${sats.toLocaleString()} sats`,
    known: true,
  }
}

// The single pure projection consumed by the scene + the tests.
export const hudStatusProjection = (
  input: HudStatusInput,
): HudStatusProjection => ({
  nodeLight: nodeLightState(input.nodeLaunchStatus),
  sessionsMeter: sessionsMeterState(input.node),
  balanceMeter: balanceMeterState(input.node),
})
