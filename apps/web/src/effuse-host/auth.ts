import { createAuthService, getConfig } from "@workos/authkit-session"
import { Effect } from "effect"

import { WebCookieSessionStorage } from "../auth/sessionCookieStorage"
import {
  decodeE2eJwtClaims,
  E2E_JWT_ISSUER,
  makeE2eClearCookieHeader,
  makeE2eJwks,
  makeE2eSetCookieHeader,
  readE2eTokenFromRequest,
  mintE2eJwt,
} from "../auth/e2eAuth"
import {
  clearSessionCookie,
  sendMagicAuthCode,
  verifyMagicAuthCode,
} from "../auth/workosAuth"
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext"
import { TelemetryService } from "../effect/telemetry"

import { getWorkerRuntime } from "./runtime"
import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId"
import type { WorkerEnv } from "./env"

type SessionPayload = {
  readonly ok: true
  readonly userId: string | null
  readonly sessionId: string | null
  readonly token: string | null
  readonly user: {
    readonly id: string
    readonly email: string | null
    readonly firstName: string | null
    readonly lastName: string | null
  } | null
}

type StartBody = {
  readonly email?: unknown
}

type VerifyBody = {
  readonly email?: unknown
  readonly code?: unknown
}

type E2eLoginBody = {
  readonly seed?: unknown
  readonly email?: unknown
  readonly firstName?: unknown
  readonly lastName?: unknown
}

function json(body: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  })
}

function normalizeEmail(raw: string): string {
  return raw.trim().toLowerCase()
}

function normalizeCode(raw: string): string {
  return raw.replace(/\s+/g, "")
}

const authkit = createAuthService<Request, Response>({
  sessionStorageFactory: (config) => new WebCookieSessionStorage(config),
})

const requireE2eBypassAuth = (request: Request, env: WorkerEnv): boolean => {
  const secret = typeof env.OA_E2E_BYPASS_SECRET === "string" ? env.OA_E2E_BYPASS_SECRET : ""
  if (!secret) return false
  const authz = request.headers.get("authorization") ?? ""
  return authz === `Bearer ${secret}`
}

const handleE2eJwks = async (env: WorkerEnv): Promise<Response> => {
  const privateJwkJson = typeof env.OA_E2E_JWT_PRIVATE_JWK === "string" ? env.OA_E2E_JWT_PRIVATE_JWK : ""
  if (!privateJwkJson) return new Response("Not found", { status: 404 })

  const { runtime } = getWorkerRuntime(env)
  return runtime.runPromise(
    makeE2eJwks({ privateJwkJson }).pipe(
      Effect.match({
        onFailure: () => json({ ok: false, error: "jwks_failed" }, { status: 500 }),
        onSuccess: (jwks) => json(jwks, { status: 200 }),
      }),
    ),
  )
}

const normalizeSeed = (raw: unknown): string | null => {
  if (typeof raw !== "string") return null
  const s = raw.trim()
  if (!s) return null
  if (s.length > 200) return null
  return s
}

const normalizeName = (raw: unknown): string | null => {
  if (typeof raw !== "string") return null
  const s = raw.trim()
  if (!s) return null
  return s.slice(0, 80)
}

const sha256Hex = async (text: string): Promise<string> => {
  const bytes = new TextEncoder().encode(text)
  const digest = await crypto.subtle.digest("SHA-256", bytes)
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("")
}

