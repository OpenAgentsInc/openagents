/**
 * Provider factory and registry
 */

import { createAnthropicProvider } from './anthropic';
import { createGoogleProvider } from './google';
import { createOpenRouterProvider } from './openrouter';
import { createOllamaProvider } from './ollama';
import { ProviderType } from '@openagents/core/src/chat/errors';

// Provider interface
export interface Provider {
  id: string;
  name: string;
  model: any; // AI SDK model instance
  supportsTools: boolean;
  contextWindowSize: number;
  headers?: Record<string, string>;
}

/**
 * Create a provider by type and model
 */
export function createProvider(
  type: ProviderType,
  modelId: string,
  apiKey: string,
  options: Record<string, any> = {}
): Provider {
  switch (type) {
    case 'anthropic':
      return createAnthropicProvider(modelId, apiKey, options);
    case 'google':
      return createGoogleProvider(modelId, apiKey, options);
    case 'openrouter':
      return createOpenRouterProvider(modelId, apiKey, options);
    case 'ollama':
      return createOllamaProvider(modelId, options);
    default:
      throw new Error(`Provider type "${type}" is not supported`);
  }
}

/**
 * Detect provider type from model ID
 */
export function detectProviderFromModel(modelId: string): ProviderType {
  // Check for Claude models
  if (modelId.startsWith('claude-')) {
    return 'anthropic';
  }
  
  // Check for Google models
  if (modelId.startsWith('gemini-') || modelId === 'gemini' || modelId.startsWith('models/gemini')) {
    return 'google';
  }
  
  // Check for slash pattern (OpenRouter)
  if (modelId.includes('/')) {
    return 'openrouter';
  }
  
  // Default for models that don't match specific patterns
  // This should be more sophisticated in a real implementation
  return 'ollama';
}