import { afterAll, beforeAll, describe, expect, it } from "bun:test"
import { execFileSync } from "node:child_process"
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  ensureManagedNode,
  findDevPylonEntry,
  findPackagedPylonEntry,
  superviseManagedNode,
  type LaunchedProcess,
  type NodeLaunchStatus,
  type SpawnNodeInput,
} from "../src/bun/node-launcher"

const repoRoot = "/work/openagents"
const pylonEntry = join(repoRoot, "apps", "pylon", "src", "index.ts")
const deepCwd = join(repoRoot, "apps", "autopilot-desktop")

describe("findDevPylonEntry", () => {
  it("walks up from cwd to the repo's pylon entrypoint", () => {
    const found = findDevPylonEntry(deepCwd, path => path === pylonEntry)
    expect(found).toEqual({ entry: pylonEntry, repoRoot })
  })

  it("returns null when no pylon entry is reachable (packaged build)", () => {
    expect(findDevPylonEntry("/Applications/Autopilot.app", () => false)).toBeNull()
  })
})

describe("findPackagedPylonEntry", () => {
  const resourcesDir = "/Applications/Autopilot.app/Contents/Resources"
  const bundleDir = join(resourcesDir, "app", "pylon-node")
  const jsEntry = join(bundleDir, "index.js")
  const tsEntry = join(bundleDir, "index.ts")

  it("prefers a prebuilt single-file index.js bundle", () => {
    const found = findPackagedPylonEntry(resourcesDir, path => path === jsEntry)
    expect(found).toEqual({ entry: jsEntry, bundleDir })
  })

  it("falls back to a TS source entry when no index.js is shipped", () => {
    const found = findPackagedPylonEntry(resourcesDir, path => path === tsEntry)
    expect(found).toEqual({ entry: tsEntry, bundleDir })
  })

  it("returns null when no bundle is shipped (early/unsigned build)", () => {
    expect(findPackagedPylonEntry(resourcesDir, () => false)).toBeNull()
  })

  it("returns null when there is no resources dir (dev/test run)", () => {
    expect(findPackagedPylonEntry(null, () => true)).toBeNull()
    expect(findPackagedPylonEntry("", () => true)).toBeNull()
  })
})

// #5027 (Phase 2): the empirical packaged layout. electrobun copies each
// `build.copy` dest under `<RESOURCES_FOLDER>/app/`, and at runtime
// `PATHS.RESOURCES_FOLDER` resolves to `<App>.app/Contents/Resources`. A `copy`
// dest of `"pylon-node/index.js"` therefore lands at
// `Contents/Resources/app/pylon-node/index.js`, which is exactly
// `join(RESOURCES_FOLDER, PACKAGED_PYLON_DIR, "index.js")` — the path
// `findPackagedPylonEntry` resolves. These tests prove that end to end against
// the REAL filesystem (default `existsSync`), not a mocked predicate:
//   1. a built-by-hand fixture mirroring the packaged tree, and
//   2. when present, the actual `electrobun build` artifact (its `.tar.zst`
//      extracted), so a layout/path drift in either electrobun or the launcher
//      is caught.
describe("findPackagedPylonEntry against a real packaged layout", () => {
  let tmpRoot: string

  beforeAll(() => {
    tmpRoot = mkdtempSync(join(tmpdir(), "autopilot-desktop-pkg-"))
  })

  afterAll(() => {
    rmSync(tmpRoot, { recursive: true, force: true })
  })

  it("resolves the bundled entry in a fixture mirroring Resources/app/pylon-node", () => {
    // Mirror what electrobun produces: <Resources>/app/pylon-node/index.js.
    const resourcesDir = join(tmpRoot, "Contents", "Resources")
    const bundleDir = join(resourcesDir, "app", "pylon-node")
    mkdirSync(bundleDir, { recursive: true })
    const entry = join(bundleDir, "index.js")
    writeFileSync(entry, "// bundled headless pylon node\n")

    // Default fileExists (real existsSync) — proves the on-disk layout matches
    // what PACKAGED_PYLON_DIR expects relative to RESOURCES_FOLDER.
    const found = findPackagedPylonEntry(resourcesDir)
    expect(found).toEqual({ entry, bundleDir })
  })

  // The actual `bun run build:canary` artifact, when it has been built. The
  // launcher must resolve the entry inside the real `.app` payload. We extract
  // the self-extracting `.tar.zst` (the runtime-installed tree) into a temp dir
  // and point findPackagedPylonEntry at its Resources, with default existsSync.
  const builtAppDir = join(
    import.meta.dir,
    "..",
    "build",
    "canary-macos-arm64",
    "Autopilot-canary.app",
    "Contents",
    "Resources",
  )
  const builtTarball = existsSync(builtAppDir)
    ? // find the single *.tar.zst payload next to metadata.json
      (() => {
        try {
          const out = execFileSync("/bin/sh", [
            "-c",
            `ls "${builtAppDir}"/*.tar.zst 2>/dev/null | head -1`,
          ])
            .toString()
            .trim()
          return out.length > 0 ? out : null
        } catch {
          return null
        }
      })()
    : null

  const itIfBuilt = builtTarball ? it : it.skip
  itIfBuilt(
    "resolves the bundled entry inside the real electrobun build artifact",
    () => {
      const extractDir = join(tmpRoot, "extracted")
      mkdirSync(extractDir, { recursive: true })
      // Extract the self-extracting payload to the on-disk tree the runtime sees.
      execFileSync("/usr/bin/tar", [
        "--zstd",
        "-xf",
        builtTarball as string,
        "-C",
        extractDir,
      ])
      const resourcesDir = join(
        extractDir,
        "Autopilot-canary.app",
        "Contents",
        "Resources",
      )
      const found = findPackagedPylonEntry(resourcesDir)
      expect(found).not.toBeNull()
      expect(found?.entry).toBe(
        join(resourcesDir, "app", "pylon-node", "index.js"),
      )
      expect(existsSync(found?.entry as string)).toBe(true)
    },
  )
})

