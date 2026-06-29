export const APPLE_FM_BRIDGE_HELPER_BASENAME = "foundation-bridge" as const
export const APPLE_FM_BRIDGE_ELECTROBUN_COPY_SOURCE =
  "resources/apple-fm-bridge/foundation-bridge" as const
export const APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST =
  "apple-fm-bridge/foundation-bridge" as const
export const APPLE_FM_BRIDGE_RESOURCES_SUBPATH =
  "app/apple-fm-bridge/foundation-bridge" as const

const MACOS_APP_RESOURCES_RELATIVE = "Contents/Resources" as const

export const KHALA_PACKAGED_APP_BUNDLE_CANDIDATES: ReadonlyArray<{
  readonly env: "dev" | "canary" | "stable"
  readonly bundleDir: string
}> = [
  { env: "dev", bundleDir: "build/dev-macos-arm64/Khala-dev.app" },
  { env: "canary", bundleDir: "build/canary-macos-arm64/Khala-canary.app" },
  { env: "stable", bundleDir: "build/stable-macos-arm64/Khala.app" },
]

function joinPosix(...segments: ReadonlyArray<string>): string {
  return segments
    .map((segment, index) =>
      index === 0
        ? segment.replace(/\/+$/, "")
        : segment.replace(/^\/+|\/+$/g, ""),
    )
    .filter((segment) => segment.length > 0)
    .join("/")
}

export function packagedAppleFmBridgePath(bundleDir: string): string {
  return joinPosix(
    bundleDir,
    MACOS_APP_RESOURCES_RELATIVE,
    APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
  )
}

export type AppleFmBridgeProbeResult = {
  readonly exists: boolean
  readonly nonEmpty: boolean
  readonly executable: boolean
}

export type AppleFmBridgeProbe = (helperPath: string) => AppleFmBridgeProbeResult

export type VerifyPackagedAppleFmBridgeInput = {
  readonly candidates?: ReadonlyArray<{ readonly env: string; readonly bundleDir: string }>
  readonly probe: AppleFmBridgeProbe
}

export type VerifyPackagedAppleFmBridgeResult = {
  readonly ok: boolean
  readonly verifiedPath: string | null
  readonly verifiedEnv: string | null
  readonly failures: ReadonlyArray<{ readonly bundleDir: string; readonly reason: string }>
}

export function verifyPackagedAppleFmBridge(
  input: VerifyPackagedAppleFmBridgeInput,
): VerifyPackagedAppleFmBridgeResult {
  const candidates = input.candidates ?? KHALA_PACKAGED_APP_BUNDLE_CANDIDATES
  const failures: Array<{ readonly bundleDir: string; readonly reason: string }> = []

  for (const candidate of candidates) {
    const helperPath = packagedAppleFmBridgePath(candidate.bundleDir)
    const result = input.probe(helperPath)
    if (!result.exists) {
      failures.push({ bundleDir: candidate.bundleDir, reason: "helper missing" })
      continue
    }
    if (!result.nonEmpty) {
      failures.push({ bundleDir: candidate.bundleDir, reason: "helper is empty" })
      continue
    }
    if (!result.executable) {
      failures.push({ bundleDir: candidate.bundleDir, reason: "helper not executable" })
      continue
    }
    return {
      ok: true,
      verifiedPath: helperPath,
      verifiedEnv: candidate.env,
      failures: [],
    }
  }

  return { ok: false, verifiedPath: null, verifiedEnv: null, failures }
}
