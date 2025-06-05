/**
 * Automatic reconnection service for Nostr relays
 * @module
 */

import type { Scope } from "effect"
import { Context, Duration, Effect, Layer, pipe, Ref, Schedule } from "effect"
import type { ConnectionError } from "../core/Errors.js"
import { type RelayConnection, RelayService } from "./RelayService.js"

export interface ReconnectConfig {
  readonly initialDelay: Duration.Duration
  readonly maxDelay: Duration.Duration
  readonly maxAttempts: number
  readonly factor: number
}

export const defaultReconnectConfig: ReconnectConfig = {
  initialDelay: Duration.seconds(1),
  maxDelay: Duration.minutes(5),
  maxAttempts: 10,
  factor: 2
}

export interface ReconnectingRelay {
  readonly url: string
  readonly connection: Ref.Ref<RelayConnection | null>
  readonly isConnected: Ref.Ref<boolean>
  readonly reconnectAttempts: Ref.Ref<number>
  readonly stop: () => Effect.Effect<void, never, never>
}

export interface RelayReconnectService {
  readonly createReconnectingRelay: (
    url: string,
    config?: Partial<ReconnectConfig>
  ) => Effect.Effect<ReconnectingRelay, ConnectionError, Scope.Scope>
}

export const RelayReconnectService = Context.GenericTag<RelayReconnectService>("nostr/RelayReconnectService")

/**
 * Create a reconnection schedule based on config
 */
const makeReconnectSchedule = (config: ReconnectConfig) =>
  pipe(
    Schedule.exponential(config.initialDelay, config.factor),
    Schedule.compose(Schedule.recurs(config.maxAttempts))
  )

/**
 * Live implementation of RelayReconnectService
 */
export const RelayReconnectServiceLive = Layer.effect(
  RelayReconnectService,
  Effect.gen(function*() {
    const relayService = yield* RelayService

    const createReconnectingRelay = (
      url: string,
      config?: Partial<ReconnectConfig>
    ): Effect.Effect<ReconnectingRelay, ConnectionError, Scope.Scope> =>
      Effect.gen(function*() {
        const finalConfig: ReconnectConfig = {
          ...defaultReconnectConfig,
          ...config
        }

        // State
        const connection = yield* Ref.make<RelayConnection | null>(null)
        const isConnected = yield* Ref.make(false)
        const reconnectAttempts = yield* Ref.make(0)
        const shouldStop = yield* Ref.make(false)

        // Connection attempt
        const attemptConnection = Effect.gen(function*() {
          const conn = yield* relayService.connect(url)
          yield* Ref.set(connection, conn)
          yield* Ref.set(isConnected, true)
          yield* Ref.set(reconnectAttempts, 0)

          // Connection established
          return conn
        })

        // Reconnection loop
        const reconnectionLoop = Effect.gen(function*() {
          const schedule = makeReconnectSchedule(finalConfig)

          yield* attemptConnection.pipe(
            Effect.retry(
              Schedule.recurWhile<unknown>(() => {
                return Effect.runSync(
                  Effect.gen(function*() {
                    const stop = yield* Ref.get(shouldStop)
                    const connected = yield* Ref.get(isConnected)
                    return !stop && !connected
                  })
                )
              }).pipe(
                Schedule.intersect(schedule),
                Schedule.tapOutput(() => Ref.update(reconnectAttempts, (n) => n + 1))
              )
            ),
            Effect.catchAll(() => Effect.succeed(undefined))
          )
        })

        // Start reconnection loop in background
        yield* Effect.fork(
          Effect.forever(
            Effect.gen(function*() {
              const connected = yield* Ref.get(isConnected)
              if (!connected) {
                yield* reconnectionLoop
              }
              yield* Effect.sleep(Duration.seconds(1))
            }).pipe(Effect.catchAll(() => Effect.succeed(undefined)))
          )
        )

        // Cleanup on scope close
        yield* Effect.addFinalizer(() => Effect.succeed(undefined))

        // Initial connection attempt
        yield* attemptConnection.pipe(
          Effect.catchAll(() => Effect.succeed(undefined))
        )

        const stop = () =>
          Effect.all([
            Ref.set(shouldStop, true),
            Effect.gen(function*() {
              const conn = yield* Ref.get(connection)
              if (conn !== null) {
                yield* Effect.succeed(undefined)
              }
            }),
            Ref.set(connection, null),
            Ref.set(isConnected, false)
          ]).pipe(Effect.asVoid)

        return {
          url,
          connection,
          isConnected,
          reconnectAttempts,
          stop
        }
      })

    return { createReconnectingRelay }
  })
)
