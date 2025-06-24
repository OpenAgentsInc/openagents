import { Context, Effect, Layer, Queue, Stream } from "effect"
import * as os from "node:os"
import WebSocket from "ws"

// Message types
export interface OverlordMessage {
  readonly type: "heartbeat" | "file_change" | "command_result" | "session_update"
  readonly machineId: string
  readonly timestamp: string
  readonly data: unknown
}

export interface ServerMessage {
  readonly type: "execute_command" | "sync_request" | "config_update"
  readonly commandId: string
  readonly timestamp: string
  readonly data: unknown
}

// WebSocket client service
export interface WebSocketClient {
  readonly connect: (url: string, auth: { userId: string; apiKey: string }) => Effect.Effect<void, Error>
  readonly disconnect: () => Effect.Effect<void>
  readonly send: (message: OverlordMessage) => Effect.Effect<void, Error>
  readonly receive: () => Stream.Stream<ServerMessage>
  readonly isConnected: () => Effect.Effect<boolean>
}

export const WebSocketClient = Context.GenericTag<WebSocketClient>("@openagentsinc/overlord/WebSocketClient")

// Implementation
export const WebSocketClientLive = Layer.effect(
  WebSocketClient,
  Effect.gen(function*() {
    let ws: WebSocket | null = null
    const messageQueue = yield* Queue.unbounded<ServerMessage>()
    let reconnectTimer: NodeJS.Timeout | null = null

    // Connect to WebSocket server
    const connect = (url: string, auth: { userId: string; apiKey: string }) =>
      Effect.gen(function*() {
        // Disconnect existing connection
        if (ws) {
          yield* disconnect()
        }

        yield* Effect.async<void, Error>((resume) => {
          ws = new WebSocket(url, {
            headers: {
              "Authorization": `Bearer ${auth.apiKey}`,
              "X-User-ID": auth.userId
            }
          })

          ws.on("open", () => {
            Effect.runSync(Effect.logInfo("WebSocket connected"))

            // Send initial handshake
            const handshake: OverlordMessage = {
              type: "heartbeat",
              machineId: getMachineId(),
              timestamp: new Date().toISOString(),
              data: {
                version: "0.0.0",
                platform: process.platform,
                hostname: os.hostname()
              }
            }

            ws!.send(JSON.stringify(handshake))
            resume(Effect.succeed(undefined))
          })

          ws.on("message", (data) => {
            try {
              const message = JSON.parse(data.toString()) as ServerMessage
              Effect.runSync(Queue.offer(messageQueue, message))
            } catch (error) {
              Effect.runSync(Effect.logError(`Failed to parse message: ${error}`))
            }
          })

          ws.on("error", (error) => {
            Effect.runSync(Effect.logError(`WebSocket error: ${error}`))
            resume(Effect.fail(new Error(`WebSocket error: ${error.message}`)))
          })

          ws.on("close", (code, reason) => {
            Effect.runSync(Effect.logInfo(`WebSocket closed: ${code} ${reason}`))

            // Attempt reconnect after 5 seconds
            if (!reconnectTimer) {
              reconnectTimer = setTimeout(() => {
                reconnectTimer = null
                Effect.runPromise(
                  connect(url, auth).pipe(
                    Effect.catchAll(() => Effect.void)
                  )
                )
              }, 5000)
            }
          })
        })
      })

    // Disconnect from WebSocket
    const disconnect = () =>
      Effect.gen(function*() {
        if (reconnectTimer) {
          clearTimeout(reconnectTimer)
          reconnectTimer = null
        }

        if (ws && ws.readyState === WebSocket.OPEN) {
          ws.close()
          ws = null
        }

        yield* Effect.logInfo("WebSocket disconnected")
      })

    // Send message
    const send = (message: OverlordMessage) =>
      Effect.gen(function*() {
        if (!ws || ws.readyState !== WebSocket.OPEN) {
          yield* Effect.fail(new Error("WebSocket not connected"))
          return
        }

        ws.send(JSON.stringify(message))
      })

    // Receive messages
    const receive = () => Stream.fromQueue(messageQueue)

    // Check connection status
    const isConnected = () => Effect.succeed(ws !== null && ws.readyState === WebSocket.OPEN)

    // Helper to generate machine ID
    const getMachineId = (): string => {
      // In production, this would be a persistent ID
      return `${os.hostname()}-${os.platform()}-${os.arch()}`
    }

    return {
      connect,
      disconnect,
      send,
      receive,
      isConnected
    }
  })
)
