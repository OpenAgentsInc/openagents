/**
 * OpenRouter provider implementation
 */

import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import { Provider } from './index';

/**
 * Create an OpenRouter provider
 */
export function createOpenRouterProvider(
  modelId: string,
  apiKey: string,
  options: Record<string, any> = {}
): Provider {
  if (!apiKey) {
    throw new Error('OpenRouter API key is required');
  }
  
  // Create the OpenRouter client
  const openRouter = createOpenRouter({
    apiKey,
    baseURL: options.baseURL || "https://openrouter.ai/api/v1"
  });
  
  // Set headers for OpenRouter - important for authentication and usage tracking
  const headers = {
    'Authorization': `Bearer ${apiKey}`,
    'HTTP-Referer': 'https://openagents.com',
    'X-Title': 'OpenAgents Coder'
  };
  
  // Try to determine context window size from model ID
  // This is approximate and should ideally be fetched from OpenRouter's API
  let contextWindowSize = 8192; // Default for most models
  
  // Adjust context window size based on recognized models
  if (modelId.includes('claude-3')) {
    if (modelId.includes('opus')) {
      contextWindowSize = 200000;
    } else if (modelId.includes('sonnet')) {
      contextWindowSize = 200000;
    } else if (modelId.includes('haiku')) {
      contextWindowSize = 48000;
    }
  } else if (modelId.includes('claude-2') || modelId.includes('claude-instant')) {
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
  
  // Try to determine if the model supports tools
  // This is approximate and should ideally be fetched from OpenRouter's API
  const supportsTools = 
    modelId.includes('claude-3') ||
    modelId.includes('gpt-4') ||
    modelId.includes('gpt-3.5-turbo') ||
    modelId.includes('mistral-large') ||
    modelId.includes('claude-2.1') ||
    modelId.includes('gemini');
  
  return {
    id: modelId,
    name: modelId,
    model: openRouter(modelId),
    supportsTools,
    contextWindowSize,
    headers
  };
}