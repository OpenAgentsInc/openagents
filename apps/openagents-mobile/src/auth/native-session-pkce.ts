import { Schema } from "effect"

declare const require: (id: string) => unknown

import {
  clearNativeSessionCredential,
  loadNativeSessionCredential,
  saveNativeSessionCredential,
  type NativeSessionCredential,
  type SecureStoreLike,
} from "./native-session-vault"
import {
  OPENAGENTS_MOBILE_AUTH_SESSION_URL,
  OPENAGENTS_NATIVE_REFRESH_HEADER,
} from "./native-session-recovery"

export const OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID = "openagents-khala-mobile"
export const OPENAGENTS_MOBILE_OPENAUTH_PROVIDER = "github"
export const OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI = "openagents://auth"
export const OPENAGENTS_MOBILE_OPENAUTH_DISCOVERY = {
  authorizationEndpoint: "https://auth.openagents.com/authorize",
  tokenEndpoint: "https://auth.openagents.com/token",
} as const

export const OPENAGENTS_MOBILE_OPENAUTH_REQUEST = {
  clientId: OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
  codeChallengeMethod: "S256",
  extraParams: { provider: OPENAGENTS_MOBILE_OPENAUTH_PROVIDER },
  redirectUri: OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
  responseType: "code",
  usePKCE: true,
} as const

type AuthSessionResult = Readonly<{
  type: string
  params?: Readonly<Record<string, string>>
}>

export type AuthRequestLike = Readonly<{
  codeVerifier?: string
  promptAsync: (
    discovery: typeof OPENAGENTS_MOBILE_OPENAUTH_DISCOVERY,
    options: Readonly<{ preferEphemeralSession: true }>,
  ) => Promise<AuthSessionResult>
}>

export type TokenResponseLike = Readonly<{
  accessToken: string
  refreshToken?: string
  expiresIn?: number
}>

type FetchLike = (
  input: string,
  init: RequestInit,
) => Promise<Pick<Response, "json" | "ok" | "status">>

const VerifiedSessionSchema = Schema.Struct({
  authenticated: Schema.Literal(true),
  user: Schema.Struct({ userId: Schema.String }),
  tokens: Schema.optional(Schema.Struct({
    access: Schema.String,
    refresh: Schema.String,
    expiresIn: Schema.Number,
  })),
})

const RevokedSessionSchema = Schema.Struct({
  signedOut: Schema.Literal(true),
  accessRevoked: Schema.Literal(true),
  refreshRevoked: Schema.Literal(true),
})

const nativeFetch: FetchLike = async (input, init) => {
  const { fetch } = require("expo/fetch") as typeof import("expo/fetch")
  return fetch(input, init)
}

const createNativeAuthRequest = async (): Promise<AuthRequestLike> => {
  const { AuthRequest, CodeChallengeMethod } = require("expo-auth-session") as typeof import("expo-auth-session")
  return new AuthRequest({
    ...OPENAGENTS_MOBILE_OPENAUTH_REQUEST,
    codeChallengeMethod: CodeChallengeMethod.S256,
  })
}

const exchangeNativeCode = async (
  config: Readonly<{
    clientId: string
    code: string
    extraParams: Readonly<{ code_verifier: string }>
    redirectUri: string
  }>,
  discovery: typeof OPENAGENTS_MOBILE_OPENAUTH_DISCOVERY,
): Promise<TokenResponseLike> => {
  const { exchangeCodeAsync } = require("expo-auth-session") as typeof import("expo-auth-session")
  return exchangeCodeAsync(config, discovery)
}

const nonEmpty = (value: string | undefined): string | null => {
  const normalized = value?.trim() ?? ""
  return normalized === "" ? null : normalized
}

const validLifetime = (value: number | undefined): boolean =>
  value === undefined || (Number.isFinite(value) && value > 0)

export type NativeSessionSignInResult = Readonly<{
  state: "verified" | "cancelled" | "unavailable"
}>

export type NativeSessionSignOutResult = Readonly<{
  state: "signed_out" | "unavailable"
}>

