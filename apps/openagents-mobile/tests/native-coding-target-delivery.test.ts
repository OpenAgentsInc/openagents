import { describe, expect, test } from "bun:test"

import {
  MAX_PENDING_NATIVE_CODING_TARGETS,
  openNativeCodingTargetDelivery,
} from "../src/coding/native-coding-target-delivery"
import type { MobileCodingTargetResolution } from "../src/coding/mobile-coding-navigation"

const target = {
  schema: "openagents.mobile.coding_target.v1" as const,
  repositoryRef: "repository.mobile",
  sessionRef: "session.mobile",
  threadRef: "thread.mobile",
}

describe("contract openagents_mobile.coding.native_target_delivery.v1", () => {
  test("production app owns initial/live URL and notification response listeners plus teardown", async () => {
    const source = await Bun.file(new URL("../src/app.tsx", import.meta.url)).text()
    expect(source).toContain('Linking.getInitialURL()')
    expect(source).toContain('Linking.addEventListener("url"')
    expect(source).toContain("getLastNotificationResponseAsync()")
    expect(source).toContain("addNotificationResponseReceivedListener")
    expect(source).toContain("linkSubscription?.remove()")
    expect(source).toContain("notificationSubscription?.remove()")
    expect(source).toContain("targetDelivery?.close()")
  })

  test("queues until live authority, then activates the exact resolved target once", async () => {
    let live = false
    const activated: Array<Readonly<{ sessionRef: string; source: string }>> = []
    const delivery = openNativeCodingTargetDelivery({
      resolve: async (): Promise<MobileCodingTargetResolution> => live
        ? { state: "ready", target, repository: {} as never, worktree: {} as never, session: {} as never }
        : { state: "rejected", reason: "authority_unavailable", affectedRef: target.sessionRef },
      activate: async (value, source) => {
        activated.push({ sessionRef: value.sessionRef, source })
        return true
      },
    })
    delivery.enqueue({ source: "deep_link", url: "openagents://coding/session/session.mobile" })
    await delivery.flush()
    expect(delivery.pendingCount()).toBe(1)
    expect(activated).toEqual([])
    live = true
    await Promise.all([delivery.flush(), delivery.flush()])
    expect(delivery.pendingCount()).toBe(0)
    expect(activated).toEqual([{ sessionRef: "session.mobile", source: "deep_link" }])

    delivery.enqueue({ source: "notification", payload: target })
    await delivery.flush()
    expect(activated).toEqual([
      { sessionRef: "session.mobile", source: "deep_link" },
      { sessionRef: "session.mobile", source: "notification" },
    ])
  })

  test("drops stale targets, bounds the queue, and teardown rejects late delivery", async () => {
    const rejected: string[] = []
    const delivery = openNativeCodingTargetDelivery({
      resolve: async () => ({ state: "rejected", reason: "stale_session", affectedRef: "stale" }),
      activate: async () => { throw new Error("must not activate") },
      rejected: value => rejected.push(value.reason),
    })
    for (let index = 0; index < MAX_PENDING_NATIVE_CODING_TARGETS + 5; index += 1) {
      delivery.enqueue({ source: "notification", payload: { index } })
    }
    expect(delivery.pendingCount()).toBe(MAX_PENDING_NATIVE_CODING_TARGETS)
    await delivery.flush()
    expect(rejected).toHaveLength(MAX_PENDING_NATIVE_CODING_TARGETS)
    expect(delivery.pendingCount()).toBe(0)
    delivery.close()
    delivery.enqueue({ source: "notification", payload: target })
    await delivery.flush()
    expect(delivery.pendingCount()).toBe(0)
  })
})
