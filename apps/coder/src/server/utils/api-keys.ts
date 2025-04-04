/**
 * API key management utilities
 */

import { 
  ApiKeyValidationError,
  ProviderType
} from '@openagents/core/src/chat/errors';

/**
 * API keys from various sources
 */
export interface ApiKeys {
  openrouter?: string;
  anthropic?: string;
  google?: string;
  ollama?: string;
  ollamaBaseUrl?: string;
}

/**
 * Get API keys from request and environment
 */
export function getApiKeys(requestApiKeys: Record<string, string> = {}): ApiKeys {
  const keys: ApiKeys = {
    openrouter: requestApiKeys.openrouter || process.env.OPENROUTER_API_KEY || "",
    anthropic: requestApiKeys.anthropic || process.env.ANTHROPIC_API_KEY || "",
    google: requestApiKeys.google || process.env.GOOGLE_API_KEY || "",
    ollama: undefined
  };
  
  // For Ollama, get the base URL if provided
  const ollamaBaseUrl = requestApiKeys.ollama || 
                         requestApiKeys.ollamaBaseUrl || 
                         process.env.OLLAMA_BASE_URL || 
                         "http://localhost:11434/api";
  
  // Store Ollama URL separately
  keys.ollamaBaseUrl = ollamaBaseUrl;
  
  return keys;
}

/**
 * Validate API key for a specific provider
 */
export function validateApiKey(provider: ProviderType, apiKeys: ApiKeys): void {
  switch (provider) {
    case 'anthropic':
      if (!apiKeys.anthropic) {
        throw new ApiKeyValidationError({
          message: 'Anthropic API key not configured',
          provider: 'anthropic',
          userMessage: 'Anthropic API Key not configured. Please add your API key in the Settings > API Keys tab to use Claude models.'
        });
      }
      break;
      
    case 'openrouter':
      if (!apiKeys.openrouter) {
        throw new ApiKeyValidationError({
          message: 'OpenRouter API key not configured',
          provider: 'openrouter',
          userMessage: 'OpenRouter API Key not configured. Please add your API key in the Settings > API Keys tab to use OpenRouter models.'
        });
      }
      break;
      
    case 'google':
      if (!apiKeys.google) {
        throw new ApiKeyValidationError({
          message: 'Google API key not configured',
          provider: 'google',
          userMessage: 'Google API Key not configured. Please add your API key in the Settings > API Keys tab to use Gemini models.'
        });
      }
      break;
      
    // Ollama doesn't need an API key, but we log its status
    case 'ollama':
      console.log(`[Server] Using Ollama with base URL: ${apiKeys.ollamaBaseUrl}`);
      break;
  }
}

/**
 * Get provider options from API keys
 */
export function getProviderOptions(provider: ProviderType, apiKeys: ApiKeys): Record<string, any> {
  switch (provider) {
    case 'ollama':
      return {
        baseURL: apiKeys.ollamaBaseUrl
      };
      
    default:
      return {};
  }
}