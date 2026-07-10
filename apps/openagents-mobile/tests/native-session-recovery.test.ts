import { beforeEach, describe, expect, test } from "bun:test"

import {
  OPENAGENTS_MOBILE_AUTH_SESSION_URL,
  OPENAGENTS_NATIVE_REFRESH_HEADER,
  recoverVerifiedNativeSession,
} from "../src/auth/native-session-recovery"
import {
  loadNativeSessionCredential,
  saveNativeSessionCredential,
  type SecureStoreLike,
} from "../src/auth/native-session-vault"

const values = new Map<string, string>()
const store: SecureStoreLike = {
  AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: "device-only",
  deleteItemAsync: async key => {
    values.delete(key)
  },
  getItemAsync: async key => values.get(key) ?? null,
  setItemAsync: async (key, value) => {
    values.set(key, value)
  },
}
const loadStore = async () => store
const original = {
  ownerUserId: "owner.fixture",
  accessToken: "access-original",
  refreshToken: "refresh-original",
}

const response = (
  status: number,
  body: unknown,
): Pick<Response, "json" | "ok" | "status"> => ({
  status,
  ok: status >= 200 && status < 300,
  json: async () => body,
})

describe("contract openagents_mobile.session.recovered_validation_rotation.v1", () => {
  beforeEach(async () => {
    values.clear()
    await saveNativeSessionCredential(original, loadStore)
  })

  test("verifies through the exact bearer and refresh headers without projecting them", async () => {
    const calls: Array<Readonly<{ input: string; init: RequestInit }>> = []
    const result = await recoverVerifiedNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async (input, init) => {
        calls.push({ input, init })
        return response(200, {
          authenticated: true,
          user: { userId: "owner.fixture", email: "private@example.test" },
        })
      },
    })
    expect(result).toEqual({ state: "verified", rotated: false })
    expect(calls).toHaveLength(1)
    expect(calls[0]?.input).toBe(OPENAGENTS_MOBILE_AUTH_SESSION_URL)
    expect(calls[0]?.init.headers).toEqual({
      authorization: "Bearer access-original",
      [OPENAGENTS_NATIVE_REFRESH_HEADER]: "refresh-original",
    })
    expect(JSON.stringify(result)).not.toContain("access-original")
    expect(JSON.stringify(result)).not.toContain("refresh-original")
    expect(JSON.stringify(result)).not.toContain("owner.fixture")
  })

  test("atomically rewrites the vault when the server rotates tokens", async () => {
    const result = await recoverVerifiedNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async () => response(200, {
        authenticated: true,
        user: { userId: "owner.fixture" },
        tokens: {
          access: "access-rotated",
          refresh: "refresh-rotated",
          expiresIn: 3600,
        },
      }),
    })
    expect(result).toEqual({ state: "verified", rotated: true })
    expect(await loadNativeSessionCredential(loadStore)).toEqual({
      ownerUserId: "owner.fixture",
      accessToken: "access-rotated",
      refreshToken: "refresh-rotated",
    })
  })

  test("purges a denied credential", async () => {
    const result = await recoverVerifiedNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async () => response(401, { authenticated: false }),
    })
    expect(result).toEqual({ state: "denied", rotated: false })
    expect(await loadNativeSessionCredential(loadStore)).toBeNull()
  })

  test("purges a credential if the server-derived owner changes", async () => {
    const result = await recoverVerifiedNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async () => response(200, {
        authenticated: true,
        user: { userId: "different-owner" },
      }),
    })
    expect(result).toEqual({ state: "denied", rotated: false })
    expect(await loadNativeSessionCredential(loadStore)).toBeNull()
  })

  test("retains the credential but reports unavailable on network or schema failure", async () => {
    const network = await recoverVerifiedNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async () => {
        throw new Error("offline with access-original")
      },
    })
    expect(network).toEqual({ state: "unavailable", rotated: false })
    expect(await loadNativeSessionCredential(loadStore)).toEqual(original)

    const malformed = await recoverVerifiedNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async () => response(200, { authenticated: true }),
    })
    expect(malformed).toEqual({ state: "unavailable", rotated: false })
    expect(await loadNativeSessionCredential(loadStore)).toEqual(original)

    const invalidRotation = await recoverVerifiedNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async () => response(200, {
        authenticated: true,
        user: { userId: "owner.fixture" },
        tokens: {
          access: "access-would-be-invalid",
          refresh: "refresh-would-be-invalid",
          expiresIn: 0,
        },
      }),
    })
    expect(invalidRotation).toEqual({ state: "unavailable", rotated: false })
    expect(await loadNativeSessionCredential(loadStore)).toEqual(original)
    expect(JSON.stringify([network, malformed, invalidRotation])).not.toContain(
      "access-original",
    )
  })
})
