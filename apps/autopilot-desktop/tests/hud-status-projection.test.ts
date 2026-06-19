import { describe, expect, test } from "bun:test"

import {
  HUD_BALANCE_GAUGE_REFERENCE_SATS,
  HUD_SESSIONS_GAUGE_CAP,
  hudStatusProjection,
} from "../src/shared/hud-status-projection"
import type { NodeStateMessage, WalletStatusRow } from "../src/shared/rpc"
import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"

// HUD H7 (#5504): the pure model → HUD-element-state projection that drives the
// live status/meters overlay. These tests pin the honest mapping (node light
// tone, sessions/balance gauge values + raw text) AND the explicit
// offline/unknown behavior when a signal is absent — the whole point of the
// projection is that it never fabricates a reading.

const session = (
  state: SessionSummary["state"],
  ref: string,
): SessionSummary => ({
  sessionRef: ref,
  adapter: "codex",
  state,
  accountRefHash: null,
  updatedAt: "2026-06-19T12:00:00.000Z",
})

const wallet = (balanceSats: number | null): WalletStatusRow => ({
  configured: true,
  daemonOnline: true,
  balanceSats,
  receiveReady: true,
  sendReady: false,
  readiness: "ready",
})

const node = (
  partial: Partial<NodeStateMessage> = {},
): NodeStateMessage => ({
  ok: true,
  schema: "control.v1",
  sessions: [],
  ...partial,
})

describe("HUD H7 node status light", () => {
  test("online → steady success light", () => {
    const { nodeLight } = hudStatusProjection({
      nodeLaunchStatus: "online",
      node: node(),
    })
    expect(nodeLight).toEqual({
      id: "node",
      label: "node online",
      tone: "success",
      pulse: false,
    })
  })

  test("adopted → success too (the node was already up)", () => {
    const { nodeLight } = hudStatusProjection({
      nodeLaunchStatus: "adopted",
      node: node(),
    })
    expect(nodeLight.tone).toBe("success")
    expect(nodeLight.pulse).toBe(false)
  })

  test("launching → a pulsing info light, never success", () => {
    const { nodeLight } = hudStatusProjection({
      nodeLaunchStatus: "launching",
      node: node(),
    })
    expect(nodeLight.tone).toBe("info")
    expect(nodeLight.pulse).toBe(true)
    expect(nodeLight.label).toContain("starting")
  })

  test("failed → an honest pulsing error 'offline' light", () => {
    const { nodeLight } = hudStatusProjection({
      nodeLaunchStatus: "failed",
      node: node(),
    })
    expect(nodeLight.tone).toBe("error")
    expect(nodeLight.label).toContain("offline")
  })

  test("unavailable → a warning light", () => {
    const { nodeLight } = hudStatusProjection({
      nodeLaunchStatus: "unavailable",
      node: node(),
    })
    expect(nodeLight.tone).toBe("warning")
  })

  test("null (no status yet) → neutral 'connecting…', NOT online", () => {
    const { nodeLight } = hudStatusProjection({
      nodeLaunchStatus: null,
      node: null,
    })
    expect(nodeLight.tone).toBe("neutral")
    expect(nodeLight.label).toBe("connecting…")
  })

  test("an unrecognized status string degrades to neutral 'connecting…'", () => {
    const { nodeLight } = hudStatusProjection({
      nodeLaunchStatus: "some-future-state",
      node: node(),
    })
    expect(nodeLight.tone).toBe("neutral")
    expect(nodeLight.label).toBe("connecting…")
  })
})

