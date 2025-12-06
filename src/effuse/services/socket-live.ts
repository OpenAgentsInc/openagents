/**
 * Effuse Socket Service - Live Implementation
 *
 * Wraps the existing SocketClient with Effect semantics.
 */

import { Effect, Layer, Stream } from "effect"
import {
  SocketServiceTag,
  SocketError,
  type SocketService,
  type StartTBRunOptions,
} from "./socket.js"
import { SocketClient, type SocketClientOptions } from "../../mainview/socket-client.js"
import type { HudMessage } from "../../hud/protocol.js"

/**
 * Wrap a Promise-based method in an Effect with SocketError.
 */
const wrapRequest = <T>(
  fn: () => Promise<T>,
  errorReason: SocketError["reason"] = "request_failed"
): Effect.Effect<T, SocketError> =>
  Effect.tryPromise({
    try: fn,
    catch: (error) =>
      new SocketError(
        errorReason,
        error instanceof Error ? error.message : String(error)
      ),
  })

/**
 * Create a SocketService from a SocketClient instance.
 */
const makeSocketService = (client: SocketClient): SocketService => {
  // We'll create the message stream lazily
  let messageStream: Stream.Stream<HudMessage, never> | null = null

  return {
    connect: () =>
      Effect.tryPromise({
        try: () => client.connect(),
        catch: (error) =>
          new SocketError(
            "connection_failed",
            error instanceof Error ? error.message : String(error)
          ),
      }),

    disconnect: () => Effect.sync(() => client.disconnect()),

    isConnected: () => Effect.sync(() => client.isConnected()),

    get messages() {
      if (!messageStream) {
        // Create a stream from the client's message handler
        messageStream = Stream.async<HudMessage>((emit) => {
          const unsubscribe = client.onMessage((message) => {
            // emit.single returns a Promise - must handle it to avoid unhandled rejections
            void emit.single(message).catch(() => {
              // Stream might be closed, queue might be full, etc.
              // Silently ignore - this is fire-and-forget event emission
            })
          })
          // Return cleanup function
          return Effect.sync(unsubscribe)
        })
      }
      return messageStream
    },

    // TB Operations
    loadTBSuite: (suitePath) => wrapRequest(() => client.loadTBSuite(suitePath)),

    startTBRun: (options: StartTBRunOptions) =>
      wrapRequest(() => client.startTBRun(options)),

    stopTBRun: () => wrapRequest(() => client.stopTBRun()),

    loadRecentTBRuns: (count) => wrapRequest(() => client.loadRecentTBRuns(count)),

    loadTBRunDetails: (runId) => wrapRequest(() => client.loadTBRunDetails(runId)),

    // Task Operations
    loadReadyTasks: (limit) => wrapRequest(() => client.loadReadyTasks(limit)),

    assignTaskToMC: (taskId, options) =>
      wrapRequest(() => client.assignTaskToMC(taskId, options)),

    // Trajectory Operations
    loadUnifiedTrajectories: (limit) =>
      wrapRequest(() => client.loadUnifiedTrajectories(limit)),
  }
}

/**
 * Create a SocketService layer with a new client instance.
 */
export const SocketServiceLive = (options?: SocketClientOptions) =>
  Layer.succeed(SocketServiceTag, makeSocketService(new SocketClient(options)))

/**
 * Create a SocketService layer from an existing client.
 */
export const SocketServiceFromClient = (client: SocketClient) =>
  Layer.succeed(SocketServiceTag, makeSocketService(client))

/**
 * Default SocketService layer using default client options.
 */
export const SocketServiceDefault = SocketServiceLive()
