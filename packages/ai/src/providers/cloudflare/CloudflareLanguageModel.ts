/**
 * @since 1.0.0
 */
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as Stream from "effect/Stream"
import { AiError } from "../../core/AiError.js"
import * as AiLanguageModel from "../../core/AiLanguageModel.js"
import * as AiResponse from "../../core/AiResponse.js"
import { CloudflareClient } from "./CloudflareClient.js"
import { convertToCloudflareMessages } from "./internal/utilities.js"

/**
 * @since 1.0.0
 * @category Context
 */
export class CloudflareLanguageModel extends Context.Tag("@openagentsinc/ai-cloudflare/CloudflareLanguageModel")<
  CloudflareLanguageModel,
  AiLanguageModel.AiLanguageModel.Service<never>
>() {}

/**
 * @since 1.0.0
 * @category Constructors
 */
export const makeLanguageModel = (options: {
  readonly model: string
  readonly temperature?: number | undefined
  readonly maxTokens?: number | undefined
  readonly topP?: number | undefined
  readonly frequencyPenalty?: number | undefined
  readonly presencePenalty?: number | undefined
  readonly stop?: string | ReadonlyArray<string> | undefined
}): Effect.Effect<AiLanguageModel.AiLanguageModel.Service<never>, never, CloudflareClient> =>
  Effect.gen(function*() {
    const client = yield* CloudflareClient

    const streamText = (
      input: AiLanguageModel.AiLanguageModelOptions
    ): Stream.Stream<AiResponse.AiResponse, AiError, never> => {
      const messages = convertToCloudflareMessages(input.prompt.messages)

      return client.stream({
        model: options.model,
        messages,
        temperature: options.temperature,
        max_tokens: options.maxTokens,
        top_p: options.topP,
        frequency_penalty: options.frequencyPenalty,
        presence_penalty: options.presencePenalty,
        stop: options.stop,
        stream: true
      }).pipe(
        Stream.catchAll((error) =>
          Stream.fail(
            new AiError({
              module: "CloudflareLanguageModel",
              method: "streamText",
              description: `Cloudflare API Error: ${error.message}`
            })
          )
        )
      )
    }

    const generateText = (
      input: AiLanguageModel.AiLanguageModelOptions
    ): Effect.Effect<AiResponse.AiResponse, AiError, never> => {
      // Collect the stream into a single response
      return Stream.runCollect(streamText(input)).pipe(
        Effect.map((chunks) => {
          const responses: Array<AiResponse.AiResponse> = []
          for (const chunk of chunks) {
            for (const part of chunk.parts) {
              responses.push(AiResponse.AiResponse.make({ parts: [part] }, { disableValidation: true }))
            }
          }
          return AiResponse.AiResponse.make({
            parts: responses.flatMap((r) => r.parts)
          }, { disableValidation: true })
        })
      )
    }

    return yield* AiLanguageModel.make({
      generateText,
      streamText
    })
  })

/**
 * @since 1.0.0
 * @category Layers
 */
export const layer = (options: {
  readonly model: string
  readonly temperature?: number | undefined
  readonly maxTokens?: number | undefined
  readonly topP?: number | undefined
  readonly frequencyPenalty?: number | undefined
  readonly presencePenalty?: number | undefined
  readonly stop?: string | ReadonlyArray<string> | undefined
}): Layer.Layer<CloudflareLanguageModel, never, CloudflareClient> =>
  Layer.effect(CloudflareLanguageModel, makeLanguageModel(options))

/**
 * @since 1.0.0
 * @category Models
 */
export const models = {
  // Text Generation Models
  LLAMA_3_1_8B_INSTRUCT: "@cf/meta/llama-3.1-8b-instruct",
  LLAMA_3_1_70B_INSTRUCT: "@cf/meta/llama-3.1-70b-instruct",
  LLAMA_3_2_11B_VISION: "@cf/meta/llama-3.2-11b-vision-instruct",
  LLAMA_3_2_3B_INSTRUCT: "@cf/meta/llama-3.2-3b-instruct",
  LLAMA_3_2_1B_INSTRUCT: "@cf/meta/llama-3.2-1b-instruct",

  // Gemma Models
  GEMMA_2_9B_IT: "@cf/google/gemma-2-9b-it",
  GEMMA_7B_IT: "@cf/google/gemma-7b-it",

  // Mistral Models
  MISTRAL_7B_INSTRUCT: "@cf/mistral/mistral-7b-instruct-v0.1",

  // Qwen Models
  QWEN_1_5_7B_CHAT: "@cf/qwen/qwen1.5-7b-chat-awq",
  QWEN_1_5_14B_CHAT: "@cf/qwen/qwen1.5-14b-chat-awq",

  // Code Models
  DEEPSEEK_CODER_6_7B: "@cf/deepseek-ai/deepseek-coder-6.7b-instruct-awq",
  CODELLAMA_7B_INSTRUCT: "@cf/meta/code-llama-13b-instruct-awq",

  // Phi Models
  PHI_2: "@cf/microsoft/phi-2",

  // Embedding Models
  BGE_BASE_EN_V1_5: "@cf/baai/bge-base-en-v1.5",
  BGE_LARGE_EN_V1_5: "@cf/baai/bge-large-en-v1.5",
  BGE_SMALL_EN_V1_5: "@cf/baai/bge-small-en-v1.5"
} as const

/**
 * @since 1.0.0
 * @category Presets
 */
export const presets = {
  /**
   * High-performance general purpose model
   */
  llama31_70b: (options?: Partial<Parameters<typeof makeLanguageModel>[0]>) =>
    makeLanguageModel({
      model: models.LLAMA_3_1_70B_INSTRUCT,
      temperature: 0.7,
      maxTokens: 4096,
      ...options
    }),

  /**
   * Balanced performance and speed
   */
  llama31_8b: (options?: Partial<Parameters<typeof makeLanguageModel>[0]>) =>
    makeLanguageModel({
      model: models.LLAMA_3_1_8B_INSTRUCT,
      temperature: 0.7,
      maxTokens: 4096,
      ...options
    }),

  /**
   * Vision-capable model
   */
  llama32_vision: (options?: Partial<Parameters<typeof makeLanguageModel>[0]>) =>
    makeLanguageModel({
      model: models.LLAMA_3_2_11B_VISION,
      temperature: 0.7,
      maxTokens: 4096,
      ...options
    }),

  /**
   * Fast and efficient for simple tasks
   */
  llama32_3b: (options?: Partial<Parameters<typeof makeLanguageModel>[0]>) =>
    makeLanguageModel({
      model: models.LLAMA_3_2_3B_INSTRUCT,
      temperature: 0.7,
      maxTokens: 2048,
      ...options
    }),

  /**
   * Code generation specialist
   */
  deepseek_coder: (options?: Partial<Parameters<typeof makeLanguageModel>[0]>) =>
    makeLanguageModel({
      model: models.DEEPSEEK_CODER_6_7B,
      temperature: 0.3,
      maxTokens: 4096,
      ...options
    }),

  /**
   * Multilingual support
   */
  gemma_9b: (options?: Partial<Parameters<typeof makeLanguageModel>[0]>) =>
    makeLanguageModel({
      model: models.GEMMA_2_9B_IT,
      temperature: 0.7,
      maxTokens: 4096,
      ...options
    })
} as const
