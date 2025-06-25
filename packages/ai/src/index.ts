/**
 * AI completion response
 * @since 1.0.0
 */
export * as AiService from "./AiService.js"

/**
 * Core AI interfaces and types
 * @since 1.0.0
 */
export * as AiLanguageModel from "./core/AiLanguageModel.js"
export * as AiPrompt from "./core/AiInput.js"  
export * as AiMessage from "./core/AiInput.js"
export * as AiResponse from "./core/AiResponse.js"
export * as AiError from "./core/AiError.js"
export * as AiEmbeddingModel from "./core/AiEmbeddingModel.js"

/**
 * AI services
 * @since 1.0.0
 */
export * as MessageEmbeddingService from "./services/MessageEmbeddingService.js"
// TODO: Re-enable when Convex integration is complete
// export * as ConvexEmbeddingService from "./services/ConvexEmbeddingService.js"

/**
 * Standardized types aligned with Vercel AI SDK v5
 * @since 1.0.0
 */
export * from "./types/index.js"

/**
 * Ollama provider for local LLMs
 * @since 1.0.0
 */
export * as Ollama from "./providers/ollama/index.js"

/**
 * OpenRouter provider for 300+ models
 * @since 1.0.0
 */
export * as OpenRouter from "./providers/openrouter/index.js"

/**
 * Cloudflare Workers AI provider for edge-native inference
 * @since 1.0.0
 */
export * as Cloudflare from "./providers/cloudflare/index.js"

/**
 * OpenAI provider for GPT models and embeddings
 * @since 1.0.0
 */
export * as OpenAI from "./providers/openai/index.js"

/**
 * Internal exports for CLI integration
 * @since 1.0.0
 */
export * as internal from "./internal.js"
