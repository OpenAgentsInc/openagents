/**
 * @since 1.0.0
 */
import type { HttpClient } from "@effect/platform/HttpClient"
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import { dual } from "effect/Function"

/**
 * @since 1.0.0
 * @category Context
 */
export class CloudflareConfig extends Context.Tag("@openagentsinc/ai-cloudflare/CloudflareConfig")<
  CloudflareConfig,
  CloudflareConfig.Service
>() {
  /**
   * @since 1.0.0
   */
  static readonly getOrUndefined: Effect.Effect<typeof CloudflareConfig.Service | undefined> = Effect.map(
    Effect.context<never>(),
    (context) => context.unsafeMap.get(CloudflareConfig.key)
  )
}

/**
 * @since 1.0.0
 */
export declare namespace CloudflareConfig {
  /**
   * @since 1.0.0
   * @category Models
   */
  export interface Service {
    readonly transformClient?: (client: HttpClient) => HttpClient
    readonly accountId?: string
    readonly useOpenAIEndpoints?: boolean
    readonly maxNeuronsPerDay?: number
    readonly preferredRegion?: string
  }
}

/**
 * @since 1.0.0
 * @category Configuration
 */
export const withAccountId: {
  (accountId: string): <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>
  <A, E, R>(self: Effect.Effect<A, E, R>, accountId: string): Effect.Effect<A, E, R>
} = dual<
  (accountId: string) => <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>,
  <A, E, R>(self: Effect.Effect<A, E, R>, accountId: string) => Effect.Effect<A, E, R>
>(
  2,
  (self, accountId) =>
    Effect.flatMap(
      CloudflareConfig.getOrUndefined,
      (config) => Effect.provideService(self, CloudflareConfig, { ...config, accountId })
    )
)

/**
 * @since 1.0.0
 * @category Configuration
 */
export const withOpenAIEndpoints: {
  (useOpenAI: boolean): <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>
  <A, E, R>(self: Effect.Effect<A, E, R>, useOpenAI: boolean): Effect.Effect<A, E, R>
} = dual<
  (useOpenAI: boolean) => <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>,
  <A, E, R>(self: Effect.Effect<A, E, R>, useOpenAI: boolean) => Effect.Effect<A, E, R>
>(
  2,
  (self, useOpenAIEndpoints) =>
    Effect.flatMap(
      CloudflareConfig.getOrUndefined,
      (config) => Effect.provideService(self, CloudflareConfig, { ...config, useOpenAIEndpoints })
    )
)
