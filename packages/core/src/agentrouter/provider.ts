/**
 * Agent Router provider implementation using OpenRouter
 */

import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import type { LanguageModel } from 'ai';

export interface AgentRouterProvider {
  id: string;
  name: string;
  model: LanguageModel;
  headers: Record<string, string>;
  contextWindowSize: number;
  supportsTools: boolean;
}

/**
 * Create an Agent Router provider using OpenRouter
 */
export function createAgentRouterProvider(
  modelId: string,
  apiKey: string,
  options: Record<string, any> = {}
): AgentRouterProvider {
  if (!apiKey) {
    throw new Error('OpenRouter API key is required');
  }

  // Create the OpenRouter client
  const openRouter = createOpenRouter({
    apiKey,
    baseURL: options.baseURL || "https://openrouter.ai/api/v1"
  });

  // Set headers for OpenRouter
  const headers = {
    'Authorization': `Bearer ${apiKey}`,
    'HTTP-Referer': 'https://openagents.com',
    'X-Title': 'OpenAgents Router'
  };

  // Determine context window size from model ID
  let contextWindowSize = 8192; // Default
  if (modelId.includes('claude-3')) {
    if (modelId.includes('opus')) {
      contextWindowSize = 200000;
    } else if (modelId.includes('sonnet')) {
      contextWindowSize = 200000;
    } else if (modelId.includes('haiku')) {
      contextWindowSize = 48000;
    }
  } else if (modelId.includes('claude-2')) {
    contextWindowSize = 100000;
  } else if (modelId.includes('gpt-4-turbo')) {
    contextWindowSize = 128000;
  } else if (modelId.includes('gpt-4')) {
    contextWindowSize = 8000;
  } else if (modelId.includes('gpt-3.5-turbo')) {
    contextWindowSize = 16000;
  } else if (modelId.includes('mistral') || modelId.includes('mixtral')) {
    contextWindowSize = 32000;
  }

  const model = openRouter(modelId) as unknown as LanguageModel;

  return {
    id: modelId,
    name: modelId,
    model,
    headers,
    contextWindowSize,
    supportsTools: true
  };
}
