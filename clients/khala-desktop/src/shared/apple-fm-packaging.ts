export const APPLE_FM_BRIDGE_HELPER_BASENAME = "foundation-bridge" as const

export const APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST =
  "apple-fm-bridge/foundation-bridge" as const

export const APPLE_FM_BRIDGE_RESOURCES_SUBPATH =
  "app/apple-fm-bridge/foundation-bridge" as const

const MACOS_APP_RESOURCES_RELATIVE = "Contents/Resources" as const

export const KHALA_DESKTOP_PACKAGED_APP_BUNDLE_CANDIDATES: ReadonlyArray<{
  readonly env: "dev" | "canary" | "stable"
  readonly bundleDir: string
}> = [
  { env: "dev", bundleDir: "build/dev-macos-arm64/Khala-dev.app" },
  { env: "canary", bundleDir: "build/canary-macos-arm64/Khala-canary.app" },
  { env: "stable", bundleDir: "build/stable-macos-arm64/Khala.app" },
]

export type AppleFmBridgeProbeResult = {
  readonly executable: boolean
  readonly exists: boolean
  readonly nativeExecutable: boolean
  readonly nonEmpty: boolean
}

export type AppleFmBridgeProbe = (helperPath: string) => AppleFmBridgeProbeResult

export type VerifyPackagedAppleFmBridgeInput = {
  readonly candidates?: ReadonlyArray<{ readonly env: string; readonly bundleDir: string }>
  readonly probe: AppleFmBridgeProbe
}

export type VerifyPackagedAppleFmBridgeResult = {
  readonly failures: ReadonlyArray<{ readonly bundleDir: string; readonly reason: string }>
  readonly ok: boolean
  readonly verifiedEnv: string | null
  readonly verifiedPath: string | null
}

const joinPosix = (...segments: ReadonlyArray<string>): string =>
  segments
    .map((segment, index) =>
      index === 0
        ? segment.replace(/\/+$/, "")
        : segment.replace(/^\/+|\/+$/g, ""),
    )
    .filter((segment) => segment.length > 0)
    .join("/")

export const packagedAppleFmBridgePath = (bundleDir: string): string =>
  joinPosix(bundleDir, MACOS_APP_RESOURCES_RELATIVE, APPLE_FM_BRIDGE_RESOURCES_SUBPATH)

export const verifyPackagedAppleFmBridge = (
  input: VerifyPackagedAppleFmBridgeInput,
): VerifyPackagedAppleFmBridgeResult => {
  const candidates = input.candidates ?? KHALA_DESKTOP_PACKAGED_APP_BUNDLE_CANDIDATES
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
    if (!result.nativeExecutable) {
      failures.push({ bundleDir: candidate.bundleDir, reason: "helper is not a native Mach-O executable" })
      continue
    }

    return {
      failures: [],
      ok: true,
      verifiedEnv: candidate.env,
      verifiedPath: helperPath,
    }
  }

  return {
    failures,
    ok: false,
    verifiedEnv: null,
    verifiedPath: null,
  }
}
