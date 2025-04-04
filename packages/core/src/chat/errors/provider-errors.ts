/**
 * Provider-specific error classes
 */

import { ChatError, ChatErrorOptions, ErrorCategory } from './base-error';

// Provider types
export type ProviderType = 
  | 'anthropic'
  | 'openrouter'
  | 'google'
  | 'ollama'
  | 'openai'
  | 'lmstudio'
  | 'unknown';

// Provider error options
export interface ProviderErrorOptions extends Omit<ChatErrorOptions, 'category'> {
  provider: ProviderType;
  statusCode?: number;
  requestId?: string;
}

/**
 * Base class for all provider-related errors
 */
export class ProviderError extends ChatError {
  public readonly provider: ProviderType;
  public readonly statusCode?: number;
  public readonly requestId?: string;
  
  constructor(options: ProviderErrorOptions) {
    super({
      ...options,
      category: 'provider',
    });
    this.name = 'ProviderError';
    this.provider = options.provider;
    this.statusCode = options.statusCode;
    this.requestId = options.requestId;
    
    // Add provider information to metadata
    this.metadata.provider = options.provider;
    if (options.statusCode) this.metadata.statusCode = options.statusCode;
    if (options.requestId) this.metadata.requestId = options.requestId;
  }
}

/**
 * Authentication-related provider errors
 */
export class AuthenticationError extends ProviderError {
  constructor(options: ProviderErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Authentication failed for ${options.provider} provider. Please check your API key in Settings.`
    });
    this.name = 'AuthenticationError';
  }
}

/**
 * Rate limit exceeded errors
 */
export class RateLimitError extends ProviderError {
  constructor(options: ProviderErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Rate limit exceeded for ${options.provider} provider. Please wait a moment before trying again.`
    });
    this.name = 'RateLimitError';
  }
}

/**
 * Model not found errors
 */
export class ModelNotFoundError extends ProviderError {
  public readonly modelId: string;
  
  constructor(options: ProviderErrorOptions & { modelId: string }) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Model "${options.modelId}" not found or unavailable from ${options.provider}. Please select a different model.`
    });
    this.name = 'ModelNotFoundError';
    this.modelId = options.modelId;
    this.metadata.modelId = options.modelId;
  }
}

/**
 * Model configuration errors
 */
export class ModelConfigurationError extends ProviderError {
  constructor(options: ProviderErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Invalid model configuration for ${options.provider} provider. Please check your settings.`
    });
    this.name = 'ModelConfigurationError';
  }
}

/**
 * API resource exhausted errors
 */
export class ResourceExhaustedError extends ProviderError {
  constructor(options: ProviderErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Resource quota exceeded for ${options.provider} provider. Please check your account status or try again later.`
    });
    this.name = 'ResourceExhaustedError';
  }
}

/**
 * Service unavailable errors
 */
export class ServiceUnavailableError extends ProviderError {
  constructor(options: ProviderErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `${options.provider} service is currently unavailable. Please try again later.`
    });
    this.name = 'ServiceUnavailableError';
  }
}

/**
 * Invalid request errors
 */
export class InvalidRequestError extends ProviderError {
  constructor(options: ProviderErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Invalid request sent to ${options.provider} provider. This may be a bug in the application.`
    });
    this.name = 'InvalidRequestError';
  }
}