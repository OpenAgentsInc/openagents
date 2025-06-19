/**
 * @since 1.0.0
 */
import * as Sse from "@effect/experimental/Sse"
import * as HttpBody from "@effect/platform/HttpBody"
import * as HttpClient from "@effect/platform/HttpClient"
import type * as HttpClientError from "@effect/platform/HttpClientError"
import * as HttpClientRequest from "@effect/platform/HttpClientRequest"
import * as Config from "effect/Config"
import type { ConfigError } from "effect/ConfigError"
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import { identity } from "effect/Function"
import * as Layer from "effect/Layer"
import * as Option from "effect/Option"
import * as Predicate from "effect/Predicate"
import type * as Redacted from "effect/Redacted"
import * as Stream from "effect/Stream"
// import type { ToolCallId } from "../../core/AiInput.js" // Unused for now
import * as AiResponse from "../../core/AiResponse.js"
import { CloudflareConfig } from "./CloudflareConfig.js"

const constDisableValidation = { disableValidation: true } as const

/**
 * @since 1.0.0
 * @category Models
 */
export type StreamCompletionRequest = {
  readonly model: string
  readonly messages: ReadonlyArray<{
    readonly role: string
    readonly content: string | ReadonlyArray<unknown>
    readonly name?: string | undefined
    readonly tool_calls?: ReadonlyArray<unknown> | undefined
    readonly tool_call_id?: string | undefined
  }>
  readonly temperature?: number | undefined
  readonly max_tokens?: number | undefined
  readonly top_p?: number | undefined
  readonly frequency_penalty?: number | undefined
  readonly presence_penalty?: number | undefined
  readonly stop?: string | ReadonlyArray<string> | undefined
  readonly stream?: boolean | undefined
}

/**
 * @since 1.0.0
 * @category Models
 */
export type SimpleCompletionRequest = {
  readonly model: string
  readonly prompt: string
  readonly temperature?: number | undefined
  readonly max_tokens?: number | undefined
  readonly stream?: boolean | undefined
}

/**
 * @since 1.0.0
 * @category Context
 */
export class CloudflareClient extends Context.Tag("@openagentsinc/ai-cloudflare/CloudflareClient")<
  CloudflareClient,
  CloudflareClient.Service
>() {}

/**
 * @since 1.0.0
 */
export declare namespace CloudflareClient {
  /**
   * @since 1.0.0
   * @category Models
   */
  export interface Service {
    readonly streamRequest: <A>(
      request: HttpClientRequest.HttpClientRequest
    ) => Stream.Stream<A, HttpClientError.HttpClientError, never>
    readonly stream: (
      request: StreamCompletionRequest
    ) => Stream.Stream<AiResponse.AiResponse, HttpClientError.HttpClientError, never>
  }
}

/**
 * @since 1.0.0
 * @category Constructors
 */
