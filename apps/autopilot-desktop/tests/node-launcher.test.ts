import { describe, expect, it } from "bun:test"
import { join } from "node:path"
import {
  ensureManagedNode,
  findDevPylonEntry,
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
