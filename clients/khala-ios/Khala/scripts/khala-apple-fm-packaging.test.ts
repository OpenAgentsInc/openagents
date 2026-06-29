import { describe, expect, test } from "bun:test"
import {
  KHALA_APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
  KHALA_APPLE_FM_UNAVAILABLE_MARKER_SUBPATH,
  packagedKhalaAppleFmBridgePath,
  packagedKhalaAppleFmUnavailableMarkerPath,
  verifyPackagedKhalaAppleFmBridge,
  type KhalaAppleFmBridgeProbe,
} from "./khala-apple-fm-packaging"

const healthy: KhalaAppleFmBridgeProbe = () => ({
  exists: true,
  nonEmpty: true,
  executable: true,
})

describe("Khala Apple FM packaging contract", () => {
  test("helper path matches the Pylon packaged-resource discovery path", () => {
    expect(KHALA_APPLE_FM_BRIDGE_RESOURCES_SUBPATH).toBe(
      "app/apple-fm-bridge/foundation-bridge",
    )
    expect(packagedKhalaAppleFmBridgePath("build/Release/Khala.app")).toBe(
      "build/Release/Khala.app/Contents/Resources/app/apple-fm-bridge/foundation-bridge",
    )
  })

  test("verifier accepts a non-empty executable helper", () => {
    const result = verifyPackagedKhalaAppleFmBridge({
      bundleDir: "build/Release/Khala.app",
      probe: healthy,
    })
    expect(result.ok).toBe(true)
    expect(result.status).toBe("available")
  })

  test("verifier rejects missing, empty, and non-executable helpers", () => {
    const missing = verifyPackagedKhalaAppleFmBridge({
      bundleDir: "build/Release/Khala.app",
      probe: () => ({ exists: false, nonEmpty: false, executable: false }),
    })
    expect(missing).toMatchObject({ ok: false, status: "missing" })

    const empty = verifyPackagedKhalaAppleFmBridge({
      bundleDir: "build/Release/Khala.app",
      probe: () => ({ exists: true, nonEmpty: false, executable: true }),
    })
    expect(empty).toMatchObject({ ok: false, status: "empty" })

    const notExecutable = verifyPackagedKhalaAppleFmBridge({
      bundleDir: "build/Release/Khala.app",
      probe: () => ({ exists: true, nonEmpty: true, executable: false }),
    })
    expect(notExecutable).toMatchObject({ ok: false, status: "not_executable" })
  })

  test("intentional Apple-FM-less builds require a marker", () => {
    expect(KHALA_APPLE_FM_UNAVAILABLE_MARKER_SUBPATH).toBe(
      "app/apple-fm-bridge/APPLE_FM_UNAVAILABLE.txt",
    )
    expect(packagedKhalaAppleFmUnavailableMarkerPath("build/Release/Khala.app")).toBe(
      "build/Release/Khala.app/Contents/Resources/app/apple-fm-bridge/APPLE_FM_UNAVAILABLE.txt",
    )

    const result = verifyPackagedKhalaAppleFmBridge({
      bundleDir: "build/Release/Khala.app",
      probe: () => ({ exists: false, nonEmpty: false, executable: false }),
      allowUnavailableMarker: true,
      markerProbe: () => ({ exists: true, nonEmpty: true }),
    })
    expect(result.ok).toBe(true)
    expect(result.status).toBe("unavailable")
  })

  test("allowing unavailable still fails without the explicit marker", () => {
    const result = verifyPackagedKhalaAppleFmBridge({
      bundleDir: "build/Release/Khala.app",
      probe: () => ({ exists: false, nonEmpty: false, executable: false }),
      allowUnavailableMarker: true,
      markerProbe: () => ({ exists: false, nonEmpty: false }),
    })
    expect(result).toMatchObject({ ok: false, status: "unmarked_unavailable" })
  })
})
