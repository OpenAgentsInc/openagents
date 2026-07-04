import { describe, expect, test } from "bun:test"

import {
  KHALA_MOBILE_API_KEY_ACCOUNT,
  KHALA_MOBILE_KEYCHAIN_SERVICE,
  deleteKhalaApiKey,
  loadKhalaApiKey,
  saveKhalaApiKey,
  type SecureStoreLike
} from "../src/security/keychain"

const fakeSecureStore = () => {
  const values = new Map<string, string>()
  const calls: Array<unknown> = []
  const store: SecureStoreLike = {
    AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: "after-first-unlock-this-device-only",
    deleteItemAsync: async (key, options) => {
      calls.push({ key, options, op: "delete" })
      values.delete(key)
    },
    getItemAsync: async (key, options) => {
      calls.push({ key, options, op: "get" })
      return values.get(key) ?? null
    },
    setItemAsync: async (key, value, options) => {
      calls.push({ key, options, op: "set", value })
      values.set(key, value)
    }
  }

  return { calls, store }
}

describe("Khala mobile keychain storage", () => {
  test("stores only through secure-store with the Khala keychain service", async () => {
    const fake = fakeSecureStore()
    await saveKhalaApiKey("  oa_agent_local  ", async () => fake.store)

    expect(await loadKhalaApiKey(async () => fake.store)).toBe("oa_agent_local")
    expect(fake.calls).toContainEqual({
      key: KHALA_MOBILE_API_KEY_ACCOUNT,
      op: "set",
      options: {
        keychainAccessible: "after-first-unlock-this-device-only",
        keychainService: KHALA_MOBILE_KEYCHAIN_SERVICE
      },
      value: "oa_agent_local"
    })
  })

  test("deletes the secure-store key", async () => {
    const fake = fakeSecureStore()
    await saveKhalaApiKey("oa_agent_local", async () => fake.store)
    await deleteKhalaApiKey(async () => fake.store)

    expect(await loadKhalaApiKey(async () => fake.store)).toBeNull()
  })
})
