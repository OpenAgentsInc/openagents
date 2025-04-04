/**
 * Validation-related error classes
 */

import { ChatError, ChatErrorOptions } from './base-error';

// Validation error options
export interface ValidationErrorOptions extends Omit<ChatErrorOptions, 'category'> {
  field?: string;
  value?: unknown;
  validationRule?: string;
}

/**
 * Base class for validation errors
 */
export class ValidationError extends ChatError {
  public readonly field?: string;
  public readonly value?: unknown;
  public readonly validationRule?: string;
  
  constructor(options: ValidationErrorOptions) {
    super({
      ...options,
      category: 'validation',
    });
    this.name = 'ValidationError';
    this.field = options.field;
    this.value = options.value;
    this.validationRule = options.validationRule;
    
    // Add validation info to metadata
    if (options.field) this.metadata.field = options.field;
    if (options.validationRule) this.metadata.validationRule = options.validationRule;
    // Only include value if it's simple and safe to serialize
    if (options.value !== undefined && 
        (typeof options.value === 'string' || 
         typeof options.value === 'number' || 
         typeof options.value === 'boolean')) {
      this.metadata.value = options.value;
    }
  }
}

/**
 * Message validation errors (format, structure, etc.)
 */
export class MessageValidationError extends ValidationError {
  constructor(options: ValidationErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        'Invalid message format. Please ensure your message is properly formatted.'
    });
    this.name = 'MessageValidationError';
  }
}

/**
 * Message content validation errors
 */
export class ContentValidationError extends ValidationError {
  constructor(options: ValidationErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        'Invalid message content. Please check your message and try again.'
    });
    this.name = 'ContentValidationError';
  }
}

/**
 * Model validation errors (invalid model ID, etc.)
 */
export class ModelValidationError extends ValidationError {
  public readonly modelId?: string;
  
  constructor(options: ValidationErrorOptions & { modelId?: string }) {
    super({
      ...options,
      userMessage: options.userMessage || 
        options.modelId 
          ? `Invalid model ID: ${options.modelId}. Please select a valid model.` 
          : 'Invalid model specification. Please select a valid model.'
    });
    this.name = 'ModelValidationError';
    this.modelId = options.modelId;
    if (options.modelId) this.metadata.modelId = options.modelId;
  }
}

/**
 * API key validation errors
 */
export class ApiKeyValidationError extends ValidationError {
  public readonly provider?: string;
  
  constructor(options: ValidationErrorOptions & { provider?: string }) {
    super({
      ...options,
      userMessage: options.userMessage || 
        options.provider 
          ? `Missing or invalid API key for ${options.provider}. Please add your API key in Settings.` 
          : 'Missing or invalid API key. Please check your settings.'
    });
    this.name = 'ApiKeyValidationError';
    this.provider = options.provider;
    if (options.provider) this.metadata.provider = options.provider;
  }
}

/**
 * Schema validation errors
 */
export class SchemaValidationError extends ValidationError {
  constructor(options: ValidationErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        options.field 
          ? `Invalid value for ${options.field}. Please check your input and try again.` 
          : 'Invalid input data. Please check your input and try again.'
    });
    this.name = 'SchemaValidationError';
  }
}