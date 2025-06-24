/**
 * NIP-42: Authentication of clients to relays
 * @module
 */

import { randomBytes } from "@noble/hashes/utils"
import { Context, Effect, Layer } from "effect"
import { Nip42Error } from "../core/Errors.js"
import { AuthEventKind } from "../core/Schema.js"
import type { AuthChallenge, AuthEvent, EventParams, PrivateKey, Tag } from "../core/Schema.js"
import { EventService } from "../services/EventService.js"

/**
 * Service for NIP-42 authentication operations
 */
export class Nip42Service extends Context.Tag("nostr/Nip42Service")<
  Nip42Service,
  {
    /**
     * Generate a random authentication challenge
     */
    readonly generateChallenge: () => Effect.Effect<AuthChallenge, Nip42Error>

    /**
     * Create an authentication event for a challenge
     */
    readonly createAuthEvent: (params: {
      challenge: AuthChallenge
      relayUrl: string
      privateKey: PrivateKey
    }) => Effect.Effect<AuthEvent, Nip42Error>

    /**
     * Verify an authentication event
     */
    readonly verifyAuthEvent: (params: {
      event: AuthEvent
      challenge: AuthChallenge
      relayUrl: string
    }) => Effect.Effect<boolean, Nip42Error>
  }
>() {}

/**
 * Live implementation of Nip42Service
 */
export const Nip42ServiceLive = Layer.effect(
  Nip42Service,
  Effect.gen(function*() {
    const eventService = yield* EventService

    const generateChallenge = (): Effect.Effect<AuthChallenge, Nip42Error> =>
      Effect.try({
        try: () => {
          // Generate 32 bytes of random data and convert to hex
          const bytes = randomBytes(32)
          const hex = Array.from(bytes)
            .map((b) => b.toString(16).padStart(2, "0"))
            .join("")
          return hex as AuthChallenge
        },
        catch: (error) =>
          new Nip42Error({
            operation: "generateChallenge",
            reason: `Failed to generate challenge: ${String(error)}`
          })
      })

    const createAuthEvent = ({
      challenge,
      privateKey,
      relayUrl
    }: {
      challenge: AuthChallenge
      relayUrl: string
      privateKey: PrivateKey
    }): Effect.Effect<AuthEvent, Nip42Error> =>
      Effect.gen(function*() {
        // Create auth event parameters
        const params: EventParams = {
          kind: AuthEventKind,
          content: "",
          tags: [
            ["relay", relayUrl],
            ["challenge", challenge]
          ] as ReadonlyArray<Tag>
        }

        // Create and sign the event
        const event = yield* eventService.create(params, privateKey).pipe(
          Effect.mapError((error) =>
            new Nip42Error({
              operation: "createAuthEvent",
              reason: `Failed to create auth event: ${String(error)}`
            })
          )
        )

        // Validate it's a proper auth event
        if (event.kind !== AuthEventKind) {
          return yield* Effect.fail(
            new Nip42Error({
              operation: "createAuthEvent",
              reason: "Created event is not an auth event"
            })
          )
        }

        // Check required tags
        const relayTag = event.tags.find((t: Tag) => t[0] === "relay")
        const challengeTag = event.tags.find((t: Tag) => t[0] === "challenge")

        if (!relayTag || !challengeTag) {
          return yield* Effect.fail(
            new Nip42Error({
              operation: "createAuthEvent",
              reason: "Auth event missing required tags"
            })
          )
        }

        return event as AuthEvent
      })

    const verifyAuthEvent = ({
      challenge,
      event,
      relayUrl
    }: {
      event: AuthEvent
      challenge: AuthChallenge
      relayUrl: string
    }): Effect.Effect<boolean, Nip42Error> =>
      Effect.gen(function*() {
        // Check event kind
        if (event.kind !== AuthEventKind) {
          return false
        }

        // Check relay tag matches
        const relayTag = event.tags.find((t: Tag) => t[0] === "relay")
        if (!relayTag || relayTag[1] !== relayUrl) {
          return false
        }

        // Check challenge tag matches
        const challengeTag = event.tags.find((t: Tag) => t[0] === "challenge")
        if (!challengeTag || challengeTag[1] !== challenge) {
          return false
        }

        // Verify event signature
        yield* eventService.verify(event).pipe(
          Effect.mapError((error) =>
            new Nip42Error({
              operation: "verifyAuthEvent",
              reason: `Failed to verify signature: ${String(error)}`
            })
          )
        )

        return true
      }).pipe(
        Effect.catchAll(() => Effect.succeed(false))
      )

    return {
      generateChallenge,
      createAuthEvent,
      verifyAuthEvent
    }
  })
)
