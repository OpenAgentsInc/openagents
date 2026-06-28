/**
 * Packaging contract for the Khala macOS Apple FM Foundation Models bridge.
 *
 * A signed Khala macOS app that advertises Apple FM support must ship the
 * `foundation-bridge` helper at the same packaged-resource path Pylon uses:
 *
 *   <Khala.app>/Contents/Resources/app/apple-fm-bridge/foundation-bridge
 *
 * The verifier is intentionally structural: it checks presence, non-empty file
 * size, and owner-executable bit without reading file contents.
 */

export const KHALA_APPLE_FM_BRIDGE_HELPER_BASENAME = "foundation-bridge" as const

export const KHALA_APPLE_FM_BRIDGE_RESOURCES_SUBPATH =
  "app/apple-fm-bridge/foundation-bridge" as const

export const KHALA_APPLE_FM_UNAVAILABLE_MARKER_SUBPATH =
  "app/apple-fm-bridge/APPLE_FM_UNAVAILABLE.txt" as const

const MACOS_APP_RESOURCES_RELATIVE = "Contents/Resources" as const

function joinPosix(...segments: ReadonlyArray<string>): string {
  return segments
    .map((segment, index) =>
      index === 0 ? segment.replace(/\/+$/, "") : segment.replace(/^\/+|\/+$/g, ""),
    )
    .filter((segment) => segment.length > 0)
    .join("/")
}

export function packagedKhalaAppleFmBridgePath(bundleDir: string): string {
  return joinPosix(
    bundleDir,
    MACOS_APP_RESOURCES_RELATIVE,
    KHALA_APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
  )
}

export function packagedKhalaAppleFmUnavailableMarkerPath(bundleDir: string): string {
  return joinPosix(
    bundleDir,
    MACOS_APP_RESOURCES_RELATIVE,
    KHALA_APPLE_FM_UNAVAILABLE_MARKER_SUBPATH,
  )
}

export type KhalaAppleFmProbeResult = {
  readonly exists: boolean
  readonly nonEmpty: boolean
  readonly executable: boolean
}

export type KhalaAppleFmMarkerProbeResult = {
  readonly exists: boolean
  readonly nonEmpty: boolean
}

export type KhalaAppleFmBridgeProbe = (helperPath: string) => KhalaAppleFmProbeResult
export type KhalaAppleFmMarkerProbe = (markerPath: string) => KhalaAppleFmMarkerProbeResult

export type VerifyPackagedKhalaAppleFmBridgeInput = {
  readonly bundleDir: string
  readonly probe: KhalaAppleFmBridgeProbe
  readonly markerProbe?: KhalaAppleFmMarkerProbe
  readonly allowUnavailableMarker?: boolean
}

export type VerifyPackagedKhalaAppleFmBridgeResult =
  | {
      readonly ok: true
      readonly status: "available"
      readonly verifiedPath: string
    }
  | {
      readonly ok: true
      readonly status: "unavailable"
      readonly markerPath: string
    }
  | {
      readonly ok: false
      readonly status: "missing" | "empty" | "not_executable" | "unmarked_unavailable"
      readonly checkedPath: string
      readonly reason: string
    }

export function verifyPackagedKhalaAppleFmBridge(
  input: VerifyPackagedKhalaAppleFmBridgeInput,
): VerifyPackagedKhalaAppleFmBridgeResult {
  const helperPath = packagedKhalaAppleFmBridgePath(input.bundleDir)
  const result = input.probe(helperPath)

  if (result.exists && !result.nonEmpty) {
    return {
      ok: false,
      status: "empty",
      checkedPath: helperPath,
      reason: "helper is empty",
    }
  }

  if (result.exists && !result.executable) {
    return {
      ok: false,
      status: "not_executable",
      checkedPath: helperPath,
      reason: "helper not executable",
    }
  }

  if (result.exists) {
    return {
      ok: true,
      status: "available",
      verifiedPath: helperPath,
    }
  }

  if (input.allowUnavailableMarker === true && input.markerProbe !== undefined) {
    const markerPath = packagedKhalaAppleFmUnavailableMarkerPath(input.bundleDir)
    const marker = input.markerProbe(markerPath)
    if (marker.exists && marker.nonEmpty) {
      return {
        ok: true,
        status: "unavailable",
        markerPath,
      }
    }
    return {
      ok: false,
      status: "unmarked_unavailable",
      checkedPath: markerPath,
      reason: "Apple FM unavailable marker missing or empty",
    }
  }

  return {
    ok: false,
    status: "missing",
    checkedPath: helperPath,
    reason: "helper missing",
  }
}
