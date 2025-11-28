/**
 * Relay connection pool for managing multiple Nostr relays
 * @module
 */

import type { Scope } from "effect"
import { Context, Effect, HashMap, Layer, Option, Ref, Stream } from "effect"
import type { ConnectionError, MessageSendError } from "../core/Errors.js"
import { SubscriptionError } from "../core/Errors.js"
import type { EventId, Filter, NostrEvent, SubscriptionId } from "../core/Schema.js"
import { type RelayConnection, RelayService } from "./RelayService.js"

export interface PoolSubscription {
  readonly id: SubscriptionId
  readonly filters: ReadonlyArray<Filter>
  readonly events: Stream.Stream<NostrEvent, SubscriptionError | ConnectionError>
  readonly seenOn: Ref.Ref<HashMap.HashMap<EventId, ReadonlyArray<string>>>
}

export interface RelayPoolConnection {
  readonly urls: ReadonlyArray<string>
  readonly connections: Ref.Ref<HashMap.HashMap<string, RelayConnection>>
  readonly subscriptions: Ref.Ref<HashMap.HashMap<SubscriptionId, PoolSubscription>>

  readonly subscribe: (
    id: SubscriptionId,
    filters: ReadonlyArray<Filter>
  ) => Effect.Effect<PoolSubscription, SubscriptionError | ConnectionError, Scope.Scope>

  readonly unsubscribe: (id: SubscriptionId) => Effect.Effect<void, SubscriptionError>

  readonly publish: (event: NostrEvent) => Effect.Effect<
    HashMap.HashMap<string, boolean>,
    MessageSendError | ConnectionError
  >

  readonly close: () => Effect.Effect<void>

  readonly getConnectionStatus: () => Effect.Effect<HashMap.HashMap<string, "connected" | "disconnected">>
}

export interface RelayPoolService {
  readonly connect: (
    urls: ReadonlyArray<string>
  ) => Effect.Effect<RelayPoolConnection, ConnectionError, Scope.Scope>
}

export const RelayPoolService = Context.GenericTag<RelayPoolService>("nostr/RelayPoolService")

/**
 * Live implementation of RelayPoolService
 */