describe("ensureManagedNode", () => {
  const baseOptions = {
    controlBaseUrl: "http://127.0.0.1:4716",
    cwd: deepCwd,
    env: {} as Record<string, string | undefined>,
    fileExists: (path: string) => path === pylonEntry,
    probeReady: async () => true,
    probeCompatible: async () => true,
    sleep: async () => {},
    bunBin: "/usr/bin/bun",
  }

  it("adopts an already-running node and never spawns", async () => {
    let spawned = 0
    const node = await ensureManagedNode({
      ...baseOptions,
      discover: () => "/work/openagents/.pylon-tailnet",
      spawnNode: () => {
        spawned += 1
        return { pid: 1, kill: () => {} }
      },
    })

    expect(node.mode).toBe("adopted")
    expect(node.home).toBe("/work/openagents/.pylon-tailnet")
    expect(node.pid).toBeNull()
    expect(spawned).toBe(0)
    // stop() is a no-op for an adopted node.
    expect(() => node.stop()).not.toThrow()
  })

  it("refuses to adopt an incompatible already-running node", async () => {
    let spawned = 0
    const statuses: NodeLaunchStatus[] = []
    const node = await ensureManagedNode({
      ...baseOptions,
      discover: () => "/work/openagents/.pylon-tailnet",
      probeCompatible: async () => false,
      spawnNode: () => {
        spawned += 1
        return { pid: 1, kill: () => {} }
      },
      onStatus: status => statuses.push(status),
    })

    expect(node.mode).toBe("unavailable")
    expect(node.home).toBeNull()
    expect(spawned).toBe(0)
    expect(statuses).toEqual(["failed"])
  })

  it("launches the repo node into a discoverable .pylon-local home when none is found", async () => {
    const spawnCalls: SpawnNodeInput[] = []
    let killed = 0
    const spawnNode = (input: SpawnNodeInput): LaunchedProcess => {
      spawnCalls.push(input)
      return { pid: 4242, kill: () => (killed += 1) }
    }
    const node = await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      readToken: () => "deadbeef",
      spawnNode,
    })

    expect(node.mode).toBe("launched")
    expect(node.home).toBe(join(repoRoot, ".pylon-local"))
    expect(node.pid).toBe(4242)
    expect(spawnCalls).toHaveLength(1)
    expect(spawnCalls[0]!.command).toEqual(["/usr/bin/bun", pylonEntry])
    expect(spawnCalls[0]!.cwd).toBe(repoRoot)
    // The managed home is forced so discovery picks the launched node up.
    expect(spawnCalls[0]!.env.PYLON_HOME).toBe(join(repoRoot, ".pylon-local"))

    // stop() kills only a node we launched.
    node.stop()
    expect(killed).toBe(1)
  })

  it("stays discover-only (unavailable) with no repo entry and no shipped bundle", async () => {
    let spawned = 0
    const node = await ensureManagedNode({
      ...baseOptions,
      cwd: "/Applications/Autopilot.app",
      fileExists: () => false,
      discover: () => null,
      spawnNode: () => {
        spawned += 1
        return { pid: 1, kill: () => {} }
      },
    })

    expect(node.mode).toBe("unavailable")
    expect(node.home).toBeNull()
    expect(spawned).toBe(0)
  })

  it("#5027: launches the bundled headless node in a packaged build (no repo entry)", async () => {
    const resourcesDir = "/Applications/Autopilot.app/Contents/Resources"
    const bundleDir = join(resourcesDir, "app", "pylon-node")
    const bundledEntry = join(bundleDir, "index.js")
    const homeDir = "/Users/contributor"
    const expectedHome = join(
      homeDir,
      ".openagents",
      "autopilot-desktop",
      ".pylon-local",
    )
    const spawnCalls: SpawnNodeInput[] = []
    let killed = 0
    const node = await ensureManagedNode({
      ...baseOptions,
      // The bundled-Bun path inside the .app.
      bunBin: "/Applications/Autopilot.app/Contents/MacOS/bun",
      cwd: "/Applications/Autopilot.app",
      // No repo entry; only the bundled entry exists.
      fileExists: (path: string) => path === bundledEntry,
      resourcesDir,
      homeDir,
      discover: () => null,
      readToken: () => "deadbeef",
      spawnNode: (input: SpawnNodeInput): LaunchedProcess => {
        spawnCalls.push(input)
        return { pid: 5151, kill: () => (killed += 1) }
      },
    })

    expect(node.mode).toBe("launched")
    expect(node.home).toBe(expectedHome)
    expect(node.pid).toBe(5151)
    expect(spawnCalls).toHaveLength(1)
    // Launched with the bundled Bun, the bundled entry, in headless `node` mode.
    expect(spawnCalls[0]!.command).toEqual([
      "/Applications/Autopilot.app/Contents/MacOS/bun",
      bundledEntry,
      "node",
    ])
    // cwd is the bundle dir; the managed home is forced under the user's home so
    // discovery + the poller pick the launched node up unchanged.
    expect(spawnCalls[0]!.cwd).toBe(bundleDir)
    expect(spawnCalls[0]!.env.PYLON_HOME).toBe(expectedHome)

    node.stop()
    expect(killed).toBe(1)
  })

  it("#5027: prefers the dev repo entry over a bundled entry when both exist", async () => {
    const resourcesDir = "/repo/app/Resources"
    const bundledEntry = join(resourcesDir, "app", "pylon-node", "index.js")
    const spawnCalls: SpawnNodeInput[] = []
    const node = await ensureManagedNode({
      ...baseOptions,
      // Both the dev pylon entry and a bundled entry resolve.
      fileExists: (path: string) => path === pylonEntry || path === bundledEntry,
      resourcesDir,
      discover: () => null,
      readToken: () => "tok",
      spawnNode: (input: SpawnNodeInput): LaunchedProcess => {
        spawnCalls.push(input)
        return { pid: 1, kill: () => {} }
      },
    })

    expect(node.mode).toBe("launched")
    // Dev path wins: repo entry + default args (no trailing "node"), repo home.
    expect(node.home).toBe(join(repoRoot, ".pylon-local"))
    expect(spawnCalls[0]!.command).toEqual(["/usr/bin/bun", pylonEntry])
  })

  it("waits for the control token + a reachable control server before returning launched", async () => {
    let tokenReads = 0
    let probes = 0
    const node = await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      // token appears on the 3rd read.
      readToken: () => (++tokenReads >= 3 ? "tok" : null),
      probeReady: async () => {
        probes += 1
        return true
      },
      spawnNode: () => ({ pid: 7, kill: () => {} }),
      readinessIntervalMs: 1,
      readinessTimeoutMs: 1_000,
    })

    expect(node.mode).toBe("launched")
    expect(tokenReads).toBeGreaterThanOrEqual(3)
    expect(probes).toBeGreaterThanOrEqual(1)
  })

  it("emits honest launching -> online when the node comes up", async () => {
    const statuses: NodeLaunchStatus[] = []
    await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      readToken: () => "tok",
      probeReady: async () => true,
      spawnNode: () => ({ pid: 1, kill: () => {} }),
      onStatus: status => statuses.push(status),
    })
    expect(statuses).toEqual(["launching", "online"])
  })

  it("emits honest launching -> failed on readiness timeout (no fake online)", async () => {
    const statuses: NodeLaunchStatus[] = []
    const node = await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      // Token never lands -> readiness never satisfied -> failed, but the child
      // is still returned as launched so the caller can supervise/stop it.
      readToken: () => null,
      probeReady: async () => false,
      spawnNode: () => ({ pid: 1, kill: () => {} }),
      readinessIntervalMs: 1,
      readinessTimeoutMs: 5,
      onStatus: status => statuses.push(status),
    })
    expect(statuses).toEqual(["launching", "failed"])
    expect(node.mode).toBe("launched")
  })

  it("emits adopted (and never launching) for an already-running node", async () => {
    const statuses: NodeLaunchStatus[] = []
    await ensureManagedNode({
      ...baseOptions,
      discover: () => "/work/openagents/.pylon-tailnet",
      onStatus: status => statuses.push(status),
    })
    expect(statuses).toEqual(["adopted"])
  })

  it("emits unavailable for a packaged build with no repo entry", async () => {
    const statuses: NodeLaunchStatus[] = []
    await ensureManagedNode({
      ...baseOptions,
      cwd: "/Applications/Autopilot.app",
      fileExists: () => false,
      discover: () => null,
      onStatus: status => statuses.push(status),
    })
    expect(statuses).toEqual(["unavailable"])
  })
})

