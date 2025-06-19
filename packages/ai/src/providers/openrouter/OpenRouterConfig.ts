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
export class OpenRouterConfig extends Context.Tag("@openagentsinc/ai-openrouter/OpenRouterConfig")<
  OpenRouterConfig,
  OpenRouterConfig.Service
>() {
  /**
   * @since 1.0.0
   */
  static readonly getOrUndefined: Effect.Effect<typeof OpenRouterConfig.Service | undefined> = Effect.map(
    Effect.context<never>(),
    (context) => context.unsafeMap.get(OpenRouterConfig.key)
  )
}

/**
 * @since 1.0.0
 */
export declare namespace OpenRouterConfig {
  /**
   * @since 1.0.0
   * @category Models
   */
  export interface Service {
    readonly transformClient?: (client: HttpClient) => HttpClient
    readonly providerRouting?: ProviderRouting
    readonly fallbackModels?: ReadonlyArray<string>
    readonly referer?: string
    readonly title?: string
  }

  /**
   * @since 1.0.0
   * @category Models
   */
  export interface ProviderRouting {
    readonly order?: ReadonlyArray<string>
    readonly allow_fallbacks?: boolean
    readonly require_parameters?: boolean
    readonly data_collection?: "allow" | "deny"
    readonly sort?: "price" | "throughput"
  }
}

/**
 * @since 1.0.0
 * @category Configuration
 */
export const withProviderRouting: {
  (routing: OpenRouterConfig.ProviderRouting): <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>
  <A, E, R>(self: Effect.Effect<A, E, R>, routing: OpenRouterConfig.ProviderRouting): Effect.Effect<A, E, R>
} = dual<
  (routing: OpenRouterConfig.ProviderRouting) => <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>,
  <A, E, R>(self: Effect.Effect<A, E, R>, routing: OpenRouterConfig.ProviderRouting) => Effect.Effect<A, E, R>
>(
  2,
  (self, providerRouting) =>
    Effect.flatMap(
      OpenRouterConfig.getOrUndefined,
      (config) => Effect.provideService(self, OpenRouterConfig, { ...config, providerRouting })
    )
)

/**
 * @since 1.0.0
 * @category Configuration
 */
export const withFallbackModels: {
  (models: ReadonlyArray<string>): <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>
  <A, E, R>(self: Effect.Effect<A, E, R>, models: ReadonlyArray<string>): Effect.Effect<A, E, R>
} = dual<
  (models: ReadonlyArray<string>) => <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>,
  <A, E, R>(self: Effect.Effect<A, E, R>, models: ReadonlyArray<string>) => Effect.Effect<A, E, R>
>(
  2,
  (self, fallbackModels) =>
    Effect.flatMap(
      OpenRouterConfig.getOrUndefined,
      (config) => Effect.provideService(self, OpenRouterConfig, { ...config, fallbackModels })
    )
)
