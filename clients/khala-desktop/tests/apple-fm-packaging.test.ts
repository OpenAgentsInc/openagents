import { describe, expect, test } from "bun:test"

import {
  APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST,
  APPLE_FM_BRIDGE_ELECTROBUN_COPY_SOURCE,
  APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
  KHALA_PACKAGED_APP_BUNDLE_CANDIDATES,
  packagedAppleFmBridgePath,
  verifyPackagedAppleFmBridge,
  type AppleFmBridgeProbe,
} from "../src/shared/apple-fm-packaging.js"

const healthy: AppleFmBridgeProbe = () => ({
  exists: true,
  nonEmpty: true,
  executable: true,
})

describe("khala desktop Apple FM packaging", () => {
  test("electrobun copy source and destination land at Pylon's resource path", () => {
    expect(APPLE_FM_BRIDGE_ELECTROBUN_COPY_SOURCE).toBe(
      "resources/apple-fm-bridge/foundation-bridge",
    )
    expect(`app/${APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST}`).toBe(
      APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
    )
  })

  test("packaged helper path stays inside the Khala app bundle", () => {
    expect(packagedAppleFmBridgePath("build/stable-macos-arm64/Khala.app")).toBe(
      "build/stable-macos-arm64/Khala.app/Contents/Resources/app/apple-fm-bridge/foundation-bridge",
    )
  })

  test("verifier accepts the first non-empty executable helper", () => {
    const result = verifyPackagedAppleFmBridge({ probe: healthy })
    expect(result.ok).toBe(true)
    expect(result.verifiedPath).toContain("Khala-dev.app")
  })

  test("verifier rejects missing, empty, and non-executable helpers", () => {
    const missing = verifyPackagedAppleFmBridge({
      probe: () => ({ exists: false, nonEmpty: false, executable: false }),
    })
    expect(missing.ok).toBe(false)
    expect(missing.failures).toHaveLength(KHALA_PACKAGED_APP_BUNDLE_CANDIDATES.length)
    expect(missing.failures.every((failure) => failure.reason === "helper missing")).toBe(true)

    const empty = verifyPackagedAppleFmBridge({
      probe: () => ({ exists: true, nonEmpty: false, executable: true }),
    })
    expect(empty.ok).toBe(false)
    expect(empty.failures.every((failure) => failure.reason === "helper is empty")).toBe(true)

    const nonExecutable = verifyPackagedAppleFmBridge({
      probe: () => ({ exists: true, nonEmpty: true, executable: false }),
    })
    expect(nonExecutable.ok).toBe(false)
    expect(
      nonExecutable.failures.every((failure) => failure.reason === "helper not executable"),
    ).toBe(true)
  })
})
