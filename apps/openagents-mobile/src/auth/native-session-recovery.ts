import { Schema } from "effect"

declare const require: (id: string) => unknown

import {
  clearNativeSessionCredential,
  loadNativeSessionCredential,
  saveNativeSessionCredential,
  type NativeSessionCredential,
  type SecureStoreLike,
} from "./native-session-vault"

export const OPENAGENTS_MOBILE_AUTH_SESSION_URL =
  "https://openagents.com/api/mobile/auth/session"
export const OPENAGENTS_NATIVE_REFRESH_HEADER =
  "x-openagents-refresh-token"

const RotatedTokensSchema = Schema.Struct({
  access: Schema.String,
  refresh: Schema.String,
  expiresIn: Schema.Number,
})

const VerifiedNativeSessionSchema = Schema.Struct({
  authenticated: Schema.Literal(true),
  user: Schema.Struct({ userId: Schema.String }),
  tokens: Schema.optional(RotatedTokensSchema),
})

export type VerifiedNativeSessionRecovery = Readonly<{
  state: "signed_out" | "verified" | "denied" | "unavailable"
  rotated: boolean
}>

type FetchLike = (
  input: string,
  init: RequestInit,
) => Promise<Pick<Response, "json" | "ok" | "status">>

const nativeFetch: FetchLike = async (input, init) => {
  const { fetch } = require("expo/fetch") as typeof import("expo/fetch")
  return fetch(input, init)
}

const normalizedRotation = (
  current: NativeSessionCredential,
  tokens: typeof RotatedTokensSchema.Type,
): NativeSessionCredential | null => {
  const accessToken = tokens.access.trim()
  const refreshToken = tokens.refresh.trim()
  if (
    accessToken === "" ||
    refreshToken === "" ||
    !Number.isFinite(tokens.expiresIn) ||
    tokens.expiresIn <= 0
  ) return null
  return { ...current, accessToken, refreshToken }
}

const denied = (): VerifiedNativeSessionRecovery => ({
  state: "denied",
  rotated: false,
})

const unavailable = (): VerifiedNativeSessionRecovery => ({
  state: "unavailable",
  rotated: false,
})

/**
 * Validate one recovered native credential. Raw values remain inside this
 * host-only module; callers receive only a bounded state and rotation bit.
 */
export const recoverVerifiedNativeSession = async (input: Readonly<{
  fetchImpl?: FetchLike
  secureStoreLoader?: () => Promise<SecureStoreLike>
}> = {}): Promise<VerifiedNativeSessionRecovery> => {
  const loadStore = input.secureStoreLoader
  let credential: NativeSessionCredential | null
  try {
    credential = await loadNativeSessionCredential(loadStore)
  } catch {
    return unavailable()
  }
  if (credential === null) return { state: "signed_out", rotated: false }

  let response: Awaited<ReturnType<FetchLike>>
  try {
    response = await (input.fetchImpl ?? nativeFetch)(
      OPENAGENTS_MOBILE_AUTH_SESSION_URL,
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${credential.accessToken}`,
          [OPENAGENTS_NATIVE_REFRESH_HEADER]: credential.refreshToken,
        },
      },
    )
  } catch {
    return unavailable()
  }

  if (response.status === 401 || response.status === 403) {
    try {
      await clearNativeSessionCredential(loadStore)
      return denied()
    } catch {
      return unavailable()
    }
  }
  if (!response.ok) return unavailable()

  let session: typeof VerifiedNativeSessionSchema.Type
  try {
    session = Schema.decodeUnknownSync(VerifiedNativeSessionSchema)(
      await response.json(),
    )
  } catch {
    return unavailable()
  }

  if (session.user.userId.trim() !== credential.ownerUserId) {
    try {
      await clearNativeSessionCredential(loadStore)
      return denied()
    } catch {
      return unavailable()
    }
  }

  if (session.tokens === undefined) {
    return { state: "verified", rotated: false }
  }

  const rotatedCredential = normalizedRotation(credential, session.tokens)
  if (rotatedCredential === null) return unavailable()
  try {
    await saveNativeSessionCredential(rotatedCredential, loadStore)
    return { state: "verified", rotated: true }
  } catch {
    return unavailable()
  }
}
