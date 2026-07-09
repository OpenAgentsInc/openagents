import { describe, expect, test } from "bun:test"

import { makeAvatarVideoElementLatch } from "./avatar-video-latch.ts"

describe("Sarah avatar video element latch", () => {
  test("disposal rejects pending and future acquisition without raw host detail", async () => {
    const latch = makeAvatarVideoElementLatch()
    const pending = latch.acquire()
    latch.dispose()

    await expect(pending).rejects.toThrow("sarah_avatar_video_latch_disposed")
    await expect(latch.acquire()).rejects.toThrow("sarah_avatar_video_latch_disposed")
  })

  test("supply resolves current waiters and clear requires the exact element", async () => {
    const latch = makeAvatarVideoElementLatch()
    const video = {} as HTMLVideoElement
    const other = {} as HTMLVideoElement
    const pending = latch.acquire()
    latch.supply(video)
    expect(await pending).toBe(video)
    expect(await latch.acquire()).toBe(video)

    latch.clear(other)
    expect(await latch.acquire()).toBe(video)
    latch.clear(video)
    const nextPending = latch.acquire()
    latch.supply(other)
    expect(await nextPending).toBe(other)
  })
})