/**
 * Complete one native public-client authorization. One imperative AuthRequest
 * owns both state validation and its S256 verifier; credentials never leave
 * this host boundary.
 */
export const signInNativeSession = async (input: Readonly<{
  createAuthRequest?: () => Promise<AuthRequestLike>
  exchangeCode?: typeof exchangeNativeCode
  fetchImpl?: FetchLike
  secureStoreLoader?: () => Promise<SecureStoreLike>
}> = {}): Promise<NativeSessionSignInResult> => {
  try {
    const request = await (input.createAuthRequest ?? createNativeAuthRequest)()
    const result = await request.promptAsync(OPENAGENTS_MOBILE_OPENAUTH_DISCOVERY, {
      preferEphemeralSession: true,
    })
    if (result.type === "cancel" || result.type === "dismiss") {
      return { state: "cancelled" }
    }
    const code = result.type === "success" ? nonEmpty(result.params?.code) : null
    const verifier = nonEmpty(request.codeVerifier)
    if (code === null || verifier === null) return { state: "unavailable" }

    const exchanged = await (input.exchangeCode ?? exchangeNativeCode)(
      {
        clientId: OPENAGENTS_MOBILE_OPENAUTH_CLIENT_ID,
        code,
        extraParams: { code_verifier: verifier },
        redirectUri: OPENAGENTS_MOBILE_OPENAUTH_REDIRECT_URI,
      },
      OPENAGENTS_MOBILE_OPENAUTH_DISCOVERY,
    )
    const accessToken = nonEmpty(exchanged.accessToken)
    const refreshToken = nonEmpty(exchanged.refreshToken)
    if (
      accessToken === null ||
      refreshToken === null ||
      !validLifetime(exchanged.expiresIn)
    ) return { state: "unavailable" }

    const response = await (input.fetchImpl ?? nativeFetch)(
      OPENAGENTS_MOBILE_AUTH_SESSION_URL,
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${accessToken}`,
          [OPENAGENTS_NATIVE_REFRESH_HEADER]: refreshToken,
        },
      },
    )
    if (!response.ok) return { state: "unavailable" }
    const session = Schema.decodeUnknownSync(VerifiedSessionSchema)(
      await response.json(),
    )
    const ownerUserId = nonEmpty(session.user.userId)
    if (ownerUserId === null) return { state: "unavailable" }

    let credential: NativeSessionCredential = {
      ownerUserId,
      accessToken,
      refreshToken,
    }
    if (session.tokens !== undefined) {
      const rotatedAccess = nonEmpty(session.tokens.access)
      const rotatedRefresh = nonEmpty(session.tokens.refresh)
      if (
        rotatedAccess === null ||
        rotatedRefresh === null ||
        !validLifetime(session.tokens.expiresIn)
      ) return { state: "unavailable" }
      credential = {
        ownerUserId,
        accessToken: rotatedAccess,
        refreshToken: rotatedRefresh,
      }
    }
    await saveNativeSessionCredential(credential, input.secureStoreLoader)
    return { state: "verified" }
  } catch {
    return { state: "unavailable" }
  }
}

/** Revoke both server credentials before deleting the local vault record. */
export const signOutNativeSession = async (input: Readonly<{
  fetchImpl?: FetchLike
  secureStoreLoader?: () => Promise<SecureStoreLike>
}> = {}): Promise<NativeSessionSignOutResult> => {
  try {
    const credential = await loadNativeSessionCredential(input.secureStoreLoader)
    if (credential === null) return { state: "signed_out" }
    const response = await (input.fetchImpl ?? nativeFetch)(
      OPENAGENTS_MOBILE_AUTH_SESSION_URL,
      {
        method: "DELETE",
        headers: {
          authorization: `Bearer ${credential.accessToken}`,
          [OPENAGENTS_NATIVE_REFRESH_HEADER]: credential.refreshToken,
        },
      },
    )
    if (!response.ok) return { state: "unavailable" }
    Schema.decodeUnknownSync(RevokedSessionSchema)(await response.json())
    await clearNativeSessionCredential(input.secureStoreLoader)
    return { state: "signed_out" }
  } catch {
    return { state: "unavailable" }
  }
}