describe("superviseManagedNode (restart / lifecycle)", () => {
  const baseOptions = {
    controlBaseUrl: "http://127.0.0.1:4716",
    cwd: deepCwd,
    env: {} as Record<string, string | undefined>,
    fileExists: (path: string) => path === pylonEntry,
    probeReady: async () => true,
    probeCompatible: async () => true,
    sleep: async () => {},
    bunBin: "/usr/bin/bun",
    readToken: () => "tok",
    readinessIntervalMs: 1,
    readinessTimeoutMs: 5,
  }

  // A synchronous scheduler so backoff fires deterministically in tests.
  const immediateSchedule = (fn: () => void) => {
    fn()
    return { cancel: () => {} }
  }

  // Lets the fire-and-forget bring-up settle before assertions.
  const flush = async () => {
    for (let i = 0; i < 10; i++) await Promise.resolve()
  }

  it("adopts an already-running node and never spawns or restarts", async () => {
    let spawned = 0
    const sup = superviseManagedNode({
      ...baseOptions,
      discover: () => "/work/openagents/.pylon-tailnet",
      spawnNode: () => {
        spawned += 1
        return { pid: 1, kill: () => {}, onExit: undefined }
      },
      schedule: immediateSchedule,
    })
    await flush()
    expect(sup.mode()).toBe("adopted")
    expect(sup.status()).toBe("adopted")
    expect(spawned).toBe(0)
    sup.stop()
  })

  it("refuses incompatible already-running nodes without spawning over them", async () => {
    let spawned = 0
    const sup = superviseManagedNode({
      ...baseOptions,
      discover: () => "/work/openagents/.pylon-tailnet",
      probeCompatible: async () => false,
      spawnNode: () => {
        spawned += 1
        return { pid: 1, kill: () => {}, onExit: undefined }
      },
      schedule: immediateSchedule,
    })
    await flush()
    expect(sup.mode()).toBe("unavailable")
    expect(sup.status()).toBe("failed")
    expect(spawned).toBe(0)
    sup.stop()
  })

  it("spawns when absent and restarts the child on crash with backoff", async () => {
    let spawned = 0
    let killed = 0
    const exitCallbacks: Array<(info: { code: number | null }) => void> = []
    const spawnNode = (input: SpawnNodeInput): LaunchedProcess => {
      spawned += 1
      if (input.onExit) exitCallbacks.push(input.onExit)
      return { pid: 1000 + spawned, kill: () => (killed += 1) }
    }
    const backoffs: number[] = []
    const sup = superviseManagedNode({
      ...baseOptions,
      discover: () => null,
      spawnNode,
      restartBackoffMs: [10, 20],
      maxRestarts: 5,
      schedule: (fn, ms) => {
        backoffs.push(ms)
        fn()
        return { cancel: () => {} }
      },
    })

    await flush()
    expect(spawned).toBe(1)
    expect(sup.mode()).toBe("launched")

    // Simulate a crash: the child exits while we did not stop.
    exitCallbacks[0]!({ code: 1 })
    await flush()
    expect(spawned).toBe(2)

    // Another crash restarts again.
    exitCallbacks[1]!({ code: 137 })
    await flush()
    expect(spawned).toBe(3)
    // Each restart used a scheduled backoff delay from the configured schedule.
    expect(backoffs).toEqual([10, 20])
    // Stopping kills the live child we launched.
    sup.stop()
    expect(killed).toBe(1)
  })

  it("does not restart after a deliberate stop (app close)", async () => {
    let spawned = 0
    const exitCallbacks: Array<(info: { code: number | null }) => void> = []
    const sup = superviseManagedNode({
      ...baseOptions,
      discover: () => null,
      spawnNode: input => {
        spawned += 1
        if (input.onExit) exitCallbacks.push(input.onExit)
        return { pid: 7, kill: () => {} }
      },
      schedule: immediateSchedule,
    })
    await flush()
    expect(spawned).toBe(1)

    sup.stop()
    // A late exit notification (e.g. the kill we just issued) must NOT restart.
    exitCallbacks[0]!({ code: 0 })
    await flush()
    expect(spawned).toBe(1)
  })

  it("stops restarting and reports failed after exhausting the restart budget", async () => {
    let spawned = 0
    const statuses: NodeLaunchStatus[] = []
    const exitCallbacks: Array<(info: { code: number | null }) => void> = []
    const sup = superviseManagedNode({
      ...baseOptions,
      discover: () => null,
      spawnNode: input => {
        spawned += 1
        if (input.onExit) exitCallbacks.push(input.onExit)
        return { pid: 1, kill: () => {} }
      },
      restartBackoffMs: [1],
      maxRestarts: 2,
      stableUptimeMs: 1_000_000, // never "stable", so the budget is not reset.
      now: () => 0,
      schedule: immediateSchedule,
      onStatus: s => statuses.push(s),
    })
    await flush()
    // initial spawn + 2 restarts = 3 spawns, then the budget is exhausted.
    exitCallbacks[0]!({ code: 1 })
    await flush()
    exitCallbacks[1]!({ code: 1 })
    await flush()
    exitCallbacks[2]!({ code: 1 })
    await flush()
    expect(spawned).toBe(3)
    expect(sup.status()).toBe("failed")
    sup.stop()
  })

  it("resets the restart budget after the child stays up past stableUptimeMs", async () => {
    let spawned = 0
    const statuses: NodeLaunchStatus[] = []
    const exitCallbacks: Array<(info: { code: number | null }) => void> = []
    // A controllable clock: each read advances time so a child that ran a while
    // looks "stable" before its crash, resetting the restart budget.
    let nowMs = 0
    const sup = superviseManagedNode({
      ...baseOptions,
      discover: () => null,
      spawnNode: input => {
        spawned += 1
        if (input.onExit) exitCallbacks.push(input.onExit)
        return { pid: 1, kill: () => {} }
      },
      restartBackoffMs: [1],
      maxRestarts: 1,
      stableUptimeMs: 100,
      // bringUp records startedAt at now(); the exit reads now() again. Advancing
      // by > stableUptimeMs between them makes every crash count as "stable".
      now: () => (nowMs += 1_000),
      schedule: immediateSchedule,
      onStatus: s => statuses.push(s),
    })
    await flush()
    expect(spawned).toBe(1)

    // Three crashes in a row; because each child was "stable" the budget resets
    // every time, so we never exhaust it and never report failed.
    exitCallbacks[0]!({ code: 1 })
    await flush()
    exitCallbacks[1]!({ code: 1 })
    await flush()
    exitCallbacks[2]!({ code: 1 })
    await flush()
    expect(spawned).toBe(4)
    expect(statuses).not.toContain("failed")
    sup.stop()
  })

  it("stays unavailable (no spawn, no restart) for a packaged build", async () => {
    let spawned = 0
    const sup = superviseManagedNode({
      ...baseOptions,
      cwd: "/Applications/Autopilot.app",
      fileExists: () => false,
      discover: () => null,
      spawnNode: () => {
        spawned += 1
        return { pid: 1, kill: () => {} }
      },
      schedule: immediateSchedule,
    })
    await flush()
    expect(sup.mode()).toBe("unavailable")
    expect(sup.status()).toBe("unavailable")
    expect(spawned).toBe(0)
    sup.stop()
  })
})

