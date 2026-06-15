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
  const resourcesDir = "/Applications/Autopilot Desktop.app/Contents/Resources"
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
    "Autopilot Desktop-canary.app",
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
        "Autopilot Desktop-canary.app",
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
    const resourcesDir = "/Applications/Autopilot Desktop.app/Contents/Resources"
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
      bunBin: "/Applications/Autopilot Desktop.app/Contents/MacOS/bun",
      cwd: "/Applications/Autopilot Desktop.app",
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
      "/Applications/Autopilot Desktop.app/Contents/MacOS/bun",
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
