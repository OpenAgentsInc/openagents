import { Effect, Layer } from "effect"
import { AiService } from "../AiService.js"
import { ClaudeCodeClient } from "./ClaudeCodeClient.js"
import { ClaudeCodeConfig, ClaudeCodeConfigDefault } from "../config/ClaudeCodeConfig.js"

/**
 * Claude Code provider for AI Service
 * @since 1.0.0
 */
export const ClaudeCodeProviderLive = Layer.effect(
  AiService,
  Effect.gen(function* () {
    const claude = yield* ClaudeCodeClient

    return {
      hello: (name: string) =>
        Effect.succeed(`Hello ${name} from Claude Code!`),

      complete: (prompt: string) =>
        Effect.gen(function* () {
          // Check availability first
          const isAvailable = yield* claude.checkAvailability().pipe(
            Effect.catchTag("ClaudeCodeNotFoundError", () => Effect.succeed(false))
          )

          if (!isAvailable) {
            return {
              content: "Claude Code CLI is not available. Please ensure 'claude' is installed and in your PATH.",
              model: "error",
              usage: {
                promptTokens: 0,
                completionTokens: 0,
                totalTokens: 0
              }
            }
          }

          // Execute the prompt
          const response = yield* claude.prompt(prompt, {
            outputFormat: "json"
          })

          // Handle different response types
          if ("usage" in response && response.usage) {
            return {
              content: response.content,
              model: response.model,
              usage: {
                promptTokens: response.usage.input_tokens,
                completionTokens: response.usage.output_tokens,
                totalTokens: response.usage.total_tokens
              }
            }
          }

          // Fallback for text responses or missing usage data
          return {
            content: "content" in response ? response.content : String(response),
            model: "model" in response ? response.model : "claude",
            usage: {
              promptTokens: 0,
              completionTokens: 0,
              totalTokens: 0
            }
          }
        }).pipe(
          Effect.catchAll((error) =>
            Effect.succeed({
              content: `Error: ${error._tag}: ${JSON.stringify(error)}`,
              model: "error",
              usage: {
                promptTokens: 0,
                completionTokens: 0,
                totalTokens: 0
              }
            })
          )
        )
    }
  })
).pipe(
  Layer.provide(ClaudeCodeClient.ClaudeCodeClientLive),
  Layer.provide(ClaudeCodeConfigDefault)
)

/**
 * Create a Claude Code provider with custom configuration
 * @since 1.0.0
 */
export const makeClaudeCodeProvider = (config: Partial<ClaudeCodeConfig>) =>
  ClaudeCodeProviderLive.pipe(
    Layer.provide(Layer.succeed(ClaudeCodeConfig, {
      model: config.model ?? "claude-3-opus-20240229",
      outputFormat: config.outputFormat ?? "json",
      cliPath: config.cliPath ?? "claude",
      allowedTools: config.allowedTools ?? [],
      defaultTimeout: config.defaultTimeout ?? 60000,
      systemPrompt: config.systemPrompt,
      appendSystemPrompt: config.appendSystemPrompt
    }))
  )