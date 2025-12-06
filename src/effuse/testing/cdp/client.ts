/**
 * CDP Client
 *
 * Effect-native WebSocket client for Chrome DevTools Protocol.
 * Uses Bun's native WebSocket and JSON-RPC correlation.
 */

import { Context, Deferred, Effect, Queue, Scope, Stream } from "effect"
import { CDPError } from "../errors.js"

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/** CDP JSON-RPC message */
export interface CDPMessage {
  id?: number
  method?: string
  params?: Record<string, unknown>
  result?: unknown
  error?: { code: number; message: string }
}

/** CDP event from Chrome */
export interface CDPEvent {
  method: string
  params: Record<string, unknown>
}

/** Pending request with deferred result */
interface PendingRequest {
  deferred: Deferred.Deferred<unknown, CDPError>
  method: string
}

// ─────────────────────────────────────────────────────────────────────────────
// CDPClient Service Interface
// ─────────────────────────────────────────────────────────────────────────────

export interface CDPClient {
  /** Send a CDP command and wait for response */
  readonly send: <T = unknown>(
    method: string,
    params?: Record<string, unknown>
  ) => Effect.Effect<T, CDPError>

  /** Subscribe to CDP events by method name */
  readonly on: (method: string) => Stream.Stream<CDPEvent, never>

  /** Subscribe to all CDP events */
  readonly events: Stream.Stream<CDPEvent, never>

  /** Close the connection */
  readonly close: Effect.Effect<void, never>

  /** Get the session ID (if any) */
  readonly sessionId: string | undefined
}

export class CDPClientTag extends Context.Tag("effuse/testing/CDPClient")<
  CDPClientTag,
  CDPClient
>() {}

// ─────────────────────────────────────────────────────────────────────────────
// Implementation
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Create a CDP client connected to a WebSocket endpoint.
 */
export const makeCDPClient = (
  wsEndpoint: string,
  sessionId?: string
): Effect.Effect<CDPClient, CDPError, Scope.Scope> =>
  Effect.gen(function* () {
    let nextId = 1
    const pending = new Map<number, PendingRequest>()

    // Create event queue for broadcasting events
    const eventQueue = yield* Queue.unbounded<CDPEvent>()

    // Create WebSocket connection
    const ws = yield* Effect.try({
      try: () => new WebSocket(wsEndpoint),
      catch: (e) =>
        new CDPError({
          reason: "connection_failed",
          message: `Failed to create WebSocket: ${e}`,
        }),
    })

    // Wait for connection to open
    yield* Effect.async<void, CDPError>((resume) => {
      const timeout = setTimeout(() => {
        resume(
          Effect.fail(
            new CDPError({
              reason: "connection_failed",
              message: "WebSocket connection timeout",
            })
          )
        )
      }, 10000)

      ws.onopen = () => {
        clearTimeout(timeout)
        resume(Effect.void)
      }

      ws.onerror = (e) => {
        clearTimeout(timeout)
        resume(
          Effect.fail(
            new CDPError({
              reason: "connection_failed",
              message: `WebSocket error: ${e}`,
            })
          )
        )
      }
    })

    // Set up message handler
    ws.onmessage = (event) => {
      try {
        const msg: CDPMessage = JSON.parse(event.data as string)

        // Response to a command
        if (msg.id !== undefined) {
          const req = pending.get(msg.id)
          if (req) {
            pending.delete(msg.id)
            if (msg.error) {
              Effect.runSync(
                Deferred.fail(
                  req.deferred,
                  new CDPError({
                    reason: "protocol_error",
                    message: `${req.method}: ${msg.error.message}`,
                    code: msg.error.code,
                  })
                )
              )
            } else {
              Effect.runSync(Deferred.succeed(req.deferred, msg.result))
            }
          }
        }

        // Event from Chrome
        if (msg.method) {
          const cdpEvent: CDPEvent = {
            method: msg.method,
            params: msg.params ?? {},
          }
          Effect.runSync(Queue.offer(eventQueue, cdpEvent))
        }
      } catch {
        // Ignore parse errors
      }
    }

    ws.onclose = () => {
      // Fail all pending requests
      for (const [, req] of pending) {
        Effect.runSync(
          Deferred.fail(
            req.deferred,
            new CDPError({
              reason: "page_closed",
              message: "WebSocket connection closed",
            })
          )
        )
      }
      pending.clear()
    }

    // Clean up on scope finalization
    yield* Effect.addFinalizer(() =>
      Effect.sync(() => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.close()
        }
      })
    )

    const client: CDPClient = {
      sessionId,

      send: <T = unknown>(
        method: string,
        params?: Record<string, unknown>
      ): Effect.Effect<T, CDPError> =>
        Effect.gen(function* () {
          const id = nextId++
          const deferred = yield* Deferred.make<unknown, CDPError>()

          pending.set(id, { deferred, method })

          const message: CDPMessage = { id, method }
          if (params) message.params = params
          if (sessionId) (message.params ??= {}).sessionId = sessionId

          ws.send(JSON.stringify(message))

          return (yield* Deferred.await(deferred)) as T
        }),

      on: (method: string): Stream.Stream<CDPEvent, never> =>
        Stream.fromQueue(eventQueue).pipe(
          Stream.filter((e) => e.method === method)
        ),

      events: Stream.fromQueue(eventQueue),

      close: Effect.sync(() => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.close()
        }
      }),
    }

    return client
  })

/**
 * Layer that provides CDPClient connected to a WebSocket endpoint.
 */
export const CDPClientLayer = (wsEndpoint: string, sessionId?: string) =>
  Effect.toLayer(makeCDPClient(wsEndpoint, sessionId), CDPClientTag)
