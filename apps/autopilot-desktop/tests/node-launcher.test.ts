import { describe, expect, it } from "bun:test"
import { join } from "node:path"
import {
  ensureManagedNode,
  findDevPylonEntry,
  type LaunchedProcess,
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

  it("stays discover-only (unavailable) with no repo entry — packaged build, Phase 2", async () => {
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
})
