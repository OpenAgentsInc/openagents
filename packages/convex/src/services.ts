/**
 * Effect-based Convex service implementations for OpenAgents
 * @since 1.0.0
 */

import * as Effect from "effect/Effect"
import * as Context from "effect/Context"
import * as Layer from "effect/Layer"
import * as Config from "effect/Config"
import * as Schema from "effect/Schema"
import * as Data from "effect/Data"
import * as Schedule from "effect/Schedule"
import { pipe } from "effect/Function"
import { ConvexHttpClient } from "convex/browser"
import type { FunctionReference, OptionalRestArgs } from "convex/server"

/**
 * Tagged error types for Convex operations
 */
export class ConvexConnectionError extends Data.TaggedError("ConvexConnectionError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

export class ConvexQueryError extends Data.TaggedError("ConvexQueryError")<{
  readonly functionName: string
  readonly message: string
  readonly cause?: unknown
}> {}

export class ConvexMutationError extends Data.TaggedError("ConvexMutationError")<{
  readonly functionName: string
  readonly message: string
  readonly cause?: unknown
}> {}

export class ConvexActionError extends Data.TaggedError("ConvexActionError")<{
  readonly functionName: string
  readonly message: string
  readonly cause?: unknown
}> {}

/**
 * Convex service interface with Effect-based operations
 */
export interface ConvexService {
  /**
   * Execute a Convex query
   */
  readonly query: <T>(
    functionReference: FunctionReference<"query", "public", any, T>,
    args?: OptionalRestArgs<FunctionReference<"query", "public", any, T>>
  ) => Effect.Effect<T, ConvexQueryError | ConvexConnectionError>

  /**
   * Execute a Convex mutation
   */
  readonly mutation: <T>(
    functionReference: FunctionReference<"mutation", "public", any, T>,
    args?: OptionalRestArgs<FunctionReference<"mutation", "public", any, T>>
  ) => Effect.Effect<T, ConvexMutationError | ConvexConnectionError>

  /**
   * Execute a Convex action
   */
  readonly action: <T>(
    functionReference: FunctionReference<"action", "public", any, T>,
    args?: OptionalRestArgs<FunctionReference<"action", "public", any, T>>
  ) => Effect.Effect<T, ConvexActionError | ConvexConnectionError>

  /**
   * Subscribe to real-time updates (returns cleanup function)
   */
  readonly subscribe: <T>(
    functionReference: FunctionReference<"query", "public", any, T>,
    args: OptionalRestArgs<FunctionReference<"query", "public", any, T>>,
    callback: (value: T) => void
  ) => Effect.Effect<() => void, ConvexConnectionError>
}

export const ConvexService = Context.GenericTag<ConvexService>("@openagentsinc/convex/ConvexService")

/**
 * Configuration for Convex service
 */
export const ConvexConfig = Schema.Struct({
  convexUrl: Schema.String,
  authToken: Schema.optional(Schema.String)
})

export type ConvexConfig = Schema.Schema.Type<typeof ConvexConfig>

/**
 * Live implementation of ConvexService
 */
const make = Effect.gen(function* () {
  const convexUrl = yield* Config.string("CONVEX_URL")
  const authToken = yield* Config.option(Config.string("CONVEX_AUTH_TOKEN"))

  const client = new ConvexHttpClient(convexUrl)

  // Set auth token if provided
  if (authToken._tag === "Some") {
    client.setAuth(authToken.value)
  }

  return ConvexService.of({
    query: (functionReference, args) =>
      Effect.tryPromise({
        try: () => client.query(functionReference, args),
        catch: (error) => new ConvexQueryError({
          functionName: "query",
          message: error instanceof Error ? error.message : "Unknown query error",
          cause: error
        })
      }),

    mutation: (functionReference, args) =>
      Effect.tryPromise({
        try: () => client.mutation(functionReference, args),
        catch: (error) => new ConvexMutationError({
          functionName: "mutation",
          message: error instanceof Error ? error.message : "Unknown mutation error",
          cause: error
        })
      }),

    action: (functionReference, args) =>
      Effect.tryPromise({
        try: () => client.action(functionReference, args),
        catch: (error) => new ConvexActionError({
          functionName: "action",
          message: error instanceof Error ? error.message : "Unknown action error",
          cause: error
        })
      }),

    subscribe: (functionReference, args, callback) =>
      Effect.sync(() => {
        // Note: Real-time subscriptions require ConvexClient (WebSocket), not ConvexHttpClient
        // For now, return a no-op function. This should be updated when proper client is available
        return () => {}
      })
  })
})

/**
 * Layer for ConvexService
 */
export const ConvexServiceLive = Layer.effect(ConvexService, make)

/**
 * Utility for running Effect programs with ConvexService
 */
export const withConvex = <A>(
  effect: Effect.Effect<A, any, ConvexService>
): Effect.Effect<A, any, never> =>
  pipe(effect, Effect.provide(ConvexServiceLive))

/**
 * Schema mapping utilities for converting between Effect schemas and Convex data
 */
export namespace SchemaMapping {
  /**
   * Convert Effect Schema validation to Convex-compatible data
   */
  export const encodeForConvex = <A, I>(schema: Schema.Schema<A, I>) =>
    (value: A) =>
      Schema.encode(schema)(value)

  /**
   * Decode Convex data using Effect Schema
   */
  export const decodeFromConvex = <A, I>(schema: Schema.Schema<A, I>) =>
    (value: I) =>
      Schema.decode(schema)(value)

  /**
   * Create a bidirectional mapping between Effect Schema and Convex
   */
  export const createMapping = <A, I>(schema: Schema.Schema<A, I>) => ({
    encode: encodeForConvex(schema),
    decode: decodeFromConvex(schema)
  })
}

/**
 * Retry policies for Convex operations
 */
export namespace RetryPolicies {

  /**
   * Default retry policy for transient errors
   */
  export const defaultRetry = Schedule.exponential("100 millis").pipe(
    Schedule.intersect(Schedule.recurs(3))
  )

  /**
   * Aggressive retry for critical operations
   */
  export const aggressiveRetry = Schedule.exponential("50 millis").pipe(
    Schedule.intersect(Schedule.recurs(5))
  )

  /**
   * Conservative retry for expensive operations
   */
  export const conservativeRetry = Schedule.exponential("500 millis").pipe(
    Schedule.intersect(Schedule.recurs(2))
  )
}

/**
 * Helper functions for common Convex operations with Effect
 */
export namespace ConvexHelpers {
  /**
   * Query with automatic retry and error handling
   */
  export const queryWithRetry = <T>(
    functionReference: FunctionReference<"query", "public", any, T>,
    args?: OptionalRestArgs<FunctionReference<"query", "public", any, T>>
  ) =>
    Effect.gen(function* () {
      const convex = yield* ConvexService
      return yield* pipe(
        convex.query(functionReference, args),
        Effect.retry(RetryPolicies.defaultRetry),
        Effect.catchTags({
          ConvexConnectionError: (error: ConvexConnectionError) =>
            Effect.logError(`Convex connection failed: ${error.message}`).pipe(
              Effect.zipRight(Effect.fail(error))
            )
        })
      )
    })

  /**
   * Mutation with automatic retry and error handling
   */
  export const mutationWithRetry = <T>(
    functionReference: FunctionReference<"mutation", "public", any, T>,
    args?: OptionalRestArgs<FunctionReference<"mutation", "public", any, T>>
  ) =>
    Effect.gen(function* () {
      const convex = yield* ConvexService
      return yield* pipe(
        convex.mutation(functionReference, args),
        Effect.retry(RetryPolicies.conservativeRetry),
        Effect.catchTags({
          ConvexConnectionError: (error: ConvexConnectionError) =>
            Effect.logError(`Convex connection failed: ${error.message}`).pipe(
              Effect.zipRight(Effect.fail(error))
            )
        })
      )
    })
}