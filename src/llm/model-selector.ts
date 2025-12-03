import type { Provider } from "./model-types.js";
import { getModels } from "./models.js";

const DEFAULT_MODEL: Record<Provider, string> = {
  anthropic: "claude-sonnet-4-5",
  openai: "gpt-5.1-codex",
  google: "gemini-2.5-pro",
  openrouter: "openrouter/auto",
  xai: "grok-4.1-fast",
  groq: "llama-3.1-8b-instruct",
  cerebras: "cerebras/llama3.1-70b",
  zai: "glm-4.6",
};

export const getDefaultModelForProvider = (provider: Provider): string | null => {
  if (provider in DEFAULT_MODEL) return DEFAULT_MODEL[provider];
  const models = getModels(provider as any);
  return models.length > 0 ? models[0].id : null;
};

export const cycleModel = (provider: Provider, current?: string): string | null => {
  const models = getModels(provider as any);
  if (models.length === 0) return null;
  if (!current) return models[0].id;
  const idx = models.findIndex((m) => m.id === current);
  const next = (idx + 1) % models.length;
  return models[next]?.id ?? models[0].id;
};
