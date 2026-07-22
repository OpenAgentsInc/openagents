/**
 * Seven-agents Part 2 (#9183): the OpenCode host-run harness lane.
 *
 * OpenCode has NO ACP wire — it speaks an HTTP + SSE control plane. The SDK's
 * `makeLiveOpencodeTransport` spawns and OWNS an `opencode serve` process
 * (inheriting the developer's live `~/.config/opencode` auth), and
 * `makeOpencodeAdapter` drives the neutral turn over it. So OpenCode runs as a
 * HOST-RUN SDK-harness lane (the #9167 built-in-harness family), NOT a trusted
 * ACP peer. `harness-lowering` maps the neutral stream onto the renderer
 * envelope.
 *
 * Owner-local by construction: the spawned server uses the developer's live
 * opencode config and its configured default model (this lane advertises an
 * opaque display model and never forces a specific one). This lane NEVER runs a
 * login flow, NEVER changes PATH, and NEVER runs an install command — it only
 * DETECTS the `opencode` binary and, when absent, the boot roster shows an
 * honest `unavailable` reason. A turn is reachable only when the binary is
 * detected.
 */

import { makeLiveOpencodeTransport, makeOpencodeAdapter } from "@openagentsinc/agent-harness-contract"
import { Effect } from "effect"
import { ClaudeLocalEventChannel } from "./claude-local-contract.ts"
import type { HarnessLaneAvailability } from "./goose-lane.ts"
import {
  makeHarnessProviderLane,
  type HarnessProviderLaneDriver,
} from "./harness-provider-lane.ts"
import { probeHarnessBinary, type HarnessBinaryProbe } from "./harness-binary-probe.ts"
import {
  makeHarnessSdkTurnDriver,
  type HarnessSdkTurnInput,
  type HarnessTurnAdapter,
} from "./harness-sdk-turn-runner.ts"
import type { ProviderLane } from "./provider-lane.ts"
import type { ProviderLaneCapabilityReport } from "./provider-lane-capabilities.ts"

export const OPENCODE_LANE_REF = "harness:opencode" as const
const OPENCODE_MODEL = "opencode-configured" as const

const opencodeCapabilities: ProviderLaneCapabilityReport = {
  laneRef: OPENCODE_LANE_REF,
  provider: "opencode",
  models: [OPENCODE_MODEL],
  features: {
    skills: false,
    planOnly: false,
    reasoningEffort: false,
    images: false,
    fullAuto: false,
    interrupt: true,
    queueFollowup: false,
    steerTurn: false,
    steerChild: false,
    answerQuestion: false,
  },
  recovery: "interrupt_on_restart",
  composer: {
    displayName: "OpenCode",
    reasoningEfforts: [],
    permissionModes: ["owner_full"],
    approvals: "host_mediated",
    extensions: [],
  },
  policy: {
    source: "native-static-declaration",
    profileRef: OPENCODE_LANE_REF,
    evidence: "experimental",
    allowedModels: [OPENCODE_MODEL],
    allowedFeatures: ["interrupt"],
    allowedExtensions: [],
  },
}

export type OpencodeLane = Readonly<{
  lane: ProviderLane<null>
  capabilities: ProviderLaneCapabilityReport
  availability: () => Promise<HarnessLaneAvailability>
  interrupt: (turnRef: string) => boolean
}>

/** Build the OpenCode lane. Detection is memoized after the first probe round. */
export const makeOpencodeLane = (input: Readonly<{
  resolveWorkspace: () => string
  environment?: Readonly<Record<string, string | undefined>>
  eventChannel?: string
  probe?: () => Promise<HarnessBinaryProbe>
  /**
   * Injectable turn-adapter builder for tests: bypasses the live
   * `opencode serve` transport with a scripted SDK adapter so the lane's turn
   * path is provable without a real opencode binary. Production always uses the
   * live transport.
   */
  prepareTurnForTest?: (input: HarnessSdkTurnInput) => Effect.Effect<HarnessTurnAdapter, unknown>
}>): OpencodeLane => {
  const environment = input.environment ?? process.env
  const runProbe =
    input.probe ??
    (() =>
      probeHarnessBinary({
        executable: "opencode",
        displayName: "OpenCode CLI",
        versionArgs: ["--version"],
        environment,
      }))
  // Cache ONLY a successful detection, so installing OpenCode after launch is
  // picked up on the next capability refresh.
  let detected: HarnessBinaryProbe | null = null
  const detect = async (): Promise<HarnessBinaryProbe> => {
    if (detected !== null && detected.state === "detected") return detected
    detected = await runProbe()
    return detected
  }

  const prepareTurnLive = (_turnInput: HarnessSdkTurnInput): Effect.Effect<HarnessTurnAdapter, unknown> =>
    Effect.gen(function* () {
      const probe = yield* Effect.promise(detect)
      if (probe.state !== "detected") {
        return yield* Effect.die(new Error(probe.reason))
      }
      const directory = input.resolveWorkspace()
      const transport = yield* makeLiveOpencodeTransport({
        binaryPath: probe.realPath,
        directory,
      })
      const adapter = makeOpencodeAdapter({ transport, directory })
      return { adapter, shutdown: () => transport.shutdown() }
    })

  const driver: HarnessProviderLaneDriver = makeHarnessSdkTurnDriver({
    source: { lane: "ai_sdk_harness_sandbox", adapterKind: "opencode" },
    prepareTurn: input.prepareTurnForTest ?? prepareTurnLive,
  })

  const lane = makeHarnessProviderLane({
    laneRef: OPENCODE_LANE_REF,
    graphLaneRef: "opencode_harness",
    eventChannel: input.eventChannel ?? ClaudeLocalEventChannel,
    capabilities: opencodeCapabilities,
    driver,
  })

  const availability = async (): Promise<HarnessLaneAvailability> => {
    const probe = await detect()
    return probe.state === "detected"
      ? { state: "available", models: [OPENCODE_MODEL] }
      : { state: "unavailable", reason: probe.reason }
  }

  return { lane, capabilities: opencodeCapabilities, availability, interrupt: driver.interrupt }
}
