import { beforeEach, describe, expect, test } from "bun:test"

import {
  OPENAGENTS_DESKTOP_OPENAUTH_AUTHORIZE_URL,
  OPENAGENTS_DESKTOP_OPENAUTH_CLIENT_ID,
  OPENAGENTS_DESKTOP_OPENAUTH_TOKEN_URL,
  openDesktopAuthLoopbackListener,
  signInDesktopSession,
  signOutDesktopSession,
} from "../src/desktop-session-pkce.ts"
import {
  OPENAGENTS_DESKTOP_AUTH_SESSION_URL,
  OPENAGENTS_DESKTOP_REFRESH_HEADER,
} from "../src/desktop-session-recovery.ts"
import type {
  DesktopSessionCredential,
  DesktopSessionVault,
} from "../src/desktop-session-vault.ts"

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

describe("contract openagents_desktop.session.loopback_pkce_entry_exit.v1", () => {
  beforeEach(() => { stored = null })

  test("binds literal loopback and ignores invalid callbacks until exact state + code", async () => {
    const listener = await openDesktopAuthLoopbackListener({
      state: "state-fixture",
      timeoutMs: 2_000,
    })
    try {
      const redirect = new URL(listener.redirectUri)
      expect(redirect.hostname).toBe("127.0.0.1")
      expect(redirect.pathname).toBe("/auth/callback")
      expect(Number(redirect.port)).toBeGreaterThanOrEqual(1024)

      const wrong = await fetch(`${listener.redirectUri}?state=wrong&code=private-code`)
      expect(wrong.status).toBe(400)
      expect(await wrong.text()).not.toContain("private-code")

      const valid = await fetch(`${listener.redirectUri}?state=state-fixture&code=code-fixture`)
      expect(valid.status).toBe(200)
      expect(await valid.text()).not.toContain("code-fixture")
      expect(await listener.waitForCallback()).toEqual({
        state: "code",
        code: "code-fixture",
      })
    } finally {
      listener.close()
    }
  })

  test("builds exact authorize/exchange tuples, verifies server owner, and saves rotation", async () => {
    let authorizeUrl = ""
    const fetchCalls: Array<Readonly<{ input: string; init: RequestInit }>> = []
    const result = await signInDesktopSession({
      vault,
      timeoutMs: 2_000,
      openExternal: async url => {
        authorizeUrl = url
        const authorize = new URL(url)
        const redirectUri = authorize.searchParams.get("redirect_uri")
        const state = authorize.searchParams.get("state")
        if (redirectUri === null || state === null) return
        setTimeout(() => {
          void fetch(`${redirectUri}?state=${encodeURIComponent(state)}&code=code-fixture`)
        }, 0)
      },
      fetchImpl: async (input, init) => {
        fetchCalls.push({ input, init })
        if (input === OPENAGENTS_DESKTOP_OPENAUTH_TOKEN_URL) {
          return response(200, {
            access_token: "access-exchanged",
            refresh_token: "refresh-exchanged",
            expires_in: 3600,
          })
        }
        return response(200, {
          authenticated: true,
          user: { userId: "owner.fixture" },
          tokens: {
            access: "access-rotated",
            refresh: "refresh-rotated",
            expiresIn: 3600,
          },
        })
      },
    })

    const authorize = new URL(authorizeUrl)
    expect(`${authorize.origin}${authorize.pathname}`).toBe(
      OPENAGENTS_DESKTOP_OPENAUTH_AUTHORIZE_URL,
    )
    expect(authorize.searchParams.get("client_id")).toBe(
      OPENAGENTS_DESKTOP_OPENAUTH_CLIENT_ID,
    )
    expect(authorize.searchParams.get("provider")).toBe("github")
    expect(authorize.searchParams.get("response_type")).toBe("code")
    expect(authorize.searchParams.get("code_challenge_method")).toBe("S256")
    expect(authorize.searchParams.get("code_challenge")).toMatch(/^[A-Za-z0-9_-]{43}$/)
    expect(authorize.searchParams.get("state")).toMatch(/^[A-Za-z0-9_-]{43}$/)
    expect(authorize.searchParams.get("redirect_uri")).toMatch(
      /^http:\/\/127\.0\.0\.1:\d+\/auth\/callback$/,
    )

    expect(fetchCalls).toHaveLength(2)
    expect(fetchCalls[0]?.input).toBe(OPENAGENTS_DESKTOP_OPENAUTH_TOKEN_URL)
    const exchangeBody = new URLSearchParams(String(fetchCalls[0]?.init.body))
    expect(exchangeBody.get("client_id")).toBe("openagents-desktop")
    expect(exchangeBody.get("grant_type")).toBe("authorization_code")
    expect(exchangeBody.get("code")).toBe("code-fixture")
    expect(exchangeBody.get("redirect_uri")).toBe(
      authorize.searchParams.get("redirect_uri"),
    )
    expect(exchangeBody.get("code_verifier")).toMatch(/^[A-Za-z0-9_-]{43}$/)
    expect(fetchCalls[1]).toEqual({
      input: OPENAGENTS_DESKTOP_AUTH_SESSION_URL,
      init: {
        method: "GET",
        headers: {
          authorization: "Bearer access-exchanged",
          [OPENAGENTS_DESKTOP_REFRESH_HEADER]: "refresh-exchanged",
        },
      },
    })
    expect(stored).toEqual({
      ownerUserId: "owner.fixture",
      accessToken: "access-rotated",
      refreshToken: "refresh-rotated",
    })
    expect(result).toEqual({ state: "verified" })
    expect(JSON.stringify(result)).not.toContain("fixture")
  })

  test("treats a state-valid OAuth error as cancellation and saves nothing", async () => {
    const result = await signInDesktopSession({
      vault,
      timeoutMs: 2_000,
      openExternal: async url => {
        const authorize = new URL(url)
        const redirectUri = authorize.searchParams.get("redirect_uri")!
        const state = authorize.searchParams.get("state")!
        setTimeout(() => {
          void fetch(`${redirectUri}?state=${encodeURIComponent(state)}&error=access_denied`)
        }, 0)
      },
      fetchImpl: async () => {
        throw new Error("exchange must not run")
      },
    })
    expect(result).toEqual({ state: "cancelled" })
    expect(stored).toBeNull()
  })

  test("times out and closes without saving", async () => {
    const result = await signInDesktopSession({
      vault,
      timeoutMs: 10,
      openExternal: async () => undefined,
    })
    expect(result).toEqual({ state: "unavailable" })
    expect(stored).toBeNull()
  })

  test("revokes both credential classes before clearing and retains on incomplete proof", async () => {
    const credential = {
      ownerUserId: "owner.fixture",
      accessToken: "access-fixture",
      refreshToken: "refresh-fixture",
    }
    stored = credential
    let presentDuringRequest = false
    const signedOut = await signOutDesktopSession({
      vault,
      fetchImpl: async (input, init) => {
        presentDuringRequest = stored !== null
        expect(input).toBe(OPENAGENTS_DESKTOP_AUTH_SESSION_URL)
        expect(init).toEqual({
          method: "DELETE",
          headers: {
            authorization: "Bearer access-fixture",
            [OPENAGENTS_DESKTOP_REFRESH_HEADER]: "refresh-fixture",
          },
        })
        return response(200, {
          signedOut: true,
          accessRevoked: true,
          refreshRevoked: true,
        })
      },
    })
    expect(presentDuringRequest).toBe(true)
    expect(signedOut).toEqual({ state: "signed_out" })
    expect(stored).toBeNull()

    stored = credential
    const incomplete = await signOutDesktopSession({
      vault,
      fetchImpl: async () => response(200, {
        signedOut: true,
        accessRevoked: true,
        refreshRevoked: false,
      }),
    })
    expect(incomplete).toEqual({ state: "unavailable" })
    expect(stored).toEqual(credential)
  })
})
