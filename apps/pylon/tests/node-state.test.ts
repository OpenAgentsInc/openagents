import { describe, expect, test } from "bun:test"
import {
  appendLogEntry,
  classifyServiceLogLevel,
  formatLogTimestamp,
  initialWalletPaneState,
  isLogEntryVisible,
  telemetryPaneStateFromInventory,
  walletPaneStateFromStatus,
  walletTransitionMessage,
  type PylonEvent,
  type PylonLogEntry,
} from "../src/node/state"

describe("wallet pane state", () => {
  test("null status maps to the initial offline state", () => {
    expect(walletPaneStateFromStatus(null)).toEqual(initialWalletPaneState)
  })

  test("online status with balance maps to online pane state", () => {
    const state = walletPaneStateFromStatus({
      daemonOnline: true,
      balanceSats: 1234,
      readiness: "receive-ready",
    })
    expect(state).toEqual({ daemonOnline: true, balanceSats: 1234, readiness: "receive-ready" })
  })

  test("daemon online without a known balance is not treated as online", () => {
    const previous = walletPaneStateFromStatus({
      daemonOnline: true,
      balanceSats: null,
      readiness: "balance-unknown",
    })
    expect(walletTransitionMessage(initialWalletPaneState, previous)).toBeNull()
  })

  test("offline-to-online transition produces a connect message once", () => {
    const next = walletPaneStateFromStatus({
      daemonOnline: true,
      balanceSats: 50,
      readiness: "receive-ready",
    })
    const message = walletTransitionMessage(initialWalletPaneState, next)
    expect(message).toContain("connected")
    expect(message).toContain("receive-ready")
    // Steady state afterwards: no repeated message.
    expect(walletTransitionMessage(next, next)).toBeNull()
  })

  test("online-to-offline transition produces the offline message once", () => {
    const online = walletPaneStateFromStatus({
      daemonOnline: true,
      balanceSats: 50,
      readiness: "receive-ready",
    })
    const message = walletTransitionMessage(online, initialWalletPaneState)
    expect(message).toContain("OFFLINE")
    expect(walletTransitionMessage(initialWalletPaneState, initialWalletPaneState)).toBeNull()
  })
})

describe("telemetry pane state", () => {
  test("missing inventory maps to UNAVAILABLE", () => {
    const state = telemetryPaneStateFromInventory(null, "unknown")
    expect(state.state).toBe("UNAVAILABLE")
    expect(state.model).toBe("inventory unavailable")
    expect(state.vram).toBe("--")
  })

  test("eligible inventory with a ready backend maps to INVENTORY FRESH", () => {
    const state = telemetryPaneStateFromInventory(
      {
        eligibleInventoryCount: 2,
        accelerator: { vramGb: 24 },
        backendHealth: [
          { state: "ready", modelRef: "model.qwen3" },
          { state: "blocked", modelRef: null },
        ],
      },
      "configured",
    )
    expect(state).toEqual({
      state: "INVENTORY FRESH",
      model: "model.qwen3",
      vram: "24.0 GB",
      psionic: "configured",
    })
  })

  test("zero eligible inventory and no ready backend maps to BLOCKED with defaults", () => {
    const state = telemetryPaneStateFromInventory(
      {
        eligibleInventoryCount: 0,
        accelerator: { vramGb: null },
        backendHealth: [{ state: "blocked", modelRef: "model.x" }],
      },
      "absent",
    )
    expect(state).toEqual({
      state: "INVENTORY BLOCKED",
      model: "None",
      vram: "--",
      psionic: "absent",
    })
  })
})

describe("log feed", () => {
  const entry = (n: number): PylonLogEntry => ({
    at: new Date(n).toISOString(),
    level: "info",
    message: `entry ${n}`,
  })

  test("appendLogEntry keeps a bounded ring buffer", () => {
    let entries: ReadonlyArray<PylonLogEntry> = []
    for (let i = 0; i < 7; i += 1) {
      entries = appendLogEntry(entries, entry(i), 5)
    }
    expect(entries).toHaveLength(5)
    expect(entries[0]?.message).toBe("entry 2")
    expect(entries[4]?.message).toBe("entry 6")
  })

  test("verbose entries are hidden by default and shown in verbose mode", () => {
    expect(isLogEntryVisible({ level: "verbose" }, false)).toBe(false)
    expect(isLogEntryVisible({ level: "verbose" }, true)).toBe(true)
    expect(isLogEntryVisible({ level: "info" }, false)).toBe(true)
    expect(isLogEntryVisible({ level: "error" }, false)).toBe(true)
  })

  test("classifyServiceLogLevel keeps failures visible and chatter quiet", () => {
    expect(classifyServiceLogLevel("[NIP-90] subscription opened on relay X")).toBe("verbose")
    expect(classifyServiceLogLevel("[NIP-90] job failed: timeout")).toBe("error")
    expect(classifyServiceLogLevel("[NIP-90] relay error: connection refused")).toBe("error")
  })

  test("formatLogTimestamp renders HH:MM:SS and tolerates garbage", () => {
    expect(formatLogTimestamp(new Date(2026, 5, 10, 9, 5, 3).toISOString())).toBe("09:05:03")
    expect(formatLogTimestamp("not-a-date")).toBe("--:--:--")
  })
})

describe("PylonEvent serializability", () => {
  test("every event variant survives a JSON round trip", () => {
    const at = new Date(0).toISOString()
    const events: PylonEvent[] = [
      { type: "log", at, level: "info", message: "hello" },
      { type: "wallet", at, wallet: { daemonOnline: true, balanceSats: 7, readiness: "receive-ready" } },
      {
        type: "telemetry",
        at,
        telemetry: { state: "INVENTORY FRESH", model: "m", vram: "1.0 GB", psionic: "configured" },
      },
      { type: "operator", at, text: "Operate: automated" },
    ]
    for (const event of events) {
      expect(JSON.parse(JSON.stringify(event))).toEqual(event)
    }
  })
})
