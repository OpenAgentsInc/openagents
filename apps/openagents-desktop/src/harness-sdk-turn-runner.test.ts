import { makeOpencodeAdapter } from "@openagentsinc/agent-harness-contract"
import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import type { ClaudeLocalEvent } from "./claude-local-contract.ts"
import { makeHarnessSdkTurnDriver } from "./harness-sdk-turn-runner.ts"

/**
 * Seven-agents Part 2 (#9183): the generic host-run SDK-harness turn driver.
 * A scripted opencode adapter (no live server) stands in for any harness so the
 * neutral-stream → frozen-envelope lowering path is proved without a binary.
 */

const runInput = (emit: (event: ClaudeLocalEvent) => void) => ({
  threadRef: "thread-1",
  turnRef: "turn-1",
  model: "opencode-configured",
  history: [],
  message: "hello",
  background: false,
  emit,
})

describe("harness SDK turn driver", () => {
  test("runs one turn through the SDK adapter and lowers it onto the frozen renderer envelope", async () => {
    const driver = makeHarnessSdkTurnDriver({
      source: { lane: "ai_sdk_harness_sandbox", adapterKind: "opencode" },
      // The default opencode script streams "Hello world" then finishes.
      prepareTurn: () =>
        Effect.succeed({
          adapter: makeOpencodeAdapter(),
          shutdown: () => Effect.void,
        }),
    })
    const events: ClaudeLocalEvent[] = []
    const result = await driver.runTurn(runInput((event) => events.push(event)))

    expect(result.ok).toBe(true)
    if (result.ok) expect(result.text).toBe("Hello world")
    // The lowered envelope must open with turn_started and close with turn_completed.
    expect(events[0]?.kind).toBe("turn_started")
    expect(events.at(-1)?.kind).toBe("turn_completed")
    expect(events.some((event) => event.kind === "text_delta")).toBe(true)
  })

  test("a failed adapter build becomes a typed session failure, never a raw provider error", async () => {
    const driver = makeHarnessSdkTurnDriver({
      source: { lane: "ai_sdk_harness_sandbox", adapterKind: "opencode" },
      prepareTurn: () => Effect.die(new Error("opencode is not installed")),
    })
    const result = await driver.runTurn(runInput(() => {}))
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.reason).toBe("session_failed")
      expect(result.detail.toLowerCase()).toContain("opencode")
    }
  })

  test("interrupt returns false when no turn with that ref is active", () => {
    const driver = makeHarnessSdkTurnDriver({
      source: { lane: "ai_sdk_harness_sandbox", adapterKind: "opencode" },
      prepareTurn: () => Effect.succeed({ adapter: makeOpencodeAdapter(), shutdown: () => Effect.void }),
    })
    expect(driver.interrupt("no-such-turn")).toBe(false)
  })
})
