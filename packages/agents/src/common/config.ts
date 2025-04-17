import { env } from "cloudflare:workers";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';

/**
 * Initialize OpenRouter client with API key from environment
 */
export const openrouter = createOpenRouter({ apiKey: env.OPENROUTER_API_KEY });

/**
 * Main model for complex agent tasks
 */
export const model = openrouter("google/gemini-2.5-pro-preview-03-25");

/**
 * Smaller model for simpler structured generation tasks
 */
export const smallModel = openrouter("openai/gpt-4o-mini");