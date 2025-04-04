/**
 * Anthropic provider implementation
 */

import { anthropic, createAnthropic } from '@ai-sdk/anthropic';
import { Provider } from './index';
import { ModelNotFoundError } from '@openagents/core/src/chat/errors';

/**
 * Create an Anthropic provider
 */
export function createAnthropicProvider(
  modelId: string,
  apiKey: string,
  options: Record<string, any> = {}
): Provider {
  if (!apiKey) {
    throw new Error('Anthropic API key is required');
  }
  
  // Validate that this is a Claude model
  if (!modelId.startsWith('claude-')) {
    throw new ModelNotFoundError({
      message: `Invalid Anthropic model ID: ${modelId}. Anthropic models must start with "claude-".`,
      provider: 'anthropic',
      modelId
    });
  }
  
  // Create the Anthropic client
  const anthropicClient = createAnthropic({
    apiKey,
    // Allow custom baseURL if provided
    ...(options.baseURL ? { baseURL: options.baseURL } : {})
  });
  
  // Determine context window size based on model version
  let contextWindowSize = 200000; // Default Claude 3 Opus/Sonnet
  
  if (modelId.includes('haiku')) {
    contextWindowSize = 48000;
  } else if (modelId.includes('claude-2')) {
    contextWindowSize = 100000;
  } else if (modelId.includes('claude-instant') || modelId.includes('claude-1')) {
    contextWindowSize = 100000;
  }
  
  return {
    id: modelId,
    name: `Anthropic ${modelId}`,
    model: anthropicClient(modelId),
    supportsTools: !modelId.includes('claude-instant') && !modelId.includes('claude-1'),
    contextWindowSize,
    headers: {} // Anthropic headers are handled internally by the SDK
  };
}