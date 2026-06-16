import { describe, expect, test } from "bun:test"
import {
  createBootstrapSummary,
  isSupportedPlatform,
  PYLON_DEFAULT_CAPABILITY_REFS,
  parseBootstrapArgs,
  resolvePylonHome,
} from "../src/bootstrap"

describe("Pylon bootstrap release surface", () => {
  test("supports macOS and Linux only for v0.3", () => {
    expect(isSupportedPlatform("darwin")).toBe(true)
    expect(isSupportedPlatform("linux")).toBe(true)
    expect(isSupportedPlatform("win32")).toBe(false)
  })

  test("parses launch bootstrap flags into a public-safe summary", () => {
    const options = parseBootstrapArgs([
      "--register-openagents",
      "--setup-mdk-wallet",
      "--pylon-ref",
      "pylon.local.test",
      "--display-name",
      "Local Test Pylon",
      "--resource-mode",
      "background_20",
      "--capability-ref",
      "cap.gepa.retained.v1",
      "--json",
    ])
    const summary = createBootstrapSummary(options, { PYLON_HOME: "/tmp/pylon-test" }, "darwin")

    expect(summary.packageName).toBe("@openagentsinc/pylon")
    expect(summary.version).toBe("1.0.0-rc.9")
    expect(summary.bin).toBe("pylon")
    expect(summary.platform.supportedTargets).toEqual(["darwin", "linux"])
    expect(summary.bootstrap.registerOpenAgents).toBe(true)
    expect(summary.bootstrap.setupMdkWallet).toBe(true)
    expect(summary.bootstrap.pylonRef).toBe("pylon.local.test")
    expect(summary.bootstrap.displayName).toBe("Local Test Pylon")
    expect(summary.bootstrap.capabilityRefs).toEqual(["cap.gepa.retained.v1", ...PYLON_DEFAULT_CAPABILITY_REFS])
    expect(summary.updatePolicy.sourceBuildFallback).toBe("disabled")
  })

  test("resolves deterministic home, config, cache, and release paths", () => {
    expect(resolvePylonHome({ PYLON_HOME: "/tmp/pylon-home" })).toEqual({
      home: "/tmp/pylon-home",
      config: "/tmp/pylon-home/config.json",
      cache: "/tmp/pylon-home/cache",
      releases: "/tmp/pylon-home/cache/releases",
    })
  })
})