const handleE2eLogin = async (request: Request, env: WorkerEnv): Promise<Response> => {
  const privateJwkJson = typeof env.OA_E2E_JWT_PRIVATE_JWK === "string" ? env.OA_E2E_JWT_PRIVATE_JWK : ""
  if (!privateJwkJson) return new Response("Not found", { status: 404 })
  if (!requireE2eBypassAuth(request, env)) return new Response("Unauthorized", { status: 401 })

  let body: E2eLoginBody
  try {
    body = (await request.json()) as E2eLoginBody
  } catch {
    body = {}
  }

  const seed = normalizeSeed(body.seed)
  const emailRaw = typeof body.email === "string" ? body.email : ""
  const email = emailRaw ? normalizeEmail(emailRaw) : null
  const firstName = normalizeName(body.firstName)
  const lastName = normalizeName(body.lastName)

  const userId =
    seed != null
      ? `user_e2e_${(await sha256Hex(seed)).slice(0, 26)}`
      : `user_e2e_${crypto.randomUUID().replaceAll("-", "").slice(0, 26)}`

  const user = {
    id: userId,
    email: email ?? `${userId}@e2e.openagents.invalid`,
    firstName: firstName ?? "E2E",
    lastName: lastName ?? "Test",
  }

  const { runtime } = getWorkerRuntime(env)
  return runtime.runPromise(
    Effect.gen(function* () {
      const accessToken = yield* mintE2eJwt({ privateJwkJson, user })
      const headers = new Headers({ "content-type": "application/json; charset=utf-8" })
      // Auth cookie (E2E-only).
      headers.append("Set-Cookie", makeE2eSetCookieHeader(accessToken))
      // Prelaunch bypass cookie (so tests can access /autopilot even when VITE_PRELAUNCH=1).
      headers.append("Set-Cookie", "prelaunch_bypass=1; Path=/; Max-Age=604800; Secure; SameSite=Lax")
      return new Response(JSON.stringify({ ok: true, userId: user.id, email: user.email }), { status: 200, headers })
    }).pipe(
      Effect.catchAll((err) => {
        console.error("[auth.e2e.login]", err)
        return Effect.succeed(json({ ok: false, error: "e2e_login_failed" }, { status: 500 }))
      }),
    ),
  )
}

const handleSession = async (request: Request, env: WorkerEnv): Promise<Response> => {
  let auth: any
  let refreshedSessionData: string | undefined
  try {
    const result = await authkit.withAuth(request)
    auth = result.auth
    refreshedSessionData = result.refreshedSessionData
  } catch {
    auth = { user: null }
    refreshedSessionData = undefined
  }

  const user = auth.user
    ? {
        id: auth.user.id,
        email: auth.user.email ?? null,
        firstName: auth.user.firstName ?? null,
        lastName: auth.user.lastName ?? null,
      }
    : null

  // Convex auth token:
  // - Prefer a Worker-minted JWT (stable issuer, JWKS served from this Worker).
  // - Fall back to the WorkOS access token when configured/available.
  //
  // Rationale: WorkOS sessions can be present even when `accessToken` is absent/opaque. When that
  // happens, the UI would appear "authed" but Convex would be unauthenticated and core mutations
  // (e.g. ensureOwnedThread) would fail with a silent "Server Error Called by client".
  const privateJwkJson = typeof env.OA_E2E_JWT_PRIVATE_JWK === "string" ? env.OA_E2E_JWT_PRIVATE_JWK : ""
  let oaJwt: string | null = null
  if (user && privateJwkJson) {
    try {
      const { runtime } = getWorkerRuntime(env)
      oaJwt = await runtime.runPromise(
        mintE2eJwt({
          privateJwkJson,
          user: {
            id: user.id,
            email: user.email,
            firstName: user.firstName,
            lastName: user.lastName,
          },
          // Short-lived; clients refresh via /api/auth/session (cached for a few seconds client-side).
          ttlSeconds: 60 * 30,
        }),
      )
    } catch {
      oaJwt = null
    }
  }

  // WorkOS session is primary. If absent, fall back to E2E session cookie.
  const e2eToken = !user ? readE2eTokenFromRequest(request) : null
  const e2eClaims = e2eToken ? decodeE2eJwtClaims(e2eToken) : null

  const now = Math.floor(Date.now() / 1000)
  const e2eValid =
    e2eClaims &&
    (typeof e2eClaims.iss !== "string" || e2eClaims.iss === E2E_JWT_ISSUER) &&
    (typeof e2eClaims.exp !== "number" || e2eClaims.exp > now) &&
    typeof e2eClaims.sub === "string" &&
    e2eClaims.sub.length > 0

  const payload: SessionPayload = e2eValid
    ? {
        ok: true,
        userId: e2eClaims.sub,
        sessionId: null,
        token: e2eToken!,
        user: {
          id: e2eClaims.sub,
          email: e2eClaims.email ?? null,
          firstName: e2eClaims.firstName ?? null,
          lastName: e2eClaims.lastName ?? null,
        },
      }
    : {
        ok: true,
        userId: user?.id ?? null,
        sessionId: auth.user ? (auth.sessionId ?? null) : null,
        token: oaJwt ?? (auth.user ? (auth.accessToken ?? null) : null),
        user,
      }

  // If WorkOS refreshed the session, persist it back into the cookie.
  if (refreshedSessionData) {
    const { headers } = await authkit.saveSession(undefined, refreshedSessionData)
    const setCookie = (headers as unknown as Record<string, string>)["Set-Cookie"];
    if (typeof setCookie === "string") {
      return json(payload, { status: 200, headers: { "Set-Cookie": setCookie } })
    }
  }

  return json(payload, { status: 200 })
}

