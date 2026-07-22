/**
 * Seven-agents (#9183): the Pi host-run harness lane — now RUNNABLE end-to-end
 * through the in-process host session-factory seam.
 *
 * Pi is an IN-PROCESS Node library, not a subprocess and not an ACP/HTTP peer.
 * Its SDK adapter (`makePiHarnessAdapter`) is deliberately dependency-free and
 * requires an injected `createSession: PiSessionFactory` — the real Pi
 * `createAgentSession` running inside the OpenAgents host. Unlike Goose
 * (`goose acp` stdio) and OpenCode (`opencode serve` HTTP/SSE) there is no
 * live-transport spawner; the desktop owns the in-process session factory
 * itself. {@link makePiSessionHost} is that seam: it constructs an owner-local
 * `PiSessionFactory` (optional Pi library + owner-local Gemini key + isolated
 * per-account agent dir + pinned `gemini-3.6-flash`), and this lane drives it
 * through `makePiHarnessAdapter` → the shared `harness-sdk-turn-runner` →
 * `harness-lowering` → the frozen renderer envelope, exactly like Goose/OpenCode
 * but with an injected in-process session instead of a spawned transport.
 *
 * Honest readiness (the same invariant Goose/OpenCode honor): the lane is
 * `available` ONLY when the host can actually construct a Pi session — the Pi
 * library resolves AND an owner-local key is present. Otherwise it is
 * `unavailable` with the precise reason (library not installed / no key), and a
 * turn is never faked. This lane NEVER runs an install command, NEVER changes
 * PATH, and NEVER runs a login flow.
 */

import { makePiHarnessAdapter } from "@openagentsinc/agent-harness-contract"
import { Effect } from "effect"
import { ClaudeLocalEventChannel } from "./claude-local-contract.ts"
import type { HarnessLaneAvailability } from "./goose-lane.ts"
import {
  makeHarnessProviderLane,
  type HarnessProviderLaneDriver,
} from "./harness-provider-lane.ts"
import {
  makeHarnessSdkTurnDriver,
  type HarnessSdkTurnInput,
  type HarnessTurnAdapter,
} from "./harness-sdk-turn-runner.ts"
import { makePiSessionHost, PI_DEFAULT_MODEL, type PiSessionHost } from "./pi-session-host.ts"
import type { ProviderLane } from "./provider-lane.ts"
import type { ProviderLaneCapabilityReport } from "./provider-lane-capabilities.ts"

export const PI_LANE_REF = "harness:pi" as const

export const piCapabilities: ProviderLaneCapabilityReport = {
  laneRef: PI_LANE_REF,
  provider: "pi",
  models: [PI_DEFAULT_MODEL],
  features: {
    skills: false,
    planOnly: false,
    reasoningEffort: false,
    images: false,
    // #9187: a Full-Auto action lane. The in-process host runs with
    // `permissionMode: "allow-all"` and the turn driver never submits an
    // approval, so a background built-in tool call is auto-allowed and the turn
    // never parks (the `autoResolveQuestions` invariant in full-auto-lane.ts).
    fullAuto: true,
    interrupt: true,
    queueFollowup: false,
    steerTurn: false,
    steerChild: false,
    answerQuestion: false,
  },
  recovery: "interrupt_on_restart",
  composer: {
    displayName: "Pi",
    reasoningEfforts: [],
    permissionModes: ["owner_full"],
    approvals: "host_mediated",
    extensions: [],
  },
  policy: {
    source: "native-static-declaration",
    profileRef: PI_LANE_REF,
    evidence: "experimental",
    allowedModels: [PI_DEFAULT_MODEL],
    allowedFeatures: ["interrupt", "fullAuto"],
    allowedExtensions: [],
  },
}

export type PiLane = Readonly<{
  lane: ProviderLane<null>
  capabilities: ProviderLaneCapabilityReport
  availability: () => Promise<HarnessLaneAvailability>
  interrupt: (turnRef: string) => boolean
}>

/**
 * Build the Pi lane. `agentDir` MUST be an isolated per-account directory the
 * desktop owns (never the owner's live `~/.pi`); `resolveWorkspace` is the
 * owner-local working directory shared with the other lanes. Both `host` and
 * `prepareTurnForTest` are injectable so the lane's full turn path is provable
 * without a real Pi install or key; production uses the live in-process host.
 */
export const makePiLane = (input: Readonly<{
  agentDir: string
  resolveWorkspace: () => string
  environment?: Readonly<Record<string, string | undefined>>
  eventChannel?: string
  /** Injectable session host (defaults to the live in-process host). */
  host?: PiSessionHost
  /**
   * Injectable turn-adapter builder for tests: bypasses the live in-process
   * host with a scripted adapter so the lane's turn path is provable without a
   * real Pi library. Production always uses the live host.
   */
  prepareTurnForTest?: (input: HarnessSdkTurnInput) => Effect.Effect<HarnessTurnAdapter, unknown>
}>): PiLane => {
  const host =
    input.host ??
    makePiSessionHost({
      agentDir: input.agentDir,
      resolveWorkspace: input.resolveWorkspace,
      ...(input.environment === undefined ? {} : { environment: input.environment }),
    })

  // Drive a real turn through the in-process Pi session. The host resolves the
  // owner-local factory (library + key + isolated agent dir); a not-ready host
  // dies typed so the turn runner recovers it to a typed session failure — the
  // dispatcher never sees a raw error, and readiness gating already refuses the
  // turn before it starts.
  const prepareTurnLive = (
    _turnInput: HarnessSdkTurnInput,
  ): Effect.Effect<HarnessTurnAdapter, unknown> =>
    Effect.gen(function* () {
      const resolution = yield* Effect.promise(() => host.resolve())
      if (resolution.state !== "ready") {
        return yield* Effect.die(new Error(resolution.reason))
      }
      const adapter = makePiHarnessAdapter({
        createSession: resolution.createSession,
        agentDir: resolution.agentDir,
        workspaceDir: input.resolveWorkspace(),
      })
      // Pi runs in-process: there is no separate transport/process to tear
      // down. The adapter disposes its own session surface via stop/destroy.
      return { adapter, shutdown: () => Effect.void }
    })

  const driver: HarnessProviderLaneDriver = makeHarnessSdkTurnDriver({
    source: { lane: "ai_sdk_core", adapterKind: "openagents_native" },
    prepareTurn: input.prepareTurnForTest ?? prepareTurnLive,
  })

  const lane = makeHarnessProviderLane({
    laneRef: PI_LANE_REF,
    graphLaneRef: "pi_harness",
    eventChannel: input.eventChannel ?? ClaudeLocalEventChannel,
    capabilities: piCapabilities,
    driver,
  })

  const availability = async (): Promise<HarnessLaneAvailability> => {
    const resolution = await host.resolve()
    return resolution.state === "ready"
      ? { state: "available", models: [PI_DEFAULT_MODEL] }
      : { state: "unavailable", reason: resolution.reason }
  }

  return { lane, capabilities: piCapabilities, availability, interrupt: driver.interrupt }
}