export const make = (options: {
  /**
   * The API key to use to communicate with Cloudflare Workers AI API.
   */
  readonly apiKey: Redacted.Redacted
  /**
   * The account ID for the Cloudflare account.
   */
  readonly accountId: string
  /**
   * The URL to use to communicate with Cloudflare Workers AI API.
   */
  readonly apiUrl?: string | undefined
  /**
   * Whether to use OpenAI-compatible endpoints.
   */
  readonly useOpenAIEndpoints?: boolean | undefined
  /**
   * A method which can be used to transform the underlying `HttpClient`.
   */
  readonly transformClient?: ((client: HttpClient.HttpClient) => HttpClient.HttpClient) | undefined
}): Effect.Effect<CloudflareClient.Service, never, HttpClient.HttpClient | CloudflareConfig> =>
  Effect.gen(function*() {
    const config = yield* Effect.serviceOption(CloudflareConfig)
    const mergedConfig = Option.match(config, {
      onNone: () => ({}),
      onSome: (c) => c
    })

    const accountId = options.accountId || ("accountId" in mergedConfig ? mergedConfig.accountId : undefined)
    const useOpenAIEndpoints = options.useOpenAIEndpoints ??
      ("useOpenAIEndpoints" in mergedConfig ? mergedConfig.useOpenAIEndpoints : false)

    if (!accountId) {
      throw new Error("Account ID is required for Cloudflare Workers AI")
    }

    const baseUrl = options.apiUrl ?? "https://api.cloudflare.com/client/v4"
    const endpoint = useOpenAIEndpoints
      ? `${baseUrl}/accounts/${accountId}/ai/v1`
      : `${baseUrl}/accounts/${accountId}/ai/run`

    const httpClient = (yield* HttpClient.HttpClient).pipe(
      HttpClient.mapRequest((request) => {
        return request.pipe(
          HttpClientRequest.prependUrl(endpoint),
          HttpClientRequest.bearerToken(options.apiKey),
          HttpClientRequest.acceptJson
        )
      }),
      options.transformClient ?
        options.transformClient :
        ("transformClient" in mergedConfig && mergedConfig.transformClient ? mergedConfig.transformClient : identity)
    )
    const httpClientOk = HttpClient.filterStatusOk(httpClient)

    const streamRequest = <A = unknown>(request: HttpClientRequest.HttpClientRequest) =>
      httpClientOk.execute(request).pipe(
        Effect.map((r) => r.stream),
        Stream.unwrapScoped,
        Stream.decodeText(),
        Stream.pipeThroughChannel(Sse.makeChannel()),
        Stream.takeWhile((event: Sse.Event) => event.data !== "[DONE]"),
        Stream.map((event: Sse.Event) => JSON.parse(event.data) as A)
      )

    const stream = (request: StreamCompletionRequest) =>
      Stream.suspend(() => {
        let isFirstChunk = true
        let finishReason: AiResponse.FinishReason = "unknown"
        let usage: AiResponse.Usage = {
          inputTokens: 0,
          outputTokens: 0,
          totalTokens: 0,
          reasoningTokens: 0,
          cacheReadInputTokens: 0,
          cacheWriteInputTokens: 0
        }
        const metadata: Record<string, unknown> = {}

        // Use OpenAI-compatible endpoint if configured
        if (useOpenAIEndpoints) {
          return streamRequest<RawCompletionChunk>(HttpClientRequest.post("/chat/completions", {
            body: HttpBody.unsafeJson({
              ...request,
              stream: true
            })
          })).pipe(
            Stream.filterMap((chunk) => {
              const parts: Array<AiResponse.Part> = []

              // Add response metadata immediately once available
              if (isFirstChunk) {
                isFirstChunk = false
                parts.push(
                  new AiResponse.MetadataPart({
                    id: chunk.id,
                    model: chunk.model,
                    timestamp: new Date(chunk.created * 1000)
                  }, constDisableValidation)
                )
              }

              // Track usage information
              if (Predicate.isNotNullable(chunk.usage)) {
                usage = {
                  inputTokens: chunk.usage.prompt_tokens,
                  outputTokens: chunk.usage.completion_tokens,
                  totalTokens: chunk.usage.total_tokens,
                  reasoningTokens: 0,
                  cacheReadInputTokens: 0,
                  cacheWriteInputTokens: 0
                }
              }

              for (let i = 0; i < chunk.choices.length; i++) {
                const choice = chunk.choices[i]

                // Track the finish reason for the response
                if (Predicate.isNotNullable(choice.finish_reason)) {
                  finishReason = resolveFinishReason(choice.finish_reason)
                  parts.push(
                    new AiResponse.FinishPart({
                      usage,
                      reason: finishReason,
                      providerMetadata: { "cloudflare": metadata }
                    }, constDisableValidation)
                  )
                }

                // Handle text deltas
                if (Predicate.isNotNullable(choice.delta.content)) {
                  parts.push(
                    new AiResponse.TextPart({
                      text: choice.delta.content
                    }, constDisableValidation)
                  )
                }
              }

              return parts.length === 0
                ? Option.none()
                : Option.some(AiResponse.AiResponse.make({ parts }, constDisableValidation))
            })
          )
        } else {
          // Use native Cloudflare endpoint with simple prompt format
          const prompt = request.messages.map((msg) => {
            if (msg.role === "system") return `System: ${msg.content}`
            if (msg.role === "user") return `User: ${msg.content}`
            return `Assistant: ${msg.content}`
          }).join("\n")

          return streamRequest<CloudflareResponse>(HttpClientRequest.post(`/${request.model}`, {
            body: HttpBody.unsafeJson({
              prompt,
              stream: request.stream ?? false,
              max_tokens: request.max_tokens,
              temperature: request.temperature
            })
          })).pipe(
            Stream.filterMap((response) => {
              const parts: Array<AiResponse.Part> = []

              // Add response metadata immediately once available
              if (isFirstChunk) {
                isFirstChunk = false
                parts.push(
                  new AiResponse.MetadataPart({
                    id: "cloudflare-" + Math.random().toString(36).substring(2),
                    model: request.model,
                    timestamp: new Date()
                  }, constDisableValidation)
                )
              }

              if (response.success && response.result?.response) {
                parts.push(
                  new AiResponse.TextPart({
                    text: response.result.response
                  }, constDisableValidation)
                )

                // Add finish part for complete responses
                if (!request.stream) {
                  parts.push(
                    new AiResponse.FinishPart({
                      usage,
                      reason: "stop",
                      providerMetadata: { "cloudflare": metadata }
                    }, constDisableValidation)
                  )
                }
              }

              return parts.length === 0
                ? Option.none()
                : Option.some(AiResponse.AiResponse.make({ parts }, constDisableValidation))
            })
          )
        }
      })

    return { streamRequest, stream }
  })

