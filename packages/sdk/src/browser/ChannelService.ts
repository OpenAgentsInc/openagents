/**
 * Browser Channel Service with Effect.js
 * Real-time channel operations using WebSocket streams
 */

import { Context, Data, Effect, Layer, Option, Ref, Schema, Stream } from "effect"
import { WebSocketService } from "./WebSocketService.js"

// Nostr event type (inline definition to avoid import issues)
type NostrEvent = {
  id: string
  pubkey: string
  created_at: number
  kind: number
  tags: ReadonlyArray<ReadonlyArray<string>>
  content: string
  sig: string
}

// Channel schemas
export const Channel = Schema.Struct({
  id: Schema.String,
  name: Schema.String,
  about: Schema.String,
  picture: Schema.optional(Schema.String),
  created_at: Schema.Number,
  pubkey: Schema.String,
  message_count: Schema.Number,
  last_message_at: Schema.optional(Schema.Number)
})
export type Channel = Schema.Schema.Type<typeof Channel>

export const ChannelMessage = Schema.Struct({
  id: Schema.String,
  channel_id: Schema.String,
  pubkey: Schema.String,
  content: Schema.String,
  created_at: Schema.Number,
  tags: Schema.Array(Schema.Array(Schema.String))
})
export type ChannelMessage = Schema.Schema.Type<typeof ChannelMessage>

// Parameters
export const CreateChannelParams = Schema.Struct({
  name: Schema.String,
  about: Schema.String,
  picture: Schema.optional(Schema.String)
})
export type CreateChannelParams = Schema.Schema.Type<typeof CreateChannelParams>

export const SendMessageParams = Schema.Struct({
  channelId: Schema.String,
  content: Schema.String,
  replyTo: Schema.optional(Schema.String)
})
export type SendMessageParams = Schema.Schema.Type<typeof SendMessageParams>

// Errors
export class ChannelError extends Data.TaggedError("ChannelError")<{
  reason: "connection_failed" | "invalid_channel" | "send_failed" | "subscription_failed"
  message: string
  cause?: unknown
}> {}

// Nostr message types
type NostrMessage =
  | ["EVENT", string, NostrEvent]
  | ["EOSE", string]
  | ["OK", string, boolean, string]
  | ["CLOSED", string, string]
  | ["NOTICE", string]

// Channel Service
export class ChannelService extends Context.Tag("sdk/ChannelService")<
  ChannelService,
  {
    readonly channels: Stream.Stream<Channel, ChannelError>
    readonly messages: (channelId: string) => Stream.Stream<ChannelMessage, ChannelError>
    readonly createChannel: (params: CreateChannelParams) => Effect.Effect<Channel, ChannelError>
    readonly sendMessage: (params: SendMessageParams) => Effect.Effect<void, ChannelError>
  }
>() {}

// Generate subscription ID
const generateSubId = () => `sub-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`

