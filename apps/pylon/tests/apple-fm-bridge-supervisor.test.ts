import { describe, expect, test } from "bun:test"
import {
  APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
  APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS,
  backoffDelayMs,
  createAppleFmBridgeSupervisorState,
  reduceAppleFmBridgeSupervisor,
  type AppleFmBridgeSupervisorEvent,
  type AppleFmBridgeSupervisorState,
} from "../src/node/apple-fm-bridge-supervisor"

function drive(
  state: AppleFmBridgeSupervisorState,
  events: ReadonlyArray<AppleFmBridgeSupervisorEvent>,
): AppleFmBridgeSupervisorState {
  return events.reduce(
    (acc, event) => reduceAppleFmBridgeSupervisor(acc, event).state,
    state,
  )
}

describe("Apple FM bridge supervisor policy", () => {
  test("start_requested from idle asks the caller to spawn", () => {
    const { state, action } = reduceAppleFmBridgeSupervisor(
      createAppleFmBridgeSupervisorState(),
      { kind: "start_requested", nowMs: 0 },
    )
    expect(action).toEqual({ kind: "spawn" })
    expect(state.phase).toBe("starting")
  })

  test("does not double-spawn while starting or running", () => {
    let state = createAppleFmBridgeSupervisorState()
    state = drive(state, [{ kind: "start_requested", nowMs: 0 }])

    const whileStarting = reduceAppleFmBridgeSupervisor(state, {
      kind: "start_requested",
      nowMs: 1,
    })
    expect(whileStarting.action).toEqual({ kind: "none" })

    const running = reduceAppleFmBridgeSupervisor(state, {
      kind: "process_started",
      nowMs: 2,
    })
    expect(running.action).toEqual({ kind: "mark_running" })
    expect(running.state.phase).toBe("running")

    const whileRunning = reduceAppleFmBridgeSupervisor(running.state, {
      kind: "start_requested",
      nowMs: 3,
    })
    expect(whileRunning.action).toEqual({ kind: "none" })
  })

  test("an exit schedules a backoff restart with the base delay", () => {
    let state = createAppleFmBridgeSupervisorState()
    state = drive(state, [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
    ])

    const { state: next, action } = reduceAppleFmBridgeSupervisor(state, {
      kind: "process_exited",
      nowMs: 1_000,
      exitCode: 1,
    })

    expect(action).toEqual({
      kind: "schedule_restart",
      delayMs: APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.baseBackoffMs,
      attempt: 1,
    })
    expect(next.phase).toBe("backoff")
    expect(next.backoffUntilMs).toBe(1_000 + APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.baseBackoffMs)
  })

  test("backoff grows exponentially and is capped at maxBackoffMs", () => {
    const config = APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS
    expect(backoffDelayMs(config, 1)).toBe(500)
    expect(backoffDelayMs(config, 2)).toBe(1_000)
    expect(backoffDelayMs(config, 3)).toBe(2_000)
    // 500 * 2^9 = 256000 -> clamped to 30000
    expect(backoffDelayMs(config, 10)).toBe(config.maxBackoffMs)
  })

  test("a tick before the backoff expires does nothing; after it spawns", () => {
    let state = createAppleFmBridgeSupervisorState()
    state = drive(state, [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
      { kind: "process_exited", nowMs: 0, exitCode: 1 },
    ])
    expect(state.phase).toBe("backoff")

    const early = reduceAppleFmBridgeSupervisor(state, { kind: "tick", nowMs: 100 })
    expect(early.action).toEqual({ kind: "none" })
    expect(early.state.phase).toBe("backoff")

    const ready = reduceAppleFmBridgeSupervisor(state, { kind: "tick", nowMs: 999_999 })
    expect(ready.action).toEqual({ kind: "spawn" })
    expect(ready.state.phase).toBe("starting")
  })

  test("gives up with a stable blocker ref after exceeding the restart budget", () => {
    let state = createAppleFmBridgeSupervisorState({
      maxRestartsInWindow: 2,
      restartWindowMs: 60_000,
    })
    state = drive(state, [{ kind: "start_requested", nowMs: 0 }])

    // Two restarts inside the window are allowed.
    let now = 0
    for (let i = 0; i < 2; i += 1) {
      state = drive(state, [
        { kind: "process_started", nowMs: now },
        { kind: "process_exited", nowMs: now + 10, exitCode: 1 },
        { kind: "tick", nowMs: now + 10_000 },
      ])
      now += 10_000
      expect(state.phase).toBe("starting")
    }

    // The third crash in-window exceeds the budget -> give up.
    const giveUp = reduceAppleFmBridgeSupervisor(
      { ...state, phase: "running", runningSinceMs: now },
      { kind: "process_exited", nowMs: now + 10, exitCode: 1 },
    )
    expect(giveUp.action).toEqual({
      kind: "give_up",
      blockerRef: APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
    })
    expect(giveUp.state.phase).toBe("given_up")
    expect(giveUp.state.givenUpBlockerRef).toBe(
      APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
    )

    // Once given up, the policy refuses further spawns until reset.
    const afterGiveUp = reduceAppleFmBridgeSupervisor(giveUp.state, {
      kind: "start_requested",
      nowMs: now + 20,
    })
    expect(afterGiveUp.action).toEqual({ kind: "none" })
  })

  test("a stable, healthy helper forgives old crashes and resets backoff", () => {
    let state = createAppleFmBridgeSupervisorState()
    state = drive(state, [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
      { kind: "process_exited", nowMs: 100, exitCode: 1 },
      { kind: "tick", nowMs: 10_000 },
      { kind: "process_started", nowMs: 10_000 },
    ])
    expect(state.consecutiveRestarts).toBe(1)
    expect(state.restartTimestamps.length).toBe(1)

    // Healthy past the stable threshold resets the counters.
    const stableAt = 10_000 + APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.stableUptimeResetMs
    const healthy = reduceAppleFmBridgeSupervisor(state, {
      kind: "health_ok",
      nowMs: stableAt,
    })
    expect(healthy.state.consecutiveRestarts).toBe(0)
    expect(healthy.state.restartTimestamps).toEqual([])

    // The next crash therefore restarts at the base delay again.
    const exit = reduceAppleFmBridgeSupervisor(healthy.state, {
      kind: "process_exited",
      nowMs: stableAt + 5,
      exitCode: 1,
    })
    expect(exit.action).toEqual({
      kind: "schedule_restart",
      delayMs: APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.baseBackoffMs,
      attempt: 1,
    })
  })

  test("old restart attempts outside the window do not count toward the budget", () => {
    let state = createAppleFmBridgeSupervisorState({
      maxRestartsInWindow: 1,
      restartWindowMs: 1_000,
    })
    state = {
      ...state,
      phase: "running",
      runningSinceMs: 0,
      restartTimestamps: [0], // one attempt long ago
    }

    // Exit well past the window: the stale attempt is pruned, so we still
    // restart instead of giving up.
    const { state: next, action } = reduceAppleFmBridgeSupervisor(state, {
      kind: "process_exited",
      nowMs: 10_000,
      exitCode: 1,
    })
    expect(action.kind).toBe("schedule_restart")
    expect(next.phase).toBe("backoff")
    expect(next.restartTimestamps).toEqual([10_000])
  })
})
