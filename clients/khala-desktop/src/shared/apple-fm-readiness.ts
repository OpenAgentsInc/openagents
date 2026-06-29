export const KHALA_APPLE_FM_READINESS_SCHEMA =
  "openagents.khala_desktop.apple_fm.readiness.v0.1" as const
export const APPLE_FM_BACKEND_KIND = "apple_fm_bridge" as const
export const APPLE_FM_LOCAL_PROFILE_ID = "apple-fm-local" as const
export const APPLE_FM_MODEL_ID = "apple-foundation-model" as const
export const APPLE_FM_CAPABILITY = "probe.backend.apple_fm_bridge" as const
export const KHALA_APPLE_FM_TOKEN_PROVIDER =
  "pylon-apple-fm-own-capacity" as const
export const KHALA_APPLE_FM_DEMAND_SOURCE =
  "khala_apple_fm_delegation" as const

export type KhalaAppleFmSidecarState =
  | "not_supported"
  | "helper_missing"
  | "launching"
  | "adopted"
  | "running"
  | "ready"
  | "unavailable"
  | "failed"
  | "stopped"

export type AppleFmRuntimePlatform = {
  readonly platform: string
  readonly arch: string
}

export type PylonAppleFmStatusPublicInput = {
  readonly [key: string]: unknown
  readonly available?: unknown
  readonly status?: unknown
  readonly backendKind?: unknown
  readonly profileId?: unknown
  readonly model?: unknown
  readonly capability?: unknown
  readonly advertisedCapabilities?: unknown
  readonly unavailableReason?: unknown
  readonly message?: unknown
  readonly blockerRefs?: unknown
  readonly supervisor?: unknown
}

export type SanitizedPylonAppleFmStatus = {
  readonly available: boolean
  readonly status: string
  readonly backendKind: string
  readonly profileId: string
  readonly model: string
  readonly capability: string
  readonly advertisedCapabilities: ReadonlyArray<string>
  readonly unavailableReason: string | null
  readonly message: string | null
  readonly blockerRefs: ReadonlyArray<string>
  readonly supervisor: {
    readonly health: string
    readonly phase: string
    readonly supervised: boolean
    readonly blockerRefs: ReadonlyArray<string>
    readonly contentRedacted: true
  } | null
}

export type KhalaAppleFmReadiness = {
  readonly schema: typeof KHALA_APPLE_FM_READINESS_SCHEMA
  readonly kind: "khala_desktop_apple_fm_readiness"
  readonly supported: boolean
  readonly available: boolean
  readonly state: KhalaAppleFmSidecarState
  readonly backendKind: typeof APPLE_FM_BACKEND_KIND
  readonly profileId: typeof APPLE_FM_LOCAL_PROFILE_ID
  readonly model: typeof APPLE_FM_MODEL_ID
  readonly capability: typeof APPLE_FM_CAPABILITY
  readonly provider: typeof KHALA_APPLE_FM_TOKEN_PROVIDER
  readonly demandKind: "own_capacity"
  readonly demandSource: typeof KHALA_APPLE_FM_DEMAND_SOURCE
  readonly usageTruth: "estimated"
  readonly pylonControlConfigured: boolean
  readonly pylon: SanitizedPylonAppleFmStatus | null
  readonly blockerRefs: ReadonlyArray<string>
  readonly observedAt: string
  readonly contentRedacted: true
}

export type BuildKhalaAppleFmReadinessInput = {
  readonly platform: AppleFmRuntimePlatform
  readonly helperFound: boolean
  readonly helperExecutable?: boolean
  readonly helperLaunchState?: "idle" | "launching" | "running" | "failed" | "stopped" | "adopted"
  readonly pylonControlConfigured?: boolean
  readonly pylonStatus?: PylonAppleFmStatusPublicInput | null
  readonly observedAt?: string
}

const stringArray = (value: unknown): ReadonlyArray<string> =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string" && item.trim() !== "")
    : []

const optionalString = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null

const requiredString = (value: unknown, fallback: string): string =>
  optionalString(value) ?? fallback

const asRecord = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}

export function appleFmSupportedOn(platform: AppleFmRuntimePlatform): boolean {
  return platform.platform === "darwin" && platform.arch === "arm64"
}

