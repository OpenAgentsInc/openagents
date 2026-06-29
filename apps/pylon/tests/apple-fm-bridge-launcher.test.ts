import { describe, expect, test } from "bun:test"
import type { DiscoveredAppleFmBridgeHelper } from "../src/node/apple-fm-bridge-helper"
import { APPLE_FM_BRIDGE_DEFAULT_PORT } from "../src/node/apple-fm-bridge-helper"
import { APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS } from "../src/node/apple-fm-bridge-supervisor"
import type {
  AppleFmBridgeSupervisorTimer,
  AppleFmBridgeSupervisorTimerHandle,
} from "../src/node/apple-fm-bridge-supervisor-driver"
import {
  buildAppleFmBridgeSpawnSpec,
  createAppleFmBridgeLauncher,
  type AppleFmBridgeProcessCallbacks,
  type AppleFmBridgeProcessHandle,
  type AppleFmBridgeSpawnSpec,
} from "../src/node/apple-fm-bridge-launcher"

const HELPER: DiscoveredAppleFmBridgeHelper = {
  path: "/opt/pylon/apple-fm-bridge/foundation-bridge",
  source: "packaged-resource",
}

/** Controllable clock + timer + a fake process spawner. */
function makeHarness() {
  let nowMs = 0
  type Pending = { id: number; fireAt: number; callback: () => void }
  let nextId = 1
  let pending: Pending[] = []

  const timer: AppleFmBridgeSupervisorTimer = {
    set(callback, delayMs) {
      const entry: Pending = { id: nextId++, fireAt: nowMs + delayMs, callback }
      pending.push(entry)
      return entry.id
    },
    clear(handle: AppleFmBridgeSupervisorTimerHandle) {
      pending = pending.filter((p) => p.id !== handle)
    },
  }

  type Proc = {
    spec: AppleFmBridgeSpawnSpec
    callbacks: AppleFmBridgeProcessCallbacks
    killed: boolean
  }
  const procs: Proc[] = []
  let killCount = 0

  const launcher = createAppleFmBridgeLauncher({
    helper: HELPER,
    now: () => nowMs,
    timer,
    spawnProcess: (spec, callbacks): AppleFmBridgeProcessHandle => {
      const proc: Proc = { spec, callbacks, killed: false }
      procs.push(proc)
      // Auto-report started, mirroring a process that comes up immediately.
      callbacks.onStarted()
      return {
        kill() {
          if (!proc.killed) {
            proc.killed = true
            killCount += 1
          }
        },
      }
    },
  })

  return {
    launcher,
    procs,
    get killCount() {
      return killCount
    },
    /** Exit the most-recently spawned process. */
    exitLast(exitCode: number | null, signal: string | null = null) {
      const proc = procs[procs.length - 1]
      if (proc === undefined) throw new Error("no process to exit")
      proc.callbacks.onExited(exitCode, signal)
    },
    advance(deltaMs: number) {
      nowMs += deltaMs
      let due = pending.filter((p) => p.fireAt <= nowMs)
      while (due.length > 0) {
        pending = pending.filter((p) => p.fireAt > nowMs)
        for (const entry of due) entry.callback()
        due = pending.filter((p) => p.fireAt <= nowMs)
      }
    },
    pendingCount: () => pending.length,
  }
}

describe("Apple FM bridge launcher", () => {
  test("buildAppleFmBridgeSpawnSpec uses the helper path and the default port", () => {
    const spec = buildAppleFmBridgeSpawnSpec(HELPER)
    expect(spec.command).toBe(HELPER.path)
    expect(spec.args).toEqual(["--port", String(APPLE_FM_BRIDGE_DEFAULT_PORT)])
  })

  test("start spawns the helper once and reports running", () => {
    const h = makeHarness()
    h.launcher.start()
    h.launcher.start()
    expect(h.procs.length).toBe(1)
    expect(h.procs[0]?.spec.command).toBe(HELPER.path)
    expect(h.launcher.status().health).toBe("running")
  })

  test("a crash respawns the helper after the backoff timer fires", () => {
    const h = makeHarness()
    h.launcher.start()
    h.exitLast(1)
    expect(h.procs.length).toBe(1)
    expect(h.launcher.status().health).toBe("recovering")

    h.advance(APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.baseBackoffMs)
    expect(h.procs.length).toBe(2)
    expect(h.launcher.status().health).toBe("running")
  })

  test("a duplicate exit signal is ignored (no double restart accounting)", () => {
    const h = makeHarness()
    h.launcher.start()
    const proc = h.procs[0]!
    proc.callbacks.onExited(1, null)
    proc.callbacks.onExited(1, null)
    // Only one restart should be pending despite the doubled exit.
    expect(h.pendingCount()).toBe(1)
  })

  test("a crash loop gives up, kills nothing live, and reports stopped + blocker", () => {
    const h = makeHarness()
    const max = APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.maxRestartsInWindow
    h.launcher.start()
    for (let i = 0; i < max; i++) {
      h.exitLast(1)
      h.advance(APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.baseBackoffMs * 2 ** i)
    }
    h.exitLast(1)
    const status = h.launcher.status()
    expect(status.health).toBe("stopped")
    expect(status.blockerRefs).toContain(
      "blocker.pylon.apple_fm.bridge_supervisor.crash_loop",
    )
  })

  test("stop kills the live process and cancels a pending restart", () => {
    const h = makeHarness()
    h.launcher.start()
    h.exitLast(1)
    expect(h.pendingCount()).toBe(1)
    h.launcher.stop()
    expect(h.pendingCount()).toBe(0)
    // No respawn after stop even once the would-be backoff elapses.
    h.advance(APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.maxBackoffMs)
    expect(h.procs.length).toBe(1)
  })

  test("stop kills a currently-running process", () => {
    const h = makeHarness()
    h.launcher.start()
    expect(h.killCount).toBe(0)
    h.launcher.stop()
    expect(h.killCount).toBe(1)
  })

  test("status summary carries no sensitive content or helper path", () => {
    const h = makeHarness()
    h.launcher.start()
    const json = JSON.stringify(h.launcher.status()).toLowerCase()
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
    expect(h.launcher.status().contentRedacted).toBe(true)
  })
})
