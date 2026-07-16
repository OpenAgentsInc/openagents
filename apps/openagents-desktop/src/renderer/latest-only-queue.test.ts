import { describe, expect, test } from "vite-plus/test"

import { makeLatestOnlyQueue } from "./latest-only-queue.ts"

describe("latest-only renderer projection queue", () => {
  test("retains only the in-flight and newest snapshot under a burst", async () => {
    let releaseFirst!: () => void
    const firstBlocked = new Promise<void>(resolve => { releaseFirst = resolve })
    const processed: number[] = []
    const queue = makeLatestOnlyQueue<number>(async value => {
      processed.push(value)
      if (value === 0) await firstBlocked
    })

    queue.submit(0)
    for (let value = 1; value <= 10_000; value += 1) queue.submit(value)
    expect(processed).toEqual([0])

    releaseFirst()
    await queue.flush()
    expect(processed).toEqual([0, 10_000])
  })

  test("flush includes a value submitted at the drain boundary", async () => {
    const processed: string[] = []
    let queue!: ReturnType<typeof makeLatestOnlyQueue<string>>
    queue = makeLatestOnlyQueue<string>(async value => {
      processed.push(value)
      if (value === "first") queue.submit("boundary")
    })

    queue.submit("first")
    await queue.flush()
    expect(processed).toEqual(["first", "boundary"])
  })
})
