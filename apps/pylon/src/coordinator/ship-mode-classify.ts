import type { ShipMode } from "./ship-mode.js"

export type ShipModeFingerprintInput = {
  previousRuntimeFingerprint: string
  nextRuntimeFingerprint: string
  changedPaths: string[]
}

export type FingerprintShipMode = Extract<ShipMode, "ota" | "rebuild">

export type ShipModeFingerprintResult = {
  mode: FingerprintShipMode
  reason: string
}

export function classifyShipModeFromFingerprint(
  input: ShipModeFingerprintInput,
): ShipModeFingerprintResult {
  const nativeConfigPath = input.changedPaths.find(isNativeConfigPath)

  if (nativeConfigPath !== undefined) {
    return {
      mode: "rebuild",
      reason: `Native/config path changed (${nativeConfigPath}); a new native build is required.`,
    }
  }

  if (input.previousRuntimeFingerprint !== input.nextRuntimeFingerprint) {
    return {
      mode: "rebuild",
      reason: `Runtime fingerprint changed from ${input.previousRuntimeFingerprint} to ${input.nextRuntimeFingerprint}; a new native build is required.`,
    }
  }

  return {
    mode: "ota",
    reason: `Runtime fingerprint ${input.nextRuntimeFingerprint} is unchanged; JS-only changes can ship OTA.`,
  }
}

function isNativeConfigPath(path: string): boolean {
  const normalized = path.replaceAll("\\", "/").replace(/^\.\/+/, "")
  const fileName = normalized.split("/").at(-1) ?? normalized

  return (
    normalized === "ios" ||
    normalized.startsWith("ios/") ||
    normalized === "android" ||
    normalized.startsWith("android/") ||
    fileName.startsWith("app.config") ||
    fileName === "package.json" ||
    fileName.endsWith(".podspec") ||
    fileName.startsWith("babel.config")
  )
}
