import { beforeEach, describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"

import {
  clearNativeSessionCredential,
  loadNativeSessionCredential,
  NativeSessionVaultError,
  OPENAGENTS_NATIVE_SESSION_EPOCH,
  OPENAGENTS_NATIVE_SESSION_KEY,
  OPENAGENTS_NATIVE_SESSION_KEYCHAIN_SERVICE,
  recoverNativeSession,
  saveNativeSessionCredential,
  type SecureStoreLike,
} from "../src/auth/native-session-vault"

const values = new Map<string, string>()
const calls: Array<Readonly<Record<string, unknown>>> = []

const store: SecureStoreLike = {
  AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: "after-first-unlock-device-only",
  deleteItemAsync: async (key, options) => {
    calls.push({ op: "delete", key, options })
    values.delete(key)
  },
  getItemAsync: async (key, options) => {
    calls.push({ op: "get", key, options })
    return values.get(key) ?? null
  },
  setItemAsync: async (key, value, options) => {
    calls.push({ op: "set", key, options })
    values.set(key, value)
  },
}

const loadStore = async () => store
const credential = {
  ownerUserId: "owner.fixture",
  accessToken: "fixture-access",
  refreshToken: "fixture-refresh",
}

describe("contract openagents_mobile.session.secure_store_custody.v1", () => {
  beforeEach(() => {
    values.clear()
    calls.length = 0
  })

  test("round-trips one versioned record under device-only keychain options", async () => {
    await saveNativeSessionCredential(credential, loadStore)
    expect(await loadNativeSessionCredential(loadStore)).toEqual(credential)
    expect(values.size).toBe(1)
    expect(JSON.parse(values.get(OPENAGENTS_NATIVE_SESSION_KEY)!)).toEqual({
      schemaVersion: 1,
      credentialEpoch: OPENAGENTS_NATIVE_SESSION_EPOCH,
      ...credential,
    })
    expect(calls).toContainEqual({
      op: "set",
      key: OPENAGENTS_NATIVE_SESSION_KEY,
      options: {
        keychainAccessible: "after-first-unlock-device-only",
        keychainService: OPENAGENTS_NATIVE_SESSION_KEYCHAIN_SERVICE,
      },
    })
  })

  test("purges malformed, partial, empty, and wrong-epoch records", async () => {
    const invalid = [
      "not-json",
      JSON.stringify({ schemaVersion: 1 }),
      JSON.stringify({
        schemaVersion: 1,
        credentialEpoch: OPENAGENTS_NATIVE_SESSION_EPOCH,
        ...credential,
        accessToken: "",
      }),
      JSON.stringify({
        schemaVersion: 1,
        credentialEpoch: "retired-epoch",
        ...credential,
      }),
    ]
    for (const raw of invalid) {
      values.set(OPENAGENTS_NATIVE_SESSION_KEY, raw)
      expect(await loadNativeSessionCredential(loadStore)).toBeNull()
      expect(values.has(OPENAGENTS_NATIVE_SESSION_KEY)).toBe(false)
    }
  })

  test("classifies recovery without projecting credential fields", async () => {
    expect(await recoverNativeSession(loadStore)).toEqual({ state: "signed_out" })
    await saveNativeSessionCredential(credential, loadStore)
    const recovery = await recoverNativeSession(loadStore)
    expect(recovery).toEqual({ state: "credential_present_unverified" })
    expect(JSON.stringify(recovery)).not.toContain("owner.fixture")
    expect(JSON.stringify(recovery)).not.toContain("fixture-access")
    expect(JSON.stringify(recovery)).not.toContain("fixture-refresh")
  })

  test("production composition keeps raw credential loading outside view state", () => {
    const appSource = readFileSync(
      new URL("../src/app.tsx", import.meta.url),
      "utf8",
    )
    const homeSource = readFileSync(
      new URL("../src/screens/home-core.ts", import.meta.url),
      "utf8",
    )
    const appConfig = JSON.parse(readFileSync(
      new URL("../app.json", import.meta.url),
      "utf8",
    )) as { expo: { plugins: ReadonlyArray<string> } }
    expect(appSource).toContain("recoverNativeSession")
    expect(appSource).not.toContain("loadNativeSessionCredential")
    expect(homeSource).not.toContain("accessToken")
    expect(homeSource).not.toContain("refreshToken")
    expect(homeSource).not.toContain("ownerUserId")
    expect(appConfig.expo.plugins).toContain("expo-secure-store")
  })

  test("clears idempotently", async () => {
    await saveNativeSessionCredential(credential, loadStore)
    await clearNativeSessionCredential(loadStore)
    await clearNativeSessionCredential(loadStore)
    expect(await loadNativeSessionCredential(loadStore)).toBeNull()
  })

  test("rejects incomplete writes and reports storage failures without secrets", async () => {
    await expect(saveNativeSessionCredential(
      { ...credential, refreshToken: " " },
      loadStore,
    )).rejects.toMatchObject({ reason: "invalid_credential" })

    const unavailable = async (): Promise<SecureStoreLike> => {
      throw new Error("fixture native failure with fixture-access")
    }
    let caught: unknown
    try {
      await loadNativeSessionCredential(unavailable)
    } catch (error) {
      caught = error
    }
    expect(caught).toBeInstanceOf(NativeSessionVaultError)
    expect((caught as NativeSessionVaultError).reason).toBe("secure_store_unavailable")
    expect((caught as Error).message).toBe(
      "native secure session storage is unavailable",
    )
    expect((caught as Error).message).not.toContain("fixture-access")
    expect((caught as Error).cause).toBeUndefined()
  })
})
