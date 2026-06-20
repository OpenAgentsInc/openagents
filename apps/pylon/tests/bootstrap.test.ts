import { afterEach, describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  createBootstrapSummary,
  isSupportedPlatform,
  PYLON_DEFAULT_CAPABILITY_REFS,
  parseBootstrapArgs,
  resolvePylonHome,
  selectPylonHomeResolution,
} from "../src/bootstrap"
import { PYLON_VERSION } from "../src/version"

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
    // Track the authoritative version constant, not a hardcoded string that goes
    // stale on every cut (the rc.13 drift trap).
    expect(summary.version).toBe(PYLON_VERSION)
    expect(summary.bin).toBe("pylon")
    expect(summary.platform.supportedTargets).toEqual(["darwin", "linux"])
    expect(summary.bootstrap.registerOpenAgents).toBe(true)
    expect(summary.bootstrap.setupMdkWallet).toBe(true)
    expect(summary.bootstrap.pylonRef).toBe("pylon.local.test")
    expect(summary.bootstrap.displayName).toBe("Local Test Pylon")
    expect(summary.bootstrap.capabilityRefs).toEqual(["cap.gepa.retained.v1", ...PYLON_DEFAULT_CAPABILITY_REFS])
    expect(summary.updatePolicy.sourceBuildFallback).toBe("disabled")
  })

  // WSL reports `platform === "linux"`, so the raw `supported` check passes and a
  // WSL host would be silently treated as the proven `linux` target — directly
  // contradicting the documented macOS/Linux-only scope-out. The bootstrap
  // summary must surface WSL and gate self-serve install on `inScope`, not on the
  // raw `supported` flag.
  describe("WSL host scope-out (windows_wsl_consumer_install_coverage_missing)", () => {
    test("a WSL host (linux + WSL env signal) is detected and held out of scope", () => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--json"]),
        { PYLON_HOME: "/tmp/pylon-test", WSL_DISTRO_NAME: "Ubuntu" },
        "linux",
      )
      expect(summary.platform.current).toBe("linux")
      // Raw platform check still passes — that is exactly why `inScope` is needed.
      expect(summary.platform.supported).toBe(true)
      expect(summary.platform.wsl).toBe(true)
      expect(summary.platform.inScope).toBe(false)
    })

    test("a native linux host (no WSL signal) is in scope", () => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--json"]),
        { PYLON_HOME: "/tmp/pylon-test" },
        "linux",
      )
      expect(summary.platform.wsl).toBe(false)
      expect(summary.platform.inScope).toBe(true)
    })

    test("macOS is in scope and never flagged as WSL", () => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--json"]),
        { PYLON_HOME: "/tmp/pylon-test", WSL_DISTRO_NAME: "Ubuntu" },
        "darwin",
      )
      // The WSL env signal is ignored off-linux: WSL is a linux-userland concern.
      expect(summary.platform.wsl).toBe(false)
      expect(summary.platform.inScope).toBe(true)
    })

    test("native Windows is out of scope (not in scope, not WSL)", () => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--json"]),
        { PYLON_HOME: "/tmp/pylon-test" },
        "win32",
      )
      expect(summary.platform.supported).toBe(false)
      expect(summary.platform.wsl).toBe(false)
      expect(summary.platform.inScope).toBe(false)
    })
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

// Bug 1 (the Orwell report on v1.0.x): with no PYLON_HOME set, the CLI silently
// fell back to `~/.pylon`, a SEEDLESS home on his machine →
// seedPresent:false → daemonOnline:false → balanceSats:null. His real node home
// was `~/.openagents/pylon` (seed there → identitySource:
// historical_config_identity_path, daemon online, 5,672 sats). The CLI must
// AUTO-RESOLVE the seed-bearing home instead of defaulting to a bare `~/.pylon`.
describe("PYLON_HOME auto-resolution (Bug 1: Orwell wrong-home)", () => {
  const roots: string[] = []
  function fakeHome(): string {
    const dir = mkdtempSync(join(tmpdir(), "pylon-home-resolve-"))
    roots.push(dir)
    return dir
  }
  function seedHome(home: string): string {
    mkdirSync(home, { recursive: true })
    // PUBLIC-SAFE: write a non-secret marker file, NOT a real seed. Resolution
    // only tests for the file's presence; it never reads the contents.
    writeFileSync(join(home, "identity.mnemonic"), "test-marker-not-a-real-seed\n", { mode: 0o600 })
    return home
  }

  afterEach(() => {
    while (roots.length > 0) {
      try {
        rmSync(roots.pop()!, { recursive: true, force: true })
      } catch {
        // best-effort
      }
    }
  })

  test("an explicit PYLON_HOME always wins (override is never broken)", () => {
    const fake = fakeHome()
    seedHome(join(fake, ".openagents", "pylon")) // a seed elsewhere must not steal it
    const resolution = selectPylonHomeResolution({ PYLON_HOME: "/tmp/explicit-home" }, fake)
    expect(resolution.home).toBe("/tmp/explicit-home")
    expect(resolution.source).toBe("explicit_pylon_home")
  })

  test("with no PYLON_HOME, prefers the seed-bearing ~/.openagents/pylon over a bare ~/.pylon", () => {
    const fake = fakeHome()
    // The Orwell shape: ~/.pylon exists but is SEEDLESS; ~/.openagents/pylon has the seed.
    mkdirSync(join(fake, ".pylon"), { recursive: true })
    seedHome(join(fake, ".openagents", "pylon"))
    const resolution = selectPylonHomeResolution({}, fake)
    expect(resolution.home).toBe(join(fake, ".openagents", "pylon"))
    expect(resolution.source).toBe("discovered_openagents_pylon")
  })

  test("with no PYLON_HOME, uses ~/.pylon when ONLY it holds the seed", () => {
    const fake = fakeHome()
    seedHome(join(fake, ".pylon"))
    const resolution = selectPylonHomeResolution({}, fake)
    expect(resolution.home).toBe(join(fake, ".pylon"))
    expect(resolution.source).toBe("discovered_dot_pylon")
  })

  test("a fresh machine (no seed anywhere) defaults to ~/.openagents/pylon (colocated with the identity path)", () => {
    const fake = fakeHome()
    const resolution = selectPylonHomeResolution({}, fake)
    expect(resolution.home).toBe(join(fake, ".openagents", "pylon"))
    expect(resolution.source).toBe("legacy_default")
  })

  test("the public-safe source label never leaks the seed (path label only)", () => {
    const fake = fakeHome()
    seedHome(join(fake, ".openagents", "pylon"))
    const resolution = selectPylonHomeResolution({}, fake)
    // The label is a coarse provenance enum, not a path-with-contents.
    expect(["explicit_pylon_home", "discovered_openagents_pylon", "discovered_dot_pylon", "legacy_default"]).toContain(
      resolution.source,
    )
    expect(resolution.source).not.toContain("test-marker-not-a-real-seed")
  })
})
