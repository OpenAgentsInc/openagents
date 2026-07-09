import { describe, expect, test } from "bun:test"

import {
  beginBoundedAvatarStop,
  type AvatarStopClock,
} from "./avatar-stop-deadline.ts"

type TimerHandle = ReturnType<typeof setTimeout>

class FakeStopClock implements AvatarStopClock {
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

describe("Sarah bounded avatar stop", () => {
  test("a hung stop returns typed timeout while retaining its eventual truth", async () => {
    const clock = new FakeStopClock()
    let finish!: () => void
    const attempt = beginBoundedAvatarStop(
      () => new Promise<void>((resolve) => { finish = resolve }),
      { clock, deadlineMs: 100 },
    )
    await Promise.resolve()

    clock.expire()
    expect(await attempt.outcome).toBe("timed_out")

    finish()
    expect(await attempt.completion).toBe("stopped")
  })

  test("a stop rejection is typed and never escapes", async () => {
    const clock = new FakeStopClock()
    const attempt = beginBoundedAvatarStop(
      () => Promise.reject(new Error("provider detail")),
      { clock, deadlineMs: 100 },
    )

    expect(await attempt.outcome).toBe("failed")
    expect(await attempt.completion).toBe("failed")
    expect(clock.tasks.size).toBe(0)
  })
})
