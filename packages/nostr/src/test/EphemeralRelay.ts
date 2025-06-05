/**
 * Ephemeral in-memory relay for testing
 * @module
 */

import type { Scope } from "effect"
import { Console, Context, Effect, HashMap, Layer, Ref, Schema } from "effect"
import { createServer } from "http"
import type { WebSocket } from "ws"
import { WebSocketServer } from "ws"
import { ClientMessage, type Filter, type NostrEvent, type RelayMessage, type SubscriptionId } from "../core/Schema.js"

interface ClientConnection {
  readonly id: string
  readonly ws: WebSocket
  readonly subscriptions: HashMap.HashMap<SubscriptionId, ReadonlyArray<Filter>>
}

export interface EphemeralRelay {
  readonly url: string
  readonly start: () => Effect.Effect<void, never, Scope.Scope>
  readonly getStoredEvents: () => Effect.Effect<ReadonlyArray<NostrEvent>>
  readonly clearEvents: () => Effect.Effect<void>
  readonly getConnectionCount: () => Effect.Effect<number>
}

/**
 * Service for ephemeral relay testing
 */
export class EphemeralRelayService extends Context.Tag("test/EphemeralRelayService")<
  EphemeralRelayService,
  EphemeralRelay
>() {}

/**
 * Check if an event matches a filter
 */
const matchesFilter = (event: NostrEvent, filter: Filter): boolean => {
  if (filter.ids && !filter.ids.includes(event.id)) return false
  if (filter.authors && !filter.authors.includes(event.pubkey)) return false
  if (filter.kinds && !filter.kinds.includes(event.kind)) return false
  if (filter.since && event.created_at < filter.since) return false
  if (filter.until && event.created_at > filter.until) return false

  // Check tag filters
  const tagFilters = Object.entries(filter).filter(([key]) => key.startsWith("#"))
  for (const [tagName, values] of tagFilters) {
    const tagKey = tagName.substring(1)
    const eventTagValues = event.tags
      .filter((tag) => tag[0] === tagKey)
      .map((tag) => tag[1])

    if (values && !values.some((v: string) => eventTagValues.includes(v))) {
      return false
    }
  }

  return true
}

/**
 * Create an ephemeral relay for testing
 */