// AO-1/AO-2 (#5442/#5443, EPIC #5441): the launcher self-registers the agent on
// first run and injects the onboarding env switches into the node child so the
// existing Pylon runtime lights up presence / payout-target / Tassadar.
describe("ensureManagedNode auto-onboarding (AO-1/AO-2)", () => {
  const baseOptions = {
    controlBaseUrl: "http://127.0.0.1:4716",
    cwd: deepCwd,
    env: {} as Record<string, string | undefined>,
    fileExists: (path: string) => path === pylonEntry,
    probeReady: async () => true,
    probeCompatible: async () => true,
    sleep: async () => {},
    bunBin: "/usr/bin/bun",
    readToken: () => "tok",
    readinessIntervalMs: 1,
    readinessTimeoutMs: 5,
  }

  it("keeps the child isolated when no token is persisted (first run)", async () => {
    const spawnCalls: SpawnNodeInput[] = []
    await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      autoOnboarding: true,
      readPersistedToken: () => null,
      registerAgent: async () => ({ outcome: "identity_pending" }),
      spawnNode: (input: SpawnNodeInput): LaunchedProcess => {
        spawnCalls.push(input)
        return { pid: 1, kill: () => {} }
      },
    })
    const env = spawnCalls[0]!.env
    expect(env.PYLON_HOME).toBe(join(repoRoot, ".pylon-local"))
    // No token yet -> no product env, presence, or assignment worker.
    expect(env.PYLON_OPENAGENTS_BASE_URL).toBeUndefined()
    expect(env.OPENAGENTS_AGENT_TOKEN).toBeUndefined()
    expect(env.PYLON_ASSIGNMENT_WORKER).toBeUndefined()
  })

  it("injects all three onboarding switches when a token is already persisted (relaunch)", async () => {
    const spawnCalls: SpawnNodeInput[] = []
    await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      autoOnboarding: true,
      readPersistedToken: () => "oa_agent_persisted",
      registerAgent: async () => ({
        outcome: "reused",
        credential: {
          token: "oa_agent_persisted",
          tokenPrefix: "oa_agent_per",
          userId: "u",
          externalId: "npub1x",
          registeredAt: "2026-06-18T00:00:00.000Z",
        },
      }),
      spawnNode: (input: SpawnNodeInput): LaunchedProcess => {
        spawnCalls.push(input)
        return { pid: 1, kill: () => {} }
      },
    })
    const env = spawnCalls[0]!.env
    expect(env.OPENAGENTS_AGENT_TOKEN).toBe("oa_agent_persisted")
    expect(env.PYLON_OPENAGENTS_BASE_URL).toBe("https://openagents.com")
    expect(env.PYLON_ASSIGNMENT_WORKER).toBe("1")
  })

  it("self-registers after online and fires onTokenMinted on a fresh registration", async () => {
    let registered = 0
    let minted = 0
    await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      autoOnboarding: true,
      readPersistedToken: () => null,
      registerAgent: async () => {
        registered += 1
        return {
          outcome: "registered",
          credential: {
            token: "oa_agent_new",
            tokenPrefix: "oa_agent_new",
            userId: "u",
            externalId: "npub1x",
            registeredAt: "2026-06-18T00:00:00.000Z",
          },
        }
      },
      onTokenMinted: () => (minted += 1),
      spawnNode: () => ({ pid: 1, kill: () => {} }),
    })
    expect(registered).toBe(1)
    expect(minted).toBe(1)
  })

  it("does not fire onTokenMinted when the token was reused (no restart needed)", async () => {
    let minted = 0
    await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      autoOnboarding: true,
      readPersistedToken: () => "oa_agent_persisted",
      registerAgent: async () => ({
        outcome: "reused",
        credential: {
          token: "oa_agent_persisted",
          tokenPrefix: "oa_agent_per",
          userId: "u",
          externalId: "npub1x",
          registeredAt: "2026-06-18T00:00:00.000Z",
        },
      }),
      onTokenMinted: () => (minted += 1),
      spawnNode: () => ({ pid: 1, kill: () => {} }),
    })
    expect(minted).toBe(0)
  })

  it("does not register when the node never reaches online (failed readiness)", async () => {
    let registered = 0
    await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      autoOnboarding: true,
      readToken: () => null, // token never lands -> readiness fails
      probeReady: async () => false,
      registerAgent: async () => {
        registered += 1
        return { outcome: "identity_pending" }
      },
      spawnNode: () => ({ pid: 1, kill: () => {} }),
    })
    expect(registered).toBe(0)
  })

  it("leaves the env untouched when auto-onboarding is disabled", async () => {
    const spawnCalls: SpawnNodeInput[] = []
    await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      // autoOnboarding omitted (defaults off)
      spawnNode: (input: SpawnNodeInput): LaunchedProcess => {
        spawnCalls.push(input)
        return { pid: 1, kill: () => {} }
      },
    })
    const env = spawnCalls[0]!.env
    expect(env.PYLON_OPENAGENTS_BASE_URL).toBeUndefined()
    expect(env.OPENAGENTS_AGENT_TOKEN).toBeUndefined()
    expect(env.PYLON_ASSIGNMENT_WORKER).toBeUndefined()
  })

  // AO-3 (#5444): identity-choice threading into the launcher.
  it("create-new (default): boots the managed home and passes the chosen name to registration", async () => {
    const spawnCalls: SpawnNodeInput[] = []
    let registeredName: string | null | undefined = undefined
    await ensureManagedNode({
      ...baseOptions,
      discover: () => null,
      autoOnboarding: true,
      readPersistedToken: () => null,
      onboardingDisplayName: "My Studio Agent",
      // useExistingHome omitted/null -> create-new managed home.
      registerAgent: async input => {
        registeredName = input.displayName
        return { outcome: "identity_pending" }
      },
      spawnNode: (input: SpawnNodeInput): LaunchedProcess => {
        spawnCalls.push(input)
        return { pid: 1, kill: () => {} }
      },
    })
    // Fresh managed home (not an existing Pylon).
    expect(spawnCalls[0]!.env.PYLON_HOME).toBe(join(repoRoot, ".pylon-local"))
    // The chosen name reaches self-registration.
    expect(registeredName).toBe("My Studio Agent")
  })

  it("use-existing: boots the chosen existing home (not the managed home) and does not pass a name", async () => {
    const existingHome = "/Users/example/.openagents/pylon"
    const spawnCalls: SpawnNodeInput[] = []
    let registeredHome: string | undefined
    let registeredName: string | null | undefined = "unset"
    await ensureManagedNode({
      ...baseOptions,
      discover: () => null, // existing node not currently running -> launch it
      autoOnboarding: true,
      readPersistedToken: () => null,
      useExistingHome: existingHome,
      onboardingDisplayName: "ignored-for-existing",
      registerAgent: async input => {
        registeredHome = input.home
        registeredName = input.displayName
        return { outcome: "reused", credential: {
          token: "oa_agent_existing",
          tokenPrefix: "oa_agent_exi",
          userId: "u",
          externalId: "npub1existing",
          registeredAt: "2026-06-18T00:00:00.000Z",
        } }
      },
      spawnNode: (input: SpawnNodeInput): LaunchedProcess => {
        spawnCalls.push(input)
        return { pid: 1, kill: () => {} }
      },
    })
    // The existing seed-bearing home is what we launch into (carry-over), not a
    // fresh managed home (no fork, no overwrite of a different home).
    expect(spawnCalls[0]!.env.PYLON_HOME).toBe(existingHome)
    expect(registeredHome).toBe(existingHome)
    // A name is never forced onto an existing (already-named) identity.
    expect(registeredName).toBeNull()
  })

  it("use-existing: adopts an already-running existing node via its home (never double-spawns)", async () => {
    const existingHome = "/Users/example/.openagents/pylon"
    let spawned = 0
    const discoverArgs: Array<string | undefined> = []
    const node = await ensureManagedNode({
      ...baseOptions,
      autoOnboarding: true,
      useExistingHome: existingHome,
      discover: opts => {
        discoverArgs.push(opts.env)
        return existingHome // already running at the chosen home
      },
      spawnNode: () => {
        spawned += 1
        return { pid: 1, kill: () => {} }
      },
    })
    expect(node.mode).toBe("adopted")
    expect(node.home).toBe(existingHome)
    // Discovery was offered the chosen existing home, and nothing was spawned.
    expect(discoverArgs[0]).toBe(existingHome)
    expect(spawned).toBe(0)
  })
})

