export const KHALA_CODE_ON_DEVICE_DECIDER_INTERFACE_VERSION =
  "khala-code-on-device-decider-v1" as const

export const KHALA_CODE_APPLE_FM_DECIDER_BACKEND_ID =
  "khala-code-apple-fm-decider" as const

export const KHALA_CODE_GPT_OSS_DECIDER_BACKEND_ID =
  "khala-code-gpt-oss-decider" as const

export type KhalaCodeOnDeviceDeciderPlatform =
  | "macos"
  | "ios"
  | "linux"
  | "windows"
  | "unsupported"

export type KhalaCodeOnDeviceDeciderBackendKind =
  | "apple_fm"
  | "self_hosted_gpt_oss"

export type KhalaCodeOnDeviceDeciderRequest = {
  readonly prompt: string
  readonly workspaceSummary?: string
  readonly candidateActions: ReadonlyArray<string>
}

export type KhalaCodeOnDeviceDeciderDecision = {
  readonly selectedAction: string | null
  readonly confidence: number
  readonly reason: string
}

export type KhalaCodeOnDeviceDeciderBackend = {
  readonly id: string
  readonly kind: KhalaCodeOnDeviceDeciderBackendKind
  readonly interfaceVersion: typeof KHALA_CODE_ON_DEVICE_DECIDER_INTERFACE_VERSION
  decide(
    request: KhalaCodeOnDeviceDeciderRequest,
  ): Promise<KhalaCodeOnDeviceDeciderDecision>
}

export type KhalaCodeOnDeviceDeciderSelectionInput = {
  readonly enabled?: boolean
  readonly platform: KhalaCodeOnDeviceDeciderPlatform | NodeJS.Platform | string
  readonly appleFmAvailable?: boolean
  readonly gptOssAvailable?: boolean
}

export type KhalaCodeOnDeviceDeciderBackendSelection = {
  readonly id:
    | typeof KHALA_CODE_APPLE_FM_DECIDER_BACKEND_ID
    | typeof KHALA_CODE_GPT_OSS_DECIDER_BACKEND_ID
  readonly kind: KhalaCodeOnDeviceDeciderBackendKind
  readonly interfaceVersion: typeof KHALA_CODE_ON_DEVICE_DECIDER_INTERFACE_VERSION
}

export type KhalaCodeOnDeviceDeciderSelection =
  | {
      readonly status: "disabled"
      readonly enabled: false
      readonly platform: KhalaCodeOnDeviceDeciderPlatform
      readonly backend: null
      readonly failSoft: true
      readonly blockerRefs: ReadonlyArray<string>
    }
  | {
      readonly status: "ready"
      readonly enabled: true
      readonly platform: KhalaCodeOnDeviceDeciderPlatform
      readonly backend: KhalaCodeOnDeviceDeciderBackendSelection
      readonly failSoft: true
      readonly blockerRefs: ReadonlyArray<string>
    }
  | {
      readonly status: "unavailable"
      readonly enabled: true
      readonly platform: KhalaCodeOnDeviceDeciderPlatform
      readonly backend: null
      readonly failSoft: true
      readonly blockerRefs: ReadonlyArray<string>
    }

export function normalizeKhalaCodeOnDeviceDeciderPlatform(
  platform: KhalaCodeOnDeviceDeciderSelectionInput["platform"],
): KhalaCodeOnDeviceDeciderPlatform {
  switch (platform) {
    case "darwin":
    case "macos":
      return "macos"
    case "ios":
      return "ios"
    case "linux":
      return "linux"
    case "win32":
    case "windows":
      return "windows"
    default:
      return "unsupported"
  }
}

export function selectKhalaCodeOnDeviceDecider(
  input: KhalaCodeOnDeviceDeciderSelectionInput,
): KhalaCodeOnDeviceDeciderSelection {
  const platform = normalizeKhalaCodeOnDeviceDeciderPlatform(input.platform)

  if (input.enabled !== true) {
    return {
      status: "disabled",
      enabled: false,
      platform,
      backend: null,
      failSoft: true,
      blockerRefs: ["blocker.khala_code.on_device_decider.disabled"],
    }
  }

  if (platform === "macos" || platform === "ios") {
    if (input.appleFmAvailable === true) {
      return {
        status: "ready",
        enabled: true,
        platform,
        backend: {
          id: KHALA_CODE_APPLE_FM_DECIDER_BACKEND_ID,
          kind: "apple_fm",
          interfaceVersion: KHALA_CODE_ON_DEVICE_DECIDER_INTERFACE_VERSION,
        },
        failSoft: true,
        blockerRefs: [],
      }
    }

    return {
      status: "unavailable",
      enabled: true,
      platform,
      backend: null,
      failSoft: true,
      blockerRefs: [
        "blocker.khala_code.on_device_decider.apple_fm_unavailable",
      ],
    }
  }

  if (platform === "linux" || platform === "windows") {
    if (input.gptOssAvailable === true) {
      return {
        status: "ready",
        enabled: true,
        platform,
        backend: {
          id: KHALA_CODE_GPT_OSS_DECIDER_BACKEND_ID,
          kind: "self_hosted_gpt_oss",
          interfaceVersion: KHALA_CODE_ON_DEVICE_DECIDER_INTERFACE_VERSION,
        },
        failSoft: true,
        blockerRefs: [],
      }
    }

    return {
      status: "unavailable",
      enabled: true,
      platform,
      backend: null,
      failSoft: true,
      blockerRefs: [
        "blocker.khala_code.on_device_decider.gpt_oss_unavailable",
      ],
    }
  }

  return {
    status: "unavailable",
    enabled: true,
    platform,
    backend: null,
    failSoft: true,
    blockerRefs: ["blocker.khala_code.on_device_decider.unsupported_platform"],
  }
}
