/**
 * Google provider implementation
 */

import { google, createGoogleGenerativeAI } from '@ai-sdk/google';
import { Provider } from './index';
import { ModelNotFoundError } from '@openagents/core/src/chat/errors';

/**
 * Create a Google provider
 */
export function createGoogleProvider(
  modelId: string,
  apiKey: string,
  options: Record<string, any> = {}
): Provider {
  if (!apiKey) {
    throw new Error('Google API key is required');
  }
  
  // Validate modelId format
  if (!modelId.startsWith('gemini-') && !modelId.includes('gemini')) {
    throw new ModelNotFoundError({
      message: `Invalid Google model ID: ${modelId}. Google models should start with "gemini-".`,
      provider: 'google',
      modelId
    });
  }
  
  // Create the Google client
  const googleClient = createGoogleGenerativeAI({
    apiKey,
    // Allow custom baseURL if provided
    ...(options.baseURL ? { baseURL: options.baseURL } : {})
  });
  
  // Determine context window size based on model
  let contextWindowSize = 32000; // Default for Gemini Pro
  
  if (modelId.includes('flash')) {
    contextWindowSize = 16000;
  } else if (modelId.includes('1.5-pro')) {
    contextWindowSize = 1000000;
  } else if (modelId.includes('1.5-flash')) {
    contextWindowSize = 1000000;
  }
  
  return {
    id: modelId,
    name: `Google ${modelId}`,
    model: googleClient(modelId),
    supportsTools: true, // All Gemini models support function calling
    contextWindowSize,
    headers: {} // Google headers are handled internally by the SDK
  };
}