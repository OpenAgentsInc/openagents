import { Effect, Layer, Option, Ref } from "effect"

import type { L402Credential } from "../contracts/l402.js"
import { CredentialCacheService } from "../services/credentialCache.js"

export const CredentialCacheInMemoryLayer = Layer.effect(
  CredentialCacheService,
  Effect.gen(function* () {
    const cacheRef = yield* Ref.make<ReadonlyMap<string, L402Credential>>(new Map())

    return CredentialCacheService.of({
      getByHost: (host: string) =>
        Ref.get(cacheRef).pipe(
          Effect.map((cache) => {
            const found = cache.get(host)
            return found ? Option.some(found) : Option.none<L402Credential>()
          }),
        ),
      putByHost: (host: string, credential: L402Credential) =>
        Ref.update(cacheRef, (cache) => {
          const next = new Map(cache)
          next.set(host, credential)
          return next
        }).pipe(Effect.asVoid),
      clearHost: (host: string) =>
        Ref.update(cacheRef, (cache) => {
          if (!cache.has(host)) return cache
          const next = new Map(cache)
          next.delete(host)
          return next
        }).pipe(Effect.asVoid),
    })
  }),
)
