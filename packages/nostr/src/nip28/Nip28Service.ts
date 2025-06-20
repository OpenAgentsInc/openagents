/**
 * NIP-28: Public Chat Channel Service
 * Implements channel creation, messaging, and subscription functionality
 * @module
 */

import { Chunk, Context, Data, Effect, Layer, Schema, Stream } from "effect"
import type { EventId, EventParams, Filter, NostrEvent, PrivateKey, PublicKey, SubscriptionId } from "../core/Schema.js"
import { EventService } from "../services/EventService.js"
import { RelayService } from "../services/RelayService.js"

// --- Custom Error Types ---
export class Nip28InvalidInputError extends Data.TaggedError(
  "Nip28InvalidInputError"
)<{
  message: string
  cause?: unknown
}> {}

export class Nip28PublishError extends Data.TaggedError("Nip28PublishError")<{
  message: string
  cause?: unknown
}> {}

export class Nip28FetchError extends Data.TaggedError("Nip28FetchError")<{
  message: string
  cause?: unknown
}> {}

export class Nip28ChannelNotFoundError extends Data.TaggedError("Nip28ChannelNotFoundError")<{
  channelId: string
}> {}

// --- Schemas for NIP-28 Content ---
export const ChannelMetadataContentSchema = Schema.Struct({
  name: Schema.String, // NIP-28 implies name is required for kind 40 content
  about: Schema.optional(Schema.String),
  picture: Schema.optional(Schema.String),
  relays: Schema.optional(Schema.Array(Schema.String)) // NIP-28 mentions this for kind 40 content too
})
export type ChannelMetadataContent = Schema.Schema.Type<
  typeof ChannelMetadataContentSchema
>

export const ChannelMessageContentSchema = Schema.String // Content is just a string for kind 42

export const ModerationReasonContentSchema = Schema.Struct({
  reason: Schema.String
})
export type ModerationReasonContent = Schema.Schema.Type<
  typeof ModerationReasonContentSchema
>

// --- Parameter Types for Service Methods ---
export interface CreateChannelParams {
  name: string
  about?: string
  picture?: string
  relays?: Array<string> // Relays to include in the kind 40 content
  privateKey: PrivateKey
}

export interface ChannelMetadata {
  name: string
  about?: string
  picture?: string
  creatorPubkey: PublicKey
  channelId: EventId // Kind 40 event ID
  relays?: Array<string>
}

export interface SendChannelMessageParams {
  channelId: EventId // ID of the Kind 40 event
  content: string // Message content
  privateKey: PrivateKey // Sender's private key
  replyToEventId?: EventId // Optional: for threaded replies
  replyToPubkey?: PublicKey // Pubkey of the user being replied to
  relayHint?: string // Optional: relay hint for the channel creation event
}

export interface ChannelMessage extends NostrEvent {
  // Channel messages are public, no decryption needed
  channelId: EventId
  replyToEventId?: EventId
  replyToPubkey?: PublicKey
}

export interface HideMessageParams {
  messageEventId: EventId // ID of the kind 42 event to hide
  reason?: string
  privateKey: PrivateKey
}

export interface MuteUserParams {
  userPubkey: PublicKey
  reason?: string
  privateKey: PrivateKey
}

// --- Service Tag ---
export class Nip28Service extends Context.Tag("nostr/Nip28Service")<
  Nip28Service,
  {
    /**
     * Creates a new public chat channel (Kind 40).
     */
    readonly createChannel: (
      params: CreateChannelParams
    ) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>

    /**
     * Gets metadata for a channel from its creation event (Kind 40).
     */
    readonly getChannelMetadata: (
      channelId: EventId
    ) => Effect.Effect<ChannelMetadata, Nip28FetchError | Nip28ChannelNotFoundError>

    /**
     * Updates metadata for a channel (Kind 41).
     */
    readonly setChannelMetadata: (params: {
      channelId: EventId
      name?: string
      about?: string
      picture?: string
      privateKey: PrivateKey
    }) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>

    /**
     * Sends a message to a channel (Kind 42).
     * Messages are public and not encrypted.
     */
    readonly sendChannelMessage: (
      params: SendChannelMessageParams
    ) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>

    /**
     * Fetches messages for a channel (Kind 42).
     * Messages are sorted by created_at ascending (oldest first).
     */
    readonly getChannelMessages: (
      channelId: EventId,
      filterOptions?: Partial<Filter>
    ) => Effect.Effect<Array<ChannelMessage>, Nip28FetchError>

    /**
     * Subscribes to new messages for a channel.
     */
    readonly subscribeToChannelMessages: (
      channelId: EventId
    ) => Stream.Stream<ChannelMessage, Nip28FetchError>

    /**
     * Hide a message (Kind 43) - client-side moderation
     */
    readonly hideMessage: (
      params: HideMessageParams
    ) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>

    /**
     * Mute a user (Kind 44) - client-side moderation
     */
    readonly muteUser: (
      params: MuteUserParams
    ) => Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError>
  }