describe("superviseManagedNode token-minted restart (AO-1/AO-2)", () => {
  const baseOptions = {
    controlBaseUrl: "http://127.0.0.1:4716",
    cwd: deepCwd,
    env: {} as Record<string, string | undefined>,
    fileExists: (path: string) => path === pylonEntry,
    probeReady: async () => true,
    probeCompatible: async () => true,
    sleep: async () => {},
    bunBin: "/usr/bin/bun",
    readinessIntervalMs: 1,
    readinessTimeoutMs: 5,
  }

  const flush = async () => {
    for (let i = 0; i < 20; i++) await Promise.resolve()
  }

  it("restarts the child once (with the token injected) after a fresh registration, without consuming the crash budget", async () => {
    let spawned = 0
    let killed = 0
    let registerCalls = 0
    const tokenSeen: Array<string | undefined> = []
    const exitCallbacks: Array<(info: { code: number | null }) => void> = []

    const sup = superviseManagedNode({
      ...baseOptions,
      discover: () => null,
      readToken: () => "tok",
      autoOnboarding: true,
      // No token on the first spawn; a token exists after the first registration.
      readPersistedToken: () =>
        registerCalls > 0 ? "oa_agent_minted" : null,
      registerAgent: async () => {
        registerCalls += 1
        // First call mints; subsequent calls (after restart) reuse.
        return registerCalls === 1
          ? {
              outcome: "registered",
              credential: {
                token: "oa_agent_minted",
                tokenPrefix: "oa_agent_min",
                userId: "u",
                externalId: "npub1x",
                registeredAt: "2026-06-18T00:00:00.000Z",
              },
            }
          : {
              outcome: "reused",
              credential: {
                token: "oa_agent_minted",
                tokenPrefix: "oa_agent_min",
                userId: "u",
                externalId: "npub1x",
                registeredAt: "2026-06-18T00:00:00.000Z",
              },
            }
      },
      spawnNode: (input: SpawnNodeInput): LaunchedProcess => {
        spawned += 1
        tokenSeen.push(input.env.OPENAGENTS_AGENT_TOKEN)
        if (input.onExit) exitCallbacks.push(input.onExit)
        const onExit = input.onExit
        return {
          pid: 1000 + spawned,
          // Real spawn: a kill causes the child to exit, firing onExit (which
          // the supervisor uses to perform the restart). Mirror that here.
          kill: () => {
            killed++
            onExit?.({ code: null })
          },
        }
      },
      maxRestarts: 1, // a crash restart budget of 1; the token restart must NOT consume it
      restartBackoffMs: [5],
      schedule: (fn, _ms) => {
        fn()
        return { cancel: () => {} }
      },
    })

    await flush()
    // First spawn (no token) + a token-minted restart (with the token).
    expect(spawned).toBe(2)
    expect(tokenSeen[0]).toBeUndefined()
    expect(tokenSeen[1]).toBe("oa_agent_minted")
    expect(sup.mode()).toBe("launched")
    expect(sup.status()).toBe("online")

    // The token restart must not have consumed the crash budget: a real crash
    // still restarts once more.
    const lastExit = exitCallbacks[exitCallbacks.length - 1]!
    lastExit({ code: 1 })
    await flush()
    expect(spawned).toBe(3)

    sup.stop()
    expect(killed).toBeGreaterThan(0)
  })
})
