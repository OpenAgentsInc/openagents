/**
 * @since 1.0.0
 */
import * as ReadonlyArray from "effect/Array"
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import * as Stream from "effect/Stream"
import { AiError } from "../../core/AiError.js"
import * as AiLanguageModel from "../../core/AiLanguageModel.js"
import type * as AiResponse from "../../core/AiResponse.js"
import * as InternalUtilities from "./internal/utilities.js"
import { OpenRouterClient, type StreamCompletionRequest } from "./OpenRouterClient.js"
import { OpenRouterConfig } from "./OpenRouterConfig.js"

/**
 * @since 1.0.0
 * @category Context
 */
export class OpenRouterLanguageModel extends Context.Tag("@openagentsinc/ai-openrouter/OpenRouterLanguageModel")<
  OpenRouterLanguageModel,
  AiLanguageModel.AiLanguageModel.Service<never>
>() {}

/**
 * @since 1.0.0
 * @category Constructors
 */
export const makeLanguageModel = (
  {
    modelId = "openrouter/auto"
  }: {
    readonly modelId?: string
  } = {}
): Effect.Effect<AiLanguageModel.AiLanguageModel.Service<never>, never, OpenRouterClient | OpenRouterConfig> =>
  Effect.gen(function*() {
    const client = yield* OpenRouterClient
    const config = yield* Effect.serviceOption(OpenRouterConfig)

    const mergedConfig = Option.match(config, {
      onNone: () => ({}),
      onSome: (c) => c
    })

    const doGenerate = (
      options: AiLanguageModel.AiLanguageModelOptions
    ): Effect.Effect<AiResponse.AiResponse, AiError> => {
      const systemMessages = Option.match(options.system, {
        onNone: () => [],
        onSome: (system) => [{ role: "system", content: system }]
      })
      const convertedMessages = InternalUtilities.convertToOpenRouterMessages(options.prompt.messages)
      const messages = [...systemMessages, ...convertedMessages] as ReadonlyArray<{
        readonly role: string
        readonly content: string | ReadonlyArray<unknown>
        readonly name?: string
        readonly tool_calls?: ReadonlyArray<unknown>
        readonly tool_call_id?: string
      }>

      const openRouterRequest: StreamCompletionRequest = {
        model: modelId,
        models: "fallbackModels" in mergedConfig ? mergedConfig.fallbackModels : undefined,
        messages,
        tools: options.tools.length > 0 ?
          options.tools.map((tool) => ({
            type: "function",
            function: {
              name: tool.name,
              description: tool.description,
              parameters: tool.parameters
            }
          })) :
          undefined,
        tool_choice: resolveToolChoice(options.toolChoice),
        provider: "providerRouting" in mergedConfig ? mergedConfig.providerRouting : undefined
      }

      return Effect.scoped(
        Stream.runCollect(client.stream(openRouterRequest)).pipe(
          Effect.map((chunks) => ReadonlyArray.flatten(ReadonlyArray.fromIterable(chunks))),
          Effect.mapError((error) =>
            new AiError({
              module: "OpenRouterLanguageModel",
              method: "generate",
              description: error.message || "Failed to generate response"
            })
          )
        )
      )
    }

    const doGenerateStream = (
      options: AiLanguageModel.AiLanguageModelOptions
    ): Stream.Stream<AiResponse.AiResponse, AiError> => {
      const systemMessages = Option.match(options.system, {
        onNone: () => [],
        onSome: (system) => [{ role: "system", content: system }]
      })
      const convertedMessages = InternalUtilities.convertToOpenRouterMessages(options.prompt.messages)
      const messages = [...systemMessages, ...convertedMessages] as ReadonlyArray<{
        readonly role: string
        readonly content: string | ReadonlyArray<unknown>
        readonly name?: string
        readonly tool_calls?: ReadonlyArray<unknown>
        readonly tool_call_id?: string
      }>

      const openRouterRequest: StreamCompletionRequest = {
        model: modelId,
        models: "fallbackModels" in mergedConfig ? mergedConfig.fallbackModels : undefined,
        messages,
        tools: options.tools.length > 0 ?
          options.tools.map((tool) => ({
            type: "function",
            function: {
              name: tool.name,
              description: tool.description,
              parameters: tool.parameters
            }
          })) :
          undefined,
        tool_choice: resolveToolChoice(options.toolChoice),
        provider: "providerRouting" in mergedConfig ? mergedConfig.providerRouting : undefined
      }

      return client.stream(openRouterRequest).pipe(
        Stream.mapError((error) =>
          new AiError({
            module: "OpenRouterLanguageModel",
            method: "generateStream",
            description: error.message || "Failed to stream response"
          })
        )
      )
    }

    return yield* AiLanguageModel.make({
      generateText: doGenerate,
      streamText: doGenerateStream
    })
  })

// =============================================================================
// Utilities
// =============================================================================

const resolveToolChoice = (
  toolChoice: AiLanguageModel.ToolChoice<any>
): { type: string; function?: { name: string } } | undefined => {
  if (toolChoice === "auto") {
    return { type: "auto" }
  } else if (toolChoice === "none") {
    return { type: "none" }
  } else if (toolChoice === "required") {
    return { type: "required" }
  } else if ("tool" in toolChoice) {
    return { type: "function", function: { name: toolChoice.tool } }
  }
  return undefined
}
