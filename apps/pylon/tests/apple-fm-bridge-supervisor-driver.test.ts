import { describe, expect, test } from "bun:test"
import {
  APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS,
  backoffDelayMs,
} from "../src/node/apple-fm-bridge-supervisor"
import {
  createAppleFmBridgeSupervisorDriver,
  type AppleFmBridgeSupervisorTimer,
  type AppleFmBridgeSupervisorTimerHandle,
} from "../src/node/apple-fm-bridge-supervisor-driver"

/** Controllable fake clock + timer so backoff scheduling stays deterministic. */
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

  const spawns: number[] = []
  const giveUps: string[] = []
  let runningCount = 0

  const driver = createAppleFmBridgeSupervisorDriver({
    now: () => nowMs,
    timer,
    spawn: () => spawns.push(nowMs),
    markRunning: () => {
      runningCount += 1
    },
    giveUp: (ref) => giveUps.push(ref),
  })

  return {
    driver,
    spawns,
    giveUps,
    pendingCount: () => pending.length,
    get runningCount() {
      return runningCount
    },
    /** Advance the clock and fire any timers whose deadline has passed. */
    advance(deltaMs: number) {
      nowMs += deltaMs
      // Fire due timers (may schedule new ones; loop until quiescent).
      let due = pending.filter((p) => p.fireAt <= nowMs)
      while (due.length > 0) {
        pending = pending.filter((p) => p.fireAt > nowMs)
        for (const entry of due) entry.callback()
        due = pending.filter((p) => p.fireAt <= nowMs)
      }
    },
  }
}

describe("Apple FM bridge supervisor driver", () => {
  test("requestStart spawns once and a duplicate request does not double-spawn", () => {
    const h = makeHarness()
    h.driver.requestStart()
    h.driver.requestStart()
    expect(h.spawns.length).toBe(1)
    expect(h.driver.phase()).toBe("starting")
  })

  test("notifyStarted marks running and surfaces running status", () => {
    const h = makeHarness()
    h.driver.requestStart()
    h.driver.notifyStarted()
    expect(h.runningCount).toBe(1)
    expect(h.driver.phase()).toBe("running")
    expect(h.driver.status().health).toBe("running")
  })

  test("a crash schedules a backoff restart that fires on its own timer", () => {
    const h = makeHarness()
    h.driver.requestStart()
    h.driver.notifyStarted()
    h.driver.notifyExited(1)

    expect(h.driver.phase()).toBe("backoff")
    expect(h.pendingCount()).toBe(1)
    expect(h.spawns.length).toBe(1)

    // Before the delay elapses: no respawn.
    h.advance(APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.baseBackoffMs - 1)
    expect(h.spawns.length).toBe(1)

    // Crossing the delay fires the timer, which re-feeds a tick -> spawn.
    h.advance(1)
    expect(h.spawns.length).toBe(2)
    expect(h.driver.phase()).toBe("starting")
    expect(h.pendingCount()).toBe(0)
  })

  test("repeated crashes give up after the window budget and report the blocker", () => {
    const h = makeHarness()
    const max = APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.maxRestartsInWindow
    h.driver.requestStart()

    // Each cycle: started -> exited -> backoff timer fires -> respawn.
    // Advance by exactly the per-attempt backoff so all crashes land inside the
    // sliding restart window (sum of default backoffs is well under it).
    for (let i = 0; i < max; i++) {
      h.driver.notifyStarted()
      h.driver.notifyExited(1)
      h.advance(backoffDelayMs(APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS, i + 1))
    }

    // One more crash exceeds the budget -> give up.
    h.driver.notifyStarted()
    h.driver.notifyExited(1)

    expect(h.driver.phase()).toBe("given_up")
    expect(h.giveUps).toEqual([
      "blocker.pylon.apple_fm.bridge_supervisor.crash_loop",
    ])
    expect(h.driver.status().health).toBe("stopped")
    expect(h.pendingCount()).toBe(0)
  })

  test("a healthy heartbeat past the stable window clears restart escalation", () => {
    const h = makeHarness()
    h.driver.requestStart()
    h.driver.notifyStarted()
    h.driver.notifyExited(1)
    h.advance(APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.baseBackoffMs)
    h.driver.notifyStarted()
    expect(h.driver.snapshot().consecutiveRestarts).toBe(1)

    h.advance(APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.stableUptimeResetMs)
    h.driver.notifyHealthy()
    expect(h.driver.snapshot().consecutiveRestarts).toBe(0)
    expect(h.driver.snapshot().restartTimestamps).toEqual([])
  })

  test("dispose cancels a pending backoff timer so no respawn occurs", () => {
    const h = makeHarness()
    h.driver.requestStart()
    h.driver.notifyStarted()
    h.driver.notifyExited(1)
    expect(h.pendingCount()).toBe(1)

    h.driver.dispose()
    expect(h.pendingCount()).toBe(0)
    h.advance(APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.maxBackoffMs)
    expect(h.spawns.length).toBe(1)
  })

  test("status summary carries no sensitive keys", () => {
    const h = makeHarness()
    h.driver.requestStart()
    h.driver.notifyStarted()
    const summary = h.driver.status()
    const json = JSON.stringify(summary).toLowerCase()
    for (const banned of ["token", "secret", "bearer", "prompt", "/users/"]) {
      expect(json.includes(banned)).toBe(false)
    }
    expect(summary.contentRedacted).toBe(true)
  })
})
