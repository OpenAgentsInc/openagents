import { createHash, randomBytes } from "node:crypto"
import { createServer, type Server } from "node:http"

import { Schema } from "effect"

import type {
  DesktopSessionCredential,
  DesktopSessionVault,
} from "./desktop-session-vault.ts"
import {
  OPENAGENTS_DESKTOP_AUTH_SESSION_URL,
  OPENAGENTS_DESKTOP_REFRESH_HEADER,
} from "./desktop-session-recovery.ts"

export const OPENAGENTS_DESKTOP_OPENAUTH_CLIENT_ID = "openagents-desktop"
export const OPENAGENTS_DESKTOP_OPENAUTH_AUTHORIZE_URL =
  "https://auth.openagents.com/authorize"
export const OPENAGENTS_DESKTOP_OPENAUTH_TOKEN_URL =
  "https://auth.openagents.com/token"
export const OPENAGENTS_DESKTOP_OPENAUTH_LOOPBACK_HOST = "127.0.0.1"
export const OPENAGENTS_DESKTOP_OPENAUTH_LOOPBACK_PATH = "/auth/callback"
export const OPENAGENTS_DESKTOP_OPENAUTH_TIMEOUT_MS = 120_000

const TokenResponseSchema = Schema.Struct({
  access_token: Schema.String,
  refresh_token: Schema.String,
  expires_in: Schema.optional(Schema.Number),
})

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

type FetchLike = (
  input: string,
  init: RequestInit,
) => Promise<Pick<Response, "json" | "ok" | "status">>

type LoopbackCallback =
  | Readonly<{ state: "code"; code: string }>
  | Readonly<{ state: "cancelled" }>
  | Readonly<{ state: "unavailable" }>

export type DesktopAuthLoopbackListener = Readonly<{
  redirectUri: string
  waitForCallback: () => Promise<LoopbackCallback>
  close: () => void
}>

const callbackPage = `<!doctype html><html><head><meta charset="utf-8"><meta name="referrer" content="no-referrer"><title>OpenAgents</title></head><body><p>You can return to OpenAgents Desktop.</p></body></html>`

const writeCallbackResponse = (
  response: import("node:http").ServerResponse,
  status: number,
): void => {
  response.writeHead(status, {
    "cache-control": "no-store",
    "content-security-policy": "default-src 'none'; style-src 'unsafe-inline'",
    "content-type": "text/html; charset=utf-8",
    "referrer-policy": "no-referrer",
    "x-content-type-options": "nosniff",
  })
  response.end(callbackPage)
}

export const openDesktopAuthLoopbackListener = async (input: Readonly<{
  state: string
  timeoutMs?: number
}>): Promise<DesktopAuthLoopbackListener> => {
  let server: Server | null = null
  let settled = false
  let resolveCallback: (callback: LoopbackCallback) => void = () => undefined
  const callback = new Promise<LoopbackCallback>(resolve => {
    resolveCallback = resolve
  })
  let timeout: ReturnType<typeof setTimeout> | undefined

  const close = (): void => {
    if (timeout !== undefined) clearTimeout(timeout)
    timeout = undefined
    server?.close()
    server = null
  }
  const finish = (result: LoopbackCallback): void => {
    if (settled) return
    settled = true
    close()
    resolveCallback(result)
  }

  server = createServer((request, response) => {
    if (request.method !== "GET" || request.url === undefined) {
      writeCallbackResponse(response, 400)
      return
    }
    const url = new URL(request.url, "http://127.0.0.1")
    if (url.pathname !== OPENAGENTS_DESKTOP_OPENAUTH_LOOPBACK_PATH) {
      writeCallbackResponse(response, 404)
      return
    }
    if (url.searchParams.get("state") !== input.state) {
      writeCallbackResponse(response, 400)
      return
    }
    const error = url.searchParams.get("error")?.trim() ?? ""
    if (error !== "") {
      writeCallbackResponse(response, 200)
      finish({ state: "cancelled" })
      return
    }
    const code = url.searchParams.get("code")?.trim() ?? ""
    if (code === "") {
      writeCallbackResponse(response, 400)
      return
    }
    writeCallbackResponse(response, 200)
    finish({ state: "code", code })
  })

  await new Promise<void>((resolve, reject) => {
    server?.once("error", reject)
    server?.listen(0, OPENAGENTS_DESKTOP_OPENAUTH_LOOPBACK_HOST, () => resolve())
  }).catch(error => {
    close()
    throw error
  })

  const address = server.address()
  if (address === null || typeof address === "string" || address.port < 1024) {
    close()
    throw new Error("desktop auth loopback listener is unavailable")
  }
  timeout = setTimeout(
    () => finish({ state: "unavailable" }),
    input.timeoutMs ?? OPENAGENTS_DESKTOP_OPENAUTH_TIMEOUT_MS,
  )

  return {
    redirectUri: `http://${OPENAGENTS_DESKTOP_OPENAUTH_LOOPBACK_HOST}:${address.port}${OPENAGENTS_DESKTOP_OPENAUTH_LOOPBACK_PATH}`,
    waitForCallback: () => callback,
    close,
  }
}

