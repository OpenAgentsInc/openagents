import { makeOpencodeAdapter } from "@openagentsinc/agent-harness-contract"
import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import type { ClaudeLocalEvent } from "./claude-local-contract.ts"
import { GOOSE_LANE_REF, makeGooseLane } from "./goose-lane.ts"
import type { HarnessBinaryProbe } from "./harness-binary-probe.ts"

/**
 * Seven-agents Part 2 (#9183) behavior oracle:
 * openagents_desktop.chat.host_run_harness_lanes.v1 — Goose reports
 * `available` only when its binary is detected, and a detected lane runs one
 * real turn through the SDK adapter lowering onto the frozen renderer envelope.
 * The probe is injected so the test never depends on a real goose install and
 * never spawns a login/install.
 */

const detected: HarnessBinaryProbe = {
  state: "detected",
  resolvedPath: "/opt/bin/goose",
  realPath: "/opt/bin/goose",
  reportedVersion: "goose 1.0.0",
}
const notDetected: HarnessBinaryProbe = {
  state: "not_detected",
  reason: "Goose CLI is not installed or not on PATH.",
}

describe("goose host-run harness lane", () => {
  test("availability is available only when the goose binary is detected", async () => {
    const up = makeGooseLane({ resolveWorkspace: () => "/tmp/ws", probe: async () => detected })
    expect(await up.availability()).toEqual({ state: "available", models: ["goose-configured"] })

    const down = makeGooseLane({ resolveWorkspace: () => "/tmp/ws", probe: async () => notDetected })
    const availability = await down.availability()
    expect(availability.state).toBe("unavailable")
    if (availability.state === "unavailable") expect(availability.reason).toContain("Goose CLI")
  })

  test("the capability report is honest: harness lane ref, Goose display name, interrupt only", () => {
    const goose = makeGooseLane({ resolveWorkspace: () => "/tmp/ws", probe: async () => detected })
    const report = goose.capabilities
    expect(report.laneRef).toBe(GOOSE_LANE_REF)
    expect(report.provider).toBe("goose")
    expect(report.composer.displayName).toBe("Goose")
    expect(report.features.interrupt).toBe(true)
    // #9187: a Full-Auto action lane, admitted in allowedFeatures (not an
    // over-claim). The live `goose acp` transport auto-allows every background
    // permission request, so a background turn never parks.
    expect(report.features.fullAuto).toBe(true)
    expect(report.policy.allowedFeatures).toContain("fullAuto")
    expect(report.policy.source).toBe("native-static-declaration")
  })

  test("a detected lane runs one real turn through the SDK adapter and lowers it onto the envelope", async () => {
    const goose = makeGooseLane({
      resolveWorkspace: () => "/tmp/ws",
      probe: async () => detected,
      // Stand in for the live `goose acp` transport with a scripted adapter.
      prepareTurnForTest: () =>
        Effect.succeed({ adapter: makeOpencodeAdapter(), shutdown: () => Effect.void }),
    })
    const events: ClaudeLocalEvent[] = []
    const result = await goose.lane.runTurn({
      request: { threadRef: "thread-1", turnRef: "turn-1", message: "hi" },
      model: "goose-configured",
      context: null,
      history: [],
      message: "hi",
      background: false,
      emit: (event) => events.push(event),
    })
    expect(result.ok).toBe(true)
    expect(events[0]?.kind).toBe("turn_started")
    expect(events.at(-1)?.kind).toBe("turn_completed")
  })

  test("the lane admits an ordinary turn and refuses plan-only it cannot do (honest capability closure)", () => {
    const goose = makeGooseLane({ resolveWorkspace: () => "/tmp/ws", probe: async () => detected })
    expect(goose.lane.admit({ threadRef: "t", turnRef: "u", message: "m" }).ok).toBe(true)
    expect(
      goose.lane.admit({ threadRef: "t", turnRef: "u", message: "m", permissionMode: "plan_only" }).ok,
    ).toBe(false)
  })
})
