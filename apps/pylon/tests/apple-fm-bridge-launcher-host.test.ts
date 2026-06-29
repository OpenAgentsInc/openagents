import { describe, expect, test } from "bun:test"
import {
  APPLE_FM_BRIDGE_DEFAULT_PORT,
  APPLE_FM_BRIDGE_PATH_ENV,
  type DiscoveredAppleFmBridgeHelper,
} from "../src/node/apple-fm-bridge-helper"
import type {
  AppleFmBridgeProcessCallbacks,
  AppleFmBridgeProcessHandle,
  AppleFmBridgeSpawnSpec,
} from "../src/node/apple-fm-bridge-launcher"
import {
  createDefaultAppleFmBridgeLauncher,
  createSetTimeoutSupervisorTimer,
} from "../src/node/apple-fm-bridge-launcher-host"
import type {
  AppleFmBridgeSupervisorTimer,
  AppleFmBridgeSupervisorTimerHandle,
} from "../src/node/apple-fm-bridge-supervisor-driver"

const HELPER: DiscoveredAppleFmBridgeHelper = {
  path: "/opt/pylon/apple-fm-bridge/foundation-bridge",
  source: "packaged-resource",
}

/** Controllable timer + recording spawner so assembly stays deterministic. */
function makeFakes() {
  let nextId = 1
  type Pending = { id: number; callback: () => void }
  let pending: Pending[] = []
  const timer: AppleFmBridgeSupervisorTimer = {
    set(callback) {
      const id = nextId++
      pending.push({ id, callback })
      return id
    },
    clear(handle: AppleFmBridgeSupervisorTimerHandle) {
      pending = pending.filter((p) => p.id !== handle)
    },
  }

  const specs: AppleFmBridgeSpawnSpec[] = []
  let killCount = 0
  const spawnProcess = (
    spec: AppleFmBridgeSpawnSpec,
    callbacks: AppleFmBridgeProcessCallbacks,
  ): AppleFmBridgeProcessHandle => {
    specs.push(spec)
    callbacks.onStarted()
    return {
      kill() {
        killCount += 1
      },
    }
  }

  return {
    timer,
    spawnProcess,
    specs,
    pendingCount: () => pending.length,
    get killCount() {
      return killCount
    },
  }
}

describe("createDefaultAppleFmBridgeLauncher", () => {
  test("returns null when no helper is discovered (graceful on non-Apple host)", () => {
    const result = createDefaultAppleFmBridgeLauncher({
      discoverHelper: () => null,
    })
    expect(result).toBeNull()
  })

  test("assembles a launcher around the discovered helper", () => {
    const fakes = makeFakes()
    const result = createDefaultAppleFmBridgeLauncher({
      discoverHelper: () => HELPER,
      spawnProcess: fakes.spawnProcess,
      timer: fakes.timer,
      now: () => 0,
    })
    expect(result).not.toBeNull()
    expect(result?.helper).toEqual(HELPER)

    result?.launcher.start()
    expect(fakes.specs.length).toBe(1)
    expect(fakes.specs[0]).toEqual({
      command: HELPER.path,
      args: ["--port", String(APPLE_FM_BRIDGE_DEFAULT_PORT)],
    })
    expect(result?.launcher.status().health).toBe("running")
  })

  test("threads discover options through real helper discovery (env source)", () => {
    const result = createDefaultAppleFmBridgeLauncher({
      discover: {
        env: { [APPLE_FM_BRIDGE_PATH_ENV]: "/srv/foundation-bridge" },
        fileExists: (path) => path === "/srv/foundation-bridge",
      },
      spawnProcess: makeFakes().spawnProcess,
      timer: makeFakes().timer,
      now: () => 0,
    })
    expect(result?.helper).toEqual({
      path: "/srv/foundation-bridge",
      source: "env",
    })
  })

  test("forwards port + extra args into the spawn spec", () => {
    const fakes = makeFakes()
    const result = createDefaultAppleFmBridgeLauncher({
      discoverHelper: () => HELPER,
      spawnProcess: fakes.spawnProcess,
      timer: fakes.timer,
      now: () => 0,
      port: 12345,
      extraArgs: ["--verbose"],
    })
    result?.launcher.start()
    expect(fakes.specs[0]).toEqual({
      command: HELPER.path,
      args: ["--port", "12345", "--verbose"],
    })
  })

  test("assembled status carries no sensitive content or helper path", () => {
    const fakes = makeFakes()
    const result = createDefaultAppleFmBridgeLauncher({
      discoverHelper: () => HELPER,
      spawnProcess: fakes.spawnProcess,
      timer: fakes.timer,
      now: () => 0,
    })
    result?.launcher.start()
    const json = JSON.stringify(result?.launcher.status()).toLowerCase()
    for (const banned of [
      "token",
      "secret",
      "bearer",
      "prompt",
      "foundation-bridge",
      "/opt/",
    ]) {
      expect(json.includes(banned)).toBe(false)
    }
    expect(result?.launcher.status().contentRedacted).toBe(true)
  })
})

describe("createSetTimeoutSupervisorTimer", () => {
  test("fires the callback after the delay", async () => {
    const timer = createSetTimeoutSupervisorTimer()
    let fired = false
    timer.set(() => {
      fired = true
    }, 1)
    await new Promise((resolve) => setTimeout(resolve, 10))
    expect(fired).toBe(true)
  })

  test("clear cancels a pending callback", async () => {
    const timer = createSetTimeoutSupervisorTimer()
    let fired = false
    const handle = timer.set(() => {
      fired = true
    }, 5)
    timer.clear(handle)
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(fired).toBe(false)
  })
})
