import { makeOpencodeAdapter } from "@openagentsinc/agent-harness-contract"
import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import type { ClaudeLocalEvent } from "./claude-local-contract.ts"
import type { HarnessBinaryProbe } from "./harness-binary-probe.ts"
import { OPENCODE_LANE_REF, makeOpencodeLane } from "./opencode-local-runtime.ts"

/**
 * Seven-agents Part 2 (#9183): OpenCode host-run harness lane — available only
 * when the `opencode` binary is detected; a detected lane runs one real turn
 * through the SDK adapter lowering onto the frozen renderer envelope.
 */

const detected: HarnessBinaryProbe = {
  state: "detected",
  resolvedPath: "/opt/bin/opencode",
  realPath: "/opt/bin/opencode",
  reportedVersion: "opencode/0.9.9",
}
const notDetected: HarnessBinaryProbe = {
  state: "not_detected",
  reason: "OpenCode CLI is not installed or not on PATH.",
}

describe("opencode host-run harness lane", () => {
  test("availability is available only when the opencode binary is detected", async () => {
    const up = makeOpencodeLane({ resolveWorkspace: () => "/tmp/ws", probe: async () => detected })
    expect(await up.availability()).toEqual({ state: "available", models: ["opencode-configured"] })

    const down = makeOpencodeLane({ resolveWorkspace: () => "/tmp/ws", probe: async () => notDetected })
    const availability = await down.availability()
    expect(availability.state).toBe("unavailable")
    if (availability.state === "unavailable") expect(availability.reason).toContain("OpenCode CLI")
  })

  test("the capability report is honest: harness lane ref, OpenCode display name", () => {
    const opencode = makeOpencodeLane({ resolveWorkspace: () => "/tmp/ws", probe: async () => detected })
    expect(opencode.capabilities.laneRef).toBe(OPENCODE_LANE_REF)
    expect(opencode.capabilities.provider).toBe("opencode")
    expect(opencode.capabilities.composer.displayName).toBe("OpenCode")
    expect(opencode.capabilities.features.interrupt).toBe(true)
    // #9187: a Full-Auto action lane, admitted in allowedFeatures (not an
    // over-claim). The response-driven turn settles under a bounded timeout, so
    // a background turn fails closed rather than parking forever.
    expect(opencode.capabilities.features.fullAuto).toBe(true)
    expect(opencode.capabilities.policy.allowedFeatures).toContain("fullAuto")
  })

  test("a detected lane runs one real turn through the SDK adapter and lowers it onto the envelope", async () => {
    const opencode = makeOpencodeLane({
      resolveWorkspace: () => "/tmp/ws",
      probe: async () => detected,
      prepareTurnForTest: () =>
        Effect.succeed({ adapter: makeOpencodeAdapter(), shutdown: () => Effect.void }),
    })
    const events: ClaudeLocalEvent[] = []
    const result = await opencode.lane.runTurn({
      request: { threadRef: "thread-1", turnRef: "turn-1", message: "hi" },
      model: "opencode-configured",
      context: null,
      history: [],
      message: "hi",
      background: false,
      emit: (event) => events.push(event),
    })
    expect(result.ok).toBe(true)
    if (result.ok) expect(result.text).toBe("Hello world")
    expect(events[0]?.kind).toBe("turn_started")
    expect(events.at(-1)?.kind).toBe("turn_completed")
  })
})