/**
 * @since 1.0.0
 * @category Layers
 */
export const layer = (options: {
  readonly apiKey: Redacted.Redacted
  readonly accountId: string
  readonly apiUrl?: string | undefined
  readonly useOpenAIEndpoints?: boolean | undefined
  readonly transformClient?: (client: HttpClient.HttpClient) => HttpClient.HttpClient
}): Layer.Layer<CloudflareClient, never, HttpClient.HttpClient | CloudflareConfig> =>
  Layer.effect(CloudflareClient, make(options))

/**
 * @since 1.0.0
 * @category Layers
 */
export const layerConfig = (
  options: Config.Config.Wrap<{
    readonly apiKey: Redacted.Redacted
    readonly accountId: string
    readonly apiUrl?: string | undefined
    readonly useOpenAIEndpoints?: boolean | undefined
    readonly transformClient?: (client: HttpClient.HttpClient) => HttpClient.HttpClient
  }>
): Layer.Layer<CloudflareClient, ConfigError, HttpClient.HttpClient | CloudflareConfig> =>
  Config.unwrap(options).pipe(
    Effect.flatMap(make),
    Layer.effect(CloudflareClient)
  )

// =============================================================================
// Types
// =============================================================================

interface RawCompletionChunk {
  readonly id: string
  readonly object: "chat.completion.chunk"
  readonly created: number
  readonly model: string
  readonly choices: ReadonlyArray<RawChoice>
  readonly usage: RawUsage | null
}

interface RawChoice {
  readonly index: number
  readonly finish_reason: "stop" | "length" | "content_filter" | "function_call" | "tool_calls" | null
  readonly delta: RawDelta
}

type RawDelta = {
  readonly index?: number
  readonly role?: string
  readonly content: string
} | {
  readonly index?: number
  readonly role?: string
  readonly content?: null
  readonly tool_calls: Array<RawToolDelta>
}

interface RawUsage {
  readonly prompt_tokens: number
  readonly completion_tokens: number
  readonly total_tokens: number
}

type RawToolCall = {
  readonly index: number
  readonly id: string
  readonly type: "function"
  readonly function: {
    readonly name: string
    arguments: string
  }
}

type RawToolDelta = RawToolCall | {
  readonly index: number
  readonly function: {
    readonly arguments: string
  }
}

interface CloudflareResponse {
  readonly result?: {
    readonly response: string
  }
  readonly success: boolean
  readonly errors: ReadonlyArray<string>
  readonly messages: ReadonlyArray<string>
}

// =============================================================================
// Utilities
// =============================================================================

const resolveFinishReason = (reason: string): AiResponse.FinishReason => {
  switch (reason) {
    case "stop":
      return "stop"
    case "length":
      return "length"
    case "content_filter":
      return "content-filter"
    case "tool_calls":
    case "function_call":
      return "tool-calls"
    default:
      return "unknown"
  }
}
