// Tests for the consumer-install platform-support classifier + claim guard.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.windows_wsl_consumer_install_coverage_missing
import { describe, expect, it } from "bun:test"
import {
  CONSUMER_INSTALL_SUPPORTED_TARGETS,
  classifyConsumerInstallPlatform,
  type ConsumerInstallPlatformClaim,
  verifyConsumerInstallPlatformClaim,
  WINDOWS_WSL_BLOCKER_REF,
} from "./consumer-install-platform-support.js"

function claim(
  overrides: Partial<ConsumerInstallPlatformClaim> = {},
): ConsumerInstallPlatformClaim {
  return {
    schema: "openagents.pylon.consumer_install_platform_claim.v0.1",
    supportedTargets: ["darwin", "linux"],
    windowsInScope: false,
    wslInScope: false,
    anyPlatformClaimed: false,
    ...overrides,
  }
}

describe("classifyConsumerInstallPlatform", () => {
  it("treats macOS and Linux as supported with no blocker ref", () => {
    for (const platform of CONSUMER_INSTALL_SUPPORTED_TARGETS) {
      const result = classifyConsumerInstallPlatform(platform)
      expect(result.disposition).toBe("supported")
      expect(result.blockerRefs).toEqual([])
      expect(result.guidanceRefs).toEqual([])
      expect(result.reasonRef).toBe("reason.platform.supported_target")
      expect(result.contentRedacted).toBe(true)
    }
  })

  it("treats native Windows as out-of-scope with honest guidance + blocker ref", () => {
    const result = classifyConsumerInstallPlatform("win32")
    expect(result.disposition).toBe("out-of-scope")
    expect(result.reasonRef).toBe("reason.platform.windows_out_of_scope")
    expect(result.blockerRefs).toEqual([WINDOWS_WSL_BLOCKER_REF])
    expect(result.guidanceRefs).toContain("doc.pylon.platform_support")
  })

  it("treats other platforms (e.g. freebsd) as out-of-scope", () => {
    const result = classifyConsumerInstallPlatform("freebsd")
    expect(result.disposition).toBe("out-of-scope")
    expect(result.reasonRef).toBe("reason.platform.unsupported_target")
    expect(result.blockerRefs).toEqual([WINDOWS_WSL_BLOCKER_REF])
  })

  it("never emits private/machine fields", () => {
    const result = classifyConsumerInstallPlatform("win32")
    expect(Object.keys(result).sort()).toEqual(
      [
        "blockerRefs",
        "contentRedacted",
        "disposition",
        "guidanceRefs",
        "platform",
        "reasonRef",
        "schema",
        "supportedTargets",
      ].sort(),
    )
  })
})

describe("verifyConsumerInstallPlatformClaim", () => {
  it("accepts an honest macOS/Linux-only claim without over-promising", () => {
    const result = verifyConsumerInstallPlatformClaim(claim())
    expect(result.valid).toBe(true)
    expect(result.overpromises).toBe(false)
    expect(result.reasons).toEqual([])
  })

  it("flags an any-platform claim as over-promising", () => {
    const result = verifyConsumerInstallPlatformClaim(claim({ anyPlatformClaimed: true }))
    expect(result.overpromises).toBe(true)
    expect(result.reasons).toContain("any-platform-claimed")
  })

  it("flags windows-in-scope and wsl-in-scope", () => {
    const result = verifyConsumerInstallPlatformClaim(
      claim({ windowsInScope: true, wslInScope: true }),
    )
    expect(result.overpromises).toBe(true)
    expect(result.reasons).toContain("windows-claimed-in-scope")
    expect(result.reasons).toContain("wsl-claimed-in-scope")
  })

  it("flags an out-of-scope token in the supported-target list", () => {
    const result = verifyConsumerInstallPlatformClaim(
      claim({ supportedTargets: ["darwin", "linux", "win32"] }),
    )
    expect(result.overpromises).toBe(true)
    expect(result.reasons).toContain("out-of-scope-target:win32")
  })

  it("flags a missing required target", () => {
    const result = verifyConsumerInstallPlatformClaim(
      claim({ supportedTargets: ["darwin"] }),
    )
    expect(result.overpromises).toBe(true)
    expect(result.reasons).toContain("missing-required-target:linux")
  })

  it("flags an unexpected extra target", () => {
    const result = verifyConsumerInstallPlatformClaim(
      claim({ supportedTargets: ["darwin", "linux", "android"] }),
    )
    expect(result.overpromises).toBe(true)
    expect(result.reasons).toContain("unexpected-extra-target:android")
  })

  it("is case-insensitive about target tokens", () => {
    const result = verifyConsumerInstallPlatformClaim(
      claim({ supportedTargets: ["Darwin", "LINUX", "Windows"] }),
    )
    expect(result.reasons).toContain("out-of-scope-target:windows")
  })

  it("rejects an unexpected key (closed allowlist)", () => {
    const result = verifyConsumerInstallPlatformClaim({
      ...claim(),
      rawHostId: "leak",
    })
    expect(result.valid).toBe(false)
    expect(result.overpromises).toBe(true)
    expect(result.reasons).toContain("unexpected-key:rawHostId")
  })

  it("rejects a bad schema and bad types", () => {
    const result = verifyConsumerInstallPlatformClaim({
      schema: "wrong",
      supportedTargets: "darwin",
      windowsInScope: "no",
      wslInScope: false,
      anyPlatformClaimed: false,
    })
    expect(result.valid).toBe(false)
    expect(result.reasons).toContain("bad-schema")
    expect(result.reasons).toContain("bad-supported-targets")
    expect(result.reasons).toContain("bad-windowsInScope")
  })

  it("rejects non-objects", () => {
    expect(verifyConsumerInstallPlatformClaim(null).valid).toBe(false)
    expect(verifyConsumerInstallPlatformClaim([]).overpromises).toBe(true)
    expect(verifyConsumerInstallPlatformClaim("x").reasons).toContain("not-an-object")
  })
})
