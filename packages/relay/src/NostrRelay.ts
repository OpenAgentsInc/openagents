/**
 * Pylon - Nostr relay server implementation
 * @module
 */

import type { HttpServerRequest as Request } from "@effect/platform"
import { HttpRouter, HttpServerResponse } from "@effect/platform"
import type { Scope } from "effect"
import { Context, Effect, HashMap, Layer, Queue, Ref, Stream } from "effect"
import type { WebSocket } from "ws"
import { EventStorage, FilterMatcher } from "./services/index.js"
import type { ClientMessage, Filter, RelayMessage, SubscriptionId } from "./types.js"

interface ClientConnection {
  readonly id: string
  readonly ws: WebSocket
  readonly subscriptions: Ref.Ref<HashMap.HashMap<SubscriptionId, Array<Filter>>>
  readonly outgoingQueue: Queue.Queue<RelayMessage>
}

export interface NostrRelayConfig {
  readonly name: string
  readonly description: string
  readonly pubkey?: string
  readonly contact?: string
  readonly supported_nips: ReadonlyArray<number>
  readonly software: string
  readonly version: string
  readonly limitation: {
    readonly max_message_length: number
    readonly max_subscriptions: number
    readonly max_filters: number
    readonly max_limit: number
    readonly max_subid_length: number
    readonly max_event_tags: number
    readonly max_content_length: number
    readonly min_pow_difficulty: number
    readonly auth_required: boolean
    readonly payment_required: boolean
    readonly restricted_writes: boolean
  }
}

export const defaultRelayConfig: NostrRelayConfig = {
  name: "Pylon Nostr Relay",
  description: "An Effect-based Nostr relay implementation",
  supported_nips: [1, 11, 12, 15, 20],
  software: "https://github.com/OpenAgentsInc/openagents",
  version: "0.1.0",
  limitation: {
    max_message_length: 16384,
    max_subscriptions: 20,
    max_filters: 10,
    max_limit: 5000,
    max_subid_length: 100,
    max_event_tags: 100,
    max_content_length: 8196,
    min_pow_difficulty: 0,
    auth_required: false,
    payment_required: false,
    restricted_writes: false
  }
}

export interface NostrRelayService {
  readonly config: NostrRelayConfig
  readonly handleWebSocket: (ws: WebSocket, req: Request.HttpServerRequest) => Effect.Effect<void, never, Scope.Scope>
  readonly getInfo: () => NostrRelayConfig
}

export const NostrRelayService = Context.GenericTag<NostrRelayService>("pylon/NostrRelayService")

/**
 * Live implementation of NostrRelayService
 */
