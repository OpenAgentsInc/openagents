import { describe, expect, test } from "bun:test"
import {
  APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST,
  APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
  PACKAGED_APP_BUNDLE_CANDIDATES,
  packagedAppleFmBridgePath,
  verifyPackagedAppleFmBridge,
  type AppleFmBridgeProbe,
} from "../src/shared/apple-fm-packaging"

const healthy: AppleFmBridgeProbe = () => ({
  exists: true,
  nonEmpty: true,
  executable: true,
})

describe("apple-fm packaging contract", () => {
  test("electrobun copy dest lands at the Pylon discovery sub-path", () => {
    // electrobun copies build.copy dests under <Resources>/app/, so the dest
    // joined onto `app/` must equal the Pylon packaged-resource sub-path.
    expect(`app/${APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST}`).toBe(
      APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
    )
  })

  test("packaged helper path is inside the .app bundle Resources", () => {
    const p = packagedAppleFmBridgePath("build/stable-macos-arm64/Autopilot.app")
    expect(p).toBe(
      "build/stable-macos-arm64/Autopilot.app/Contents/Resources/app/apple-fm-bridge/foundation-bridge",
    )
  })

  test("verifier accepts a non-empty executable helper in any candidate", () => {
    const result = verifyPackagedAppleFmBridge({ probe: healthy })
    expect(result.ok).toBe(true)
    expect(result.verifiedPath).toContain("apple-fm-bridge/foundation-bridge")
    expect(result.verifiedEnv).not.toBeNull()
  })

  test("verifier picks the first candidate that ships a usable helper", () => {
    const stableOnly: AppleFmBridgeProbe = (helperPath) =>
      helperPath.includes("stable-macos-arm64")
        ? { exists: true, nonEmpty: true, executable: true }
        : { exists: false, nonEmpty: false, executable: false }
    const result = verifyPackagedAppleFmBridge({ probe: stableOnly })
    expect(result.ok).toBe(true)
    expect(result.verifiedEnv).toBe("stable")
  })

  test("verifier rejects a missing helper with a secret-free reason", () => {
    const result = verifyPackagedAppleFmBridge({
      probe: () => ({ exists: false, nonEmpty: false, executable: false }),
    })
    expect(result.ok).toBe(false)
    expect(result.verifiedPath).toBeNull()
    expect(result.failures.length).toBe(PACKAGED_APP_BUNDLE_CANDIDATES.length)
    expect(result.failures.every((f) => f.reason === "helper missing")).toBe(true)
  })

  test("verifier rejects an empty placeholder helper", () => {
    const result = verifyPackagedAppleFmBridge({
      probe: () => ({ exists: true, nonEmpty: false, executable: false }),
    })
    expect(result.ok).toBe(false)
    expect(result.failures.every((f) => f.reason === "helper is empty")).toBe(true)
  })

  test("verifier rejects a non-executable helper", () => {
    const result = verifyPackagedAppleFmBridge({
      probe: () => ({ exists: true, nonEmpty: true, executable: false }),
    })
    expect(result.ok).toBe(false)
    expect(result.failures.every((f) => f.reason === "helper not executable")).toBe(true)
  })

  test("failure reasons carry no file contents or paths beyond the bundle dir", () => {
    const result = verifyPackagedAppleFmBridge({
      probe: () => ({ exists: false, nonEmpty: false, executable: false }),
    })
    for (const f of result.failures) {
      expect(f.bundleDir.startsWith("build/")).toBe(true)
      expect(f.reason.includes("/")).toBe(false)
    }
  })
})
