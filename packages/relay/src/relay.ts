/**
 * Core Nostr relay implementation
 * Handles WebSocket connections, subscriptions, and NIP-01 protocol
 */
import { Schema as NostrSchema } from "@openagentsinc/nostr"
import { Context, Effect, HashMap, Layer, Queue, Ref, Schema, Stream } from "effect"
import { RelayDatabase } from "./database.js"

// Type aliases for cleaner code
type NostrEvent = NostrSchema.NostrEvent
type Filter = NostrSchema.Filter
type ClientMessage = NostrSchema.ClientMessage
type SubscriptionId = NostrSchema.SubscriptionId

// Connection state management
interface ConnectionState {
  readonly id: string
  readonly subscriptions: HashMap.HashMap<SubscriptionId, SubscriptionState>
  readonly isActive: boolean
  readonly connectedAt: Date
}

interface SubscriptionState {
  readonly id: SubscriptionId
  readonly filters: Array<Filter>
  readonly queue: Queue.Queue<NostrEvent>
  readonly isActive: boolean
}

// Error types
export class RelayError extends Schema.TaggedError<RelayError>()(
  "RelayError",
  {
    message: Schema.String,
    connectionId: Schema.optional(Schema.String),
    subscriptionId: Schema.optional(Schema.String)
  }
) {}

export class MessageError extends Schema.TaggedError<MessageError>()(
  "MessageError",
  {
    message: Schema.String,
    messageType: Schema.String,
    rawMessage: Schema.Unknown
  }
) {}

// Relay service interface
export class NostrRelay extends Context.Tag("NostrRelay")<
  NostrRelay,
  {
    readonly handleConnection: (connectionId: string) => Effect.Effect<ConnectionHandler, RelayError>
    readonly getConnectionCount: () => Effect.Effect<number>
    readonly getActiveSubscriptions: () => Effect.Effect<number>
    readonly broadcastEvent: (event: NostrEvent) => Effect.Effect<number> // Returns count of connections notified
    readonly getStats: () => Effect.Effect<RelayStats>
  }
>() {}

// Connection handler for individual WebSocket connections
export interface ConnectionHandler {
  readonly processMessage: (rawMessage: string) => Effect.Effect<void, MessageError>
  readonly close: () => Effect.Effect<void>
  readonly subscribe: (
    subscriptionId: SubscriptionId,
    filters: Array<Filter>
  ) => Effect.Effect<Stream.Stream<NostrEvent>, RelayError>
  readonly unsubscribe: (subscriptionId: SubscriptionId) => Effect.Effect<void>
}

export interface RelayStats {
  readonly totalConnections: number
  readonly activeConnections: number
  readonly totalSubscriptions: number
  readonly eventsStored: number
  readonly eventsServed: number
  readonly uptime: number
}

// NIP-01 message parsing
const parseClientMessage = (raw: string): Effect.Effect<ClientMessage, MessageError> =>
  Effect.gen(function*() {
    let parsed: unknown
    try {
      parsed = JSON.parse(raw)
    } catch {
      return yield* Effect.fail(
        new MessageError({
          message: "Invalid JSON",
          messageType: "unknown",
          rawMessage: raw
        })
      )
    }

    if (!Array.isArray(parsed) || parsed.length < 1) {
      return yield* Effect.fail(
        new MessageError({
          message: "Message must be array with at least one element",
          messageType: "unknown",
          rawMessage: parsed
        })
      )
    }

    const [type, ...args] = parsed

    switch (type) {
      case "EVENT":
        if (args.length !== 1 || typeof args[0] !== "object") {
          return yield* Effect.fail(
            new MessageError({
              message: "EVENT message must have exactly one event object",
              messageType: "EVENT",
              rawMessage: parsed
            })
          )
        }
        return ["EVENT", args[0] as NostrEvent] as ClientMessage

      case "REQ": {
        if (args.length < 2 || typeof args[0] !== "string") {
          return yield* Effect.fail(
            new MessageError({
              message: "REQ message must have subscription ID and at least one filter",
              messageType: "REQ",
              rawMessage: parsed
            })
          )
        }

        // Validate subscription ID
        const subscriptionIdResult = yield* Schema.decodeUnknown(NostrSchema.SubscriptionId)(args[0]).pipe(
          Effect.mapError(() =>
            new MessageError({
              message: "Invalid subscription ID format",
              messageType: "REQ",
              rawMessage: parsed
            })
          )
        )

        return ["REQ", subscriptionIdResult, ...args.slice(1)] as ClientMessage
      }

      case "CLOSE": {
        if (args.length !== 1 || typeof args[0] !== "string") {
          return yield* Effect.fail(
            new MessageError({
              message: "CLOSE message must have exactly one subscription ID",
              messageType: "CLOSE",
              rawMessage: parsed
            })
          )
        }

        // Validate subscription ID
        const closeSubscriptionIdResult = yield* Schema.decodeUnknown(NostrSchema.SubscriptionId)(args[0]).pipe(
          Effect.mapError(() =>
            new MessageError({
              message: "Invalid subscription ID format",
              messageType: "CLOSE",
              rawMessage: parsed
            })
          )
        )

        return ["CLOSE", closeSubscriptionIdResult] as ClientMessage
      }

      default:
        return yield* Effect.fail(
          new MessageError({
            message: `Unknown message type: ${type}`,
            messageType: String(type),
            rawMessage: parsed
          })
        )
    }
  })