export const makeEphemeralRelay = (port = 0): Effect.Effect<EphemeralRelay> =>
  Effect.gen(function*() {
    // State
    const events = yield* Ref.make<HashMap.HashMap<string, NostrEvent>>(HashMap.empty())
    const clients = yield* Ref.make<HashMap.HashMap<string, ClientConnection>>(HashMap.empty())
    let actualPort = port

    const start = (): Effect.Effect<void, never, Scope.Scope> =>
      Effect.gen(function*() {
        const server = createServer()
        const wss = new WebSocketServer({ server })

        // Handle new connections
        wss.on("connection", (ws) => {
          const clientId = Math.random().toString(36).substring(7)

          Effect.runSync(Effect.gen(function*() {
            // Add client
            const client: ClientConnection = {
              id: clientId,
              ws,
              subscriptions: HashMap.empty()
            }
            yield* Ref.update(clients, HashMap.set(clientId, client))

            // Handle messages
            ws.on("message", (data) => {
              Effect.runSync(Effect.gen(function*() {
                const message = yield* Schema.decodeUnknown(ClientMessage)(
                  JSON.parse(data.toString())
                ).pipe(Effect.orElseSucceed(() => null))

                if (!message) return

                switch (message[0]) {
                  case "EVENT": {
                    const event = message[1]

                    // Store event
                    yield* Ref.update(events, HashMap.set(event.id, event))

                    // Send OK
                    const okMessage: RelayMessage = ["OK", event.id, true, ""]
                    ws.send(JSON.stringify(okMessage))

                    // Broadcast to matching subscriptions
                    const allClients = yield* Ref.get(clients)
                    yield* Effect.forEach(HashMap.values(allClients), (otherClient) =>
                      Effect.forEach(HashMap.entries(otherClient.subscriptions), ([subId, filters]) => {
                        if (filters.some((f) => matchesFilter(event, f))) {
                          const eventMessage: RelayMessage = ["EVENT", subId, event]
                          return Effect.sync(() =>
                            otherClient.ws.send(JSON.stringify(eventMessage))
                          ).pipe(Effect.catchAll(() => Effect.succeed(undefined)))
                        }
                        return Effect.succeed(undefined)
                      }))
                    break
                  }

                  case "REQ": {
                    const [, subId, ...filters] = message

                    // Update client subscriptions
                    yield* Ref.update(
                      clients,
                      HashMap.modify(
                        clientId,
                        (client) => ({
                          ...client,
                          subscriptions: HashMap.set(client.subscriptions, subId, filters)
                        })
                      )
                    )

                    // Send matching stored events
                    const storedEvents = yield* Ref.get(events)
                    const allEvents = Array.from(HashMap.values(storedEvents))
                    const matching = allEvents.filter((event) =>
                      filters.some((f) =>
                        matchesFilter(event, f)
                      )
                    )

                    // Apply limit if specified
                    const limit = filters[0]?.limit
                    const toSend = limit ? matching.slice(-limit) : matching

                    // Send events
                    yield* Effect.forEach(toSend, (event) => {
                      const eventMessage: RelayMessage = ["EVENT", subId, event]
                      return Effect.sync(() => ws.send(JSON.stringify(eventMessage)))
                        .pipe(Effect.catchAll(() => Effect.succeed(undefined)))
                    })

                    // Send EOSE
                    const eoseMessage: RelayMessage = ["EOSE", subId]
                    ws.send(JSON.stringify(eoseMessage))
                    break
                  }

                  case "CLOSE": {
                    const [, subId] = message

                    // Remove subscription
                    yield* Ref.update(
                      clients,
                      HashMap.modify(
                        clientId,
                        (client) => ({
                          ...client,
                          subscriptions: HashMap.remove(client.subscriptions, subId)
                        })
                      )
                    )

                    // Send CLOSED
                    const closedMessage: RelayMessage = ["CLOSED", subId, ""]
                    ws.send(JSON.stringify(closedMessage))
                    break
                  }
                }
              }))
            })

            // Handle disconnect
            ws.on("close", () => {
              Effect.runSync(Ref.update(clients, HashMap.remove(clientId)))
            })

            ws.on("error", () => {
              Effect.runSync(Ref.update(clients, HashMap.remove(clientId)))
            })
          }))
        })

        // Start server
        yield* Effect.async<void>((resume) => {
          server.listen(port, () => {
            actualPort = (server.address() as any).port
            resume(Effect.succeed(undefined))
          })
        })

        // Add finalizer
        yield* Effect.addFinalizer(() =>
          Effect.async<void>((resume) => {
            wss.close(() => {
              server.close(() => {
                resume(Effect.succeed(undefined))
              })
            })
          })
        )

        yield* Console.log(`Ephemeral relay started on port ${actualPort}`)
      })

    const url = `ws://localhost:${actualPort}`

    const getStoredEvents = (): Effect.Effect<ReadonlyArray<NostrEvent>> =>
      Ref.get(events).pipe(Effect.map((map) => Array.from(HashMap.values(map))))

    const clearEvents = (): Effect.Effect<void> => Ref.set(events, HashMap.empty())

    const getConnectionCount = (): Effect.Effect<number> => Ref.get(clients).pipe(Effect.map(HashMap.size))

    return {
      url,
      start,
      getStoredEvents,
      clearEvents,
      getConnectionCount
    }
  })

/**
 * Layer for ephemeral relay
 */
export const EphemeralRelayLive = (port = 0): Layer.Layer<EphemeralRelayService> =>
  Layer.effect(
    EphemeralRelayService,
    makeEphemeralRelay(port)
  )

/**
 * Run a test with an ephemeral relay
 */
export const withEphemeralRelay = <A, E, R>(
  effect: Effect.Effect<A, E, R>,
  port = 0
): Effect.Effect<A, E, Exclude<R, EphemeralRelayService>> =>
  Effect.scoped(
    Effect.gen(function*() {
      const relay = yield* EphemeralRelayService
      yield* relay.start()
      return yield* effect
    })
  ).pipe(Effect.provide(EphemeralRelayLive(port)))
