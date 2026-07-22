import { describe, expect, test } from "vite-plus/test"

import type { ClaudeLocalEvent } from "./claude-local-contract.ts"
import type { HarnessBinaryProbe } from "./harness-binary-probe.ts"
import { PI_LANE_REF, makePiLane } from "./pi-local-runtime.ts"

/**
 * Seven-agents Part 2 (#9183): Pi is DETECTION-ONLY. It is an in-process Node
 * library with no desktop host session-factory seam yet, so the lane is ALWAYS
 * `unavailable` with an honest reason (refined by binary detection) and its
 * `runTurn` fails typed — it never fakes a turn.
 */

const detected: HarnessBinaryProbe = {
  state: "detected",
  resolvedPath: "/opt/bin/pi",
  realPath: "/opt/bin/pi",
  reportedVersion: "pi 0.1.0",
}
const notDetected: HarnessBinaryProbe = {
  state: "not_detected",
  reason: "Pi CLI is not installed or not on PATH.",
}

describe("pi detection-only harness lane", () => {
  test("is always unavailable, with a reason that distinguishes present-but-no-seam from not-installed", async () => {
    const present = makePiLane({ probe: async () => detected })
    const presentAvailability = await present.availability()
    expect(presentAvailability.state).toBe("unavailable")
    if (presentAvailability.state === "unavailable") {
      expect(presentAvailability.reason).toContain("detected")
      expect(presentAvailability.reason).toContain("in-process")
    }

    const absent = makePiLane({ probe: async () => notDetected })
    const absentAvailability = await absent.availability()
    expect(absentAvailability.state).toBe("unavailable")
    if (absentAvailability.state === "unavailable") {
      expect(absentAvailability.reason).toContain("not installed")
    }
  })

  test("the capability report is honest: harness lane ref, no interrupt, no runnable features", () => {
    const pi = makePiLane({ probe: async () => detected })
    expect(pi.capabilities.laneRef).toBe(PI_LANE_REF)
    expect(pi.capabilities.provider).toBe("pi")
    expect(pi.capabilities.composer.displayName).toBe("Pi")
    expect(pi.capabilities.features.interrupt).toBe(false)
  })

  test("runTurn never fakes a turn — it fails typed and emits the honest seam reason", async () => {
    const pi = makePiLane({ probe: async () => detected })
    const events: ClaudeLocalEvent[] = []
    const result = await pi.lane.runTurn({
      request: { threadRef: "thread-1", turnRef: "turn-1", message: "hi" },
      model: "pi-configured",
      context: null,
      history: [],
      message: "hi",
      background: false,
      emit: (event) => events.push(event),
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe("sdk_unavailable")
    expect(events.some((event) => event.kind === "turn_failed")).toBe(true)
    expect(pi.interrupt("turn-1")).toBe(false)
  })
})
