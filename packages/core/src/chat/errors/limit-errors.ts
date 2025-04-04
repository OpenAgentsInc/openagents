/**
 * Limit-related error classes (context length, rate limits, etc.)
 */

import { ChatError, ChatErrorOptions } from './base-error';

// Limit error options
export interface LimitErrorOptions extends Omit<ChatErrorOptions, 'category'> {
  limit?: number;
  current?: number;
  provider?: string;
  modelId?: string;
}

/**
 * Base class for limit errors
 */
export class LimitError extends ChatError {
  public readonly limit?: number;
  public readonly current?: number;
  public readonly provider?: string;
  public readonly modelId?: string;
  
  constructor(options: LimitErrorOptions) {
    super({
      ...options,
      category: 'limit',
    });
    this.name = 'LimitError';
    this.limit = options.limit;
    this.current = options.current;
    this.provider = options.provider;
    this.modelId = options.modelId;
    
    // Add limit info to metadata
    if (options.limit !== undefined) this.metadata.limit = options.limit;
    if (options.current !== undefined) this.metadata.current = options.current;
    if (options.provider) this.metadata.provider = options.provider;
    if (options.modelId) this.metadata.modelId = options.modelId;
  }
}

/**
 * Context length exceeded errors
 */
export class ContextLengthError extends LimitError {
  constructor(options: LimitErrorOptions) {
    const modelInfo = options.modelId && options.limit 
      ? ` (${options.modelId} has a limit of ${options.limit} tokens)`
      : options.limit
        ? ` (limit: ${options.limit} tokens)`
        : '';
        
    super({
      ...options,
      userMessage: options.userMessage || 
        `This conversation is too long for the model's context window${modelInfo}. Try starting a new chat or using a model with a larger context size.`
    });
    this.name = 'ContextLengthError';
  }
}

/**
 * Rate limit exceeded errors
 */
export class ApiRateLimitError extends LimitError {
  public readonly retryAfterMs?: number;
  
  constructor(options: LimitErrorOptions & { retryAfterMs?: number }) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Rate limit exceeded${options.provider ? ` for ${options.provider}` : ''}. Please wait a moment before sending another message.`
    });
    this.name = 'ApiRateLimitError';
    this.retryAfterMs = options.retryAfterMs;
    if (options.retryAfterMs) this.metadata.retryAfterMs = options.retryAfterMs;
  }
}

/**
 * Token quota exceeded errors
 */
export class TokenQuotaError extends LimitError {
  constructor(options: LimitErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Token quota exceeded${options.provider ? ` for ${options.provider}` : ''}. Please check your account status.`
    });
    this.name = 'TokenQuotaError';
  }
}

/**
 * Request size limit errors
 */
export class RequestSizeLimitError extends LimitError {
  constructor(options: LimitErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Request size limit exceeded. Your input is too large${options.limit ? ` (limit: ${options.limit})` : ''}.`
    });
    this.name = 'RequestSizeLimitError';
  }
}