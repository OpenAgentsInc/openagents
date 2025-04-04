/**
 * Model detection and configuration utilities
 */

import { MODELS } from "@openagents/core";
import { ModelValidationError, ProviderType } from "@openagents/core/src/chat/errors";

/**
 * Find model information in the MODELS array or detect based on pattern
 */
export function findModelInfo(modelId: string, preferredProvider?: string): {
  id: string;
  name: string;
  provider: string;
  author: string;
  created: number;
  description: string;
  context_length: number;
  supportsTools: boolean;
  shortDescription: string;
} {
  // First try to find in the MODELS array
  let modelInfo = MODELS.find(m => m.id === modelId);
  
  // If not found, try to detect based on pattern
  if (!modelInfo) {
    // Check for patterns in the model ID that suggest different model types
    const isClaudeModel = modelId.startsWith('claude-');
    const isLmStudioModel = modelId.includes('gemma') ||
      modelId.toLowerCase().includes('llama') ||
      modelId.includes('mistral') ||
      modelId.includes('qwen') ||
      modelId.includes('neural') ||
      modelId.includes('gpt') ||
      modelId.includes('deepseek');
      
    // Check for Google models
    const isGoogleModel = modelId.startsWith('gemini-') || 
      modelId === 'gemini' || 
      modelId.includes('models/gemini');

    // Consider any model with a / in the name as a potentially valid OpenRouter model
    const hasSlash = modelId.includes('/');

    // If it's a Claude model, set provider to Anthropic
    if (isClaudeModel) {
      console.log(`[Server] Model ${modelId} not in MODELS array but detected as Anthropic Claude model`);
      return {
        id: modelId,
        name: modelId.split('/').pop() || modelId,
        provider: 'anthropic',
        author: 'anthropic' as any,
        created: Date.now(),
        description: `Anthropic ${modelId} model`,
        context_length: modelId.includes('haiku') ? 48000 : 200000,
        supportsTools: true,
        shortDescription: `Anthropic ${modelId} model`
      };
    }
    // If it's a Google model
    else if (isGoogleModel) {
      console.log(`[Server] Model ${modelId} not in MODELS array but detected as Google model`);
      return {
        id: modelId,
        name: modelId.split('/').pop() || modelId,
        provider: 'google',
        author: 'google' as any,
        created: Date.now(),
        description: `Google ${modelId} model`,
        context_length: 32000,
        supportsTools: true,
        shortDescription: `Google ${modelId} model`
      };
    }
    // If it has a slash, assume it's an OpenRouter model
    else if (hasSlash) {
      console.log(`[Server] Model ${modelId} not in MODELS array but detected as OpenRouter model due to slash`);
      return {
        id: modelId,
        name: modelId.split('/').pop() || modelId,
        provider: 'openrouter',
        author: modelId.split('/')[0] as any || 'unknown' as any,
        created: Date.now(),
        description: `OpenRouter model: ${modelId}`,
        context_length: 8192,
        supportsTools: true,
        shortDescription: `OpenRouter model: ${modelId}`
      };
    }
    // If it looks like an Ollama model
    else if (isLmStudioModel || preferredProvider === 'ollama') {
      console.log(`[Server] Model ${modelId} detected as Ollama model`);
      return {
        id: modelId,
        name: modelId,
        provider: 'ollama',
        author: 'unknown' as any,
        created: Date.now(),
        description: `Ollama model: ${modelId}`,
        context_length: 8192,
        supportsTools: false,
        shortDescription: `Ollama model: ${modelId}`
      };
    }
    
    // Cannot determine model type
    throw new ModelValidationError({
      message: `Model "${modelId}" not found and doesn't match any known pattern`,
      modelId,
      userMessage: `Model "${modelId}" not found in the MODELS array and doesn't appear to be a valid model ID. Please select a different model.`
    });
  }
  
  return modelInfo;
}

/**
 * Validate that the provider is appropriate for the model
 */
export function validateModelProviderMatch(modelId: string, provider: ProviderType): void {
  // Prevent Claude models from being routed anywhere except Anthropic
  if (modelId.startsWith('claude-') && provider !== 'anthropic') {
    throw new ModelValidationError({
      message: `Claude model ${modelId} must use the Anthropic provider, not ${provider}`,
      modelId,
      userMessage: `ROUTING ERROR: Claude model ${modelId} must use the Anthropic provider, not ${provider}. Please select a model with the correct provider.`
    });
  }
  
  // Prevent models with a slash from going to non-OpenRouter
  if (modelId.includes('/') && provider !== 'openrouter') {
    throw new ModelValidationError({
      message: `Model ${modelId} with a '/' pattern should use the OpenRouter provider, not ${provider}`,
      modelId,
      userMessage: `ROUTING ERROR: Model ${modelId} with a '/' pattern should use the OpenRouter provider, not ${provider}. Please select a model with the correct provider.`
    });
  }
  
  // Prevent Google models from being routed elsewhere
  if ((modelId.startsWith('gemini-') || modelId === 'gemini' || modelId.includes('models/gemini')) && 
      provider !== 'google') {
    throw new ModelValidationError({
      message: `Gemini model ${modelId} must use the Google provider, not ${provider}`,
      modelId,
      userMessage: `ROUTING ERROR: Gemini model ${modelId} must use the Google provider, not ${provider}. Please select a model with the correct provider.`
    });
  }
}