// Live implementation
export const ChannelServiceLive = Layer.effect(
  ChannelService,
  Effect.gen(function*() {
    const wsService = yield* WebSocketService

    // Connect to relay
    const connection = yield* wsService.connect("ws://localhost:3003/relay").pipe(
      Effect.scoped,
      Effect.catchAll((error) =>
        Effect.fail(
          new ChannelError({
            reason: "connection_failed",
            message: error.message,
            cause: error
          })
        )
      )
    )

    // Channel cache
    const channelCache = yield* Ref.make(new Map<string, Channel>())

    // Parse Nostr messages
    const parseMessage = (data: string): Option.Option<NostrMessage> => {
      try {
        const msg = JSON.parse(data)
        if (Array.isArray(msg) && msg.length >= 2) {
          return Option.some(msg as NostrMessage)
        }
        return Option.none()
      } catch {
        return Option.none()
      }
    }

    // Subscribe to channel list
    const subscribeToChannels = Effect.gen(function*() {
      const subId = generateSubId()

      // Send subscription request
      const req = JSON.stringify([
        "REQ",
        subId,
        {
          kinds: [40], // NIP-28 channel creation
          limit: 100
        }
      ])

      yield* connection.send(req).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new ChannelError({
              reason: "subscription_failed",
              message: "Failed to subscribe to channels",
              cause: error
            })
          )
        )
      )

      // Process channel events
      return connection.messages.pipe(
        Stream.mapEffect((data) =>
          Effect.gen(function*() {
            const msg = parseMessage(data)
            if (Option.isNone(msg)) return Option.none()

            const message = msg.value
            if (message[0] === "EVENT" && message[1] === subId) {
              const event = message[2]
              if (event.kind === 40) {
                try {
                  const content = JSON.parse(event.content)
                  const channel: Channel = {
                    id: event.id,
                    name: content.name,
                    about: content.about || "",
                    picture: content.picture,
                    created_at: event.created_at,
                    pubkey: event.pubkey,
                    message_count: 0,
                    last_message_at: undefined
                  }

                  // Update cache
                  yield* Ref.update(channelCache, (cache) => {
                    const newCache = new Map(cache)
                    newCache.set(channel.id, channel)
                    return newCache
                  })

                  return Option.some(channel)
                } catch {
                  return Option.none()
                }
              }
            }
            return Option.none()
          })
        ),
        Stream.filter(Option.isSome),
        Stream.map((opt) => opt.value),
        Stream.catchAll((error) =>
          Stream.fail(
            new ChannelError({
              reason: "subscription_failed",
              message: error instanceof Error ? error.message : "Unknown error",
              cause: error
            })
          )
        )
      )
    })

    // Subscribe to channel messages
    const subscribeToMessages = (channelId: string) =>
      Effect.gen(function*() {
        const subId = generateSubId()

        // Send subscription request
        const req = JSON.stringify([
          "REQ",
          subId,
          {
            kinds: [42], // NIP-28 channel message
            "#e": [channelId],
            limit: 50
          }
        ])

        yield* connection.send(req).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new ChannelError({
                reason: "subscription_failed",
                message: "Failed to subscribe to messages",
                cause: error
              })
            )
          )
        )

        // Process message events
        return connection.messages.pipe(
          Stream.mapEffect((data) =>
            Effect.sync(() => {
              const msg = parseMessage(data)
              if (Option.isNone(msg)) return Option.none()

              const message = msg.value
              if (message[0] === "EVENT" && message[1] === subId) {
                const event = message[2]
                if (event.kind === 42) {
                  // Check if it's for our channel
                  const channelTag = event.tags.find((tag: ReadonlyArray<string>) =>
                    tag[0] === "e" && tag[1] === channelId
                  )
                  if (channelTag) {
                    const channelMessage: ChannelMessage = {
                      id: event.id,
                      channel_id: channelId,
                      pubkey: event.pubkey,
                      content: event.content,
                      created_at: event.created_at,
                      tags: event.tags
                    }
                    return Option.some(channelMessage)
                  }
                }
              }
              return Option.none()
            })
          ),
          Stream.filter(Option.isSome),
          Stream.map((opt) => opt.value)
        )
      })

    return {
      channels: Stream.unwrap(subscribeToChannels).pipe(
        Stream.catchAll((error) =>
          Stream.fail(
            new ChannelError({
              reason: "subscription_failed",
              message: error instanceof Error ? error.message : "Unknown error",
              cause: error
            })
          )
        )
      ),

      messages: (channelId: string) =>
        Stream.unwrap(subscribeToMessages(channelId)).pipe(
          Stream.catchAll((error) =>
            Stream.fail(
              new ChannelError({
                reason: "subscription_failed",
                message: error instanceof Error ? error.message : "Unknown error",
                cause: error
              })
            )
          )
        ),

      createChannel: (_params: CreateChannelParams) =>
        Effect.gen(function*() {
          // For now, return error - need private key to sign events
          // This would be implemented with proper key management
          return yield* Effect.fail(
            new ChannelError({
              reason: "send_failed",
              message: "Channel creation requires key management implementation"
            })
          )
        }),

      sendMessage: (_params: SendMessageParams) =>
        Effect.gen(function*() {
          // For now, return error - need private key to sign events
          // This would be implemented with proper key management
          return yield* Effect.fail(
            new ChannelError({
              reason: "send_failed",
              message: "Message sending requires key management implementation"
            })
          )
        })
    }
  })
)
