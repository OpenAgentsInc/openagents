import { describe, expect, test } from "bun:test"

import {
  applyAvatarCleanupObservation,
  makeAvatarSessionAttemptGate,
} from "./avatar-session-attempt-gate.ts"
import {
  beginBoundedAvatarStop,
  type AvatarStopClock,
} from "./avatar-stop-deadline.ts"

type TimerHandle = ReturnType<typeof setTimeout>

const manualStopClock = () => {
  let callback: (() => void) | null = null
  const clock: AvatarStopClock = {
    setTimeout: (next) => {
      callback = next
      return 1 as unknown as TimerHandle
    },
    clearTimeout: () => {
      callback = null
    },
  }
  return {
    clock,
    expire: () => callback?.(),
  }
}

describe("Sarah avatar session attempt gate", () => {
  test("rapid reconnect actions remain single-flight", () => {
    const gate = makeAvatarSessionAttemptGate()

    expect(gate.tryBeginReplacementTransition()).toBe(true)
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    gate.finishTransition()
    expect(gate.tryBeginReplacementTransition()).toBe(true)
  })

  test("a stale completion cannot replace a newer attempt", () => {
    const gate = makeAvatarSessionAttemptGate()
    const staleAttempt = gate.nextAttempt()
    const currentAttempt = gate.nextAttempt()

    expect(gate.accepts(staleAttempt)).toBe(false)
    expect(gate.accepts(currentAttempt)).toBe(true)
  })

  test("a disposed surface rejects late completion and every new transition", () => {
    const gate = makeAvatarSessionAttemptGate()
    const pendingAttempt = gate.nextAttempt()
    gate.dispose()

    expect(gate.accepts(pendingAttempt)).toBe(false)
    expect(gate.tryBeginTransition()).toBe(false)
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    gate.finishTransition()
    expect(gate.tryBeginTransition()).toBe(false)
  })

  test("an unconfirmed stop fails replacements closed without wedging cleanup", () => {
    const gate = makeAvatarSessionAttemptGate()
    expect(gate.tryBeginReplacementTransition()).toBe(true)
    gate.blockReplacement()
    gate.finishTransition()

    expect(gate.tryBeginReplacementTransition()).toBe(false)
    expect(gate.tryBeginTransition()).toBe(true)
    gate.finishTransition()
    gate.unblockReplacement()
    expect(gate.tryBeginReplacementTransition()).toBe(true)
  })

  test("automatic terminal cleanup blocks immediately and only exact confirmation reopens replacement", () => {
    const gate = makeAvatarSessionAttemptGate()
    gate.nextAttempt()

    applyAvatarCleanupObservation(gate, "pending")
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    expect(gate.tryBeginTransition()).toBe(true)
    gate.finishTransition()
    applyAvatarCleanupObservation(gate, "confirmed")
    expect(gate.tryBeginReplacementTransition()).toBe(true)
    gate.finishTransition()

    applyAvatarCleanupObservation(gate, "pending")
    applyAvatarCleanupObservation(gate, "unconfirmed")
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    expect(gate.tryBeginTransition()).toBe(true)
  })

  test("a hung stop releases single-flight while retry fails closed and Stop/unmount remain bounded", async () => {
    const gate = makeAvatarSessionAttemptGate()
    const timer = manualStopClock()
    let finish!: () => void
    expect(gate.tryBeginReplacementTransition()).toBe(true)
    const stop = beginBoundedAvatarStop(
      () => new Promise<void>((resolve) => { finish = resolve }),
      { clock: timer.clock, deadlineMs: 100 },
    )
    await Promise.resolve()
    timer.expire()
    expect(await stop.outcome).toBe("timed_out")
    gate.blockReplacement()
    gate.finishTransition()

    // Retry cannot overlap the unknown prior session.
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    // Stop is a cleanup transition and returns instead of inheriting the hang.
    expect(gate.tryBeginTransition()).toBe(true)
    gate.finishTransition()
    // Unmount disposal is synchronous and permanently fences the late stop.
    gate.dispose()
    finish()
    expect(await stop.completion).toBe("stopped")
    gate.unblockReplacement()
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    expect(gate.tryBeginTransition()).toBe(false)
  })
})
