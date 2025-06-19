/**
 * @since 1.0.0
 */
import * as Sse from "@effect/experimental/Sse"
import * as HttpBody from "@effect/platform/HttpBody"
import * as HttpClient from "@effect/platform/HttpClient"
import * as HttpClientError from "@effect/platform/HttpClientError"
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
import type { ToolCallId } from "../../core/AiInput.js"
import * as AiResponse from "../../core/AiResponse.js"
import { OpenRouterConfig } from "./OpenRouterConfig.js"

const constDisableValidation = { disableValidation: true } as const

/**
 * @since 1.0.0
 * @category Models
 */
export type StreamCompletionRequest = {
  readonly model: string
  readonly models?: ReadonlyArray<string> | undefined
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
  readonly tools?: ReadonlyArray<unknown> | undefined
  readonly tool_choice?: unknown
  readonly response_format?: unknown
  readonly provider?: OpenRouterConfig.ProviderRouting | undefined
}

/**
 * @since 1.0.0
 * @category Context
 */
export class OpenRouterClient extends Context.Tag("@openagentsinc/ai-openrouter/OpenRouterClient")<
  OpenRouterClient,
  OpenRouterClient.Service
>() {}

/**
 * @since 1.0.0
 */
export declare namespace OpenRouterClient {
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
   * The API key to use to communicate with OpenRouter API.
   */
  readonly apiKey: Redacted.Redacted
  /**
   * The URL to use to communicate with OpenRouter API.
   */
  readonly apiUrl?: string | undefined
  /**
   * The referer header for tracking.
   */
  readonly referer?: string | undefined
  /**
   * The title header for tracking.
   */
  readonly title?: string | undefined
  /**
   * A method which can be used to transform the underlying `HttpClient`.
   */
  readonly transformClient?: ((client: HttpClient.HttpClient) => HttpClient.HttpClient) | undefined
}): Effect.Effect<OpenRouterClient.Service, never, HttpClient.HttpClient | OpenRouterConfig> =>
  Effect.gen(function*() {
    const config = yield* Effect.serviceOption(OpenRouterConfig)
    const mergedConfig = Option.match(config, {
      onNone: () => ({}),
      onSome: (c) => c
    })

    const httpClient = (yield* HttpClient.HttpClient).pipe(
      HttpClient.mapRequest((request) => {
        const referer = options.referer || ("referer" in mergedConfig ? mergedConfig.referer : undefined)
        const title = options.title || ("title" in mergedConfig ? mergedConfig.title : undefined)

        return request.pipe(
          HttpClientRequest.prependUrl(options.apiUrl ?? "https://openrouter.ai/api/v1"),
          HttpClientRequest.bearerToken(options.apiKey),
          referer ? HttpClientRequest.setHeader("HTTP-Referer", referer) : identity,
          title ? HttpClientRequest.setHeader("X-Title", title) : identity,
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
        const toolCalls = {} as Record<number, RawToolCall & { isFinished: boolean }>
        let isFirstChunk = true
        let toolCallIndex: number | undefined = undefined
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

        return streamRequest<RawCompletionChunk>(HttpClientRequest.post("/chat/completions", {
          body: HttpBody.unsafeJson({
            ...request,
            stream: true,
            stream_options: { include_usage: true }
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

              // Store OpenRouter-specific metadata
              if (chunk.provider) {
                metadata.provider = chunk.provider
              }
              if (chunk.model_used && chunk.model_used !== chunk.model) {
                metadata.model_used = chunk.model_used
              }
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
                if (finishReason === "tool-calls" && Predicate.isNotUndefined(toolCallIndex)) {
                  finishToolCall(toolCalls[toolCallIndex], parts)
                }
                parts.push(
                  new AiResponse.FinishPart({
                    usage,
                    reason: finishReason,
                    providerMetadata: { "openrouter": metadata }
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

              // Handle tool call deltas
              if (Predicate.hasProperty(choice.delta, "tool_calls") && Array.isArray(choice.delta.tool_calls)) {
                for (const delta of choice.delta.tool_calls) {
                  // Make sure to emit any previous tool calls before starting a new one
                  if (Predicate.isNotUndefined(toolCallIndex) && toolCallIndex !== delta.index) {
                    finishToolCall(toolCalls[toolCallIndex], parts)
                    toolCallIndex = undefined
                  }

                  if (Predicate.isUndefined(toolCallIndex)) {
                    const toolCall = delta as unknown as RawToolCall
                    toolCalls[delta.index] = { ...toolCall, isFinished: false }
                    toolCallIndex = delta.index
                  } else {
                    toolCalls[delta.index].function.arguments += delta.function.arguments
                  }
                }
              }
            }

            return parts.length === 0
              ? Option.none()
              : Option.some(AiResponse.AiResponse.make({ parts }, constDisableValidation))
          })
        )
      })

    return { streamRequest, stream }
  })

/**
 * @since 1.0.0
 * @category Layers
 */
export const layer = (options: {
  readonly apiKey: Redacted.Redacted
  readonly apiUrl?: string | undefined
  readonly referer?: string | undefined
  readonly title?: string | undefined
  readonly transformClient?: (client: HttpClient.HttpClient) => HttpClient.HttpClient
}): Layer.Layer<OpenRouterClient, never, HttpClient.HttpClient | OpenRouterConfig> =>
  Layer.effect(OpenRouterClient, make(options))

/**
 * @since 1.0.0
 * @category Layers
 */
export const layerConfig = (
  options: Config.Config.Wrap<{
    readonly apiKey: Redacted.Redacted
    readonly apiUrl?: string | undefined
    readonly referer?: string | undefined
    readonly title?: string | undefined
    readonly transformClient?: (client: HttpClient.HttpClient) => HttpClient.HttpClient
  }>
): Layer.Layer<OpenRouterClient, ConfigError, HttpClient.HttpClient | OpenRouterConfig> =>
  Config.unwrap(options).pipe(
    Effect.flatMap(make),
    Layer.effect(OpenRouterClient)
  )

// =============================================================================
// Types (Reused from OpenAI)
// =============================================================================

interface RawCompletionChunk {
  readonly id: string
  readonly object: "chat.completion.chunk"
  readonly created: number
  readonly model: string
  readonly choices: ReadonlyArray<RawChoice>
  readonly usage: RawUsage | null
  // OpenRouter specific
  readonly provider?: string
  readonly model_used?: string
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

const finishToolCall = (
  toolCall: RawToolCall & { isFinished: boolean },
  parts: Array<AiResponse.Part>
) => {
  if (toolCall.isFinished) {
    return
  }
  try {
    const params = JSON.parse(toolCall.function.arguments)
    parts.push(
      new AiResponse.ToolCallPart({
        id: toolCall.id as ToolCallId,
        name: toolCall.function.name,
        params
      })
    )
    toolCall.isFinished = true
  } catch {
    // Ignore parse errors for invalid tool call arguments
  }
}
