import { Effect } from "effect"
import { SignJWT, calculateJwkThumbprint, importJWK } from "jose"

export const E2E_JWT_ISSUER = "https://openagents.com/e2e"
export const E2E_COOKIE_NAME = "oa-e2e"

export type E2eUser = {
  readonly id: string
  readonly email?: string | null
  readonly firstName?: string | null
  readonly lastName?: string | null
}

const nowSeconds = (): number => Math.floor(Date.now() / 1000)

const asError = (u: unknown): Error => (u instanceof Error ? u : new Error(String(u)))

const publicJwkFromPrivate = (privateJwk: Record<string, unknown>): Record<string, unknown> => {
  // RSA private JWK contains { kty, n, e, d, p, q, dp, dq, qi, ... }.
  // We only publish the public fields.
  const kty = privateJwk["kty"]
  const n = privateJwk["n"]
  const e = privateJwk["e"]
  if (typeof kty !== "string" || typeof n !== "string" || typeof e !== "string") {
    throw new Error("invalid OA_E2E_JWT_PRIVATE_JWK: expected RSA JWK with kty/n/e")
  }
  return { kty, n, e }
}

export const makeE2eJwks = (input: { readonly privateJwkJson: string }) =>
  Effect.tryPromise({
    try: async () => {
      const parsed = JSON.parse(input.privateJwkJson) as Record<string, unknown>
      const publicJwk = publicJwkFromPrivate(parsed)
      const kid = await calculateJwkThumbprint(publicJwk as any)
      return {
        keys: [
          {
            ...publicJwk,
            kid,
            use: "sig",
            alg: "RS256",
            key_ops: ["verify"],
          },
        ],
      } as const
    },
    catch: asError,
  })

export const mintE2eJwt = (input: {
  readonly privateJwkJson: string
  readonly user: E2eUser
  /** Seconds until expiration (default: 1h). */
  readonly ttlSeconds?: number | undefined
}) =>
  Effect.tryPromise({
    try: async () => {
      const parsed = JSON.parse(input.privateJwkJson) as Record<string, unknown>
      const publicJwk = publicJwkFromPrivate(parsed)
      const kid = await calculateJwkThumbprint(publicJwk as any)
      const key = await importJWK(parsed as any, "RS256")

      const iat = nowSeconds()
      const exp = iat + (typeof input.ttlSeconds === "number" && input.ttlSeconds > 0 ? input.ttlSeconds : 60 * 60)

      const email = input.user.email ?? null
      const firstName = input.user.firstName ?? null
      const lastName = input.user.lastName ?? null

      return await new SignJWT({
        email,
        firstName,
        lastName,
      })
        .setProtectedHeader({ alg: "RS256", kid })
        .setIssuer(E2E_JWT_ISSUER)
        .setSubject(input.user.id)
        .setIssuedAt(iat)
        .setExpirationTime(exp)
        .sign(key)
    },
    catch: asError,
  })

const b64urlToUtf8 = (b64url: string): string => {
  const b64 = b64url.replace(/-/g, "+").replace(/_/g, "/")
  const padded = b64 + "===".slice((b64.length + 3) % 4)
  const bin = atob(padded)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return new TextDecoder().decode(bytes)
}

export type E2eJwtClaims = {
  readonly sub: string
  readonly iss?: string | undefined
  readonly exp?: number | undefined
  readonly iat?: number | undefined
  readonly email?: string | null | undefined
  readonly firstName?: string | null | undefined
  readonly lastName?: string | null | undefined
}

export const decodeE2eJwtClaims = (token: string): E2eJwtClaims | null => {
  const parts = token.split(".")
  if (parts.length !== 3) return null
  try {
    const payload = JSON.parse(b64urlToUtf8(parts[1] ?? "")) as any
    const sub = typeof payload?.sub === "string" ? payload.sub : ""
    if (!sub) return null
    const iss = typeof payload?.iss === "string" ? payload.iss : undefined
    const exp = typeof payload?.exp === "number" ? payload.exp : undefined
    const iat = typeof payload?.iat === "number" ? payload.iat : undefined
    const email = payload?.email == null ? null : typeof payload?.email === "string" ? payload.email : null
    const firstName = payload?.firstName == null ? null : typeof payload?.firstName === "string" ? payload.firstName : null
    const lastName = payload?.lastName == null ? null : typeof payload?.lastName === "string" ? payload.lastName : null
    return { sub, iss, exp, iat, email, firstName, lastName }
  } catch {
    return null
  }
}

export const makeE2eSetCookieHeader = (token: string): string => {
  // HttpOnly: browser JS never reads this. The app uses /api/auth/session to surface the session.
  return `${E2E_COOKIE_NAME}=${encodeURIComponent(token)}; Path=/; Max-Age=${60 * 60}; HttpOnly; Secure; SameSite=Lax`
}

export const makeE2eClearCookieHeader = (): string =>
  `${E2E_COOKIE_NAME}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax`

export const readCookie = (cookieHeader: string | null, name: string): string | null => {
  if (!cookieHeader) return null
  // Fast path: split by `;` (cookies are small).
  const parts = cookieHeader.split(";")
  for (const p of parts) {
    const s = p.trim()
    if (!s) continue
    const eq = s.indexOf("=")
    if (eq <= 0) continue
    const k = s.slice(0, eq).trim()
    if (k !== name) continue
    const v = s.slice(eq + 1)
    try {
      return decodeURIComponent(v)
    } catch {
      return v
    }
  }
  return null
}

export const readE2eTokenFromRequest = (request: Request): string | null =>
  readCookie(request.headers.get("cookie"), E2E_COOKIE_NAME)
