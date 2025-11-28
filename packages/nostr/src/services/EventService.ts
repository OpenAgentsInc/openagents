/**
 * Service for creating and validating Nostr events
 * @module
 */

import type { ParseResult } from "effect"
import { Context, Effect, Layer, Schema } from "effect"
import { EventValidationError, InvalidEventId, InvalidSignature } from "../core/Errors.js"
import {
  type EventId,
  type EventParams,
  NostrEvent,
  type PrivateKey,
  type Signature,
  type UnixTimestamp,
  type UnsignedEvent
} from "../core/Schema.js"
import { CryptoService } from "./CryptoService.js"

/**
 * Service for event operations
 */
export class EventService extends Context.Tag("nostr/EventService")<
  EventService,
  {
    /**
     * Create a new event from parameters
     */
    readonly create: (params: EventParams, privateKey: PrivateKey) => Effect.Effect<
      NostrEvent,
      EventValidationError | InvalidEventId | InvalidSignature | ParseResult.ParseError
    >

    /**
     * Verify an event's signature and ID
     */
    readonly verify: (event: NostrEvent) => Effect.Effect<
      NostrEvent,
      InvalidEventId | InvalidSignature
    >

    /**
     * Calculate event ID
     */
    readonly calculateId: (event: UnsignedEvent) => Effect.Effect<EventId, InvalidEventId>

    /**
     * Sign an event
     */
    readonly sign: (event: UnsignedEvent & { id: EventId }, privateKey: PrivateKey) => Effect.Effect<
      Signature,
      InvalidSignature
    >

    /**
     * Serialize event for hashing (as per NIP-01)
     */
    readonly serialize: (event: UnsignedEvent) => string
  }
>() {}

/**
 * Serialize event for hashing according to NIP-01
 */
const serializeEvent = (event: UnsignedEvent): string => {
  const serialized = JSON.stringify([
    0,
    event.pubkey,
    event.created_at,
    event.kind,
    event.tags,
    event.content
  ])
  return serialized
}

/**
 * Live implementation of EventService
 */
export const EventServiceLive = Layer.effect(
  EventService,
  Effect.gen(function*() {
    const crypto = yield* CryptoService

    const serialize = (event: UnsignedEvent): string => serializeEvent(event)

    const calculateId = (event: UnsignedEvent): Effect.Effect<EventId, InvalidEventId> =>
      Effect.gen(function*() {
        const serialized = serialize(event)
        const id = yield* crypto.hash(serialized)
        return id
      }).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new InvalidEventId({
              id: "unknown",
              reason: `Failed to calculate event ID: ${error}`
            })
          )
        )
      )

    const sign = (
      event: UnsignedEvent & { id: EventId },
      privateKey: PrivateKey
    ): Effect.Effect<Signature, InvalidSignature> =>
      crypto.sign(event.id, privateKey).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new InvalidSignature({
              eventId: event.id,
              publicKey: event.pubkey,
              reason: String(error)
            })
          )
        )
      )

    const create = (
      params: EventParams,
      privateKey: PrivateKey
    ): Effect.Effect<NostrEvent, EventValidationError | InvalidEventId | InvalidSignature | ParseResult.ParseError> =>
      Effect.gen(function*() {
        // Get public key from private key
        const publicKey = yield* crypto.getPublicKey(privateKey).pipe(
          Effect.catchAll(() =>
            Effect.fail(
              new EventValidationError({
                errors: ["Invalid private key"]
              })
            )
          )
        )

        // Create unsigned event
        const unsignedEvent: UnsignedEvent = {
          pubkey: publicKey,
          created_at: Math.floor(Date.now() / 1000) as UnixTimestamp,
          kind: params.kind,
          tags: params.tags,
          content: params.content
        }

        // Calculate ID
        const id = yield* calculateId(unsignedEvent)

        // Sign event
        const sig = yield* sign({ ...unsignedEvent, id }, privateKey)

        // Create final event
        const event: NostrEvent = {
          ...unsignedEvent,
          id,
          sig
        }

        // Validate the final event
        return yield* Schema.decodeUnknown(NostrEvent)(event)
      })

    const verify = (event: NostrEvent): Effect.Effect<NostrEvent, InvalidEventId | InvalidSignature> =>
      Effect.gen(function*() {
        // Verify event ID
        const unsignedEvent: UnsignedEvent = {
          pubkey: event.pubkey,
          created_at: event.created_at,
          kind: event.kind,
          tags: event.tags,
          content: event.content
        }

        const calculatedId = yield* calculateId(unsignedEvent)
        if (calculatedId !== event.id) {
          return yield* Effect.fail(
            new InvalidEventId({
              id: event.id,
              reason: `Calculated ID ${calculatedId} does not match event ID ${event.id}`
            })
          )
        }

        // Verify signature
        const isValid = yield* crypto.verify(event.sig, event.id, event.pubkey).pipe(
          Effect.catchAll(() => Effect.succeed(false))
        )

        if (!isValid) {
          return yield* Effect.fail(
            new InvalidSignature({
              eventId: event.id,
              publicKey: event.pubkey,
              reason: "Signature verification failed"
            })
          )
        }

        return event
      })

    return {
      create,
      verify,
      calculateId,
      sign,
      serialize
    }
  })
)
