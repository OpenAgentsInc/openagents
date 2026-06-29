// Tests for the consumer-install platform-support classifier + claim guard.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.windows_wsl_consumer_install_coverage_missing
import { describe, expect, it } from "bun:test"
import {
  classifyConsumerInstallHost,
  CONSUMER_INSTALL_SUPPORTED_TARGETS,
  classifyConsumerInstallPlatform,
  type ConsumerInstallPlatformClaim,
  detectWslHost,
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

describe("detectWslHost", () => {
  it("returns false for a clean (native) environment with no WSL signals", () => {
    expect(detectWslHost({})).toBe(false)
    expect(detectWslHost({ PATH: "/usr/bin", HOME: "/home/u" })).toBe(false)
  })

  it("detects WSL from each recognized environment signal", () => {
    expect(detectWslHost({ WSL_DISTRO_NAME: "Ubuntu" })).toBe(true)
    expect(detectWslHost({ WSL_INTEROP: "/run/WSL/8_interop" })).toBe(true)
    expect(detectWslHost({ WSLENV: "PATH/l" })).toBe(true)
  })

  it("ignores empty/whitespace-only WSL env values", () => {
    expect(detectWslHost({ WSL_DISTRO_NAME: "" })).toBe(false)
    expect(detectWslHost({ WSL_DISTRO_NAME: "   " })).toBe(false)
  })

  it("detects WSL from /proc/version text (microsoft / WSL2)", () => {
    expect(
      detectWslHost(
        {},
        "Linux version 5.15.0-microsoft-standard-WSL2 (...) #1 SMP",
      ),
    ).toBe(true)
    expect(detectWslHost({}, "Linux version 6.1.0-generic (...) #1 SMP")).toBe(false)
  })

  it("never returns environment values (returns a plain boolean)", () => {
    expect(typeof detectWslHost({ WSL_DISTRO_NAME: "secret-distro" })).toBe("boolean")
  })
})

describe("classifyConsumerInstallHost (WSL-aware)", () => {
  it("classifies a WSL host out-of-scope even though it reports platform linux", () => {
    const result = classifyConsumerInstallHost({ platform: "linux", wsl: true })
    expect(result.disposition).toBe("out-of-scope")
    expect(result.reasonRef).toBe("reason.platform.wsl_out_of_scope")
    expect(result.blockerRefs).toEqual([WINDOWS_WSL_BLOCKER_REF])
    expect(result.guidanceRefs).toContain(
      "guidance.platform.use_native_macos_or_linux_host_not_wsl",
    )
    expect(result.contentRedacted).toBe(true)
  })

  it("classifies native linux (no WSL signal) as supported", () => {
    const result = classifyConsumerInstallHost({ platform: "linux", wsl: false })
    expect(result.disposition).toBe("supported")
    expect(result.blockerRefs).toEqual([])
  })

  it("matches classifyConsumerInstallPlatform when no WSL signal is given", () => {
    for (const platform of ["darwin", "linux", "win32", "freebsd"] as const) {
      expect(classifyConsumerInstallHost({ platform })).toEqual(
        classifyConsumerInstallPlatform(platform),
      )
    }
  })

  it("never emits private/machine fields for a WSL host", () => {
    const result = classifyConsumerInstallHost({ platform: "linux", wsl: true })
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
