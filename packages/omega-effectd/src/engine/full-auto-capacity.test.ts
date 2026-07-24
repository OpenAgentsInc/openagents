import { describe, expect, test } from "vite-plus/test"

import {
  admitConcurrentRun,
  projectFullAutoCapacityLedger,
  FULL_AUTO_MAX_CONCURRENT_RUNS,
  type FullAutoCapacityInputs,
} from "./full-auto-capacity.ts"
import { type FullAutoRotationReason } from "./full-auto-registry.ts"
import { type FullAutoRoutingLaneGate } from "./full-auto-routing.ts"

// The seven fleet lanes: the four native/ACP lanes plus the three host-run
// SDK-harness lanes made Full-Auto action lanes in #9187.
const LANES = [
  "codex-local",
  "claude-local",
  "acp:grok-cli",
  "acp:cursor-agent",
  "harness:opencode",
  "harness:pi",
  "harness:goose",
]

const allReadyGate: FullAutoRoutingLaneGate = (lane) =>
  LANES.includes(lane) ? { admitted: true, fullAuto: true } : null

const inputs = (
  overrides?: Partial<{
    active: Record<string, number>
    cooling: Record<string, FullAutoRotationReason>
    gate: FullAutoRoutingLaneGate
  }>,
): FullAutoCapacityInputs => ({
  laneGate: overrides?.gate ?? allReadyGate,
  activeRunsByLane: (lane) => overrides?.active?.[lane] ?? 0,
  coolingReasonByLane: (lane) => overrides?.cooling?.[lane] ?? null,
})

const stateOf = (ledger: ReadonlyArray<{ lane: string; state: string }>, lane: string) =>
  ledger.find((e) => e.lane === lane)?.state

describe("projectFullAutoCapacityLedger", () => {
  test("a ready, idle lane is available", () => {
    const ledger = projectFullAutoCapacityLedger(inputs())
    expect(ledger.every((e) => e.state === "available")).toBe(true)
    expect(ledger.map((e) => e.lane)).toEqual([...LANES].sort())
  })

  test("a lane with an active run is busy", () => {
    const ledger = projectFullAutoCapacityLedger(inputs({ active: { "codex-local": 1 } }))
    expect(stateOf(ledger, "codex-local")).toBe("busy")
  })

  test("account exhaustion reads exhausted; rate-limit and error read cooling", () => {
    const ledger = projectFullAutoCapacityLedger(
      inputs({
        cooling: {
          "codex-local": "account_exhausted",
          "claude-local": "rate_limited",
          "acp:grok-cli": "provider_error",
        },
      }),
    )
    expect(stateOf(ledger, "codex-local")).toBe("exhausted")
    expect(stateOf(ledger, "claude-local")).toBe("cooling")
    expect(stateOf(ledger, "acp:grok-cli")).toBe("cooling")
    expect(stateOf(ledger, "acp:cursor-agent")).toBe("available")
  })

  test("a not-ready lane is unavailable regardless of activity", () => {
    const ledger = projectFullAutoCapacityLedger(
      inputs({ gate: (lane) => (lane === "codex-local" ? { admitted: true, fullAuto: true } : null) }),
    )
    expect(stateOf(ledger, "codex-local")).toBe("available")
    expect(stateOf(ledger, "claude-local")).toBe("unavailable")
  })

  test("exhausted beats busy — an exhausted account is not merely busy", () => {
    const ledger = projectFullAutoCapacityLedger(
      inputs({ active: { "codex-local": 1 }, cooling: { "codex-local": "account_exhausted" } }),
    )
    expect(stateOf(ledger, "codex-local")).toBe("exhausted")
  })
})

describe("admitConcurrentRun — own-capacity-only bounded concurrency", () => {
  test("admits onto an available lane while under the cap", () => {
    const ledger = projectFullAutoCapacityLedger(inputs())
    const admission = admitConcurrentRun(ledger, 0)
    expect(admission.ok).toBe(true)
    if (admission.ok) expect(LANES).toContain(admission.lane)
  })

  test("refuses at the total active-run cap", () => {
    const ledger = projectFullAutoCapacityLedger(inputs())
    const admission = admitConcurrentRun(ledger, FULL_AUTO_MAX_CONCURRENT_RUNS)
    expect(admission).toEqual({ ok: false, reason: "active_run_limit_reached" })
  })

  test("never admits onto a busy, cooling, or exhausted lane — spreads across distinct ready lanes", () => {
    // codex busy, claude cooling, grok exhausted, all three harness lanes
    // occupied -> only cursor is available.
    const ledger = projectFullAutoCapacityLedger(
      inputs({
        active: { "codex-local": 1, "harness:opencode": 1 },
        cooling: {
          "claude-local": "rate_limited",
          "acp:grok-cli": "account_exhausted",
          "harness:pi": "provider_error",
          "harness:goose": "rate_limited",
        },
      }),
    )
    const admission = admitConcurrentRun(ledger, 1)
    expect(admission).toEqual({ ok: true, lane: "acp:cursor-agent" })
  })

  test("refuses when no lane is available even under the cap (own-capacity-only)", () => {
    const ledger = projectFullAutoCapacityLedger(
      inputs({
        active: {
          "codex-local": 1, "claude-local": 1, "acp:grok-cli": 1, "acp:cursor-agent": 1,
          // #9187: the harness fleet lanes are action lanes too, so they must
          // also be occupied for "no lane available" to hold.
          "harness:opencode": 1, "harness:pi": 1, "harness:goose": 1,
        },
      }),
    )
    // Under the cap (7 < 8) but every lane is busy — no oversubscription.
    const admission = admitConcurrentRun(ledger, 7)
    expect(admission).toEqual({ ok: false, reason: "no_available_lane" })
  })

  test("the module never raises the existing total cap", () => {
    expect(FULL_AUTO_MAX_CONCURRENT_RUNS).toBe(8)
  })
})
