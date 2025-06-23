export interface ModelConfig {
  id: string
  name: string
  provider: "cloudflare" | "openrouter"
  requiresApiKey: boolean
  description?: string
}

export const AVAILABLE_MODELS: Array<ModelConfig> = [
  // Cloudflare Models (Free)
  {
    id: "@cf/meta/llama-4-scout-17b-16e-instruct",
    name: "Llama 4 Scout 17B",
    provider: "cloudflare",
    requiresApiKey: false,
    description: "Latest Llama model, optimized for efficiency"
  },
  {
    id: "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
    name: "Llama 3.3 70B",
    provider: "cloudflare",
    requiresApiKey: false,
    description: "Fast, high-quality responses"
  },
  {
    id: "@cf/deepseek-ai/deepseek-r1-distill-qwen-32b",
    name: "DeepSeek R1 Distill 32B",
    provider: "cloudflare",
    requiresApiKey: false,
    description: "Advanced reasoning model"
  },
  {
    id: "@cf/meta/llama-3.1-8b-instruct",
    name: "Llama 3.1 8B",
    provider: "cloudflare",
    requiresApiKey: false,
    description: "Faster, smaller model"
  },

  // OpenRouter Models (Requires API Key)
  {
    id: "anthropic/claude-sonnet-4",
    name: "Claude Sonnet 4",
    provider: "openrouter",
    requiresApiKey: true,
    description: "Best for coding & reasoning"
  },
  {
    id: "anthropic/claude-opus-4",
    name: "Claude Opus 4",
    provider: "openrouter",
    requiresApiKey: true,
    description: "Most capable Claude model"
  },
  {
    id: "anthropic/claude-3.5-sonnet",
    name: "Claude 3.5 Sonnet",
    provider: "openrouter",
    requiresApiKey: true
  },
  {
    id: "openai/gpt-4o",
    name: "GPT-4o",
    provider: "openrouter",
    requiresApiKey: true,
    description: "OpenAI's latest model"
  },
  {
    id: "openai/gpt-4-turbo",
    name: "GPT-4 Turbo",
    provider: "openrouter",
    requiresApiKey: true
  },
  {
    id: "meta-llama/llama-3.1-405b-instruct",
    name: "Llama 3.1 405B",
    provider: "openrouter",
    requiresApiKey: true,
    description: "Largest open model"
  }
]

export const DEFAULT_MODEL = "@cf/meta/llama-4-scout-17b-16e-instruct"

export function getModelConfig(modelId: string): ModelConfig | undefined {
  return AVAILABLE_MODELS.find((m) => m.id === modelId)
}

export function getModelsByProvider(provider: "cloudflare" | "openrouter"): Array<ModelConfig> {
  return AVAILABLE_MODELS.filter((m) => m.provider === provider)
}
