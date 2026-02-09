import { AuthKitCore, getConfigurationProvider, getWorkOS, sessionEncryption } from "@workos/authkit-session"
import { Effect } from "effect"
import { SignJWT, calculateJwkThumbprint, importJWK } from "jose"

import { WebCookieSessionStorage } from "./sessionCookieStorage"

export const E2E_JWT_ISSUER = "https://openagents.com/e2e"

export type E2eUser = {
  readonly id: string
  readonly email?: string | null
  readonly firstName?: string | null
  readonly lastName?: string | null
}

const nowSeconds = (): number => Math.floor(Date.now() / 1000)

const asError = (u: unknown): Error => (u instanceof Error ? u : new Error(String(u)))

const getAuthKitConfig = () => getConfigurationProvider().getConfig()

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

export const makeAuthKitSessionSetCookie = (input: { readonly accessToken: string; readonly user: E2eUser }) =>
  Effect.tryPromise({
    try: async () => {
      const config = getAuthKitConfig()
      const workos = getWorkOS()

      const core = new AuthKitCore(config, workos, sessionEncryption)
      const sessionData = await core.encryptSession({
        accessToken: input.accessToken,
        // AuthKitCore expects a refresh token in the session structure. We do not use it for E2E bypass.
        refreshToken: `e2e_${crypto.randomUUID()}`,
        user: {
          id: input.user.id,
          email: input.user.email ?? undefined,
          firstName: input.user.firstName ?? undefined,
          lastName: input.user.lastName ?? undefined,
        } as any,
        impersonator: undefined,
      })

      const storage = new WebCookieSessionStorage(config)
      const { headers } = await storage.saveSession(undefined, sessionData)
      const setCookieHeader = headers?.["Set-Cookie"]
      if (typeof setCookieHeader !== "string") {
        throw new Error("missing Set-Cookie header from session storage")
      }

      return { setCookieHeader }
    },
    catch: asError,
  })
