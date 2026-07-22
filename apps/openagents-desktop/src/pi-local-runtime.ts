/**
 * Seven-agents Part 2 (#9183): the Pi harness lane — DETECTION-ONLY, honestly.
 *
 * Pi is an IN-PROCESS Node library, not a subprocess and not an ACP/HTTP peer.
 * Its SDK adapter (`makePiHarnessAdapter`) requires an injected
 * `createSession: PiSessionFactory` — the real Pi `createAgentSession` running
 * inside the OpenAgents host. Unlike Goose (`goose acp` stdio) and OpenCode
 * (`opencode serve`), there is NO live-transport spawner the desktop can drive:
 * reaching a runnable Pi turn needs (a) the Pi in-process library as a desktop
 * dependency and (b) a host session-factory seam that constructs and owns a
 * `PiSessionSurface` per turn. Neither exists yet.
 *
 * Per the acceptance ("land what's genuinely runnable; be honest about the
 * rest"), this lane wires DETECTION + a lane-as-`unavailable`-with-honest-reason
 * rather than faking `available`. It appears in the boot roster with an honest
 * status; its `runTurn` fails typed and never pretends to run. The precise
 * remaining seam is recorded on #9183.
 */

import { ClaudeLocalEventChannel } from "./claude-local-contract.ts"
import type { HarnessLaneAvailability } from "./goose-lane.ts"
import {
  makeHarnessProviderLane,
  type HarnessProviderLaneDriver,
} from "./harness-provider-lane.ts"
import { probeHarnessBinary, type HarnessBinaryProbe } from "./harness-binary-probe.ts"
import type { ProviderLane } from "./provider-lane.ts"
import type { ProviderLaneCapabilityReport } from "./provider-lane-capabilities.ts"

export const PI_LANE_REF = "harness:pi" as const
const PI_MODEL = "pi-configured" as const

/** The single honest reason Pi is not yet runnable, refined by binary detection. */
export const PI_MISSING_SEAM =
  "Pi runs in-process; the desktop has no Pi session-factory host seam yet (#9183)."

const piCapabilities: ProviderLaneCapabilityReport = {
  laneRef: PI_LANE_REF,
  provider: "pi",
  models: [PI_MODEL],
  features: {
    skills: false,
    planOnly: false,
    reasoningEffort: false,
    images: false,
    fullAuto: false,
    interrupt: false,
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
    allowedModels: [PI_MODEL],
    allowedFeatures: [],
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
 * Build the Pi lane. Detection distinguishes "the Pi CLI is present but the
 * in-process host seam is absent" from "Pi is not installed", but BOTH are
 * `unavailable` — the desktop cannot run a Pi turn either way yet.
 */
export const makePiLane = (input: Readonly<{
  environment?: Readonly<Record<string, string | undefined>>
  eventChannel?: string
  probe?: () => Promise<HarnessBinaryProbe>
}> = {}): PiLane => {
  const environment = input.environment ?? process.env
  const runProbe =
    input.probe ??
    (() =>
      probeHarnessBinary({
        executable: "pi",
        displayName: "Pi CLI",
        versionArgs: ["--version"],
        environment,
      }))
  let probed: Promise<HarnessBinaryProbe> | null = null
  const detect = (): Promise<HarnessBinaryProbe> => (probed ??= runProbe())

  const driver: HarnessProviderLaneDriver = {
    // Detection-only: never fake a turn. Fail typed with the precise seam.
    runTurn: async ({ emit }) => {
      emit({ kind: "turn_failed", reason: "sdk_unavailable", detail: PI_MISSING_SEAM })
      return { ok: false, reason: "sdk_unavailable", detail: PI_MISSING_SEAM }
    },
    interrupt: () => false,
  }

  const lane = makeHarnessProviderLane({
    laneRef: PI_LANE_REF,
    graphLaneRef: "pi_harness",
    eventChannel: input.eventChannel ?? ClaudeLocalEventChannel,
    capabilities: piCapabilities,
    driver,
  })

  const availability = async (): Promise<HarnessLaneAvailability> => {
    const probe = await detect()
    const reason =
      probe.state === "detected"
        ? `The Pi CLI is detected, but ${PI_MISSING_SEAM}`
        : `Pi is not installed, and ${PI_MISSING_SEAM}`
    return { state: "unavailable", reason }
  }

  return { lane, capabilities: piCapabilities, availability, interrupt: driver.interrupt }
}