// Create relay message
const createRelayMessage = (type: string, ...args: Array<unknown>): string => JSON.stringify([type, ...args])

// Live implementation
export const NostrRelayLive = Layer.effect(
  NostrRelay,
  Effect.gen(function*() {
    const database = yield* RelayDatabase

    // Global state
    const connections = yield* Ref.make<HashMap.HashMap<string, ConnectionState>>(HashMap.empty())
    const stats = yield* Ref.make({
      totalConnections: 0,
      eventsStored: 0,
      eventsServed: 0,
      startTime: Date.now()
    })

    const handleConnection = (connectionId: string): Effect.Effect<ConnectionHandler, RelayError> =>
      Effect.gen(function*() {
        // Initialize connection state
        const connectionState: ConnectionState = {
          id: connectionId,
          subscriptions: HashMap.empty(),
          isActive: true,
          connectedAt: new Date()
        }

        yield* Ref.update(connections, HashMap.set(connectionId, connectionState))
        yield* Ref.update(stats, (s) => ({ ...s, totalConnections: s.totalConnections + 1 }))

        // Track active subscriptions for this connection
        const activeSubscriptions = yield* Ref.make<HashMap.HashMap<SubscriptionId, SubscriptionState>>(HashMap.empty())

        const processMessage = (rawMessage: string): Effect.Effect<void, MessageError> =>
          Effect.gen(function*() {
            const message = yield* parseClientMessage(rawMessage)

            switch (message[0]) {
              case "EVENT": {
                const event = message[1]

                // Store event in database
                const stored = yield* database.storeEvent(event).pipe(
                  Effect.catchAll((error) => {
                    console.error("Failed to store event:", error)
                    return Effect.succeed(false)
                  })
                )

                // Send OK response
                const okMessage = createRelayMessage(
                  "OK",
                  event.id,
                  stored,
                  stored ? "" : "error: failed to store event"
                )

                // Broadcast to matching subscriptions if stored
                if (stored) {
                  yield* broadcastToSubscriptions(event)
                  yield* Ref.update(stats, (s) => ({ ...s, eventsStored: s.eventsStored + 1 }))
                }

                // Note: In real implementation, we'd send the OK message back via WebSocket
                console.log(`[${connectionId}] ${okMessage}`)
                break
              }

              case "REQ": {
                const [, subscriptionId, ...filters] = message

                // Create subscription
                const queue = yield* Queue.unbounded<NostrEvent>()
                const subscription: SubscriptionState = {
                  id: subscriptionId,
                  filters: filters as Array<Filter>,
                  queue,
                  isActive: true
                }

                yield* Ref.update(activeSubscriptions, HashMap.set(subscriptionId, subscription))

                // Query existing events
                const existingEvents = yield* database.queryEvents(filters as Array<Filter>).pipe(
                  Effect.catchAll((error) => {
                    console.error("Failed to query events:", error)
                    return Effect.succeed([])
                  })
                )

                // Send existing events
                for (const event of existingEvents) {
                  const eventMessage = createRelayMessage("EVENT", subscriptionId, event)
                  console.log(`[${connectionId}] ${eventMessage}`)
                  yield* Ref.update(stats, (s) => ({ ...s, eventsServed: s.eventsServed + 1 }))
                }

                // Send EOSE
                const eoseMessage = createRelayMessage("EOSE", subscriptionId)
                console.log(`[${connectionId}] ${eoseMessage}`)

                break
              }

              case "CLOSE": {
                const subscriptionId = message[1]

                // Remove subscription
                yield* Ref.update(activeSubscriptions, HashMap.remove(subscriptionId as any))

                const closedMessage = createRelayMessage("CLOSED", subscriptionId, "subscription closed")
                console.log(`[${connectionId}] ${closedMessage}`)
                break
              }
            }
          })

        const close = (): Effect.Effect<void> =>
          Effect.gen(function*() {
            yield* Ref.update(connections, HashMap.remove(connectionId))

            // Close all active subscriptions
            const subs = yield* Ref.get(activeSubscriptions)
            yield* Effect.forEach(
              HashMap.values(subs),
              (sub) => Queue.shutdown(sub.queue),
              { concurrency: "unbounded" }
            )

            console.log(`[${connectionId}] Connection closed`)
          })

        const subscribe = (
          subscriptionId: SubscriptionId,
          _filters: Array<Filter>
        ): Effect.Effect<Stream.Stream<NostrEvent>, RelayError> =>
          Effect.gen(function*() {
            const subs = yield* Ref.get(activeSubscriptions)
            const existingSub = HashMap.get(subs, subscriptionId)

            if (existingSub._tag === "Some") {
              return Stream.fromQueue(existingSub.value.queue)
            }

            return yield* Effect.fail(
              new RelayError({
                message: "Subscription not found",
                connectionId,
                subscriptionId
              })
            )
          })

        const unsubscribe = (subscriptionId: SubscriptionId): Effect.Effect<void> =>
          Effect.gen(function*() {
            const subs = yield* Ref.get(activeSubscriptions)
            const sub = HashMap.get(subs, subscriptionId)

            if (sub._tag === "Some") {
              yield* Queue.shutdown(sub.value.queue)
              yield* Ref.update(activeSubscriptions, HashMap.remove(subscriptionId))
            }
          })

        // Helper function to broadcast events to matching subscriptions
        const broadcastToSubscriptions = (event: NostrEvent): Effect.Effect<void> =>
          Effect.gen(function*() {
            const subs = yield* Ref.get(activeSubscriptions)

            yield* Effect.forEach(
              HashMap.values(subs),
              (sub) => {
                // Check if event matches any filter
                const matches = sub.filters.some((filter) => eventMatchesFilter(event, filter))

                if (matches) {
                  return Queue.offer(sub.queue, event).pipe(
                    Effect.catchAll(() => Effect.succeed(void 0)) // Ignore failed offers
                  )
                }

                return Effect.succeed(void 0)
              },
              { concurrency: "unbounded" }
            )
          })

        return {
          processMessage,
          close,
          subscribe,
          unsubscribe
        }
      })

    const getConnectionCount = (): Effect.Effect<number> =>
      Effect.gen(function*() {
        const conns = yield* Ref.get(connections)
        return HashMap.size(conns)
      })

    const getActiveSubscriptions = (): Effect.Effect<number> =>
      Effect.gen(function*() {
        const conns = yield* Ref.get(connections)
        let totalSubs = 0

        for (const conn of HashMap.values(conns)) {
          totalSubs += HashMap.size(conn.subscriptions)
        }

        return totalSubs
      })

    const broadcastEvent = (_event: NostrEvent): Effect.Effect<number> => {
      const notified = 0

      // In a real implementation, this would iterate through all active connections
      // and check their subscriptions for matches, then send the event via WebSocket

      return Effect.succeed(notified)
    }

    const getStats = (): Effect.Effect<RelayStats> =>
      Effect.gen(function*() {
        const currentStats = yield* Ref.get(stats)
        const connectionCount = yield* getConnectionCount()
        const subscriptionCount = yield* getActiveSubscriptions()

        return {
          totalConnections: currentStats.totalConnections,
          activeConnections: connectionCount,
          totalSubscriptions: subscriptionCount,
          eventsStored: currentStats.eventsStored,
          eventsServed: currentStats.eventsServed,
          uptime: Date.now() - currentStats.startTime
        }
      })

    return {
      handleConnection,
      getConnectionCount,
      getActiveSubscriptions,
      broadcastEvent,
      getStats
    }
  })
)

// Helper function to check if event matches filter
const eventMatchesFilter = (event: NostrEvent, filter: Filter): boolean => {
  if (filter.ids && !filter.ids.includes(event.id)) return false
  if (filter.authors && !filter.authors.includes(event.pubkey)) return false
  if (filter.kinds && !filter.kinds.includes(event.kind)) return false
  if (filter.since && event.created_at < filter.since) return false
  if (filter.until && event.created_at > filter.until) return false

  // Check tag filters
  for (const [key, values] of Object.entries(filter)) {
    if (key.startsWith("#") && Array.isArray(values)) {
      const tagName = key.slice(1)
      const hasMatchingTag = event.tags.some((tag) => tag.length >= 2 && tag[0] === tagName && values.includes(tag[1]))
      if (!hasMatchingTag) return false
    }
  }

  return true
}