export function sanitizePylonAppleFmStatus(
  input: PylonAppleFmStatusPublicInput,
): SanitizedPylonAppleFmStatus {
  const supervisor = asRecord(input.supervisor)
  const supervisorStatus =
    Object.keys(supervisor).length === 0
      ? null
      : {
          health: requiredString(supervisor.health, "unknown"),
          phase: requiredString(supervisor.phase, "unknown"),
          supervised: supervisor.supervised === true,
          blockerRefs: stringArray(supervisor.blockerRefs),
          contentRedacted: true as const,
        }

  return {
    available: input.available === true,
    status: requiredString(input.status, input.available === true ? "ready" : "unavailable"),
    backendKind: requiredString(input.backendKind, APPLE_FM_BACKEND_KIND),
    profileId: requiredString(input.profileId, APPLE_FM_LOCAL_PROFILE_ID),
    model: requiredString(input.model, APPLE_FM_MODEL_ID),
    capability: requiredString(input.capability, APPLE_FM_CAPABILITY),
    advertisedCapabilities: stringArray(input.advertisedCapabilities),
    unavailableReason: optionalString(input.unavailableReason),
    message: optionalString(input.message),
    blockerRefs: stringArray(input.blockerRefs),
    supervisor: supervisorStatus,
  }
}

export function buildKhalaAppleFmReadiness(
  input: BuildKhalaAppleFmReadinessInput,
): KhalaAppleFmReadiness {
  const supported = appleFmSupportedOn(input.platform)
  const pylon =
    input.pylonStatus === null || input.pylonStatus === undefined
      ? null
      : sanitizePylonAppleFmStatus(input.pylonStatus)
  const pylonReady =
    pylon !== null &&
    pylon.available &&
    pylon.status === "ready" &&
    pylon.advertisedCapabilities.includes(pylon.capability) &&
    pylon.blockerRefs.length === 0
  const helperUsable = input.helperFound && input.helperExecutable !== false

  const blockers = new Set<string>()
  let state: KhalaAppleFmSidecarState = "unavailable"

  if (!supported) {
    state = "not_supported"
    blockers.add("blocker.khala_desktop.apple_fm.unsupported_platform")
  } else if (!input.helperFound) {
    state = "helper_missing"
    blockers.add("blocker.khala_desktop.apple_fm.helper_missing")
  } else if (!helperUsable) {
    state = "helper_missing"
    blockers.add("blocker.khala_desktop.apple_fm.helper_not_executable")
  } else if (pylonReady) {
    state = "ready"
  } else if (input.helperLaunchState === "adopted") {
    state = "adopted"
  } else if (input.helperLaunchState === "running") {
    state = "running"
  } else if (input.helperLaunchState === "launching") {
    state = "launching"
  } else if (input.helperLaunchState === "failed") {
    state = "failed"
    blockers.add("blocker.khala_desktop.apple_fm.sidecar_failed")
  } else if (input.helperLaunchState === "stopped") {
    state = "stopped"
    blockers.add("blocker.khala_desktop.apple_fm.sidecar_stopped")
  }

  if (supported && helperUsable && !pylonReady) {
    if (input.pylonControlConfigured === true) {
      blockers.add("blocker.khala_desktop.apple_fm.pylon_not_ready")
    } else {
      blockers.add("blocker.khala_desktop.apple_fm.pylon_control_unconfigured")
    }
  }

  for (const blockerRef of pylon?.blockerRefs ?? []) blockers.add(blockerRef)
  for (const blockerRef of pylon?.supervisor?.blockerRefs ?? []) blockers.add(blockerRef)

  return {
    schema: KHALA_APPLE_FM_READINESS_SCHEMA,
    kind: "khala_desktop_apple_fm_readiness",
    supported,
    available: state === "ready",
    state,
    backendKind: APPLE_FM_BACKEND_KIND,
    profileId: APPLE_FM_LOCAL_PROFILE_ID,
    model: APPLE_FM_MODEL_ID,
    capability: APPLE_FM_CAPABILITY,
    provider: KHALA_APPLE_FM_TOKEN_PROVIDER,
    demandKind: "own_capacity",
    demandSource: KHALA_APPLE_FM_DEMAND_SOURCE,
    usageTruth: "estimated",
    pylonControlConfigured: input.pylonControlConfigured === true,
    pylon,
    blockerRefs: [...blockers].sort(),
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  }
}
