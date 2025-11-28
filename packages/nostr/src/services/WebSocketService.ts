/**
 * WebSocket connection management service
 * @module
 */

import type { Scope } from "effect"
import { Context, Effect, Layer, Option, Queue, Ref, Stream } from "effect"
import { WebSocket } from "ws"
import { ConnectionError, MessageSendError } from "../core/Errors.js"

export interface WebSocketConnection {
  readonly url: string
  readonly ws: WebSocket
  readonly readyState: () => number
  readonly send: (message: string) => Effect.Effect<void, MessageSendError>
  readonly close: (code?: number, reason?: string) => Effect.Effect<void>
  readonly messages: Stream.Stream<string, ConnectionError>
}

/**
 * Service for WebSocket operations
 */
export class WebSocketService extends Context.Tag("nostr/WebSocketService")<
  WebSocketService,
  {
    /**
     * Connect to a WebSocket URL
     */
    readonly connect: (url: string) => Effect.Effect<WebSocketConnection, ConnectionError, Scope.Scope>

    /**
     * Check if a connection is open
     */
    readonly isOpen: (connection: WebSocketConnection) => boolean
  }
>() {}

/**
 * Live implementation of WebSocketService
 */
export const WebSocketServiceLive = Layer.succeed(
  WebSocketService,
  {
    connect: (url: string): Effect.Effect<WebSocketConnection, ConnectionError, Scope.Scope> =>
      Effect.gen(function*() {
        const ws = new WebSocket(url)
        const messageQueue = yield* Queue.unbounded<string>()
        const errorRef = yield* Ref.make<Option.Option<ConnectionError>>(Option.none())

        // Set up event handlers
        const setupHandlers = Effect.sync(() => {
          ws.on("error", (error) => {
            const connError = new ConnectionError({
              url,
              reason: error.message,
              code: (error as any).code
            })
            Ref.set(errorRef, Option.some(connError)).pipe(Effect.runSync)
            Queue.shutdown(messageQueue).pipe(Effect.runSync)
          })

          ws.on("close", (code, reason) => {
            const connError = new ConnectionError({
              url,
              reason: reason?.toString() || "Connection closed"
            })
            Ref.set(errorRef, Option.some(connError)).pipe(Effect.runSync)
            Queue.shutdown(messageQueue).pipe(Effect.runSync)
          })

          ws.on("message", (data) => {
            const message = data.toString()
            Queue.offer(messageQueue, message).pipe(Effect.runSync)
          })
        })

        // Wait for connection to open
        const waitForOpen = Effect.async<void, ConnectionError>((resume) => {
          if (ws.readyState === WebSocket.OPEN) {
            resume(Effect.succeed(undefined))
          } else {
            ws.once("open", () => resume(Effect.succeed(undefined)))
            ws.once("error", (error) =>
              resume(Effect.fail(
                new ConnectionError({
                  url,
                  reason: error.message,
                  code: (error as any).code
                })
              )))
          }
        })

        yield* setupHandlers
        yield* waitForOpen.pipe(
          Effect.timeoutFail({
            duration: "30 seconds",
            onTimeout: () =>
              new ConnectionError({
                url,
                reason: "Connection timeout"
              })
          })
        )

        // Add finalizer to close the connection
        yield* Effect.addFinalizer(() =>
          Effect.sync(() => {
            if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
              ws.close()
            }
          })
        )

        const connection: WebSocketConnection = {
          url,
          ws,
          readyState: () => ws.readyState,
          send: (message: string) =>
            Effect.gen(function*() {
              if (ws.readyState !== WebSocket.OPEN) {
                return yield* Effect.fail(
                  new MessageSendError({
                    url,
                    message,
                    reason: "WebSocket is not open"
                  })
                )
              }

              return yield* Effect.async<void, MessageSendError>((resume) => {
                ws.send(message, (error) => {
                  if (error) {
                    resume(Effect.fail(
                      new MessageSendError({
                        url,
                        message,
                        reason: error.message
                      })
                    ))
                  } else {
                    resume(Effect.succeed(undefined))
                  }
                })
              })
            }),
          close: (code?: number, reason?: string) =>
            Effect.sync(() => {
              ws.close(code, reason)
            }),
          messages: Stream.fromQueue(messageQueue).pipe(
            Stream.flatMap((message) =>
              Ref.get(errorRef).pipe(
                Effect.flatMap(Option.match({
                  onNone: () => Effect.succeed(message),
                  onSome: (error) => Effect.fail(error)
                })),
                Stream.fromEffect
              )
            )
          )
        }

        return connection
      }),

    isOpen: (connection: WebSocketConnection): boolean => connection.readyState() === WebSocket.OPEN
  }
)
