import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type { KhalaCodeDesktopFleetStatus } from "../src/shared/rpc"
import { mountFleetPanel } from "../src/ui/fleet-status"
import {
  assertCockpitVisualGeometry,
  COCKPIT_VISUAL_SMOKE_HARNESS,
  cockpitVisualSmokeViewports,
} from "../scripts/cockpit-visual-smoke"

describe("T5.6 cockpit visual smoke", () => {
  test("registers the fixture-only desktop and mobile cockpit plan", async () => {
    const packageJson = await Bun.file(new URL("../package.json", import.meta.url)).text()

    expect(COCKPIT_VISUAL_SMOKE_HARNESS).toBe("khala_code_t5_6_cockpit_visual_smoke")
    expect(cockpitVisualSmokeViewports()).toEqual([
      { name: "desktop", width: 1280, height: 800 },
      { name: "mobile", width: 390, height: 844 },
    ])
    expect(packageJson).toContain('"smoke:cockpit-visual"')
    expect(packageJson).toContain("scripts/cockpit-visual-smoke.ts")
  })

  test("accepts the 18 worker / 3 account geometry and rejects overlap", () => {
    assertCockpitVisualGeometry({
      accountCards: [
        { x: 24, y: 320, width: 720, height: 80 },
        { x: 24, y: 412, width: 720, height: 80 },
        { x: 24, y: 504, width: 720, height: 80 },
      ],
      fleetCounts: { x: 12, y: 72, width: 52, height: 20 },
      fleetPanel: { x: 0, y: 0, width: 780, height: 2_400 },
      gauges: [
        { x: 24, y: 90, width: 230, height: 90 },
        { x: 270, y: 90, width: 230, height: 90 },
        { x: 516, y: 90, width: 230, height: 90 },
      ],
      runHeader: { x: 24, y: 190, width: 720, height: 92 },
      viewport: { x: 0, y: 0, width: 1280, height: 2_500 },
      workerCards: Array.from({ length: 18 }, (_, index) => ({
        x: 24,
        y: 640 + index * 92,
        width: 720,
        height: 80,
      })),
    })

    expect(() =>
      assertCockpitVisualGeometry({
        accountCards: [
          { x: 24, y: 320, width: 720, height: 80 },
          { x: 24, y: 360, width: 720, height: 80 },
          { x: 24, y: 504, width: 720, height: 80 },
        ],
        fleetCounts: { x: 12, y: 72, width: 52, height: 20 },
        fleetPanel: { x: 0, y: 0, width: 780, height: 2_400 },
        gauges: [
          { x: 24, y: 90, width: 230, height: 90 },
          { x: 270, y: 90, width: 230, height: 90 },
          { x: 516, y: 90, width: 230, height: 90 },
        ],
        runHeader: { x: 24, y: 190, width: 720, height: 92 },
        viewport: { x: 0, y: 0, width: 1280, height: 2_500 },
        workerCards: Array.from({ length: 18 }, (_, index) => ({
          x: 24,
          y: 640 + index * 92,
          width: 720,
          height: 80,
        })),
      }),
    ).toThrow("account cards overlap")
  })

  test("rate-limit countdown repaints from an injected clock without sleeping", async () => {
    const window = new Window()
    const previousWindow = globalThis.window
    const previousDocument = globalThis.document
    Object.defineProperty(globalThis, "window", { configurable: true, value: window })
    Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
    window.matchMedia = (() => ({ matches: true })) as unknown as typeof window.matchMedia

    let now = new Date("2026-07-01T18:00:00.000Z")
    const root = document.createElement("div")
    const panel = mountFleetPanel(root, {
      connectAccount: async () => ({ ok: false, accountRef: "codex-a", error: "not used", output: "", userCode: null, verificationUrl: null }),
      consumeResetCredit: async () => ({ ok: true }),
      delegateRun: async () => {
        throw new Error("delegate runner should not be called")
      },
      fetch: async () => countdownStatus(),
      fleetRunControl: async () => {
        throw new Error("fleet run control should not be called")
      },
      fleetRunList: async () => ({ ok: true, runs: [] }),
      fleetRunStart: async () => {
        throw new Error("fleet run start should not be called")
      },
      fleetWorkerControl: async () => {
        throw new Error("fleet worker control should not be called")
      },
      loadGymDemoProof: () => {
        throw new Error("gym proof should not be called")
      },
      now: () => now,
      openExternal: async () => false,
      removeAccount: async () => ({ ok: true }),
      setAccountPaused: async () => ({ ok: true }),
      startDelegationOptimization: async () => {
        throw new Error("optimization should not be called")
      },
    })

    try {
      await panel.refresh()
      expect(root.textContent).toContain("resets in 1h 30m")

      now = new Date("2026-07-01T19:15:00.000Z")
      await panel.refresh()
      expect(root.textContent).toContain("resets in 15m")
      expect(root.textContent).not.toContain("resets in 1h 30m")
    } finally {
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
    }
  })
})

const countdownStatus = (): KhalaCodeDesktopFleetStatus => ({
  accounts: [{
    accountKey: null,
    accountRef: "codex-a",
    capacity: { available: 1, busy: 0, queued: 0, ready: 1 },
    email: null,
    provider: "codex",
    quotaState: "available",
    rateLimits: {
      error: null,
      provider: "codex",
      session: {
        remainingPercent: 70,
        resetDescription: null,
        resetsAtIso: "2026-07-01T19:30:00.000Z",
        usedPercent: 30,
        windowMinutes: 300,
      },
      status: "ok",
      updatedAtIso: "2026-07-01T18:00:00.000Z",
      weekly: null,
    },
    readiness: "ready",
  }],
  activeAssignments: [],
  availableCodexAssignments: 1,
  maxCodexAssignments: 1,
  observedAt: "2026-07-01T18:00:00.000Z",
  ok: true,
  processes: [],
  pylon: {
    message: "online",
    pylonRef: "pylon.local.countdown",
    status: "online",
  },
  tokenRate: {
    activeAdjustedTokensPerMinute: null,
    completedStatus: "not_measured",
    completedTokenRows: null,
    completedTokensPerMinute: null,
    inFlightTokens: null,
    inFlightTokensPerMinute: null,
    source: "unavailable",
    unavailableReason: null,
  },
})