const handleStart = async (request: Request, env: WorkerEnv): Promise<Response> => {
  let body: StartBody
  try {
    body = await request.json()
  } catch {
    return json({ ok: false, error: "invalid_json" }, { status: 400 })
  }

  const emailRaw = typeof body.email === "string" ? body.email : ""
  const email = normalizeEmail(emailRaw)
  if (!email || !email.includes("@") || email.length > 320) {
    return json({ ok: false, error: "invalid_email" }, { status: 400 })
  }

  const { runtime } = getWorkerRuntime(env)
  const url = new URL(request.url)
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing"
  const telemetryBase = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService
    }),
  )
  const requestTelemetry = telemetryBase.withFields({
    requestId,
    method: request.method,
    pathname: url.pathname,
  })
  return runtime.runPromise(
    Effect.gen(function* () {
      const telemetry = yield* TelemetryService
      yield* sendMagicAuthCode(email)
      yield* telemetry.withNamespace("auth.magic").event("magic_code.sent")
      return json({ ok: true })
    }).pipe(
      Effect.provideService(RequestContextService, makeServerRequestContext(request)),
      Effect.provideService(TelemetryService, requestTelemetry),
      Effect.catchAll((err) => {
        console.error(`[auth.start] ${formatRequestIdLogToken(requestId)}`, err)
        return Effect.succeed(json({ ok: false, error: "send_failed" }, { status: 500 }))
      }),
    ),
  )
}

const handleVerify = async (request: Request, env: WorkerEnv): Promise<Response> => {
  let body: VerifyBody
  try {
    body = await request.json()
  } catch {
    return json({ ok: false, error: "invalid_json" }, { status: 400 })
  }

  const emailRaw = typeof body.email === "string" ? body.email : ""
  const codeRaw = typeof body.code === "string" ? body.code : ""
  const email = normalizeEmail(emailRaw)
  const code = normalizeCode(codeRaw)

  if (!email || !email.includes("@") || email.length > 320) {
    return json({ ok: false, error: "invalid_email" }, { status: 400 })
  }
  // WorkOS Magic Auth uses a 6-digit code, but accept any 4-10 digit input for flexibility.
  if (!/^[0-9]{4,10}$/.test(code)) {
    return json({ ok: false, error: "invalid_code" }, { status: 400 })
  }

  const { runtime } = getWorkerRuntime(env)
  const url = new URL(request.url)
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing"
  const telemetryBase = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService
    }),
  )
  const requestTelemetry = telemetryBase.withFields({
    requestId,
    method: request.method,
    pathname: url.pathname,
  })
  return runtime.runPromise(
    Effect.gen(function* () {
      const telemetry = yield* TelemetryService
      const { userId, setCookieHeader } = yield* verifyMagicAuthCode({
        request,
        email,
        code,
      })
      yield* telemetry.withNamespace("auth.magic").event("magic_code.verified", { userId })

      return json(
        { ok: true, userId },
        {
          status: 200,
          headers: {
            "Set-Cookie": setCookieHeader,
          },
        },
      )
    }).pipe(
      Effect.provideService(RequestContextService, makeServerRequestContext(request)),
      Effect.provideService(TelemetryService, requestTelemetry),
      Effect.catchAll((err) => {
        console.error(`[auth.verify] ${formatRequestIdLogToken(requestId)}`, err)
        return Effect.succeed(json({ ok: false, error: "verify_failed" }, { status: 401 }))
      }),
    ),
  )
}

