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
    id: "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
    name: "Llama 3.3 70B",
    provider: "cloudflare",
    requiresApiKey: false,
    description: "Fast, high-quality responses"
  },
  {
    id: "@cf/meta/llama-3.1-70b-instruct",
    name: "Llama 3.1 70B",
    provider: "cloudflare",
    requiresApiKey: false
  },
  {
    id: "@cf/meta/llama-3.1-8b-instruct",
    name: "Llama 3.1 8B",
    provider: "cloudflare",
    requiresApiKey: false,
    description: "Faster, smaller model"
  },
  {
    id: "@cf/google/gemma-2-9b-it",
    name: "Gemma 2 9B",
    provider: "cloudflare",
    requiresApiKey: false
  },
  {
    id: "@cf/mistral/mistral-7b-instruct-v0.1",
    name: "Mistral 7B",
    provider: "cloudflare",
    requiresApiKey: false
  },
  {
    id: "@cf/qwen/qwen1.5-14b-chat-awq",
    name: "Qwen 1.5 14B",
    provider: "cloudflare",
    requiresApiKey: false
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

export const DEFAULT_MODEL = "@cf/meta/llama-3.3-70b-instruct-fp8-fast"

export function getModelConfig(modelId: string): ModelConfig | undefined {
  return AVAILABLE_MODELS.find((m) => m.id === modelId)
}

export function getModelsByProvider(provider: "cloudflare" | "openrouter"): Array<ModelConfig> {
  return AVAILABLE_MODELS.filter((m) => m.provider === provider)
}
