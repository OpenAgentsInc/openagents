import { beforeEach, describe, expect, mock, test } from "bun:test"

/**
 * `khala-auth-store.ts` imports `expo-secure-store` directly (no injectable
 * `SecureStoreLike`, unlike `../src/security/keychain.ts` /
 * `../src/push/push-device-store.ts`), so it's mocked at the module level
 * here with a simple in-memory map standing in for the keychain.
 */
const store = new Map<string, string>()

mock.module("expo-secure-store", () => ({
  deleteItemAsync: async (key: string) => {
    store.delete(key)
  },
  getItemAsync: async (key: string) => store.get(key) ?? null,
  setItemAsync: async (key: string, value: string) => {
    store.set(key, value)
  },
}))

const { clearStoredCredentials, loadStoredCredentials, saveStoredCredentials } = await import(
  "../src/auth/khala-auth-store"
)

const credentials = { ownerUserId: "owner-1", token: "token-1" }

// Oracle for khala_mobile.auth.stored_credential_epoch_purged_on_model_change.v1
describe("khala-auth-store credential epoch purge", () => {
  beforeEach(() => {
    store.clear()
  })

  test("nothing stored: loads null", async () => {
    expect(await loadStoredCredentials()).toBeNull()
  })

  test("saved via the current flow: round-trips unchanged", async () => {
    await saveStoredCredentials(credentials)
    expect(await loadStoredCredentials()).toEqual(credentials)
  })

  test("a credential written under an older/missing epoch (e.g. a leftover retired-flow token) is force-cleared, not resumed", async () => {
    // Simulate a pre-epoch write: ownerUserId/token present, no epoch key —
    // exactly what a leftover Tailnet-pairing or pre-GitHub-OpenAuth token
    // would look like in the keychain today.
    store.set("khala.auth.ownerUserId", credentials.ownerUserId)
    store.set("khala.auth.token", credentials.token)

    expect(await loadStoredCredentials()).toBeNull()
    // And it's actually purged, not just ignored this one time.
    expect(store.has("khala.auth.ownerUserId")).toBe(false)
    expect(store.has("khala.auth.token")).toBe(false)
  })

  test("clearStoredCredentials removes the epoch marker too", async () => {
    await saveStoredCredentials(credentials)
    await clearStoredCredentials()
    expect(store.has("khala.auth.credentialEpoch")).toBe(false)
    expect(await loadStoredCredentials()).toBeNull()
  })
})
