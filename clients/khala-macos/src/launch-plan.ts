export const APPLE_FM_BACKEND_KIND = "apple_fm_bridge" as const
export const APPLE_FM_LOCAL_PROFILE_ID = "apple-fm-local" as const
export const APPLE_FM_DEFAULT_MODEL_ID = "apple-foundation-model" as const
export const APPLE_FM_BRIDGE_DEFAULT_BASE_URL = "http://127.0.0.1:11435" as const
export const APPLE_FM_BRIDGE_HELPER_BASENAME = "foundation-bridge" as const
export const APPLE_FM_PACKAGED_HELPER_SUBPATH =
  "app/apple-fm-bridge/foundation-bridge" as const
export const PYLON_PACKAGED_NODE_SUBPATH = "app/pylon-node/index.js" as const

export const KHALA_APPLE_FM_TOKEN_PROVIDER =
  "pylon-apple-fm-own-capacity" as const
export const KHALA_APPLE_FM_DEMAND_SOURCE =
  "khala_apple_fm_delegation" as const

export type AppleFmBaseUrlSource =
  | "PROBE_APPLE_FM_BASE_URL"
  | "OPENAGENTS_APPLE_FM_BASE_URL"
  | "default"

export type KhalaMacosPylonMode =
  | "connect_existing"
  | "launch_embedded"

export type LocalAppleFmDemandAttribution = {
  readonly provider: typeof KHALA_APPLE_FM_TOKEN_PROVIDER
  readonly model: typeof APPLE_FM_DEFAULT_MODEL_ID
  readonly backendKind: typeof APPLE_FM_BACKEND_KIND
  readonly demandKind: "own_capacity"
  readonly demandSource: typeof KHALA_APPLE_FM_DEMAND_SOURCE
  readonly usageTruth: "estimated"
  readonly counterFamily: "khala_tokens_served"
}

export type KhalaMacosLaunchPlanInput = {
  readonly resourcesDir: string
  readonly homeDir: string
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly existingPylonReady?: boolean
  readonly appleFmBridgeReady?: boolean
  readonly bunPath?: string
}

export type KhalaMacosLaunchPlan = {
  readonly pylonMode: KhalaMacosPylonMode
  readonly pylonCommand: ReadonlyArray<string>
  readonly pylonHome: string
  readonly appleFmBridgePath: string
  readonly appleFmBaseUrl: string
  readonly appleFmBaseUrlSource: AppleFmBaseUrlSource
  readonly childEnv: Readonly<Record<string, string>>
  readonly capacityRefs: ReadonlyArray<string>
  readonly blockerRefs: ReadonlyArray<string>
  readonly demandAttribution: LocalAppleFmDemandAttribution
}

const trimValue = (value: string | undefined): string | null => {
  const trimmed = value?.trim() ?? ""
  return trimmed.length === 0 ? null : trimmed
}

const joinPath = (...segments: ReadonlyArray<string>): string =>
  segments
    .map((segment, index) =>
      index === 0
        ? segment.replace(/\/+$/, "")
        : segment.replace(/^\/+|\/+$/g, ""),
    )
    .filter((segment) => segment.length > 0)
    .join("/")

export function resolveAppleFmBaseUrl(
  env: Readonly<Record<string, string | undefined>> = {},
): { readonly baseUrl: string; readonly source: AppleFmBaseUrlSource } {
  const probeBaseUrl = trimValue(env.PROBE_APPLE_FM_BASE_URL)
  if (probeBaseUrl !== null) {
    return { baseUrl: probeBaseUrl, source: "PROBE_APPLE_FM_BASE_URL" }
  }

  const openagentsBaseUrl = trimValue(env.OPENAGENTS_APPLE_FM_BASE_URL)
  if (openagentsBaseUrl !== null) {
    return {
      baseUrl: openagentsBaseUrl,
      source: "OPENAGENTS_APPLE_FM_BASE_URL",
    }
  }

  return { baseUrl: APPLE_FM_BRIDGE_DEFAULT_BASE_URL, source: "default" }
}

export const localAppleFmDemandAttribution =
  (): LocalAppleFmDemandAttribution => ({
    provider: KHALA_APPLE_FM_TOKEN_PROVIDER,
    model: APPLE_FM_DEFAULT_MODEL_ID,
    backendKind: APPLE_FM_BACKEND_KIND,
    demandKind: "own_capacity",
    demandSource: KHALA_APPLE_FM_DEMAND_SOURCE,
    usageTruth: "estimated",
    counterFamily: "khala_tokens_served",
  })

export function buildKhalaMacosLaunchPlan(
  input: KhalaMacosLaunchPlanInput,
): KhalaMacosLaunchPlan {
  const env = input.env ?? {}
  const baseUrl = resolveAppleFmBaseUrl(env)
  const appleFmBridgePath = joinPath(
    input.resourcesDir,
    APPLE_FM_PACKAGED_HELPER_SUBPATH,
  )
  const pylonHome = trimValue(env.PYLON_HOME) ?? joinPath(input.homeDir, ".openagents", "khala-macos", "pylon")
  const pylonEntry = joinPath(input.resourcesDir, PYLON_PACKAGED_NODE_SUBPATH)
  const bunPath = trimValue(input.bunPath) ?? "bun"

  const pylonMode: KhalaMacosPylonMode =
    input.existingPylonReady === true ? "connect_existing" : "launch_embedded"
  const appleFmReady = input.appleFmBridgeReady === true
  const pylonReady = true

  const childEnv: Record<string, string> = {}
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) childEnv[key] = value
  }
  childEnv.PYLON_HOME = pylonHome
  childEnv.PYLON_APPLE_FM_SUPERVISE = "1"
  childEnv.OPENAGENTS_APPLE_FM_BRIDGE_PATH = appleFmBridgePath
  childEnv.PROBE_APPLE_FM_BASE_URL = baseUrl.baseUrl
  childEnv.PYLON_ASSIGNMENT_WORKER = "1"

  if (trimValue(childEnv.PYLON_OPENAGENTS_BASE_URL) === null) {
    childEnv.PYLON_OPENAGENTS_BASE_URL = "https://openagents.com"
  }

  const capacityRefs = [
    "capability.inference.apple_fm",
    `backend.${APPLE_FM_BACKEND_KIND}`,
    `model.${APPLE_FM_DEFAULT_MODEL_ID}`,
    `capacity.inference.apple_fm.ready=${appleFmReady && pylonReady ? 1 : 0}`,
    `capacity.inference.apple_fm.available=${appleFmReady && pylonReady ? 1 : 0}`,
    "load.inference.apple_fm.busy=0",
    "load.inference.apple_fm.queued=0",
    "demand.own_capacity",
    `demand.source.${KHALA_APPLE_FM_DEMAND_SOURCE}`,
  ]

  const blockerRefs = [
    ...(appleFmReady ? [] : ["blocker.khala_macos.apple_fm_bridge_unavailable"]),
    ...(pylonReady ? [] : ["blocker.khala_macos.pylon_unavailable"]),
  ]

  return {
    pylonMode,
    pylonCommand:
      pylonMode === "launch_embedded" ? [bunPath, pylonEntry, "node"] : [],
    pylonHome,
    appleFmBridgePath,
    appleFmBaseUrl: baseUrl.baseUrl,
    appleFmBaseUrlSource: baseUrl.source,
    childEnv,
    capacityRefs,
    blockerRefs,
    demandAttribution: localAppleFmDemandAttribution(),
  }
}
