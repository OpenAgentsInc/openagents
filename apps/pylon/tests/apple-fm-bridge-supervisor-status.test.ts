import { describe, expect, test } from "bun:test"
import {
  APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
  APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS,
  createAppleFmBridgeSupervisorState,
  reduceAppleFmBridgeSupervisor,
  type AppleFmBridgeSupervisorEvent,
  type AppleFmBridgeSupervisorState,
} from "../src/node/apple-fm-bridge-supervisor"
import {
  PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA,
  summarizeAppleFmBridgeSupervisor,
} from "../src/node/apple-fm-bridge-supervisor-status"

function drive(
  state: AppleFmBridgeSupervisorState,
  events: ReadonlyArray<AppleFmBridgeSupervisorEvent>,
): AppleFmBridgeSupervisorState {
  return events.reduce(
    (acc, event) => reduceAppleFmBridgeSupervisor(acc, event).state,
    state,
  )
}

describe("Apple FM bridge supervisor status projection", () => {
  test("idle state projects an unsupervised, blocker-free summary", () => {
    const summary = summarizeAppleFmBridgeSupervisor(
      createAppleFmBridgeSupervisorState(),
      0,
    )
    expect(summary.schema).toBe(PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA)
    expect(summary.health).toBe("idle")
    expect(summary.phase).toBe("idle")
    expect(summary.supervised).toBe(false)
    expect(summary.backoffRemainingMs).toBeNull()
    expect(summary.blockerRefs).toEqual([])
    expect(summary.contentRedacted).toBe(true)
  })

  test("a running helper reports running and supervised", () => {
    const state = drive(createAppleFmBridgeSupervisorState(), [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
    ])
    const summary = summarizeAppleFmBridgeSupervisor(state, 1_000)
    expect(summary.health).toBe("running")
    expect(summary.supervised).toBe(true)
    expect(summary.backoffRemainingMs).toBeNull()
  })

  test("a crash projects recovering health with remaining backoff", () => {
    const state = drive(createAppleFmBridgeSupervisorState(), [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
      { kind: "process_exited", nowMs: 1_000, exitCode: 1 },
    ])
    const base = APPLE_FM_BRIDGE_SUPERVISOR_DEFAULTS.baseBackoffMs
    // 200ms into the backoff window: base - 200 remaining.
    const summary = summarizeAppleFmBridgeSupervisor(state, 1_200)
    expect(summary.health).toBe("recovering")
    expect(summary.phase).toBe("backoff")
    expect(summary.consecutiveRestarts).toBe(1)
    expect(summary.restartsInWindow).toBe(1)
    expect(summary.backoffRemainingMs).toBe(base - 200)
  })

  test("backoff-remaining never goes negative past the timer", () => {
    const state = drive(createAppleFmBridgeSupervisorState(), [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
      { kind: "process_exited", nowMs: 0, exitCode: 1 },
    ])
    const summary = summarizeAppleFmBridgeSupervisor(state, 999_999)
    expect(summary.backoffRemainingMs).toBe(0)
  })

  test("stale restart attempts outside the window are not counted", () => {
    const state: AppleFmBridgeSupervisorState = {
      ...createAppleFmBridgeSupervisorState({ restartWindowMs: 1_000 }),
      phase: "running",
      runningSinceMs: 0,
      restartTimestamps: [0],
    }
    const summary = summarizeAppleFmBridgeSupervisor(state, 10_000)
    expect(summary.restartsInWindow).toBe(0)
  })

  test("a crash-loop give-up projects stopped health and the blocker ref", () => {
    let state = createAppleFmBridgeSupervisorState({ maxRestartsInWindow: 1 })
    state = drive(state, [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
      { kind: "process_exited", nowMs: 10, exitCode: 1 },
      { kind: "tick", nowMs: 10_000 },
      { kind: "process_started", nowMs: 10_000 },
      { kind: "process_exited", nowMs: 10_010, exitCode: 1 },
    ])
    expect(state.phase).toBe("given_up")

    const summary = summarizeAppleFmBridgeSupervisor(state, 10_020)
    expect(summary.health).toBe("stopped")
    expect(summary.supervised).toBe(false)
    expect(summary.blockerRefs).toEqual([
      APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
    ])
  })

  test("the projection carries no sensitive keys", () => {
    const state = drive(createAppleFmBridgeSupervisorState(), [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
      { kind: "process_exited", nowMs: 1_000, exitCode: 1 },
    ])
    const summary = summarizeAppleFmBridgeSupervisor(state, 1_100)
    const serialized = JSON.stringify(summary).toLowerCase()
    for (const forbidden of ["token", "bearer", "prompt", "path", "/users/", "http"]) {
      expect(serialized.includes(forbidden)).toBe(false)
    }
  })
})