const nonEmpty = (value: string | undefined): string | null => {
  const normalized = value?.trim() ?? ""
  return normalized === "" ? null : normalized
}

const randomBase64Url = (): string => randomBytes(32).toString("base64url")

const pkceChallenge = (verifier: string): string =>
  createHash("sha256").update(verifier, "ascii").digest("base64url")

export type DesktopSessionSignInResult = Readonly<{
  state: "verified" | "cancelled" | "unavailable"
}>

export type DesktopSessionSignOutResult = Readonly<{
  state: "signed_out" | "unavailable"
}>

export const signInDesktopSession = async (input: Readonly<{
  vault: DesktopSessionVault
  openExternal: (url: string) => Promise<unknown>
  fetchImpl?: FetchLike
  timeoutMs?: number
}>): Promise<DesktopSessionSignInResult> => {
  const state = randomBase64Url()
  const verifier = randomBase64Url()
  let listener: DesktopAuthLoopbackListener | undefined
  try {
    listener = await openDesktopAuthLoopbackListener({
      state,
      ...(input.timeoutMs === undefined ? {} : { timeoutMs: input.timeoutMs }),
    })
    const authorize = new URL(OPENAGENTS_DESKTOP_OPENAUTH_AUTHORIZE_URL)
    authorize.search = new URLSearchParams({
      client_id: OPENAGENTS_DESKTOP_OPENAUTH_CLIENT_ID,
      code_challenge: pkceChallenge(verifier),
      code_challenge_method: "S256",
      provider: "github",
      redirect_uri: listener.redirectUri,
      response_type: "code",
      state,
    }).toString()
    await input.openExternal(authorize.toString())

    const callback = await listener.waitForCallback()
    if (callback.state === "cancelled") return { state: "cancelled" }
    if (callback.state !== "code") return { state: "unavailable" }

    const tokenResponse = await (input.fetchImpl ?? fetch)(
      OPENAGENTS_DESKTOP_OPENAUTH_TOKEN_URL,
      {
        method: "POST",
        headers: { "content-type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({
          client_id: OPENAGENTS_DESKTOP_OPENAUTH_CLIENT_ID,
          code: callback.code,
          code_verifier: verifier,
          grant_type: "authorization_code",
          redirect_uri: listener.redirectUri,
        }).toString(),
      },
    )
    if (!tokenResponse.ok) return { state: "unavailable" }
    const tokens = Schema.decodeUnknownSync(TokenResponseSchema)(
      await tokenResponse.json(),
    )
    const accessToken = nonEmpty(tokens.access_token)
    const refreshToken = nonEmpty(tokens.refresh_token)
    if (
      accessToken === null ||
      refreshToken === null ||
      (tokens.expires_in !== undefined &&
        (!Number.isFinite(tokens.expires_in) || tokens.expires_in <= 0))
    ) return { state: "unavailable" }

    const sessionResponse = await (input.fetchImpl ?? fetch)(
      OPENAGENTS_DESKTOP_AUTH_SESSION_URL,
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${accessToken}`,
          [OPENAGENTS_DESKTOP_REFRESH_HEADER]: refreshToken,
        },
      },
    )
    if (!sessionResponse.ok) return { state: "unavailable" }
    const session = Schema.decodeUnknownSync(VerifiedSessionSchema)(
      await sessionResponse.json(),
    )
    const ownerUserId = nonEmpty(session.user.userId)
    if (ownerUserId === null) return { state: "unavailable" }

    let credential: DesktopSessionCredential = {
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
        !Number.isFinite(session.tokens.expiresIn) ||
        session.tokens.expiresIn <= 0
      ) return { state: "unavailable" }
      credential = {
        ownerUserId,
        accessToken: rotatedAccess,
        refreshToken: rotatedRefresh,
      }
    }
    input.vault.save(credential)
    return { state: "verified" }
  } catch {
    return { state: "unavailable" }
  } finally {
    listener?.close()
  }
}

export const signOutDesktopSession = async (input: Readonly<{
  vault: DesktopSessionVault
  fetchImpl?: FetchLike
}>): Promise<DesktopSessionSignOutResult> => {
  try {
    const credential = input.vault.load()
    if (credential === null) return { state: "signed_out" }
    const response = await (input.fetchImpl ?? fetch)(
      OPENAGENTS_DESKTOP_AUTH_SESSION_URL,
      {
        method: "DELETE",
        headers: {
          authorization: `Bearer ${credential.accessToken}`,
          [OPENAGENTS_DESKTOP_REFRESH_HEADER]: credential.refreshToken,
        },
      },
    )
    if (!response.ok) return { state: "unavailable" }
    Schema.decodeUnknownSync(RevokedSessionSchema)(await response.json())
    input.vault.clear()
    return { state: "signed_out" }
  } catch {
    return { state: "unavailable" }
  }
}
