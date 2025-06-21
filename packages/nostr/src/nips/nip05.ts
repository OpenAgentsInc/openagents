/**
 * NIP-05: DNS-based Internet Identifiers
 * Maps human-readable names to Nostr public keys via DNS/HTTP
 *
 * Format: <local-part>@<domain>
 * Resolution: https://<domain>/.well-known/nostr.json?name=<local-part>
 */

import { Cache, Context, Data, Duration, Effect, Layer, Schema } from "effect"
import { PublicKey } from "../core/Schema.js"

// Define Relay type locally for now
export const Relay = Schema.String.pipe(
  Schema.pattern(/^wss?:\/\/.+$/),
  Schema.brand("Relay")
)
export type Relay = Schema.Schema.Type<typeof Relay>

// --- Types ---
export const Nip05Identifier = Schema.String.pipe(
  Schema.pattern(/^[a-zA-Z0-9._-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/),
  Schema.brand("Nip05Identifier")
)
export type Nip05Identifier = Schema.Schema.Type<typeof Nip05Identifier>

export const NostrJsonResponse = Schema.Struct({
  names: Schema.Record({ key: Schema.String, value: PublicKey }),
  relays: Schema.optional(Schema.Record({
    key: PublicKey,
    value: Schema.Array(Relay)
  })),
  // Optional NIP-46 bunker URL
  nip46: Schema.optional(Schema.Record({
    key: PublicKey,
    value: Schema.Array(Schema.String)
  }))
})
export type NostrJsonResponse = Schema.Schema.Type<typeof NostrJsonResponse>

export const Nip05Profile = Schema.Struct({
  pubkey: PublicKey,
  relays: Schema.optional(Schema.Array(Relay)),
  nip46: Schema.optional(Schema.Array(Schema.String)),
  identifier: Nip05Identifier,
  verifiedAt: Schema.Number
})
export type Nip05Profile = Schema.Schema.Type<typeof Nip05Profile>

// --- Errors ---
export class Nip05Error extends Data.TaggedError("Nip05Error")<{
  reason: "invalid_identifier" | "dns_error" | "not_found" | "invalid_response" | "timeout"
  identifier: string
  message: string
  cause?: unknown
}> {}

// --- Service ---
export class Nip05Service extends Context.Tag("nips/Nip05Service")<
  Nip05Service,
  {
    /**
     * Resolve a NIP-05 identifier to a public key
     */
    readonly resolve: (
      identifier: Nip05Identifier
    ) => Effect.Effect<Nip05Profile, Nip05Error>

    /**
     * Verify that a public key owns a NIP-05 identifier
     */
    readonly verify: (
      pubkey: PublicKey,
      identifier: Nip05Identifier
    ) => Effect.Effect<boolean, Nip05Error>

    /**
     * Query well-known endpoint directly
     */
    readonly queryWellKnown: (
      domain: string,
      name?: string
    ) => Effect.Effect<NostrJsonResponse, Nip05Error>

    /**
     * Clear cache for an identifier
     */
    readonly clearCache: (
      identifier: Nip05Identifier
    ) => Effect.Effect<void>

    /**
     * Get all cached profiles
     */
    readonly getCachedProfiles: () => Effect.Effect<Map<Nip05Identifier, Nip05Profile>>
  }
>() {}

// --- Implementation ---
export const Nip05ServiceLive = Layer.effect(
  Nip05Service,
  Effect.gen(function*() {
    // Create cache with 1 hour TTL
    const profileCache = yield* Cache.make({
      capacity: 1000,
      timeToLive: Duration.hours(1),
      lookup: (identifier: Nip05Identifier) => resolveIdentifierImpl(identifier)
    })

    const resolveIdentifierImpl = (
      identifier: Nip05Identifier
    ): Effect.Effect<Nip05Profile, Nip05Error> =>
      Effect.gen(function*() {
        // Parse identifier
        const parts = identifier.split("@")
        if (parts.length !== 2) {
          return yield* Effect.fail(
            new Nip05Error({
              reason: "invalid_identifier",
              identifier,
              message: "Identifier must be in format: name@domain"
            })
          )
        }

        const [name, domain] = parts
        const isIpAddress = /^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(domain)
        const protocol = isIpAddress ? "http" : "https"
        const url = `${protocol}://${domain}/.well-known/nostr.json?name=${encodeURIComponent(name)}`

        // Fetch from well-known endpoint
        const response = yield* Effect.tryPromise({
          try: () => fetch(url),
          catch: (error) =>
            new Nip05Error({
              reason: "dns_error",
              identifier,
              message: `Failed to fetch .well-known/nostr.json: ${String(error)}`,
              cause: error as unknown
            })
        }).pipe(
          Effect.timeout(Duration.seconds(10)),
          Effect.tapError(() => Effect.log(`NIP-05 resolution failed for ${identifier}`)),
          Effect.mapError((error: any) => {
            if (error._tag === "TimeoutException") {
              return new Nip05Error({
                reason: "timeout",
                identifier,
                message: "Request timed out",
                cause: error
              })
            }
            return error
          })
        )

        // Parse response
        const body = yield* Effect.tryPromise({
          try: () => response.json(),
          catch: (error) =>
            new Nip05Error({
              reason: "invalid_response",
              identifier,
              message: `Failed to parse JSON response: ${String(error)}`,
              cause: error as unknown
            })
        })
        const parsed = yield* Schema.decodeUnknown(NostrJsonResponse)(body).pipe(
          Effect.mapError((error) =>
            new Nip05Error({
              reason: "invalid_response",
              identifier,
              message: `Invalid response format: ${error}`,
              cause: error
            })
          )
        )

        // Find the public key for this name
        const pubkey = parsed.names[name.toLowerCase()] || parsed.names[name]
        if (!pubkey) {
          return yield* Effect.fail(
            new Nip05Error({
              reason: "not_found",
              identifier,
              message: `Name '${name}' not found in domain '${domain}'`
            })
          )
        }

        // Get relays if available
        const relays = parsed.relays?.[pubkey]
        const nip46 = parsed.nip46?.[pubkey]

        const profile: Nip05Profile = {
          pubkey,
          relays,
          nip46,
          identifier,
          verifiedAt: Date.now()
        }
        return profile
      })

    const resolve = (identifier: Nip05Identifier) => profileCache.get(identifier)

    const verify = (
      pubkey: PublicKey,
      identifier: Nip05Identifier
    ): Effect.Effect<boolean, Nip05Error> =>
      Effect.gen(function*() {
        const profile = yield* resolve(identifier)
        return profile.pubkey === pubkey
      })

    const queryWellKnown = (
      domain: string,
      name?: string
    ): Effect.Effect<NostrJsonResponse, Nip05Error> =>
      Effect.gen(function*() {
        const isIpAddress = /^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(domain)
        const protocol = isIpAddress ? "http" : "https"
        let url = `${protocol}://${domain}/.well-known/nostr.json`

        if (name) {
          url += `?name=${encodeURIComponent(name)}`
        }

        const response = yield* Effect.tryPromise({
          try: () => fetch(url),
          catch: (error) =>
            new Nip05Error({
              reason: "dns_error",
              identifier: domain,
              message: `Failed to fetch .well-known/nostr.json: ${String(error)}`,
              cause: error as unknown
            })
        }).pipe(
          Effect.timeout(Duration.seconds(10)),
          Effect.mapError((error: any) => {
            if (error._tag === "TimeoutException") {
              return new Nip05Error({
                reason: "timeout",
                identifier: domain,
                message: "Request timed out",
                cause: error
              })
            }
            return error
          })
        )

        const body = yield* Effect.tryPromise({
          try: () => response.json(),
          catch: (error) =>
            new Nip05Error({
              reason: "invalid_response",
              identifier: domain,
              message: `Failed to parse JSON response: ${String(error)}`,
              cause: error as unknown
            })
        })
        const decoded = yield* Schema.decodeUnknown(NostrJsonResponse)(body).pipe(
          Effect.mapError((error) =>
            new Nip05Error({
              reason: "invalid_response",
              identifier: domain,
              message: `Invalid response format: ${error}`,
              cause: error
            })
          )
        )
        return decoded
      })

    const clearCache = (identifier: Nip05Identifier) => profileCache.invalidate(identifier)

    const getCachedProfiles = () =>
      Effect.sync(() => {
        // Note: Effect Cache doesn't expose its internal map directly
        // In production, you might want to maintain a separate Map
        // or use a different caching strategy
        return new Map<Nip05Identifier, Nip05Profile>()
      })

    return {
      resolve,
      verify,
      queryWellKnown,
      clearCache,
      getCachedProfiles
    }
  })
)

// --- Utility Functions ---

/**
 * Parse a NIP-05 identifier
 */
export const parseIdentifier = (
  identifier: string
): Effect.Effect<{ name: string; domain: string }, Nip05Error> =>
  Effect.gen(function*() {
    const validated = yield* Schema.decodeUnknown(Nip05Identifier)(identifier).pipe(
      Effect.mapError(() =>
        new Nip05Error({
          reason: "invalid_identifier",
          identifier,
          message: "Invalid identifier format"
        })
      )
    )

    const parts = validated.split("@")
    return {
      name: parts[0],
      domain: parts[1]
    }
  })

/**
 * Normalize a NIP-05 identifier
 * Handles missing @ by assuming _@domain format
 */
export const normalizeIdentifier = (
  input: string
): Effect.Effect<Nip05Identifier, Nip05Error> =>
  Effect.gen(function*() {
    let normalized = input.trim().toLowerCase()

    // If no @, assume it's just a domain with implicit _ username
    if (!normalized.includes("@")) {
      normalized = `_@${normalized}`
    }

    return yield* Schema.decodeUnknown(Nip05Identifier)(normalized).pipe(
      Effect.mapError(() =>
        new Nip05Error({
          reason: "invalid_identifier",
          identifier: input,
          message: "Invalid identifier format"
        })
      )
    )
  })

/**
 * Create a NIP-05 metadata event (kind 0) with identifier
 */
export const createMetadataEvent = (params: {
  name?: string
  about?: string
  picture?: string
  nip05: string
  lud16?: string // Lightning address
  website?: string
  banner?: string
}) => {
  const metadata = {
    name: params.name,
    about: params.about,
    picture: params.picture,
    nip05: params.nip05,
    lud16: params.lud16,
    website: params.website,
    banner: params.banner
  }

  // Remove undefined values
  const cleaned = Object.fromEntries(
    Object.entries(metadata).filter(([_, value]) => value !== undefined)
  )

  return JSON.stringify(cleaned)
}
