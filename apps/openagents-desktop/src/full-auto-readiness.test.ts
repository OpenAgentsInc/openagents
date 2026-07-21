import { describe, expect, test } from "vite-plus/test"

import { FULL_AUTO_LANE_POLICIES } from "./full-auto-lane.ts"
import {
  decodeFullAutoReadinessSnapshot,
  projectFullAutoLaneScan,
  projectFullAutoReadinessSnapshot,
  FULL_AUTO_ADVISORY_LANE,
} from "./full-auto-readiness.ts"
import { validateFullAutoRoutingPolicy, type FullAutoRoutingLaneGate } from "./full-auto-routing.ts"

const AT = "2026-07-20T00:00:00Z"

/** Build a gate from a map of lane -> admission (or absent = unknown lane). */
const gateFrom = (
  admissions: Record<string, { admitted: boolean; fullAuto: boolean }>,
): FullAutoRoutingLaneGate => (laneRef) => admissions[laneRef] ?? null

const allReadyGate = gateFrom({
  "codex-local": { admitted: true, fullAuto: true },
  "claude-local": { admitted: true, fullAuto: true },
  "acp:grok-cli": { admitted: true, fullAuto: true },
  "acp:cursor-agent": { admitted: true, fullAuto: true },
})

describe("projectFullAutoReadinessSnapshot", () => {
  test("a policy whose candidates are all admitted reads all available", () => {
    const snap = projectFullAutoReadinessSnapshot(
      [{ lane: "codex-local" }, { lane: "claude-local" }],
      allReadyGate,
      AT,
    )
    expect(snap.allReady).toBe(true)
    expect(snap.candidates.map((c) => c.state)).toEqual(["available", "available"])
    expect(snap.candidates.every((c) => c.reason === "ready")).toBe(true)
    expect(snap.boundAt).toBe(AT)
  })

  test("an unknown lane (no capability report) reads unavailable/lane_unknown", () => {
    const snap = projectFullAutoReadinessSnapshot([{ lane: "acp:grok-cli" }], gateFrom({}), AT)
    expect(snap.candidates[0]!.state).toBe("unavailable")
    expect(snap.candidates[0]!.reason).toBe("lane_unknown")
    expect(snap.allReady).toBe(false)
  })

  test("an unknown lane still being probed reads checking, not unavailable", () => {
    const snap = projectFullAutoReadinessSnapshot(
      [{ lane: "acp:grok-cli" }],
      gateFrom({}),
      AT,
      (lane) => lane === "acp:grok-cli",
    )
    expect(snap.candidates[0]!.state).toBe("checking")
  })

  test("an admitted-but-not-fullAuto lane reads unavailable/lane_not_admitted", () => {
    const snap = projectFullAutoReadinessSnapshot(
      [{ lane: "codex-local" }],
      gateFrom({ "codex-local": { admitted: true, fullAuto: false } }),
      AT,
    )
    expect(snap.candidates[0]!.reason).toBe("lane_not_admitted")
  })

  test("a lane not in the Full Auto policy set reads lane_not_full_auto_eligible", () => {
    const snap = projectFullAutoReadinessSnapshot(
      [{ lane: "acp:unknown-peer" }],
      gateFrom({ "acp:unknown-peer": { admitted: true, fullAuto: true } }),
      AT,
    )
    expect(snap.candidates[0]!.reason).toBe("lane_not_full_auto_eligible")
  })

  test("every candidate is evaluated — no first-refusal short-circuit", () => {
    const snap = projectFullAutoReadinessSnapshot(
      [{ lane: "acp:grok-cli" }, { lane: "codex-local" }],
      gateFrom({ "codex-local": { admitted: true, fullAuto: true } }),
      AT,
    )
    // grok is unknown, codex is ready — BOTH are visible.
    expect(snap.candidates.map((c) => c.state)).toEqual(["unavailable", "available"])
  })

  test("accountRef is carried through into the snapshot entry", () => {
    const snap = projectFullAutoReadinessSnapshot(
      [{ lane: "codex-local", accountRef: "codex-2" }],
      allReadyGate,
      AT,
    )
    expect(snap.candidates[0]!.accountRef).toBe("codex-2")
  })

  test("allReady agrees with validateFullAutoRoutingPolicy for the same gate", () => {
    const readyPolicy = [{ lane: "codex-local" }, { lane: "acp:grok-cli" }]
    expect(projectFullAutoReadinessSnapshot(readyPolicy, allReadyGate, AT).allReady).toBe(true)
    expect(validateFullAutoRoutingPolicy(readyPolicy, allReadyGate).ok).toBe(true)

    const brokenPolicy = [{ lane: "codex-local" }, { lane: "acp:grok-cli" }]
    const partialGate = gateFrom({ "codex-local": { admitted: true, fullAuto: true } })
    expect(projectFullAutoReadinessSnapshot(brokenPolicy, partialGate, AT).allReady).toBe(false)
    expect(validateFullAutoRoutingPolicy(brokenPolicy, partialGate).ok).toBe(false)
  })

  test("the snapshot decodes against its own schema", () => {
    const snap = projectFullAutoReadinessSnapshot([{ lane: "codex-local" }], allReadyGate, AT)
    expect(decodeFullAutoReadinessSnapshot(snap)).toEqual(snap)
  })
})

describe("projectFullAutoLaneScan — scan/lane reconciliation", () => {
  test("shows every Full-Auto-eligible action lane plus advisory Apple FM", () => {
    const scan = projectFullAutoLaneScan(allReadyGate, { appleFmState: "available" })
    const action = scan.filter((e) => e.role === "action").map((e) => e.lane)
    // All four action lanes appear, including Cursor (which the boot scan omits).
    expect(action).toEqual(Object.keys(FULL_AUTO_LANE_POLICIES).sort())
    expect(action).toContain("acp:cursor-agent")

    const advisory = scan.filter((e) => e.role === "advisory")
    expect(advisory).toHaveLength(1)
    expect(advisory[0]!.lane).toBe(FULL_AUTO_ADVISORY_LANE)
    expect(advisory[0]!.reason).toBe("advisory_only_no_action_authority")
    expect(advisory[0]!.state).toBe("available")
  })

  test("Apple FM defaults to checking and is never an action lane", () => {
    const scan = projectFullAutoLaneScan(allReadyGate)
    const apple = scan.find((e) => e.lane === FULL_AUTO_ADVISORY_LANE)
    expect(apple?.role).toBe("advisory")
    expect(apple?.state).toBe("checking")
    expect(scan.some((e) => e.lane === FULL_AUTO_ADVISORY_LANE && e.role === "action")).toBe(false)
  })

  test("a not-ready action lane is visible with its typed reason", () => {
    const scan = projectFullAutoLaneScan(
      gateFrom({
        "codex-local": { admitted: true, fullAuto: true },
        "claude-local": { admitted: true, fullAuto: true },
        "acp:grok-cli": { admitted: true, fullAuto: true },
        // cursor absent -> unknown
      }),
    )
    const cursor = scan.find((e) => e.lane === "acp:cursor-agent")
    expect(cursor?.state).toBe("unavailable")
    expect(cursor?.reason).toBe("lane_unknown")
  })
})
