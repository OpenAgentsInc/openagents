/**
 * Seven-agents Part 2 (#9183): the Goose host-run harness lane.
 *
 * Goose speaks the Agent Client Protocol over stdio (`goose acp`). Rather than
 * the Grok/Cursor trusted-peer-profile path (which needs a registered profile +
 * release conformance evidence + a dedicated runtime package — none of which
 * Goose has yet), this lane runs Goose as a HOST-RUN SDK-harness lane, the same
 * built-in-harness-lane family (#9167) codex/claude use: the SDK's
 * `makeLiveGooseAcpTransport` spawns `goose acp`, `makeAcpHarnessAdapter` drives
 * the neutral turn, and `harness-lowering` maps it onto the renderer envelope.
 *
 * Owner-local by construction: Goose reads the developer's live
 * `~/.config/goose` provider auth. This lane NEVER runs a login flow, NEVER
 * changes PATH, and NEVER runs an install command — it only DETECTS the `goose`
 * binary and guides the owner to the official distribution through the boot
 * roster's honest `unavailable` reason when it is absent. A turn is reachable
 * only when the binary is detected, so the roster never shows a dead card.
 */

import {
  GOOSE_BUILTIN_TOOLS,
  makeAcpHarnessAdapter,
  makeLiveGooseAcpTransport,
} from "@openagentsinc/agent-harness-contract"
import { Effect } from "effect"
import { ClaudeLocalEventChannel } from "./claude-local-contract.ts"
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

export const GOOSE_LANE_REF = "harness:goose" as const
const GOOSE_MODEL = "goose-configured" as const

export type HarnessLaneAvailability =
  | Readonly<{ state: "available"; models: ReadonlyArray<string> }>
  | Readonly<{ state: "unavailable"; reason: string }>

const gooseCapabilities: ProviderLaneCapabilityReport = {
  laneRef: GOOSE_LANE_REF,
  provider: "goose",
  models: [GOOSE_MODEL],
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
    displayName: "Goose",
    reasoningEfforts: [],
    permissionModes: ["owner_full"],
    approvals: "host_mediated",
    extensions: [],
  },
  policy: {
    source: "native-static-declaration",
    profileRef: GOOSE_LANE_REF,
    evidence: "experimental",
    allowedModels: [GOOSE_MODEL],
    allowedFeatures: ["interrupt"],
    allowedExtensions: [],
  },
}

export type GooseLane = Readonly<{
  lane: ProviderLane<null>
  capabilities: ProviderLaneCapabilityReport
  availability: () => Promise<HarnessLaneAvailability>
  interrupt: (turnRef: string) => boolean
}>

/**
 * Build the Goose lane. `resolveWorkspace` is the owner-local working directory
 * (shared with the ACP peer lanes); `environment` defaults to `process.env`
 * (injectable for tests). Detection is memoized after the first probe round.
 */
export const makeGooseLane = (input: Readonly<{
  resolveWorkspace: () => string
  environment?: Readonly<Record<string, string | undefined>>
  eventChannel?: string
  /** Injectable probe for tests (defaults to the real binary probe). */
  probe?: () => Promise<HarnessBinaryProbe>
  /**
   * Injectable turn-adapter builder for tests: bypasses the live `goose acp`
   * transport with a scripted SDK adapter so the lane's turn path is provable
   * without a real goose binary. Production always uses the live transport.
   */
  prepareTurnForTest?: (input: HarnessSdkTurnInput) => Effect.Effect<HarnessTurnAdapter, unknown>
}>): GooseLane => {
  const environment = input.environment ?? process.env
  const runProbe =
    input.probe ??
    (() =>
      probeHarnessBinary({
        executable: "goose",
        displayName: "Goose CLI",
        versionArgs: ["--version"],
        environment,
      }))
  // Cache ONLY a successful detection, so installing Goose after launch is
  // picked up on the next capability refresh (a missing binary is a fast
  // PATH-walk, no subprocess spawn).
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
      const transport = yield* makeLiveGooseAcpTransport({
        binaryPath: probe.realPath,
        cwd: input.resolveWorkspace(),
      })
      const adapter = makeAcpHarnessAdapter({
        harnessId: "goose",
        harnessKind: "custom",
        adapterKind: "agent_client_protocol",
        transport,
        builtinTools: GOOSE_BUILTIN_TOOLS,
        supportsBuiltinToolApprovals: false,
        supportsBuiltinToolFiltering: false,
        supportsSuspend: true,
        supportsContinue: true,
        supportsCompact: true,
        supportsDetach: true,
        continueIsLossy: false,
      })
      return { adapter, shutdown: () => transport.shutdown() }
    })

  const driver: HarnessProviderLaneDriver = makeHarnessSdkTurnDriver({
    source: { lane: "agent_client_protocol", adapterKind: "agent_client_protocol" },
    prepareTurn: input.prepareTurnForTest ?? prepareTurnLive,
  })

  const lane = makeHarnessProviderLane({
    laneRef: GOOSE_LANE_REF,
    graphLaneRef: "goose_harness",
    eventChannel: input.eventChannel ?? ClaudeLocalEventChannel,
    capabilities: gooseCapabilities,
    driver,
  })

  const availability = async (): Promise<HarnessLaneAvailability> => {
    const probe = await detect()
    return probe.state === "detected"
      ? { state: "available", models: [GOOSE_MODEL] }
      : { state: "unavailable", reason: probe.reason }
  }

  return { lane, capabilities: gooseCapabilities, availability, interrupt: driver.interrupt }
}
