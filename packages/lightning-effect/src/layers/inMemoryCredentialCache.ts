import { Effect, Layer, Ref } from "effect"

import type { L402Credential } from "../contracts/l402.js"
import { CredentialCacheService } from "../services/credentialCache.js"

type CacheEntry = Readonly<{
  readonly credential: L402Credential
  readonly expiresAtMs: number
}>

const toCacheKey = (host: string, scope: string): string =>
  `${host.trim().toLowerCase()}::${scope.trim().toLowerCase()}`

export const makeCredentialCacheInMemoryLayer = (options?: {
  readonly defaultTtlMs?: number
}) => {
  const defaultTtlMs = options?.defaultTtlMs ?? 5 * 60 * 1000

  return Layer.effect(
    CredentialCacheService,
    Effect.gen(function* () {
      const cacheRef = yield* Ref.make<ReadonlyMap<string, CacheEntry>>(new Map())

      return CredentialCacheService.of({
        getByHost: (host: string, scope: string, nowMs: number) =>
          Ref.get(cacheRef).pipe(
            Effect.map((cache) => {
              const entry = cache.get(toCacheKey(host, scope))
              if (!entry) return { _tag: "miss" as const }
              if (nowMs >= entry.expiresAtMs) {
                return { _tag: "stale" as const, credential: entry.credential }
              }
              return { _tag: "hit" as const, credential: entry.credential }
            }),
          ),
        putByHost: (
          host: string,
          scope: string,
          credential: L402Credential,
          putOptions?: { readonly ttlMs?: number },
        ) =>
          Ref.update(cacheRef, (cache) => {
            const ttlMs = Math.max(0, Math.floor(putOptions?.ttlMs ?? defaultTtlMs))
            const expiresAtMs = credential.issuedAtMs + ttlMs
            const next = new Map(cache)
            next.set(toCacheKey(host, scope), { credential, expiresAtMs })
            return next
          }).pipe(Effect.asVoid),
        markInvalid: (host: string, scope: string) =>
          Ref.update(cacheRef, (cache) => {
            const key = toCacheKey(host, scope)
            if (!cache.has(key)) return cache
            const next = new Map(cache)
            next.delete(key)
            return next
          }).pipe(Effect.asVoid),
        clearHost: (host: string, scope: string) =>
          Ref.update(cacheRef, (cache) => {
            const key = toCacheKey(host, scope)
            if (!cache.has(key)) return cache
            const next = new Map(cache)
            next.delete(key)
            return next
          }).pipe(Effect.asVoid),
      })
    }),
  )
}

export const CredentialCacheInMemoryLayer = makeCredentialCacheInMemoryLayer()
