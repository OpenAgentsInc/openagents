import { describe, expect, test } from "bun:test"

import {
  fetchMobileSyncSession,
  mobileOpenAuthDiscovery,
  mobileOpenAuthRequestConfig,
  mobileOpenAuthTokenExchangeConfig,
  normalizeHttpsBaseUrl,
  KHALA_MOBILE_OPENAUTH_CLIENT_ID,
} from "../src/auth/mobile-openauth"

describe("mobile OpenAuth boundary", () => {
  test("builds the GitHub PKCE request accepted by the OpenAuth mobile issuer", () => {
    expect(mobileOpenAuthDiscovery("https://auth.openagents.com/")).toEqual({
      authorizationEndpoint: "https://auth.openagents.com/authorize",
      tokenEndpoint: "https://auth.openagents.com/token",
    })

    expect(mobileOpenAuthRequestConfig("khala://auth")).toEqual({
      clientId: KHALA_MOBILE_OPENAUTH_CLIENT_ID,
      codeChallengeMethod: "S256",
      extraParams: { provider: "github" },
      redirectUri: "khala://auth",
      responseType: "code",
      usePKCE: true,
    })

    expect(
      mobileOpenAuthTokenExchangeConfig({
        code: "code-123",
        codeVerifier: "verifier-123",
        redirectUri: "khala://auth",
      }),
    ).toEqual({
      clientId: KHALA_MOBILE_OPENAUTH_CLIENT_ID,
      code: "code-123",
      extraParams: { code_verifier: "verifier-123" },
      redirectUri: "khala://auth",
    })
  })

  test("rejects non-HTTPS public auth endpoints", () => {
    expect(() => normalizeHttpsBaseUrl("http://openagents.test", "test URL")).toThrow(
      "test URL must use https",
    )
  })

  test("exchanges a verified mobile user bearer for the stored sync credential shape", async () => {
    const calls: Array<{ init: RequestInit; url: string }> = []
    const session = await fetchMobileSyncSession({
      accessToken: "access-token",
      apiBaseUrl: "https://openagents.com/",
      fetchImpl: async (url, init) => {
        calls.push({ init, url })

        return {
          json: async () => ({
            ownerUserId: " github:12345 ",
            syncToken: " mobile-sync-token ",
          }),
          ok: true,
          status: 200,
          statusText: "OK",
        }
      },
    })

    expect(calls).toEqual([
      {
        init: {
          headers: { authorization: "Bearer access-token" },
          method: "POST",
        },
        url: "https://openagents.com/api/mobile/session",
      },
    ])
    expect(session).toEqual({
      ownerUserId: "github:12345",
      syncToken: "mobile-sync-token",
    })
  })

  test("surfaces a safe message when the session bridge rejects the bearer", async () => {
    await expect(
      fetchMobileSyncSession({
        accessToken: "bad-token",
        apiBaseUrl: "https://openagents.com",
        fetchImpl: async () => ({
          json: async () => ({ messageSafe: "mobile session: sign-in required" }),
          ok: false,
          status: 401,
          statusText: "Unauthorized",
        }),
      }),
    ).rejects.toThrow("mobile session: sign-in required")
  })
})
