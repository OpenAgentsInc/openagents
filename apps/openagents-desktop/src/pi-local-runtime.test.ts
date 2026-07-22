import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "vite-plus/test"

import type { PiSessionEvent, PiSessionSurface } from "@openagentsinc/agent-harness-contract"
import type { ClaudeLocalEvent } from "./claude-local-contract.ts"
import { PI_LANE_REF, makePiLane } from "./pi-local-runtime.ts"
import { makePiSessionHost } from "./pi-session-host.ts"

/**
 * Seven-agents (#9183) behavior oracle:
 * openagents_desktop.chat.host_run_harness_lanes.v1 — Pi reports `available`
 * only when the in-process host can construct a session (the optional Pi
 * library resolves AND an owner-local key is present), and a ready lane runs
 * ONE real turn through `makePiHarnessAdapter` lowering onto the frozen renderer
 * envelope. The Pi library and key are injected so the test drives the FULL
 * seam (host → factory → adapter → lowering) without a real install, key, or
 * subprocess/login.
 */

/** A scripted in-process Pi session: emits a minimal but complete event feed. */
const makeScriptedSurface = (): PiSessionSurface => {
  let listener: ((event: PiSessionEvent) => void) | null = null
  return {
    subscribe: (l) => {
      listener = l
      return () => {
        listener = null
      }
    },
    prompt: async () => {
      listener?.({ type: "agent_start" })
      listener?.({
        type: "message_update",
        assistantMessageEvent: { type: "text_delta", delta: "Pi replies." },
      })
      listener?.({ type: "message_update", assistantMessageEvent: { type: "text_end" } })
      listener?.({ type: "message_update", assistantMessageEvent: { type: "done", reason: "stop" } })
      listener?.({ type: "agent_end" })
    },
    steer: async () => {},
    abort: async () => {},
    compact: async () => {},
    getSessionStats: () => ({ tokens: { input: 5, output: 3, cacheRead: 0, cacheWrite: 0 } }),
    dispose: () => {},
  }
}

/** A scripted `@earendil-works/pi-coding-agent` module. */
const scriptedModule = {
  createAgentSession: async (_options: {
    agentDir?: string
    cwd?: string
    tools?: ReadonlyArray<string>
    customTools?: ReadonlyArray<unknown>
  }) => ({ session: makeScriptedSurface() }),
}

const freshAgentDir = (): string => mkdtempSync(join(tmpdir(), "oa-pi-agent-test-"))

const readyHost = () =>
  makePiSessionHost({
    agentDir: freshAgentDir(),
    resolveWorkspace: () => "/tmp/ws",
    loadModule: async () => scriptedModule,
    loadApiKey: () => "test-gemini-key",
  })

describe("pi host-run in-process harness lane", () => {
  test("availability is available only when the in-process host constructs (library + key present)", async () => {
    const up = makePiLane({ agentDir: freshAgentDir(), resolveWorkspace: () => "/tmp/ws", host: readyHost() })
    expect(await up.availability()).toEqual({ state: "available", models: ["gemini-3.6-flash"] })

    // No owner-local key ⇒ honest unavailable, precise reason.
    const noKey = makePiLane({
      agentDir: freshAgentDir(),
      resolveWorkspace: () => "/tmp/ws",
      host: makePiSessionHost({
        agentDir: freshAgentDir(),
        resolveWorkspace: () => "/tmp/ws",
        loadModule: async () => scriptedModule,
        loadApiKey: () => null,
        loadApiKeyAsync: async () => null,
      }),
    })
    const noKeyAvailability = await noKey.availability()
    expect(noKeyAvailability.state).toBe("unavailable")
    if (noKeyAvailability.state === "unavailable") expect(noKeyAvailability.reason).toContain("Gemini API key")

    // Library not installed ⇒ honest unavailable, precise reason.
    const noLib = makePiLane({
      agentDir: freshAgentDir(),
      resolveWorkspace: () => "/tmp/ws",
      host: makePiSessionHost({
        agentDir: freshAgentDir(),
        resolveWorkspace: () => "/tmp/ws",
        loadModule: async () => {
          throw new Error("Cannot find module")
        },
        loadApiKey: () => "test-gemini-key",
      }),
    })
    const noLibAvailability = await noLib.availability()
    expect(noLibAvailability.state).toBe("unavailable")
    if (noLibAvailability.state === "unavailable")
      expect(noLibAvailability.reason).toContain("@earendil-works/pi-coding-agent")
  })

  test("the capability report is honest: harness lane ref, Pi display name, gemini-3.6-flash, interrupt", () => {
    const pi = makePiLane({ agentDir: freshAgentDir(), resolveWorkspace: () => "/tmp/ws", host: readyHost() })
    const report = pi.capabilities
    expect(report.laneRef).toBe(PI_LANE_REF)
    expect(report.provider).toBe("pi")
    expect(report.composer.displayName).toBe("Pi")
    expect(report.models).toEqual(["gemini-3.6-flash"])
    expect(report.features.interrupt).toBe(true)
    // #9187: a Full-Auto action lane, admitted in allowedFeatures (not an
    // over-claim). The in-process host runs `permissionMode: "allow-all"`, so a
    // background built-in tool call is auto-allowed and the turn never parks.
    expect(report.features.fullAuto).toBe(true)
    expect(report.policy.allowedFeatures).toContain("fullAuto")
    expect(report.policy.source).toBe("native-static-declaration")
  })

  test("a ready lane runs one real turn through makePiHarnessAdapter and lowers it onto the envelope", async () => {
    const pi = makePiLane({ agentDir: freshAgentDir(), resolveWorkspace: () => "/tmp/ws", host: readyHost() })
    const events: ClaudeLocalEvent[] = []
    const result = await pi.lane.runTurn({
      request: { threadRef: "thread-1", turnRef: "turn-1", message: "hi" },
      model: "gemini-3.6-flash",
      context: null,
      history: [],
      message: "hi",
      background: false,
      emit: (event) => events.push(event),
    })
    expect(result.ok).toBe(true)
    if (result.ok) expect(result.text).toContain("Pi replies.")
    expect(events[0]?.kind).toBe("turn_started")
    expect(events.some((event) => event.kind === "text_delta")).toBe(true)
    expect(events.at(-1)?.kind).toBe("turn_completed")
  })

  test("the lane admits an ordinary turn and refuses plan-only it cannot do (honest capability closure)", () => {
    const pi = makePiLane({ agentDir: freshAgentDir(), resolveWorkspace: () => "/tmp/ws", host: readyHost() })
    expect(pi.lane.admit({ threadRef: "t", turnRef: "u", message: "m" }).ok).toBe(true)
    expect(
      pi.lane.admit({ threadRef: "t", turnRef: "u", message: "m", permissionMode: "plan_only" }).ok,
    ).toBe(false)
  })
})
