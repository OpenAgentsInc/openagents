import { beforeEach, describe, expect, test } from "bun:test"

import {
  OPENAGENTS_DESKTOP_AUTH_SESSION_URL,
  OPENAGENTS_DESKTOP_REFRESH_HEADER,
  recoverVerifiedDesktopSession,
} from "../src/desktop-session-recovery.ts"
import type {
  DesktopSessionCredential,
  DesktopSessionVault,
} from "../src/desktop-session-vault.ts"

const original: DesktopSessionCredential = {
  ownerUserId: "owner.fixture",
  accessToken: "access-original",
  refreshToken: "refresh-original",
}

let stored: DesktopSessionCredential | null
const vault: DesktopSessionVault = {
  save: credential => { stored = credential },
  load: () => stored,
  clear: () => { stored = null },
  recover: () => ({
    state: stored === null ? "signed_out" : "credential_present_unverified",
  }),
}

const response = (
  status: number,
  body: unknown,
): Pick<Response, "json" | "ok" | "status"> => ({
  status,
  ok: status >= 200 && status < 300,
  json: async () => body,
})

describe("contract openagents_desktop.session.recovered_validation_rotation.v1", () => {
  beforeEach(() => { stored = original })

  test("verifies through the exact bearer and bounded refresh headers", async () => {
    const calls: Array<Readonly<{ input: string; init: RequestInit }>> = []
    const result = await recoverVerifiedDesktopSession({
      vault,
      fetchImpl: async (input, init) => {
        calls.push({ input, init })
        return response(200, {
          authenticated: true,
          user: { userId: "owner.fixture", email: "private@example.test" },
        })
      },
    })
    expect(calls).toEqual([{
      input: OPENAGENTS_DESKTOP_AUTH_SESSION_URL,
      init: {
        method: "GET",
        headers: {
          authorization: "Bearer access-original",
          [OPENAGENTS_DESKTOP_REFRESH_HEADER]: "refresh-original",
        },
      },
    }])
    expect(result).toEqual({ state: "verified", rotated: false })
    expect(JSON.stringify(result)).not.toContain("owner.fixture")
    expect(JSON.stringify(result)).not.toContain("original")
  })

  test("rewrites the encrypted vault before projecting a rotated verified session", async () => {
    const writes: Array<DesktopSessionCredential> = []
    const recordingVault: DesktopSessionVault = {
      ...vault,
      save: credential => {
        writes.push(credential)
        stored = credential
      },
    }
    const result = await recoverVerifiedDesktopSession({
      vault: recordingVault,
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
    expect(writes).toEqual([{
      ownerUserId: "owner.fixture",
      accessToken: "access-rotated",
      refreshToken: "refresh-rotated",
    }])
    expect(result).toEqual({ state: "verified", rotated: true })
  })

  test("purges denied credentials and server-derived owner mismatch", async () => {
    const rejected = await recoverVerifiedDesktopSession({
      vault,
      fetchImpl: async () => response(401, { authenticated: false }),
    })
    expect(rejected).toEqual({ state: "denied", rotated: false })
    expect(stored).toBeNull()

    stored = original
    const mismatch = await recoverVerifiedDesktopSession({
      vault,
      fetchImpl: async () => response(200, {
        authenticated: true,
        user: { userId: "different-owner" },
      }),
    })
    expect(mismatch).toEqual({ state: "denied", rotated: false })
    expect(stored).toBeNull()
  })

  test("retains custody but reports unavailable on transient, schema, or rotation failure", async () => {
    const network = await recoverVerifiedDesktopSession({
      vault,
      fetchImpl: async () => { throw new Error("offline access-original") },
    })
    expect(network).toEqual({ state: "unavailable", rotated: false })
    expect(stored).toEqual(original)

    const malformed = await recoverVerifiedDesktopSession({
      vault,
      fetchImpl: async () => response(200, { authenticated: true }),
    })
    expect(malformed).toEqual({ state: "unavailable", rotated: false })
    expect(stored).toEqual(original)

    const invalidRotation = await recoverVerifiedDesktopSession({
      vault,
      fetchImpl: async () => response(200, {
        authenticated: true,
        user: { userId: "owner.fixture" },
        tokens: { access: "next", refresh: "next", expiresIn: 0 },
      }),
    })
    expect(invalidRotation).toEqual({ state: "unavailable", rotated: false })
    expect(stored).toEqual(original)
  })

  test("returns signed out without a network call when custody is empty", async () => {
    stored = null
    let called = false
    const result = await recoverVerifiedDesktopSession({
      vault,
      fetchImpl: async () => {
        called = true
        return response(500, {})
      },
    })
    expect(result).toEqual({ state: "signed_out", rotated: false })
    expect(called).toBe(false)
  })
})
