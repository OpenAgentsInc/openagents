import { describe, expect, test } from "bun:test"

import {
  clearPushDeviceId,
  loadHasEverPromptedForPush,
  loadOrCreatePushDeviceId,
  readPushDeviceIdIfPresent,
  saveHasEverPromptedForPush,
  type SecureStoreLike
} from "../src/push/push-device-store"

// Oracle for khala_mobile.push.permission_prompt_on_first_task_dispatch.v1
const fakeSecureStore = () => {
  const values = new Map<string, string>()
  const store: SecureStoreLike = {
    AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: "after-first-unlock-this-device-only",
    deleteItemAsync: async key => {
      values.delete(key)
    },
    getItemAsync: async key => values.get(key) ?? null,
    setItemAsync: async (key, value) => {
      values.set(key, value)
    }
  }
  return { store, values }
}

describe("loadOrCreatePushDeviceId", () => {
  test("creates and persists a device id on first read", async () => {
    const fake = fakeSecureStore()
    const id = await loadOrCreatePushDeviceId({
      makeDeviceId: () => "generated-id",
      secureStoreLoader: async () => fake.store
    })
    expect(id).toBe("generated-id")
    expect(fake.values.get("khala.push.deviceId")).toBe("generated-id")
  })

  test("returns the SAME id on a second call — never regenerates once persisted", async () => {
    const fake = fakeSecureStore()
    let calls = 0
    const makeDeviceId = () => {
      calls += 1
      return `generated-${calls}`
    }
    const first = await loadOrCreatePushDeviceId({ makeDeviceId, secureStoreLoader: async () => fake.store })
    const second = await loadOrCreatePushDeviceId({ makeDeviceId, secureStoreLoader: async () => fake.store })
    expect(first).toBe(second)
    expect(calls).toBe(1)
  })

  test("rejects an empty generated id rather than persisting garbage", async () => {
    const fake = fakeSecureStore()
    await expect(
      loadOrCreatePushDeviceId({ makeDeviceId: () => "   ", secureStoreLoader: async () => fake.store })
    ).rejects.toThrow()
  })
})

describe("has-ever-prompted flag", () => {
  test("defaults to false, becomes true after saving, survives clearPushDeviceId", async () => {
    const fake = fakeSecureStore()
    expect(await loadHasEverPromptedForPush(async () => fake.store)).toBe(false)
    await saveHasEverPromptedForPush(async () => fake.store)
    expect(await loadHasEverPromptedForPush(async () => fake.store)).toBe(true)

    await clearPushDeviceId(async () => fake.store)
    // clearing the device id must NOT clear the prompt flag (device-level
    // permission fact, not account-level) — see push-device-store.ts's doc
    // comment on clearPushDeviceId.
    expect(await loadHasEverPromptedForPush(async () => fake.store)).toBe(true)
  })
})

describe("readPushDeviceIdIfPresent / clearPushDeviceId", () => {
  test("null before creation, set after, null again after clearing", async () => {
    const fake = fakeSecureStore()
    expect(await readPushDeviceIdIfPresent(async () => fake.store)).toBeNull()

    await loadOrCreatePushDeviceId({ makeDeviceId: () => "device-x", secureStoreLoader: async () => fake.store })
    expect(await readPushDeviceIdIfPresent(async () => fake.store)).toBe("device-x")

    await clearPushDeviceId(async () => fake.store)
    expect(await readPushDeviceIdIfPresent(async () => fake.store)).toBeNull()
  })
})
