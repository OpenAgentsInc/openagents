/**
 * Browser WebSocket Service with Effect
 * Provides managed WebSocket connections with automatic reconnection
 */

import { Context, Data, Effect, Layer, Queue, Ref, Schedule, Stream } from "effect"
import type { Scope } from "effect/Scope"

// Error types
export class WebSocketError extends Data.TaggedError("WebSocketError")<{
  reason: "connection_failed" | "timeout" | "invalid_message" | "connection_lost"
  message: string
  retryable: boolean
  cause?: unknown
}> {}

// Connection state
export type ConnectionState = "connecting" | "connected" | "disconnected" | "reconnecting"

// WebSocket message types
export type WebSocketMessage = string | ArrayBuffer | Blob

// Connection interface
export interface WebSocketConnection {
  readonly send: (message: string) => Effect.Effect<void, WebSocketError>
  readonly messages: Stream.Stream<string, WebSocketError>
  readonly state: Effect.Effect<ConnectionState>
  readonly close: () => Effect.Effect<void>
}

// WebSocket Service
export class WebSocketService extends Context.Tag("sdk/WebSocketService")<
  WebSocketService,
  {
    readonly connect: (url: string) => Effect.Effect<WebSocketConnection, WebSocketError, Scope>
    readonly disconnect: () => Effect.Effect<void>
  }
>() {}

// Live implementation
export const WebSocketServiceLive = Layer.succeed(
  WebSocketService,
  {
    connect: (url: string) =>
      Effect.gen(function*() {
        // Create state refs
        const wsRef = yield* Ref.make<WebSocket | null>(null)
        const stateRef = yield* Ref.make<ConnectionState>("connecting")
        const messageQueue = yield* Queue.unbounded<string>()
        const shouldReconnect = yield* Ref.make(true)

        // Reconnection schedule: exponential backoff with max delay
        const reconnectSchedule = Schedule.exponential("100 millis").pipe(
          Schedule.union(Schedule.spaced("30 seconds"))
        )

        // Create WebSocket connection with retry
        const createConnection = Effect.gen(function*() {
          yield* Ref.set(stateRef, "connecting")

          const ws = yield* Effect.try({
            try: () => new WebSocket(url),
            catch: (error) =>
              new WebSocketError({
                reason: "connection_failed",
                message: `Failed to create WebSocket: ${String(error)}`,
                retryable: true,
                cause: error
              })
          })

          // Set up event handlers
          ws.onopen = () => {
            Effect.runSync(Ref.set(stateRef, "connected"))
            console.log(`WebSocket connected to ${url}`)
          }

          ws.onmessage = (event) => {
            if (typeof event.data === "string") {
              Effect.runSync(Queue.offer(messageQueue, event.data))
            }
          }

          ws.onerror = (event) => {
            console.error("WebSocket error:", event)
          }

          ws.onclose = (event) => {
            Effect.runSync(Effect.gen(function*() {
              yield* Ref.set(stateRef, "disconnected")
              yield* Ref.set(wsRef, null)

              // Check if we should reconnect
              const reconnect = yield* Ref.get(shouldReconnect)
              if (reconnect && !event.wasClean) {
                yield* Ref.set(stateRef, "reconnecting")
              }
            }))
          }

          // Wait for connection to open
          yield* Effect.async<void, WebSocketError>((resume) => {
            if (ws.readyState === WebSocket.OPEN) {
              resume(Effect.succeed(undefined))
            } else {
              const openHandler = () => {
                ws.removeEventListener("open", openHandler)
                ws.removeEventListener("error", errorHandler)
                resume(Effect.succeed(undefined))
              }
              const errorHandler = () => {
                ws.removeEventListener("open", openHandler)
                ws.removeEventListener("error", errorHandler)
                resume(
                  Effect.fail(
                    new WebSocketError({
                      reason: "connection_failed",
                      message: "WebSocket failed to open",
                      retryable: true
                    })
                  )
                )
              }
              ws.addEventListener("open", openHandler)
              ws.addEventListener("error", errorHandler)
            }
          }).pipe(
            Effect.timeoutFail({
              duration: "10 seconds",
              onTimeout: () =>
                new WebSocketError({
                  reason: "timeout",
                  message: "Connection timeout",
                  retryable: true
                })
            })
          )

          yield* Ref.set(wsRef, ws)
          return ws
        })

        // Start connection with retry
        yield* createConnection.pipe(
          Effect.retry(reconnectSchedule),
          Effect.forkScoped
        )

        // Monitor connection and reconnect if needed
        yield* Effect.gen(function*() {
          while (true) {
            const state = yield* Ref.get(stateRef)
            const reconnect = yield* Ref.get(shouldReconnect)

            if (state === "disconnected" && reconnect) {
              yield* Ref.set(stateRef, "reconnecting")
              yield* createConnection.pipe(
                Effect.retry(reconnectSchedule),
                Effect.catchAll((error) => {
                  console.error("Failed to reconnect:", error)
                  return Effect.succeed(undefined)
                })
              )
            }

            yield* Effect.sleep("1 second")
          }
        }).pipe(
          Effect.forkScoped,
          Effect.interruptible
        )

        // Add cleanup
        yield* Effect.addFinalizer(() =>
          Effect.gen(function*() {
            yield* Ref.set(shouldReconnect, false)
            const ws = yield* Ref.get(wsRef)
            if (ws && ws.readyState === WebSocket.OPEN) {
              ws.close(1000, "Normal closure")
            }
            yield* Queue.shutdown(messageQueue)
          })
        )

        const connection: WebSocketConnection = {
          send: (message: string) =>
            Effect.gen(function*() {
              const ws = yield* Ref.get(wsRef)
              if (!ws || ws.readyState !== WebSocket.OPEN) {
                return yield* Effect.fail(
                  new WebSocketError({
                    reason: "connection_lost",
                    message: "WebSocket is not connected",
                    retryable: true
                  })
                )
              }

              yield* Effect.try({
                try: () => ws.send(message),
                catch: (error) =>
                  new WebSocketError({
                    reason: "invalid_message",
                    message: `Failed to send message: ${String(error)}`,
                    retryable: false,
                    cause: error
                  })
              })
            }),

          messages: Stream.fromQueue(messageQueue).pipe(
            Stream.catchAll((error) =>
              Stream.fail(
                new WebSocketError({
                  reason: "invalid_message",
                  message: String(error),
                  retryable: false
                })
              )
            )
          ),

          state: Ref.get(stateRef),

          close: () =>
            Effect.gen(function*() {
              yield* Ref.set(shouldReconnect, false)
              const ws = yield* Ref.get(wsRef)
              if (ws && ws.readyState === WebSocket.OPEN) {
                ws.close(1000, "Normal closure")
              }
            })
        }

        return connection
      }),

    disconnect: () => Effect.succeed(undefined)
  }
)