const handleSignout = async (request: Request, env: WorkerEnv): Promise<Response> => {
  const { runtime } = getWorkerRuntime(env)
  const url = new URL(request.url)
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing"
  const telemetryBase = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService
    }),
  )
  const requestTelemetry = telemetryBase.withFields({
    requestId,
    method: request.method,
    pathname: url.pathname,
  })
  return runtime.runPromise(
    Effect.gen(function* () {
      const telemetry = yield* TelemetryService
      const { setCookieHeader } = yield* clearSessionCookie()
      yield* telemetry.withNamespace("auth.session").event("session.cleared")

      const headers = new Headers({ "content-type": "application/json; charset=utf-8" })
      headers.append("Set-Cookie", setCookieHeader)
      headers.append("Set-Cookie", makeE2eClearCookieHeader())
      return new Response(JSON.stringify({ ok: true }), { status: 200, headers })
    }).pipe(
      Effect.provideService(TelemetryService, requestTelemetry),
      Effect.catchAll((err) => {
        console.error(`[auth.signout] ${formatRequestIdLogToken(requestId)}`, err)
        return Effect.succeed(json({ ok: false, error: "signout_failed" }, { status: 500 }))
      }),
    ),
  )
}

type SsoExchangeBody = {
  readonly code?: unknown
}

const handleSsoAuthorizeUrl = async (request: Request): Promise<Response> => {
  const url = new URL(request.url)
  const redirectUri = url.searchParams.get("redirect_uri")
  if (!redirectUri || redirectUri.length === 0) {
    return json({ ok: false, error: "missing_redirect_uri" }, { status: 400 })
  }
  try {
    const authUrl = await authkit.getSignInUrl({ redirectUri })
    return json({ ok: true, url: authUrl })
  } catch (err) {
    console.error("[auth.sso.authorize-url]", err)
    return json({ ok: false, error: "authorize_url_failed" }, { status: 500 })
  }
}

const handleSsoExchange = async (request: Request): Promise<Response> => {
  let body: SsoExchangeBody
  try {
    body = (await request.json()) as SsoExchangeBody
  } catch {
    return json({ ok: false, error: "invalid_json" }, { status: 400 })
  }
  const code = typeof body.code === "string" ? body.code.trim() : ""
  if (!code) {
    return json({ ok: false, error: "missing_code" }, { status: 400 })
  }
  try {
    const config = getConfig()
    const workos = authkit.getWorkOS()
    const authResponse = await workos.userManagement.authenticateWithCode({
      code,
      clientId: config.clientId,
    })
    const user = authResponse.user
    return json({
      ok: true,
      userId: user.id,
      user: {
        id: user.id,
        email: user.email ?? null,
        firstName: user.firstName ?? null,
        lastName: user.lastName ?? null,
      },
      token: authResponse.accessToken,
    })
  } catch (err) {
    console.error("[auth.sso.exchange]", err)
    return json({ ok: false, error: "exchange_failed" }, { status: 401 })
  }
}

export const handleAuthRequest = async (
  request: Request,
  env: WorkerEnv,
): Promise<Response | null> => {
  const url = new URL(request.url)

  if (url.pathname === "/api/auth/e2e/jwks") {
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405 })
    return handleE2eJwks(env)
  }

  if (url.pathname === "/api/auth/e2e/login") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 })
    return handleE2eLogin(request, env)
  }

  if (url.pathname === "/api/auth/session") {
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405 })
    return handleSession(request, env)
  }

  if (url.pathname === "/api/auth/start") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 })
    return handleStart(request, env)
  }

  if (url.pathname === "/api/auth/verify") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 })
    return handleVerify(request, env)
  }

  if (url.pathname === "/api/auth/signout") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 })
    return handleSignout(request, env)
  }

  if (url.pathname === "/api/auth/sso/authorize-url") {
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405 })
    return handleSsoAuthorizeUrl(request)
  }

  if (url.pathname === "/api/auth/sso/exchange") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 })
    return handleSsoExchange(request)
  }

  return null
}