export const NostrRelayServiceLive = Layer.effect(
  NostrRelayService,
  Effect.gen(function*() {
    const eventStorage = yield* EventStorage
    const filterMatcher = yield* FilterMatcher

    // Client management
    const clients = yield* Ref.make<HashMap.HashMap<string, ClientConnection>>(HashMap.empty())
    let clientIdCounter = 0

    const handleWebSocket = (ws: WebSocket, _req: Request.HttpServerRequest): Effect.Effect<void, never, Scope.Scope> =>
      Effect.gen(function*() {
        const clientId = `client-${++clientIdCounter}`
        const subscriptions = yield* Ref.make<HashMap.HashMap<SubscriptionId, Array<Filter>>>(HashMap.empty())
        const outgoingQueue = yield* Queue.unbounded<RelayMessage>()

        const client: ClientConnection = {
          id: clientId,
          ws,
          subscriptions,
          outgoingQueue
        }

        // Add client
        yield* Ref.update(clients, HashMap.set(clientId, client))

        // Setup cleanup
        yield* Effect.addFinalizer(() => Ref.update(clients, HashMap.remove(clientId)))

        // Send queued messages
        const sendMessages = Stream.fromQueue(outgoingQueue).pipe(
          Stream.tap((message) =>
            Effect.sync(() => {
              ws.send(JSON.stringify(message))
            })
          ),
          Stream.runDrain
        )

        // Start message sender
        yield* Effect.fork(sendMessages)

        // Handle incoming messages
        ws.on("message", (data) => {
          Effect.runSync(
            Effect.gen(function*() {
              try {
                const message = JSON.parse(data.toString()) as ClientMessage

                switch (message[0]) {
                  case "EVENT": {
                    const event = message[1]

                    // TODO: Add event validation
                    // For now, skip validation to avoid circular dependency

                    // Store event
                    yield* eventStorage.store(event)

                    // Send OK
                    yield* Queue.offer(
                      outgoingQueue,
                      ["OK", event.id, true, ""]
                    )

                    // Broadcast to matching subscriptions
                    const allClients = yield* Ref.get(clients)
                    yield* Effect.forEach(HashMap.values(allClients), (otherClient) =>
                      Effect.gen(function*() {
                        const subs = yield* Ref.get(otherClient.subscriptions)
                        yield* Effect.forEach(HashMap.entries(subs), ([subId, filters]) =>
                          Effect.gen(function*() {
                            const matches = yield* Effect.exists(
                              filters,
                              (filter) => filterMatcher.matches(event, filter)
                            )
                            if (matches) {
                              yield* Queue.offer(
                                otherClient.outgoingQueue,
                                ["EVENT", subId, event]
                              )
                            }
                          }))
                      }))
                    break
                  }

                  case "REQ": {
                    if (message.length < 2) return
                    const subId = message[1] as SubscriptionId
                    const filters = message.slice(2) as Array<Filter>

                    // Check subscription limit
                    const currentSubs = yield* Ref.get(subscriptions)
                    if (HashMap.size(currentSubs) >= defaultRelayConfig.limitation.max_subscriptions) {
                      yield* Queue.offer(
                        outgoingQueue,
                        ["NOTICE", `error: too many subscriptions`]
                      )
                      return
                    }

                    // Store subscription
                    yield* Ref.update(subscriptions, HashMap.set(subId, filters))

                    // Query stored events
                    const events = yield* eventStorage.query(filters)

                    // Send matching events
                    yield* Effect.forEach(events, (event) => Queue.offer(outgoingQueue, ["EVENT", subId, event]))

                    // Send EOSE
                    yield* Queue.offer(outgoingQueue, ["EOSE", subId])
                    break
                  }

                  case "CLOSE": {
                    const subId = message[1]
                    yield* Ref.update(subscriptions, HashMap.remove(subId))
                    break
                  }
                }
              } catch {
                yield* Queue.offer(
                  outgoingQueue,
                  ["NOTICE", `error: invalid message format`]
                )
              }
            })
          )
        })

        ws.on("close", () => {
          Effect.runSync(
            Ref.update(clients, HashMap.remove(clientId))
          )
        })

        ws.on("error", (error) => {
          console.error(`WebSocket error for ${clientId}:`, error)
          ws.close()
        })

        // Keep connection alive
        yield* Effect.never
      })

    const getInfo = () => defaultRelayConfig

    return {
      config: defaultRelayConfig,
      handleWebSocket,
      getInfo
    }
  })
)

/**
 * Create HTTP routes for the Nostr relay
 */
export const makeNostrRelayRoutes = HttpRouter.empty.pipe(
  HttpRouter.get(
    "/",
    Effect.gen(function*() {
      const relay = yield* NostrRelayService
      const info = relay.getInfo()

      // For now, always return HTML
      // TODO: Add proper header parsing for NIP-11 JSON response

      // Return HTML info page
      return HttpServerResponse.html(`
        <!DOCTYPE html>
        <html>
        <head>
          <title>${info.name}</title>
          <style>
            body { font-family: sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
            h1 { color: #333; }
            .info { background: #f5f5f5; padding: 20px; border-radius: 8px; }
            .nips { display: flex; gap: 10px; flex-wrap: wrap; }
            .nip { background: #007bff; color: white; padding: 5px 10px; border-radius: 4px; }
          </style>
        </head>
        <body>
          <h1>${info.name}</h1>
          <p>${info.description}</p>
          <div class="info">
            <h2>Relay Information</h2>
            <p><strong>Software:</strong> <a href="${info.software}">${info.software}</a></p>
            <p><strong>Version:</strong> ${info.version}</p>
            <p><strong>Supported NIPs:</strong></p>
            <div class="nips">
              ${info.supported_nips.map((nip) => `<span class="nip">NIP-${nip}</span>`).join("")}
            </div>
            <h3>Limitations</h3>
            <ul>
              <li>Max subscriptions: ${info.limitation.max_subscriptions}</li>
              <li>Max filters: ${info.limitation.max_filters}</li>
              <li>Max message length: ${info.limitation.max_message_length} bytes</li>
              <li>Auth required: ${info.limitation.auth_required ? "Yes" : "No"}</li>
            </ul>
          </div>
          <p style="margin-top: 40px; color: #666;">
            Connect with a Nostr client using: <code>ws://localhost:8080</code>
          </p>
        </body>
        </html>
      `)
    })
  )
)