>() {}

// --- Service Implementation ---
export const Nip28ServiceLive = Layer.effect(
  Nip28Service,
  Effect.gen(function*() {
    const eventService = yield* EventService
    const relayService = yield* RelayService

    const createChannel = (
      params: CreateChannelParams
    ): Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError> =>
      Effect.scoped(Effect.gen(function*() {
        // Validate inputs
        if (!params.name.trim()) {
          return yield* Effect.fail(new Nip28InvalidInputError({ message: "Channel name cannot be empty" }))
        }

        // Create channel metadata content
        const content = JSON.stringify({
          name: params.name,
          about: params.about,
          picture: params.picture,
          relays: params.relays
        })

        // Create Kind 40 event (Channel Creation)
        const eventParams: EventParams = {
          kind: 40,
          tags: [],
          content
        }

        const event = yield* eventService.create(eventParams, params.privateKey).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to create channel event",
                cause: error
              })
            )
          )
        )

        // Publish to relay
        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const published = yield* connection.publish(event).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to publish channel event",
                cause: error
              })
            )
          )
        )

        if (!published) {
          return yield* Effect.fail(new Nip28PublishError({ message: "Channel event rejected by relay" }))
        }

        return event
      }))

    const getChannelMetadata = (
      channelId: EventId
    ): Effect.Effect<ChannelMetadata, Nip28FetchError | Nip28ChannelNotFoundError> =>
      Effect.scoped(Effect.gen(function*() {
        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28FetchError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const filter: Filter = {
          ids: [channelId],
          kinds: [40],
          limit: 1
        }

        const subscription = yield* connection.subscribe("channel-metadata" as SubscriptionId, [filter]).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28FetchError({
                message: "Failed to subscribe to channel metadata",
                cause: error
              })
            )
          )
        )

        const events = yield* Stream.runCollect(subscription.events).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28FetchError({
                message: "Failed to collect channel events",
                cause: error
              })
            )
          )
        )

        if (Chunk.isEmpty(events)) {
          return yield* Effect.fail(new Nip28ChannelNotFoundError({ channelId }))
        }

        const event = Chunk.unsafeHead(events)
        const content = JSON.parse(event.content)

        return {
          name: content.name,
          about: content.about,
          picture: content.picture,
          creatorPubkey: event.pubkey,
          channelId: event.id,
          relays: content.relays
        }
      }))

    const setChannelMetadata = (params: {
      channelId: EventId
      name?: string
      about?: string
      picture?: string
      privateKey: PrivateKey
    }): Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError> =>
      Effect.scoped(Effect.gen(function*() {
        // Create updated metadata content
        const content = JSON.stringify({
          name: params.name,
          about: params.about,
          picture: params.picture
        })

        // Create Kind 41 event (Channel Metadata)
        const eventParams: EventParams = {
          kind: 41,
          tags: [["e", params.channelId]],
          content
        }

        const event = yield* eventService.create(eventParams, params.privateKey).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to create channel metadata event",
                cause: error
              })
            )
          )
        )

        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const published = yield* connection.publish(event).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to publish metadata update",
                cause: error
              })
            )
          )
        )

        if (!published) {
          return yield* Effect.fail(new Nip28PublishError({ message: "Metadata update rejected by relay" }))
        }

        return event
      }))

    const sendChannelMessage = (
      params: SendChannelMessageParams
    ): Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError> =>
      Effect.scoped(Effect.gen(function*() {
        if (!params.content.trim()) {
          return yield* Effect.fail(new Nip28InvalidInputError({ message: "Message content cannot be empty" }))
        }

        // Build tags for channel message
        const tags: Array<Array<string>> = [
          ["e", params.channelId, params.relayHint || "", "root"]
        ]

        // Add reply tags if this is a reply
        if (params.replyToEventId) {
          tags.push(["e", params.replyToEventId, "", "reply"])
        }
        if (params.replyToPubkey) {
          tags.push(["p", params.replyToPubkey])
        }

        // Create Kind 42 event (Channel Message)
        const eventParams: EventParams = {
          kind: 42,
          tags,
          content: params.content
        }

        const event = yield* eventService.create(eventParams, params.privateKey).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to create message event",
                cause: error
              })
            )
          )
        )

        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const published = yield* connection.publish(event).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to publish message",
                cause: error
              })
            )
          )
        )

        if (!published) {
          return yield* Effect.fail(new Nip28PublishError({ message: "Message rejected by relay" }))
        }

        return event
      }))

    const getChannelMessages = (
      channelId: EventId,
      filterOptions?: Partial<Filter>
    ): Effect.Effect<Array<ChannelMessage>, Nip28FetchError> =>
      Effect.scoped(Effect.gen(function*() {
        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28FetchError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const filter: Filter = {
          kinds: [42],
          "#e": [channelId],
          ...filterOptions
        }

        const subscription = yield* connection.subscribe("channel-messages" as SubscriptionId, [filter]).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28FetchError({
                message: "Failed to subscribe to channel messages",
                cause: error
              })
            )
          )
        )

        const events = yield* Stream.runCollect(subscription.events).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28FetchError({
                message: "Failed to collect channel messages",
                cause: error
              })
            )
          )
        )

        // Transform events to ChannelMessage format
        return Chunk.toReadonlyArray(events).map((event) => ({
          ...event,
          channelId,
          replyToEventId: event.tags.find((tag) => tag[0] === "e" && tag[3] === "reply")?.[1] as EventId,
          replyToPubkey: event.tags.find((tag) => tag[0] === "p")?.[1] as PublicKey
        }))
      }))

    const subscribeToChannelMessages = (
      channelId: EventId
    ): Stream.Stream<ChannelMessage, Nip28FetchError> =>
      Stream.unwrapScoped(
        Effect.gen(function*() {
          const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip28FetchError({
                  message: "Failed to connect to relay",
                  cause: error
                })
              )
            )
          )

          const filter: Filter = {
            kinds: [42],
            "#e": [channelId]
          }

          const subscription = yield* connection.subscribe("channel-messages-live" as SubscriptionId, [filter]).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip28FetchError({
                  message: "Failed to subscribe to channel messages",
                  cause: error
                })
              )
            )
          )

          return Stream.map(subscription.events, (event) => ({
            ...event,
            channelId,
            replyToEventId: event.tags.find((tag) => tag[0] === "e" && tag[3] === "reply")?.[1] as EventId,
            replyToPubkey: event.tags.find((tag) => tag[0] === "p")?.[1] as PublicKey
          })).pipe(
            Stream.catchAll((error) =>
              Stream.fail(
                new Nip28FetchError({
                  message: "Stream error in channel messages",
                  cause: error
                })
              )
            )
          )
        })
      )

    const hideMessage = (
      params: HideMessageParams
    ): Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError> =>
      Effect.scoped(Effect.gen(function*() {
        const content = params.reason || "User chose to hide this message"

        const eventParams: EventParams = {
          kind: 43,
          tags: [["e", params.messageEventId]],
          content
        }

        const event = yield* eventService.create(eventParams, params.privateKey).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to create hide message event",
                cause: error
              })
            )
          )
        )

        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const published = yield* connection.publish(event).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to publish hide message event",
                cause: error
              })
            )
          )
        )

        if (!published) {
          return yield* Effect.fail(new Nip28PublishError({ message: "Hide message event rejected by relay" }))
        }

        return event
      }))

    const muteUser = (
      params: MuteUserParams
    ): Effect.Effect<NostrEvent, Nip28InvalidInputError | Nip28PublishError> =>
      Effect.scoped(Effect.gen(function*() {
        const content = params.reason || "User chose to mute this pubkey"

        const eventParams: EventParams = {
          kind: 44,
          tags: [["p", params.userPubkey]],
          content
        }

        const event = yield* eventService.create(eventParams, params.privateKey).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to create mute user event",
                cause: error
              })
            )
          )
        )

        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const published = yield* connection.publish(event).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip28PublishError({
                message: "Failed to publish mute user event",
                cause: error
              })
            )
          )
        )

        if (!published) {
          return yield* Effect.fail(new Nip28PublishError({ message: "Mute user event rejected by relay" }))
        }

        return event
      }))

    return {
      createChannel,
      getChannelMetadata,
      setChannelMetadata,
      sendChannelMessage,
      getChannelMessages,
      subscribeToChannelMessages,
      hideMessage,
      muteUser
    }
  })
)