describe("HUD H7 active-sessions meter", () => {
  test("counts only RUNNING sessions (not completed/queued/failed/cancelled)", () => {
    const { sessionsMeter } = hudStatusProjection({
      nodeLaunchStatus: "online",
      node: node({
        sessions: [
          session("running", "a"),
          session("running", "b"),
          session("completed", "c"),
          session("queued", "d"),
          session("failed", "e"),
          session("cancelled", "f"),
        ],
      }),
    })
    expect(sessionsMeter.known).toBe(true)
    expect(sessionsMeter.valueText).toBe("2")
    expect(sessionsMeter.value).toBeCloseTo(2 / HUD_SESSIONS_GAUGE_CAP, 6)
  })

  test("zero running sessions is a KNOWN reading of 0 (not 'unknown')", () => {
    const { sessionsMeter } = hudStatusProjection({
      nodeLaunchStatus: "online",
      node: node({ sessions: [session("completed", "c")] }),
    })
    expect(sessionsMeter.known).toBe(true)
    expect(sessionsMeter.value).toBe(0)
    expect(sessionsMeter.valueText).toBe("0")
  })

  test("the gauge fill is clamped to 1 when running exceeds the cap", () => {
    const sessions = Array.from({ length: HUD_SESSIONS_GAUGE_CAP + 3 }, (_, i) =>
      session("running", `s${i}`),
    )
    const { sessionsMeter } = hudStatusProjection({
      nodeLaunchStatus: "online",
      node: node({ sessions }),
    })
    expect(sessionsMeter.value).toBe(1)
    expect(sessionsMeter.valueText).toBe(`${HUD_SESSIONS_GAUGE_CAP + 3}`)
  })

  test("no node reported → an honest 'unknown' meter (empty gauge)", () => {
    const { sessionsMeter } = hudStatusProjection({
      nodeLaunchStatus: null,
      node: null,
    })
    expect(sessionsMeter.known).toBe(false)
    expect(sessionsMeter.value).toBe(0)
    expect(sessionsMeter.valueText).toBe("unknown")
  })
})

describe("HUD H7 wallet-balance meter", () => {
  test("a known balance fills the gauge against the reference and shows sats", () => {
    const sats = HUD_BALANCE_GAUGE_REFERENCE_SATS / 2
    const { balanceMeter } = hudStatusProjection({
      nodeLaunchStatus: "online",
      node: node({ wallet: wallet(sats) }),
    })
    expect(balanceMeter.known).toBe(true)
    expect(balanceMeter.value).toBeCloseTo(0.5, 6)
    expect(balanceMeter.valueText).toBe(`${sats.toLocaleString()} sats`)
  })

  test("a zero balance is a KNOWN reading of 0 sats", () => {
    const { balanceMeter } = hudStatusProjection({
      nodeLaunchStatus: "online",
      node: node({ wallet: wallet(0) }),
    })
    expect(balanceMeter.known).toBe(true)
    expect(balanceMeter.value).toBe(0)
    expect(balanceMeter.valueText).toBe("0 sats")
  })

  test("the gauge clamps to 1 above the reference balance", () => {
    const { balanceMeter } = hudStatusProjection({
      nodeLaunchStatus: "online",
      node: node({ wallet: wallet(HUD_BALANCE_GAUGE_REFERENCE_SATS * 4) }),
    })
    expect(balanceMeter.value).toBe(1)
  })

  test("a null wallet balance → 'unknown' (not '0 sats')", () => {
    const { balanceMeter } = hudStatusProjection({
      nodeLaunchStatus: "online",
      node: node({ wallet: wallet(null) }),
    })
    expect(balanceMeter.known).toBe(false)
    expect(balanceMeter.valueText).toBe("unknown")
  })

  test("no wallet reported → 'unknown'", () => {
    const { balanceMeter } = hudStatusProjection({
      nodeLaunchStatus: "online",
      node: node({ wallet: null }),
    })
    expect(balanceMeter.known).toBe(false)
    expect(balanceMeter.valueText).toBe("unknown")
  })

  test("no node reported → 'unknown'", () => {
    const { balanceMeter } = hudStatusProjection({
      nodeLaunchStatus: null,
      node: null,
    })
    expect(balanceMeter.known).toBe(false)
    expect(balanceMeter.valueText).toBe("unknown")
  })
})

describe("HUD H7 honest-empty projection (cold start)", () => {
  test("everything is offline/unknown before any state arrives", () => {
    const projection = hudStatusProjection({
      nodeLaunchStatus: null,
      node: null,
    })
    expect(projection.nodeLight.tone).toBe("neutral")
    expect(projection.sessionsMeter.known).toBe(false)
    expect(projection.balanceMeter.known).toBe(false)
  })
})
