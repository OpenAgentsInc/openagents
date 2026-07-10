import { Schema } from "effect"

import type {
  DesktopSessionCredential,
  DesktopSessionVault,
} from "./desktop-session-vault.ts"

export const OPENAGENTS_DESKTOP_AUTH_SESSION_URL =
  "https://openagents.com/api/mobile/auth/session"
export const OPENAGENTS_DESKTOP_REFRESH_HEADER =
  "x-openagents-refresh-token"

const VerifiedDesktopSessionSchema = Schema.Struct({
  authenticated: Schema.Literal(true),
  user: Schema.Struct({ userId: Schema.String }),
  tokens: Schema.optional(Schema.Struct({
    access: Schema.String,
    refresh: Schema.String,
    expiresIn: Schema.Number,
  })),
})

type FetchLike = (
  input: string,
  init: RequestInit,
) => Promise<Pick<Response, "json" | "ok" | "status">>

export type VerifiedDesktopSessionRecovery = Readonly<{
  state: "signed_out" | "verified" | "denied" | "unavailable"
  rotated: boolean
}>

const unavailable = (): VerifiedDesktopSessionRecovery => ({
  state: "unavailable",
  rotated: false,
})

const denied = (): VerifiedDesktopSessionRecovery => ({
  state: "denied",
  rotated: false,
})

const normalizedRotation = (
  current: DesktopSessionCredential,
  tokens: typeof VerifiedDesktopSessionSchema.Type["tokens"] & {},
): DesktopSessionCredential | null => {
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

/**
 * Verify one recovered Desktop native credential inside Electron main. Raw
 * values remain within this host service; callers receive bounded state only.
 */
export const recoverVerifiedDesktopSession = async (input: Readonly<{
  vault: DesktopSessionVault
  fetchImpl?: FetchLike
}>): Promise<VerifiedDesktopSessionRecovery> => {
  let credential: DesktopSessionCredential | null
  try {
    credential = input.vault.load()
  } catch {
    return unavailable()
  }
  if (credential === null) return { state: "signed_out", rotated: false }

  let response: Awaited<ReturnType<FetchLike>>
  try {
    response = await (input.fetchImpl ?? fetch)(
      OPENAGENTS_DESKTOP_AUTH_SESSION_URL,
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${credential.accessToken}`,
          [OPENAGENTS_DESKTOP_REFRESH_HEADER]: credential.refreshToken,
        },
      },
    )
  } catch {
    return unavailable()
  }

  if (response.status === 401 || response.status === 403) {
    try {
      input.vault.clear()
      return denied()
    } catch {
      return unavailable()
    }
  }
  if (!response.ok) return unavailable()

  let session: typeof VerifiedDesktopSessionSchema.Type
  try {
    session = Schema.decodeUnknownSync(VerifiedDesktopSessionSchema)(
      await response.json(),
    )
  } catch {
    return unavailable()
  }

  if (session.user.userId.trim() !== credential.ownerUserId) {
    try {
      input.vault.clear()
      return denied()
    } catch {
      return unavailable()
    }
  }
  if (session.tokens === undefined) {
    return { state: "verified", rotated: false }
  }

  const rotated = normalizedRotation(credential, session.tokens)
  if (rotated === null) return unavailable()
  try {
    input.vault.save(rotated)
    return { state: "verified", rotated: true }
  } catch {
    return unavailable()
  }
}
