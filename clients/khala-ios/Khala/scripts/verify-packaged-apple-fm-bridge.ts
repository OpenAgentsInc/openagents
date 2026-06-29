#!/usr/bin/env bun
/**
 * Pre-notarization gate for Khala macOS Apple FM support.
 *
 * Usage:
 *   bun scripts/verify-packaged-apple-fm-bridge.ts /path/to/Khala.app
 *
 * Exit 0 only when the built app contains a non-empty executable
 * `Contents/Resources/app/apple-fm-bridge/foundation-bridge` helper. Set
 * KHALA_SKIP_APPLE_FM_BRIDGE_CHECK=1 to accept an intentionally Apple-FM-less
 * build only when the app bundle contains the unavailable marker written by the
 * Xcode copy phase.
 */
import { existsSync, statSync } from "node:fs"
import {
  verifyPackagedKhalaAppleFmBridge,
  type KhalaAppleFmBridgeProbe,
  type KhalaAppleFmMarkerProbe,
} from "./khala-apple-fm-packaging"

const bundleDir = process.argv[2]

if (bundleDir === undefined || bundleDir.trim() === "") {
  console.error("Usage: bun scripts/verify-packaged-apple-fm-bridge.ts /path/to/Khala.app")
  process.exit(2)
}

const bridgeProbe: KhalaAppleFmBridgeProbe = (helperPath) => {
  if (!existsSync(helperPath)) {
    return { exists: false, nonEmpty: false, executable: false }
  }
  const stat = statSync(helperPath)
  return {
    exists: stat.isFile(),
    nonEmpty: stat.size > 0,
    executable: stat.isFile() && (stat.mode & 0o100) !== 0,
  }
}

const markerProbe: KhalaAppleFmMarkerProbe = (markerPath) => {
  if (!existsSync(markerPath)) {
    return { exists: false, nonEmpty: false }
  }
  const stat = statSync(markerPath)
  return {
    exists: stat.isFile(),
    nonEmpty: stat.size > 0,
  }
}

const result = verifyPackagedKhalaAppleFmBridge({
  bundleDir,
  probe: bridgeProbe,
  markerProbe,
  allowUnavailableMarker: process.env.KHALA_SKIP_APPLE_FM_BRIDGE_CHECK === "1",
})

if (result.ok && result.status === "available") {
  console.log(`Khala Apple FM bridge helper verified: ${result.verifiedPath}`)
  process.exit(0)
}

if (result.ok && result.status === "unavailable") {
  console.log(`Khala Apple FM unavailable marker verified: ${result.markerPath}`)
  process.exit(0)
}

console.error(
  [
    "Khala Apple FM bridge helper is missing or unusable.",
    "A signed macOS app that advertises Apple FM support must bundle the",
    "foundation-bridge helper before notarization.",
    `Checked: ${result.checkedPath}`,
    `Reason: ${result.reason}`,
  ].join("\n"),
)
process.exit(1)
