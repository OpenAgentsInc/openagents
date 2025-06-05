/**
 * Nostr relay connection and subscription management
 * @module
 */

import type { Scope } from "effect"
import { Context, Effect, HashMap, Layer, Option, Queue, Ref, Schema, Stream } from "effect"
import type { ConnectionError, MessageSendError, RelayError } from "../core/Errors.js"
import { SubscriptionError, TimeoutError } from "../core/Errors.js"
import type { ClientMessage, Filter, NostrEvent, RelayMessage, SubscriptionId } from "../core/Schema.js"
import { WebSocketService } from "./WebSocketService.js"

export interface Subscription {
  readonly id: SubscriptionId
  readonly filters: ReadonlyArray<Filter>
  readonly events: Stream.Stream<NostrEvent, SubscriptionError | ConnectionError>
}

export interface RelayConnection {
  readonly url: string
  readonly subscribe: (
    id: SubscriptionId,
    filters: ReadonlyArray<Filter>
  ) => Effect.Effect<Subscription, SubscriptionError | ConnectionError | MessageSendError, Scope.Scope>
  readonly publish: (event: NostrEvent) => Effect.Effect<boolean, RelayError | ConnectionError | MessageSendError>
  readonly close: (subscriptionId: SubscriptionId) => Effect.Effect<void, MessageSendError>
  readonly disconnect: () => Effect.Effect<void>
}

/**
 * Service for relay operations
 */
export class RelayService extends Context.Tag("nostr/RelayService")<
  RelayService,
  {
    /**
     * Connect to a relay
     */
    readonly connect: (url: string) => Effect.Effect<RelayConnection, ConnectionError, Scope.Scope>
  }
>() {}

interface SubscriptionState {
  readonly queue: Queue.Queue<NostrEvent>
  readonly filters: ReadonlyArray<Filter>
  readonly eoseReceived: boolean
}

/**
 * Live implementation of RelayService
 */
export const RelayServiceLive = Layer.effect(
  RelayService,
  Effect.gen(function*() {
    const wsService = yield* WebSocketService

    const connect = (url: string): Effect.Effect<RelayConnection, ConnectionError, Scope.Scope> =>
      Effect.gen(function*() {
        // Connect to WebSocket
        const ws = yield* wsService.connect(url)

        // State management
        const subscriptions = yield* Ref.make<HashMap.HashMap<SubscriptionId, SubscriptionState>>(HashMap.empty())
        const pendingOk = yield* Ref.make<HashMap.HashMap<string, (result: boolean) => void>>(HashMap.empty())

        // Process incoming messages
        const processMessages = Stream.runForEach(ws.messages, (data: string) =>
          Effect.gen(function*() {
            // Parse message
            const message = yield* Schema.decodeUnknown(RelayMessage)(JSON.parse(data)).pipe(
              Effect.catchAll(() => Effect.succeed(null))
            )

            if (!message) return

            switch (message[0]) {
              case "EVENT": {
                const [, subId, event] = message
                const subs = yield* Ref.get(subscriptions)
                const sub = HashMap.get(subs, subId)

                if (Option.isSome(sub)) {
                  yield* Queue.offer(sub.value.queue, event)
                }
                break
              }

              case "OK": {
                const [, eventId, success] = message
                const pending = yield* Ref.get(pendingOk)
                const callback = HashMap.get(pending, eventId)

                if (Option.isSome(callback)) {
                  callback.value(success)
                  yield* Ref.update(pendingOk, HashMap.remove(eventId))
                }
                break
              }

              case "EOSE": {
                const [, subId] = message
                yield* Ref.update(
                  subscriptions,
                  HashMap.modify(
                    subId,
                    (state) => ({ ...state, eoseReceived: true })
                  )
                )
                break
              }

              case "CLOSED": {
                const [, subId] = message
                const subs = yield* Ref.get(subscriptions)
                const sub = HashMap.get(subs, subId)

                if (Option.isSome(sub)) {
                  yield* Queue.shutdown(sub.value.queue)
                  yield* Ref.update(subscriptions, HashMap.remove(subId))
                }
                break
              }

              case "NOTICE": {
                // Log notice messages (could be exposed as a stream later)
                console.log(`[${url}] NOTICE:`, message[1])
                break
              }
            }
          })).pipe(
            Effect.forkScoped,
            Effect.interruptible
          )

        yield* processMessages

        const subscribe = (
          id: SubscriptionId,
          filters: ReadonlyArray<Filter>
        ): Effect.Effect<Subscription, SubscriptionError | ConnectionError | MessageSendError, Scope.Scope> =>
          Effect.gen(function*() {
            // Check if subscription already exists
            const subs = yield* Ref.get(subscriptions)
            if (HashMap.has(subs, id)) {
              return yield* Effect.fail(
                new SubscriptionError({
                  subscriptionId: id,
                  reason: "Subscription ID already exists"
                })
              )
            }

            // Create subscription state
            const queue = yield* Queue.unbounded<NostrEvent>()
            const subState: SubscriptionState = {
              queue,
              filters,
              eoseReceived: false
            }

            // Add to subscriptions
            yield* Ref.update(subscriptions, HashMap.set(id, subState))

            // Send REQ message
            const reqMessage: ClientMessage = ["REQ", id, ...filters]
            yield* ws.send(JSON.stringify(reqMessage))

            // Add finalizer to clean up subscription
            yield* Effect.addFinalizer(() =>
              Effect.gen(function*() {
                yield* ws.send(JSON.stringify(["CLOSE", id])).pipe(
                  Effect.catchAll(() => Effect.succeed(undefined))
                )
                yield* Queue.shutdown(queue)
                yield* Ref.update(subscriptions, HashMap.remove(id))
              })
            )

            const subscription: Subscription = {
              id,
              filters,
              events: Stream.fromQueue(queue).pipe(
                Stream.catchAll((error) =>
                  Stream.fail(
                    new SubscriptionError({
                      subscriptionId: id,
                      reason: String(error)
                    })
                  )
                )
              )
            }

            return subscription
          })

        const publish = (event: NostrEvent): Effect.Effect<boolean, RelayError | ConnectionError | MessageSendError> =>
          Effect.gen(function*() {
            // Send EVENT message
            const eventMessage: ClientMessage = ["EVENT", event]
            yield* ws.send(JSON.stringify(eventMessage))

            // Wait for OK response
            const result = yield* Effect.async<boolean, TimeoutError>((resume) => {
              Ref.update(
                pendingOk,
                HashMap.set(
                  event.id,
                  (success: boolean) => resume(Effect.succeed(success))
                )
              ).pipe(Effect.runSync)
            }).pipe(
              Effect.timeoutFail({
                duration: "10 seconds",
                onTimeout: () =>
                  new TimeoutError({
                    operation: "publish",
                    timeoutMs: 10000
                  })
              }),
              Effect.catchAll(() => Effect.succeed(false))
            )

            return result
          })

        const close = (subscriptionId: SubscriptionId): Effect.Effect<void, MessageSendError> =>
          Effect.gen(function*() {
            const closeMessage: ClientMessage = ["CLOSE", subscriptionId]
            yield* ws.send(JSON.stringify(closeMessage))

            // Remove from local state
            yield* Ref.update(subscriptions, HashMap.remove(subscriptionId))
          })

        const disconnect = (): Effect.Effect<void> => ws.close()

        const connection: RelayConnection = {
          url,
          subscribe,
          publish,
          close,
          disconnect
        }

        return connection
      })

    return { connect }
  })
)
