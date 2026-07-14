import { beforeEach, describe, expect, test } from "vite-plus/test"

import {
  OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
  OPENAGENTS_MOBILE_OPENAUTH_DISCOVERY,
  OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
  OPENAGENTS_MOBILE_OPENAUTH_REQUEST,
  signInNativeSession,
  signOutNativeSession,
  type AuthRequestLike,
} from "../src/auth/native-session-pkce"
import {
  OPENAGENTS_MOBILE_AUTH_SESSION_URL,
  OPENAGENTS_NATIVE_REFRESH_HEADER,
} from "../src/auth/native-session-recovery"
import {
  loadNativeSessionCredential,
  saveNativeSessionCredential,
  type SecureStoreLike,
} from "../src/auth/native-session-vault"

const values = new Map<string, string>()
const store: SecureStoreLike = {
  AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: "device-only",
  deleteItemAsync: async key => { values.delete(key) },
  getItemAsync: async key => values.get(key) ?? null,
  setItemAsync: async (key, value) => { values.set(key, value) },
}
const loadStore = async () => store

const response = (
  status: number,
  body: unknown,
): Pick<Response, "json" | "ok" | "status"> => ({
  status,
  ok: status >= 200 && status < 300,
  json: async () => body,
})

describe("contract openagents_mobile.session.pkce_sign_in_sign_out.v1", () => {
  beforeEach(() => values.clear())

  test("uses one imperative GitHub S256 request, an ephemeral prompt, and canonical redirect", async () => {
    const promptCalls: Array<unknown> = []
    const exchangeCalls: Array<unknown> = []
    const fetchCalls: Array<unknown> = []
    const request: AuthRequestLike = {
      codeVerifier: "verifier-fixture",
      promptAsync: async (discovery, options) => {
        promptCalls.push({ discovery, options })
        return { type: "success", params: { code: "code-fixture" } }
      },
    }
    let created = 0
    const result = await signInNativeSession({
      createAuthRequest: async () => {
        created += 1
        return request
      },
      exchangeCode: async (config, discovery) => {
        exchangeCalls.push({ config, discovery })
        return {
          accessToken: "access-fixture",
          refreshToken: "refresh-fixture",
          expiresIn: 3600,
        }
      },
      fetchImpl: async (url, init) => {
        fetchCalls.push({ url, init })
        return response(200, {
          authenticated: true,
          user: { userId: "owner.fixture" },
        })
      },
      secureStoreLoader: loadStore,
    })

    expect(OPENAGENTS_MOBILE_OPENAUTH_REQUEST).toEqual({
      clientId: OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
      codeChallengeMethod: "S256",
      extraParams: { provider: "github" },
      redirectUri: OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
      responseType: "code",
      usePKCE: true,
    })
    expect(OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI).toBe("openagents://auth")
    expect(created).toBe(1)
    expect(promptCalls).toEqual([{
      discovery: OPENAGENTS_MOBILE_OPENAUTH_DISCOVERY,
      options: { preferEphemeralSession: true },
    }])
    expect(exchangeCalls).toEqual([{
      config: {
        clientId: "openagents-khala-mobile",
        code: "code-fixture",
        extraParams: { code_verifier: "verifier-fixture" },
        redirectUri: "openagents://auth",
      },
      discovery: OPENAGENTS_MOBILE_OPENAUTH_DISCOVERY,
    }])
    expect(fetchCalls).toEqual([{
      url: OPENAGENTS_MOBILE_AUTH_SESSION_URL,
      init: {
        method: "GET",
        headers: {
          authorization: "Bearer access-fixture",
          [OPENAGENTS_NATIVE_REFRESH_HEADER]: "refresh-fixture",
        },
      },
    }])
    expect(result).toEqual({ state: "verified" })
    expect(await loadNativeSessionCredential(loadStore)).toEqual({
      ownerUserId: "owner.fixture",
      accessToken: "access-fixture",
      refreshToken: "refresh-fixture",
    })
    expect(JSON.stringify(result)).not.toContain("fixture")
  })

  test("persists an immediate server rotation instead of exchanged tokens", async () => {
    const result = await signInNativeSession({
      createAuthRequest: async () => ({
        codeVerifier: "verifier",
        promptAsync: async () => ({ type: "success", params: { code: "code" } }),
      }),
      exchangeCode: async () => ({
        accessToken: "access-exchanged",
        refreshToken: "refresh-exchanged",
      }),
      fetchImpl: async () => response(200, {
        authenticated: true,
        user: { userId: "owner.fixture" },
        tokens: {
          access: "access-rotated",
          refresh: "refresh-rotated",
          expiresIn: 3600,
        },
      }),
      secureStoreLoader: loadStore,
    })
    expect(result).toEqual({ state: "verified" })
    expect(await loadNativeSessionCredential(loadStore)).toEqual({
      ownerUserId: "owner.fixture",
      accessToken: "access-rotated",
      refreshToken: "refresh-rotated",
    })
  })

  test("cancellation is quiet and state/error/missing-token failures save nothing", async () => {
    const cancelled = await signInNativeSession({
      createAuthRequest: async () => ({
        codeVerifier: "unused",
        promptAsync: async () => ({ type: "cancel" }),
      }),
      secureStoreLoader: loadStore,
    })
    expect(cancelled).toEqual({ state: "cancelled" })

    const stateMismatch = await signInNativeSession({
      createAuthRequest: async () => ({
        codeVerifier: "verifier",
        promptAsync: async () => ({
          type: "error",
          params: { code: "must-not-bypass-state-validation" },
        }),
      }),
      secureStoreLoader: loadStore,
    })
    expect(stateMismatch).toEqual({ state: "unavailable" })

    const missingRefresh = await signInNativeSession({
      createAuthRequest: async () => ({
        codeVerifier: "verifier",
        promptAsync: async () => ({ type: "success", params: { code: "code" } }),
      }),
      exchangeCode: async () => ({ accessToken: "access" }),
      secureStoreLoader: loadStore,
    })
    expect(missingRefresh).toEqual({ state: "unavailable" })
    expect(await loadNativeSessionCredential(loadStore)).toBeNull()
  })

  test("revokes access and refresh before clearing the vault", async () => {
    await saveNativeSessionCredential({
      ownerUserId: "owner.fixture",
      accessToken: "access-fixture",
      refreshToken: "refresh-fixture",
    }, loadStore)
    let presentDuringRequest = false
    const result = await signOutNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async (url, init) => {
        presentDuringRequest = (await loadNativeSessionCredential(loadStore)) !== null
        expect(url).toBe(OPENAGENTS_MOBILE_AUTH_SESSION_URL)
        expect(init).toEqual({
          method: "DELETE",
          headers: {
            authorization: "Bearer access-fixture",
            [OPENAGENTS_NATIVE_REFRESH_HEADER]: "refresh-fixture",
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
    expect(result).toEqual({ state: "signed_out" })
    expect(await loadNativeSessionCredential(loadStore)).toBeNull()
  })

  test("retains the vault unless the server proves both revocations", async () => {
    const credential = {
      ownerUserId: "owner.fixture",
      accessToken: "access-fixture",
      refreshToken: "refresh-fixture",
    }
    await saveNativeSessionCredential(credential, loadStore)
    const unavailable = await signOutNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async () => response(503, {}),
    })
    expect(unavailable).toEqual({ state: "unavailable" })
    expect(await loadNativeSessionCredential(loadStore)).toEqual(credential)

    const incomplete = await signOutNativeSession({
      secureStoreLoader: loadStore,
      fetchImpl: async () => response(200, {
        signedOut: true,
        accessRevoked: true,
        refreshRevoked: false,
      }),
    })
    expect(incomplete).toEqual({ state: "unavailable" })
    expect(await loadNativeSessionCredential(loadStore)).toEqual(credential)
  })
})
