import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"

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
    expect(packagedAppleFmBridgePath("build/stable-macos-arm64/Khala Code.app")).toBe(
      "build/stable-macos-arm64/Khala Code.app/Contents/Resources/app/apple-fm-bridge/foundation-bridge",
    )
  })

  test("build script verifies the packaged helper after electrobun build", () => {
    const packageJson = JSON.parse(
      readFileSync(join(import.meta.dir, "..", "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> }
    const buildScript = packageJson.scripts?.build ?? ""

    expect(buildScript).toContain("electrobun build")
    expect(buildScript).toContain("bun run verify:apple-fm-bridge")
    expect(buildScript.indexOf("electrobun build")).toBeLessThan(
      buildScript.indexOf("bun run verify:apple-fm-bridge"),
    )
  })

  test("verifier accepts the first non-empty executable helper", () => {
    const result = verifyPackagedAppleFmBridge({ probe: healthy })
    expect(result.ok).toBe(true)
    expect(result.verifiedPath).toContain("Khala Code-dev.app")
  })

  test("verifier picks the first candidate that ships a usable helper", () => {
    const stableOnly: AppleFmBridgeProbe = (helperPath) =>
      helperPath.includes("stable-macos-arm64")
        ? { exists: true, nonEmpty: true, executable: true }
        : { exists: false, nonEmpty: false, executable: false }

    const result = verifyPackagedAppleFmBridge({ probe: stableOnly })

    expect(result.ok).toBe(true)
    expect(result.verifiedEnv).toBe("stable")
    expect(result.verifiedPath).toContain("Khala Code.app")
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

  test("failure reasons carry only structural bundle diagnostics", () => {
    const result = verifyPackagedAppleFmBridge({
      probe: () => ({ exists: false, nonEmpty: false, executable: false }),
    })

    expect(result.ok).toBe(false)
    for (const failure of result.failures) {
      expect(failure.bundleDir.startsWith("build/")).toBe(true)
      expect(failure.reason.includes("/")).toBe(false)
      expect(failure.reason).not.toContain("foundation-bridge")
    }
  })
})