export const RelayPoolServiceLive = Layer.effect(
  RelayPoolService,
  Effect.gen(function*() {
    const relayService = yield* RelayService

    const connect = (urls: ReadonlyArray<string>): Effect.Effect<RelayPoolConnection, ConnectionError, Scope.Scope> =>
      Effect.gen(function*() {
        // State management
        const connections = yield* Ref.make<HashMap.HashMap<string, RelayConnection>>(HashMap.empty())
        const subscriptions = yield* Ref.make<HashMap.HashMap<SubscriptionId, PoolSubscription>>(HashMap.empty())

        // Connect to all relays
        const relayConnections = yield* Effect.forEach(urls, (url) =>
          relayService.connect(url).pipe(
            Effect.map((conn) => [url, conn] as const),
            Effect.catchAll(() => Effect.succeed(null))
          ))

        // Store successful connections
        yield* Effect.forEach(relayConnections, (result) => {
          if (result !== null) {
            const [url, conn] = result
            return Ref.update(connections, HashMap.set(url, conn))
          }
          return Effect.succeed(undefined)
        })

        // Pool methods
        const subscribe = (
          id: SubscriptionId,
          filters: ReadonlyArray<Filter>
        ): Effect.Effect<PoolSubscription, SubscriptionError | ConnectionError, Scope.Scope> =>
          Effect.gen(function*() {
            // Check if subscription already exists
            const existing = yield* Ref.get(subscriptions).pipe(
              Effect.map(HashMap.get(id))
            )
            if (Option.isSome(existing)) {
              return yield* Effect.fail(
                new SubscriptionError({
                  subscriptionId: id,
                  reason: "Subscription already exists"
                })
              )
            }

            // Create seen tracker
            const seenOn = yield* Ref.make<HashMap.HashMap<EventId, ReadonlyArray<string>>>(HashMap.empty())

            // Subscribe to all connected relays
            const currentConnections = yield* Ref.get(connections)
            const relaySubscriptions = yield* Effect.forEach(
              HashMap.entries(currentConnections),
              ([url, conn]) =>
                conn.subscribe(id, filters).pipe(
                  Effect.map((sub) => ({ url, stream: sub.events })),
                  Effect.catchAll(() => Effect.succeed(null))
                )
            )

            // Merge streams from all relays with deduplication
            const eventStream = Stream.mergeAll(
              relaySubscriptions
                .filter((sub): sub is { url: string; stream: Stream.Stream<NostrEvent, any> } => sub !== null)
                .map(({ stream, url }) =>
                  stream.pipe(
                    Stream.tap((event) =>
                      Ref.update(seenOn, (seen) => {
                        const current = HashMap.get(seen, event.id)
                        if (Option.isSome(current)) {
                          return HashMap.set(seen, event.id, [...current.value, url])
                        }
                        return HashMap.set(seen, event.id, [url])
                      })
                    ),
                    Stream.filter((event) => {
                      const seen = Effect.runSync(Ref.get(seenOn))
                      const relays = HashMap.get(seen, event.id)
                      // Only emit if this is the first relay to see this event
                      return Option.isSome(relays) && relays.value.length === 1
                    })
                  )
                ),
              { concurrency: "unbounded" }
            )

            // Create pool subscription
            const poolSubscription: PoolSubscription = {
              id,
              filters,
              events: eventStream,
              seenOn
            }

            // Store subscription
            yield* Ref.update(subscriptions, HashMap.set(id, poolSubscription))

            return poolSubscription
          })

        const unsubscribe = (id: SubscriptionId): Effect.Effect<void, SubscriptionError> =>
          Effect.gen(function*() {
            // Remove from subscriptions
            const sub = yield* Ref.get(subscriptions).pipe(
              Effect.map(HashMap.get(id))
            )

            if (Option.isNone(sub)) {
              return yield* Effect.fail(
                new SubscriptionError({
                  subscriptionId: id,
                  reason: "Subscription not found"
                })
              )
            }

            // Unsubscribe from all relays
            const currentConnections = yield* Ref.get(connections)
            yield* Effect.forEach(
              HashMap.values(currentConnections),
              (_conn) => Effect.succeed(undefined)
            )

            // Remove subscription
            yield* Ref.update(subscriptions, HashMap.remove(id))
          }).pipe(Effect.asVoid)

        const publish = (
          event: NostrEvent
        ): Effect.Effect<HashMap.HashMap<string, boolean>, MessageSendError | ConnectionError> =>
          Effect.gen(function*() {
            const currentConnections = yield* Ref.get(connections)
            const results = yield* Effect.forEach(
              HashMap.entries(currentConnections),
              ([url, conn]) =>
                conn.publish(event).pipe(
                  Effect.map((success) => [url, success] as const),
                  Effect.catchAll(() => Effect.succeed([url, false] as const))
                )
            )

            return HashMap.fromIterable(results)
          })

        const close = (): Effect.Effect<void, never, never> =>
          Effect.gen(function*() {
            // Close all connections
            const currentConnections = yield* Ref.get(connections)
            yield* Effect.forEach(
              HashMap.values(currentConnections),
              (_conn) => Effect.succeed(undefined)
            )

            // Clear state
            yield* Ref.set(connections, HashMap.empty())
            yield* Ref.set(subscriptions, HashMap.empty())
          }).pipe(Effect.asVoid)

        const getConnectionStatus = (): Effect.Effect<HashMap.HashMap<string, "connected" | "disconnected">> =>
          Effect.gen(function*() {
            const currentConnections = yield* Ref.get(connections)
            const connectedUrls = Array.from(HashMap.keys(currentConnections))

            return HashMap.fromIterable(
              urls.map((url) =>
                [
                  url,
                  connectedUrls.includes(url) ? "connected" : "disconnected"
                ] as const
              )
            )
          })

        return {
          urls,
          connections,
          subscriptions,
          subscribe,
          unsubscribe,
          publish,
          close,
          getConnectionStatus
        }
      })

    return { connect }
  })
)
