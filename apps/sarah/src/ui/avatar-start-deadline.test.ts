import { describe, expect, test } from "bun:test"

import { makeAvatarSessionAttemptGate } from "./avatar-session-attempt-gate.ts"
import {
  beginBoundedAvatarStart,
  type AvatarStartClock,
} from "./avatar-start-deadline.ts"
import {
  beginBoundedAvatarStop,
  type AvatarStopClock,
} from "./avatar-stop-deadline.ts"

type TimerHandle = ReturnType<typeof setTimeout>

class FakeStartClock implements AvatarStartClock {
  nextId = 1
  tasks = new Map<number, () => void>()

  setTimeout = (callback: () => void): TimerHandle => {
    const id = this.nextId++
    this.tasks.set(id, callback)
    return id as unknown as TimerHandle
  }

  clearTimeout = (handle: TimerHandle) => {
    this.tasks.delete(handle as unknown as number)
  }

  expire() {
    for (const [id, callback] of [...this.tasks]) {
      this.tasks.delete(id)
      callback()
    }
  }
}

const immediateStopClock: AvatarStopClock = {
  setTimeout: () => 1 as unknown as TimerHandle,
  clearTimeout: () => {},
}

describe("Sarah bounded avatar start", () => {
  test("a hung start returns typed timeout while retaining its eventual handle", async () => {
    const clock = new FakeStartClock()
    let finish!: (handle: Readonly<{ ref: string }>) => void
    const attempt = beginBoundedAvatarStart(
      () => new Promise<Readonly<{ ref: string }>>((resolve) => { finish = resolve }),
      { clock, deadlineMs: 100 },
    )
    await Promise.resolve()

    clock.expire()
    expect(await attempt.outcome).toEqual({ status: "timed_out" })

    finish({ ref: "avatar.late" })
    expect(await attempt.completion).toEqual({
      status: "started",
      value: { ref: "avatar.late" },
    })
  })

  test("a start rejection is typed and never escapes", async () => {
    const clock = new FakeStartClock()
    const attempt = beginBoundedAvatarStart(
      () => Promise.reject(new Error("provider detail")),
      { clock, deadlineMs: 100 },
    )

    expect(await attempt.outcome).toEqual({ status: "failed" })
    expect(await attempt.completion).toEqual({ status: "failed" })
    expect(clock.tasks.size).toBe(0)
  })

  test("a cleanup-unconfirmed rejection survives the typed start boundary", async () => {
    const gate = makeAvatarSessionAttemptGate()
    expect(gate.tryBeginReplacementTransition()).toBe(true)
    const clock = new FakeStartClock()
    const attempt = beginBoundedAvatarStart(
      () => Promise.reject(new Error("cleanup proof missing")),
      {
        clock,
        deadlineMs: 100,
        classifyFailure: () => "cleanup_unconfirmed",
      },
    )

    expect(await attempt.outcome).toEqual({ status: "cleanup_unconfirmed" })
    expect(await attempt.completion).toEqual({ status: "cleanup_unconfirmed" })
    expect(clock.tasks.size).toBe(0)
    gate.supersedeAttempt()
    gate.blockReplacement()
    gate.finishTransition()
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    expect(gate.tryBeginTransition()).toBe(true)
  })

  test("a hostile failure classifier cannot make completion reject", async () => {
    const attempt = beginBoundedAvatarStart(
      () => Promise.reject(new Error("provider detail")),
      {
        clock: new FakeStartClock(),
        deadlineMs: 100,
        classifyFailure: () => { throw new Error("classifier_failed") },
      },
    )

    expect(await attempt.outcome).toEqual({ status: "failed" })
    expect(await attempt.completion).toEqual({ status: "failed" })
  })

  test("late cleanup-unconfirmed completion keeps the released interaction fail-closed", async () => {
    const gate = makeAvatarSessionAttemptGate()
    expect(gate.tryBeginReplacementTransition()).toBe(true)
    gate.nextAttempt()
    const clock = new FakeStartClock()
    let rejectStart!: (error: Error) => void
    const attempt = beginBoundedAvatarStart(
      () => new Promise<never>((_resolve, reject) => { rejectStart = reject }),
      {
        clock,
        deadlineMs: 100,
        classifyFailure: () => "cleanup_unconfirmed",
      },
    )
    await Promise.resolve()
    clock.expire()
    expect(await attempt.outcome).toEqual({ status: "timed_out" })
    gate.supersedeAttempt()
    gate.blockReplacement()
    gate.finishTransition()

    rejectStart(new Error("server stop 503"))
    expect(await attempt.completion).toEqual({
      status: "cleanup_unconfirmed",
    })
    gate.blockReplacement()
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    expect(gate.tryBeginTransition()).toBe(true)
  })

  test("successful stop then hung start releases interaction, fences its late handle, and cleans it before retry", async () => {
    const gate = makeAvatarSessionAttemptGate()
    expect(gate.tryBeginReplacementTransition()).toBe(true)

    const priorStop = beginBoundedAvatarStop(() => Promise.resolve(), {
      clock: immediateStopClock,
      deadlineMs: 100,
    })
    expect(await priorStop.outcome).toBe("stopped")

    const generation = gate.nextAttempt()
    const clock = new FakeStartClock()
    let finish!: (handle: Readonly<{ stop: () => Promise<void> }>) => void
    let finishLateStop!: () => void
    let lateStopCalls = 0
    const start = beginBoundedAvatarStart(
      () =>
        new Promise<Readonly<{ stop: () => Promise<void> }>>((resolve) => {
          finish = resolve
        }),
      { clock, deadlineMs: 100 },
    )
    await Promise.resolve()
    clock.expire()
    expect(await start.outcome).toEqual({ status: "timed_out" })

    gate.supersedeAttempt()
    gate.blockReplacement()
    gate.finishTransition()
    expect(gate.tryBeginReplacementTransition()).toBe(false)
    expect(gate.tryBeginTransition()).toBe(true)
    gate.finishTransition()

    finish({
      stop: () => new Promise<void>((resolve) => {
        lateStopCalls += 1
        finishLateStop = resolve
      }),
    })
    const terminal = await start.completion
    expect(terminal.status).toBe("started")
    expect(gate.accepts(generation)).toBe(false)
    if (terminal.status !== "started") throw new Error("expected late handle")

    const cleanupClock = new FakeStartClock()
    const cleanup = beginBoundedAvatarStop(terminal.value.stop, {
      clock: cleanupClock,
      deadlineMs: 100,
    })
    await Promise.resolve()
    cleanupClock.expire()
    expect(await cleanup.outcome).toBe("timed_out")
    expect(lateStopCalls).toBe(1)
    expect(gate.tryBeginReplacementTransition()).toBe(false)

    finishLateStop()
    expect(await cleanup.completion).toBe("stopped")
    gate.unblockReplacement()
    expect(gate.tryBeginReplacementTransition()).toBe(true)
  })
})
