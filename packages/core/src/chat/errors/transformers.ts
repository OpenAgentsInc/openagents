/**
 * Error transformation utilities
 */

import { UnknownError } from './base-error';
import { 
  ProviderError, ProviderType, 
  AuthenticationError, RateLimitError, 
  ModelNotFoundError, ServiceUnavailableError,
  ResourceExhaustedError, InvalidRequestError, 
  ModelConfigurationError
} from './provider-errors';
import { 
  ValidationError, MessageValidationError, 
  ApiKeyValidationError, ModelValidationError, 
  SchemaValidationError
} from './validation-errors';
import {
  ToolError, ToolExecutionError, 
  ToolArgumentError, ToolAuthenticationError, 
  ToolNotFoundError, ToolTimeoutError,
  ResourceNotFoundError
} from './tool-errors';
import { 
  NetworkError, ConnectionTimeoutError, 
  ServerUnreachableError, HttpStatusError 
} from './network-errors';
import {
  LimitError, ContextLengthError, 
  ApiRateLimitError, TokenQuotaError
} from './limit-errors';

// Helper type to represent a provider error or similar error type
type ProviderLikeError = ProviderError | ValidationError | ToolError | NetworkError | LimitError;

/**
 * Transform unknown error to appropriate error class
 */
export function transformUnknownError(error: unknown): ProviderLikeError {
  // If it's already one of our error types, return it as is
  if (error instanceof ProviderError ||
      error instanceof ValidationError ||
      error instanceof ToolError ||
      error instanceof NetworkError ||
      error instanceof LimitError) {
    return error;
  }
  
  // Extract error message from various formats
  let errorMessage = '';
  let errorDetails: Record<string, any> = {};
  
  if (error instanceof Error) {
    errorMessage = error.message;
    // Capture additional properties if they exist
    errorDetails = {
      name: error.name,
      stack: error.stack,
      ...(error as any).cause ? { cause: (error as any).cause } : {},
      ...(error as any).code ? { code: (error as any).code } : {}
    };
  } else if (typeof error === 'object' && error !== null) {
    try {
      // Try to extract error message from object
      const errorObj = error as any;
      errorMessage = errorObj.message || errorObj.error?.message || JSON.stringify(error);
      errorDetails = { ...errorObj };
    } catch (e) {
      errorMessage = 'Unknown error object';
    }
  } else {
    errorMessage = String(error);
  }
  
  // Check for context limit errors first (common across providers)
  if (errorMessage.includes('context length of only') ||
      errorMessage.includes('maximum context length') ||
      errorMessage.includes('context window') ||
      errorMessage.includes('token limit') ||
      errorMessage.includes('history is too long') ||
      errorMessage.includes('context the overflows') ||
      errorMessage.includes('context overflow')) {
    
    // Try to extract the specific context length from the error message
    const contextLengthMatch = errorMessage.match(/context length of only (\d+)/i);
    const limit = contextLengthMatch && contextLengthMatch[1] 
      ? parseInt(contextLengthMatch[1], 10) 
      : undefined;
    
    return new ContextLengthError({
      message: errorMessage,
      originalError: error,
      limit,
      provider: determineProviderFromError(error, errorMessage) // Add provider here
    });
  }
  
  // Check for rate limit errors
  if (errorMessage.includes('rate limit') ||
      errorMessage.includes('too many requests') ||
      errorMessage.includes('exceeds the limit') ||
      errorMessage.toLowerCase().includes('ratelimit')) {
    
    return new ApiRateLimitError({
      message: errorMessage,
      originalError: error,
      provider: determineProviderFromError(error, errorMessage)
    });
  }
  
  // Check for authentication errors
  if (errorMessage.includes('authentication') ||
      errorMessage.includes('API key') ||
      errorMessage.includes('apiKey') ||
      errorMessage.toLowerCase().includes('auth') ||
      errorMessage.includes('credential') ||
      errorMessage.includes('permission')) {
    
    return new AuthenticationError({
      message: errorMessage,
      provider: determineProviderFromError(error, errorMessage),
      originalError: error
    });
  }
  
  // Check for model not found errors
  if ((errorMessage.includes('Model') || errorMessage.includes('model')) &&
      (errorMessage.includes('not found') || errorMessage.includes('unavailable') || 
       errorMessage.includes('doesn\'t exist'))) {
    
    // Try to extract model ID
    const modelIdMatch = errorMessage.match(/['"]([\w\d\-\/:.]+)['"]/);
    const modelId = modelIdMatch ? modelIdMatch[1] : 'unknown';
    
    return new ModelNotFoundError({
      message: errorMessage,
      provider: determineProviderFromError(error, errorMessage),
      modelId,
      originalError: error
    });
  }
  
  // Check for tool execution errors
  if (errorMessage.includes('tool execution') ||
      errorMessage.includes('Error executing tool') ||
      errorMessage.includes('ToolExecutionError') ||
      errorMessage.includes('Tool invocation') ||
      errorMessage.includes('AI_ToolExecutionError')) {
    
    // Try to extract tool name
    const toolNameMatch = errorMessage.match(/tool ['"]([\w\d\-_]+)['"]/i) || 
                           errorMessage.match(/tool ([\w\d\-_]+):/i);
    const toolName = toolNameMatch ? toolNameMatch[1] : 'unknown';
    
    // Check for resource not found or GitHub specific errors that should be non-fatal
    if (errorMessage.toLowerCase().includes('not found') || 
        errorMessage.toLowerCase().includes('resource not found') ||
        errorMessage.toLowerCase().includes('no such file') ||
        errorMessage.toLowerCase().includes('404')) {
      
      // Try to extract resource path if available
      const resourceMatch = errorMessage.match(/['"]([\w\d\-\/:.]+)['"]/);
      const resourcePath = resourceMatch ? resourceMatch[1] : undefined;
      
      return new ResourceNotFoundError({
        message: errorMessage,
        toolName,
        resourcePath,
        originalError: error,
        nonFatal: true
      });
    }
    
    // For GitHub authentication errors
    if (errorMessage.toLowerCase().includes('bad credentials') ||
        errorMessage.toLowerCase().includes('authentication failed')) {
      return new ToolAuthenticationError({
        message: errorMessage,
        toolName,
        originalError: error
      });
    }
    
    // Default tool execution error
    return new ToolExecutionError({
      message: errorMessage,
      toolName,
      originalError: error
    });
  }
  
  // Check for network errors
  if (errorMessage.includes('fetch') ||
      errorMessage.includes('network') ||
      errorMessage.includes('connection') ||
      errorMessage.includes('ECONNREFUSED') ||
      errorMessage.includes('ENOTFOUND') ||
      errorMessage.includes('timeout')) {
    
    // Determine the provider for the network error
    const provider = determineProviderFromError(error, errorMessage);
    
    if (errorMessage.includes('timeout')) {
      return new ConnectionTimeoutError({
        message: errorMessage,
        provider,
        originalError: error
      });
    }
    
    return new NetworkError({
      message: errorMessage,
      provider,
      originalError: error
    });
  }
  
  // Check for validation errors
  if (errorMessage.includes('validation') ||
      errorMessage.includes('ValidationError') ||
      errorMessage.includes('invalid') ||
      errorMessage.includes('schema')) {
    
    return new ValidationError({
      message: errorMessage,
      originalError: error
    });
  }
  
  // If we can't determine a specific error type, use the fallback
  return new UnknownError({
    message: errorMessage,
    originalError: error
  });
}

/**
 * Attempt to determine the provider from error information
 */
function determineProviderFromError(error: unknown, errorMessage: string): ProviderType {
  // Check for provider names in error message
  if (errorMessage.toLowerCase().includes('anthropic')) {
    return 'anthropic';
  } else if (errorMessage.toLowerCase().includes('openai')) {
    return 'openai';
  } else if (errorMessage.toLowerCase().includes('google')) {
    return 'google';
  } else if (errorMessage.toLowerCase().includes('openrouter')) {
    return 'openrouter';
  } else if (errorMessage.toLowerCase().includes('ollama')) {
    return 'ollama';
  } else if (errorMessage.toLowerCase().includes('lmstudio')) {
    return 'lmstudio';
  }
  
  // Check for claude model mentions
  if (errorMessage.includes('claude')) {
    return 'anthropic';
  }
  
  // Check for gpt model mentions
  if (errorMessage.includes('gpt-')) {
    return 'openai';
  }
  
  // If error is an object with provider info
  if (typeof error === 'object' && error !== null) {
    const errorObj = error as any;
    if (errorObj.provider) {
      return errorObj.provider as ProviderType;
    }
  }
  
  return 'unknown';
}

/**
 * Transform Anthropic-specific errors
 */
export function transformAnthropicError(error: unknown): ProviderError {
  // Parse error message and response
  let errorMessage = '';
  let statusCode: number | undefined;
  let errorType: string | undefined;
  
  if (error instanceof Error) {
    errorMessage = error.message;
    
    // Try to extract response info if available
    const errorObj = error as any;
    if (errorObj.status) {
      statusCode = errorObj.status;
    }
    if (errorObj.type) {
      errorType = errorObj.type;
    }
  } else if (typeof error === 'object' && error !== null) {
    const errorObj = error as any;
    
    // Extract from Anthropic error format
    errorMessage = errorObj.message || JSON.stringify(error);
    statusCode = errorObj.status || errorObj.statusCode;
    errorType = errorObj.type || errorObj.error?.type;
  } else {
    errorMessage = String(error);
  }
  
  // Map to appropriate error types based on Anthropic's error classification
  
  // Authentication errors
  if (statusCode === 401 || 
      errorType === 'authentication_error' || 
      errorMessage.includes('API key')) {
    return new AuthenticationError({
      message: errorMessage,
      provider: 'anthropic',
      statusCode,
      originalError: error
    });
  }
  
  // Rate limit errors
  if (statusCode === 429 || 
      errorType === 'rate_limit_error' || 
      errorMessage.includes('rate limit')) {
    return new RateLimitError({
      message: errorMessage,
      provider: 'anthropic',
      statusCode,
      originalError: error
    });
  }
  
  // Model not found/invalid
  if (statusCode === 404 && errorMessage.includes('model')) {
    // Try to extract model ID
    const modelIdMatch = errorMessage.match(/['"]([\w\d\-\/:.]+)['"]/);
    const modelId = modelIdMatch ? modelIdMatch[1] : 'unknown';
    
    return new ModelNotFoundError({
      message: errorMessage,
      provider: 'anthropic',
      statusCode,
      modelId,
      originalError: error
    });
  }
  
  // Invalid request format
  if (statusCode === 400 || errorType === 'invalid_request_error') {
    return new InvalidRequestError({
      message: errorMessage,
      provider: 'anthropic',
      statusCode,
      originalError: error
    });
  }
  
  // Context length errors (special handling for Anthropic format)
  if (errorMessage.includes('context_length_exceeded') || 
      errorMessage.includes('content length') ||
      errorMessage.includes('token limit')) {
    
    return new ContextLengthError({
      message: errorMessage,
      provider: 'anthropic',
      originalError: error
    });
  }
  
  // Service unavailable
  if (statusCode === 503 || statusCode === 502) {
    return new ServiceUnavailableError({
      message: errorMessage,
      provider: 'anthropic',
      statusCode,
      originalError: error
    });
  }
  
  // Fallback to generic provider error
  return new ProviderError({
    message: errorMessage,
    provider: 'anthropic',
    statusCode,
    originalError: error
  });
}

/**
 * Transform Google-specific errors
 */
export function transformGoogleError(error: unknown): ProviderError {
  // Similar implementation for Google errors
  let errorMessage = '';
  let statusCode: number | undefined;
  let errorType: string | undefined;
  
  if (error instanceof Error) {
    errorMessage = error.message;
    
    // Try to extract response info if available
    const errorObj = error as any;
    if (errorObj.status) {
      statusCode = errorObj.status;
    }
    if (errorObj.type) {
      errorType = errorObj.type;
    }
  } else if (typeof error === 'object' && error !== null) {
    const errorObj = error as any;
    
    // Extract from Google error format
    errorMessage = errorObj.message || JSON.stringify(error);
    statusCode = errorObj.status || errorObj.statusCode;
    errorType = errorObj.type || errorObj.error?.type;
  } else {
    errorMessage = String(error);
  }
  
  // Map to appropriate error types based on Google's error classification
  
  // Authentication errors
  if (statusCode === 401 || errorMessage.includes('API key') || errorMessage.includes('auth')) {
    return new AuthenticationError({
      message: errorMessage,
      provider: 'google',
      statusCode,
      originalError: error
    });
  }
  
  // Permission errors (Google specific)
  if (statusCode === 403 || errorMessage.includes('permission')) {
    return new AuthenticationError({
      message: errorMessage,
      provider: 'google',
      statusCode,
      originalError: error,
      userMessage: 'Permission denied. Your Google API key may not have access to Gemini models.'
    });
  }
  
  // Rate limit errors
  if (statusCode === 429 || errorMessage.includes('quota') || errorMessage.includes('rate limit')) {
    return new RateLimitError({
      message: errorMessage,
      provider: 'google',
      statusCode,
      originalError: error
    });
  }
  
  // Model not found
  if ((statusCode === 404 || statusCode === 400) && errorMessage.includes('model')) {
    // Try to extract model ID
    const modelIdMatch = errorMessage.match(/['"]([\w\d\-\/:.]+)['"]/);
    const modelId = modelIdMatch ? modelIdMatch[1] : 'unknown';
    
    return new ModelNotFoundError({
      message: errorMessage,
      provider: 'google',
      statusCode,
      modelId,
      originalError: error
    });
  }
  
  // Invalid request
  if (statusCode === 400) {
    return new InvalidRequestError({
      message: errorMessage,
      provider: 'google',
      statusCode,
      originalError: error
    });
  }
  
  // Service unavailable
  if (statusCode === 503 || statusCode === 502) {
    return new ServiceUnavailableError({
      message: errorMessage,
      provider: 'google',
      statusCode,
      originalError: error
    });
  }
  
  // Fallback to generic provider error
  return new ProviderError({
    message: errorMessage,
    provider: 'google',
    statusCode,
    originalError: error
  });
}

/**
 * Transform OpenRouter-specific errors
 */
export function transformOpenRouterError(error: unknown): ProviderError {
  // Similar implementation for OpenRouter errors
  let errorMessage = '';
  let statusCode: number | undefined;
  let errorType: string | undefined;
  
  if (error instanceof Error) {
    errorMessage = error.message;
    
    // Try to extract response info if available
    const errorObj = error as any;
    if (errorObj.status) {
      statusCode = errorObj.status;
    }
    if (errorObj.type) {
      errorType = errorObj.type;
    }
  } else if (typeof error === 'object' && error !== null) {
    const errorObj = error as any;
    
    // Extract from OpenRouter error format
    errorMessage = errorObj.message || errorObj.error?.message || JSON.stringify(error);
    statusCode = errorObj.status || errorObj.statusCode || errorObj.error?.status;
    errorType = errorObj.type || errorObj.error?.type;
  } else {
    errorMessage = String(error);
  }
  
  // Map to appropriate error types based on OpenRouter's error classification
  
  // Authentication errors
  if (statusCode === 401 || errorMessage.includes('API key') || errorMessage.includes('authentication')) {
    return new AuthenticationError({
      message: errorMessage,
      provider: 'openrouter',
      statusCode,
      originalError: error
    });
  }
  
  // Rate limit errors
  if (statusCode === 429 || errorMessage.includes('rate limit')) {
    return new RateLimitError({
      message: errorMessage,
      provider: 'openrouter',
      statusCode,
      originalError: error
    });
  }
  
  // Insufficient credits
  if (errorMessage.includes('credits') || errorMessage.includes('billing')) {
    return new ResourceExhaustedError({
      message: errorMessage,
      provider: 'openrouter',
      statusCode,
      originalError: error,
      userMessage: 'OpenRouter account has insufficient credits. Please check your billing status.'
    });
  }
  
  // Model not found or unavailable
  if ((statusCode === 404 || statusCode === 400) && 
      (errorMessage.includes('model') || errorMessage.includes('route'))) {
    // Try to extract model ID
    const modelIdMatch = errorMessage.match(/['"]([\w\d\-\/:.]+)['"]/);
    const modelId = modelIdMatch ? modelIdMatch[1] : 'unknown';
    
    return new ModelNotFoundError({
      message: errorMessage,
      provider: 'openrouter',
      statusCode,
      modelId,
      originalError: error
    });
  }
  
  // Invalid request
  if (statusCode === 400) {
    return new InvalidRequestError({
      message: errorMessage,
      provider: 'openrouter',
      statusCode,
      originalError: error
    });
  }
  
  // Service unavailable
  if (statusCode === 503 || statusCode === 502) {
    return new ServiceUnavailableError({
      message: errorMessage,
      provider: 'openrouter',
      statusCode,
      originalError: error
    });
  }
  
  // Fallback to generic provider error
  return new ProviderError({
    message: errorMessage,
    provider: 'openrouter',
    statusCode,
    originalError: error
  });
}

/**
 * Transform Ollama-specific errors
 */
export function transformOllamaError(error: unknown): ProviderError {
  // For Ollama, the error patterns are different as it's a local service
  let errorMessage = '';
  let statusCode: number | undefined;
  
  if (error instanceof Error) {
    errorMessage = error.message;
    
    // Try to extract response info if available
    const errorObj = error as any;
    if (errorObj.status) {
      statusCode = errorObj.status;
    }
  } else if (typeof error === 'object' && error !== null) {
    const errorObj = error as any;
    errorMessage = errorObj.message || errorObj.error || JSON.stringify(error);
    statusCode = errorObj.status || errorObj.statusCode;
  } else {
    errorMessage = String(error);
  }
  
  // Connection errors (common for Ollama since it's a local service)
  if (errorMessage.includes('ECONNREFUSED') || 
      errorMessage.includes('connect') ||
      errorMessage.includes('connection') ||
      errorMessage.includes('unreachable')) {
    
    // Creating a server unreachable error with Ollama provider
    return new ServerUnreachableError({
      message: errorMessage,
      provider: 'ollama',
      userMessage: 'Cannot connect to Ollama server. Please make sure Ollama is running locally.',
      originalError: error
    });
  }
  
  // Model not loaded or not found
  if (errorMessage.includes('model') && 
      (errorMessage.includes('not found') || errorMessage.includes('not loaded'))) {
    
    // Try to extract model ID
    const modelIdMatch = errorMessage.match(/['"]([\w\d\-\/:.]+)['"]/);
    const modelId = modelIdMatch ? modelIdMatch[1] : 'unknown';
    
    return new ModelNotFoundError({
      message: errorMessage,
      provider: 'ollama',
      modelId,
      originalError: error,
      userMessage: `Model "${modelId}" not found in Ollama. You may need to pull it first using 'ollama pull ${modelId}'`
    });
  }
  
  // Context length errors
  if (errorMessage.includes('context') || errorMessage.includes('token')) {
    return new ContextLengthError({
      message: errorMessage,
      provider: 'ollama',
      originalError: error
    });
  }
  
  // Invalid request
  if (statusCode === 400) {
    return new InvalidRequestError({
      message: errorMessage,
      provider: 'ollama',
      statusCode,
      originalError: error
    });
  }
  
  // Fallback to generic provider error
  return new ProviderError({
    message: errorMessage,
    provider: 'ollama',
    originalError: error
  });
}

/**
 * Transform tool execution errors
 */
export function transformToolError(error: unknown, toolName?: string): ToolError {
  let errorMessage = '';
  let errorType: string | undefined;
  
  if (error instanceof Error) {
    errorMessage = error.message;
    errorType = error.name;
  } else if (typeof error === 'object' && error !== null) {
    const errorObj = error as any;
    errorMessage = errorObj.message || JSON.stringify(error);
    errorType = errorObj.name || errorObj.type;
  } else {
    errorMessage = String(error);
  }
  
  // If it's already a ToolError, return it
  if (error instanceof ToolError) {
    return error;
  }
  
  // Use provided toolName or extract from error message
  const effectiveToolName = toolName || extractToolNameFromError(errorMessage);
  
  // First check for GitHub specific errors
  if (errorMessage.toLowerCase().includes('not found') || 
      errorMessage.toLowerCase().includes('404') ||
      errorMessage.toLowerCase().includes('resource not found') ||
      errorMessage.toLowerCase().includes('no such file')) {
    
    // Try to extract the resource path from the error
    const resourceMatch = extractResourcePathFromError(errorMessage);
    
    return new ResourceNotFoundError({
      message: errorMessage,
      toolName: effectiveToolName,
      resourcePath: resourceMatch,
      originalError: error,
      nonFatal: true // Important: make this non-fatal so the LLM can handle it
    });
  }
  
  // Auth errors
  if (errorMessage.includes('authentication') || 
      errorMessage.includes('auth') || 
      errorMessage.includes('credentials') || 
      errorMessage.includes('API key') ||
      errorMessage.includes('permission') ||
      errorMessage.includes('Bad credentials')) {
    
    return new ToolAuthenticationError({
      message: errorMessage,
      toolName: effectiveToolName,
      originalError: error
    });
  }
  
  // Tool not found
  if (errorMessage.includes('not found') || errorMessage.includes('unknown tool')) {
    return new ToolNotFoundError({
      message: errorMessage,
      toolName: effectiveToolName,
      originalError: error,
      nonFatal: true
    });
  }
  
  // Arguments errors
  if (errorMessage.includes('argument') || 
      errorMessage.includes('parameter') || 
      errorMessage.includes('invalid') ||
      errorMessage.includes('validation')) {
    
    // Try to extract the invalid argument name
    const argMatch = errorMessage.match(/argument ['"]([\w\d\-_]+)['"]/i) || 
                     errorMessage.match(/parameter ['"]([\w\d\-_]+)['"]/i);
    const invalidArgument = argMatch ? argMatch[1] : undefined;
    
    return new ToolArgumentError({
      message: errorMessage,
      toolName: effectiveToolName,
      originalError: error,
      invalidArgument,
      nonFatal: true
    });
  }
  
  // Timeout errors
  if (errorMessage.includes('timeout') || errorMessage.includes('timed out')) {
    return new ToolTimeoutError({
      message: errorMessage,
      toolName: effectiveToolName,
      originalError: error,
      nonFatal: true
    });
  }
  
  // Default to generic tool execution error
  return new ToolExecutionError({
    message: errorMessage,
    toolName: effectiveToolName,
    originalError: error
  });
}

/**
 * Extract tool name from error message
 */
function extractToolNameFromError(errorMessage: string): string {
  const toolNameMatch = errorMessage.match(/tool ['"]([\w\d\-_]+)['"]/i) || 
                         errorMessage.match(/tool ([\w\d\-_]+):/i) ||
                         errorMessage.match(/executing ([\w\d\-_]+)/i);
                         
  return toolNameMatch ? toolNameMatch[1] : 'unknown';
}

/**
 * Extract resource path from error message
 */
function extractResourcePathFromError(errorMessage: string): string | undefined {
  // Try different patterns to extract file or resource path
  
  // Check for quotes around path
  const quotedMatch = errorMessage.match(/['"]([^'"]+\.[\w]+)['"]/) || 
                      errorMessage.match(/['"]([\/\w\d\-_.]+)['"]/) ||
                      errorMessage.match(/No such file or directory: ['"]([^'"]+)['"]/);
  
  if (quotedMatch && quotedMatch[1]) {
    return quotedMatch[1];
  }
  
  // Check for explicit file path mentions
  const fileMatch = errorMessage.match(/file not found: ([\w\d\-\/_.]+)/) ||
                    errorMessage.match(/path ([\w\d\-\/_.]+) not found/) ||
                    errorMessage.match(/resource ([\w\d\-\/_.]+) not found/);
  
  if (fileMatch && fileMatch[1]) {
    return fileMatch[1];
  }
  
  // Check for path-like structures
  const pathMatch = errorMessage.match(/[\w\d]+\/[\w\d\-]+\/[\w\d\-_.\/]+/);
  
  if (pathMatch) {
    return pathMatch[0];
  }
  
  return undefined;
}