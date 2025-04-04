/**
 * Error formatting utilities for server responses
 */

import {
  ChatError,
  ErrorResponse,
  transformUnknownError,
  ToolError,
  ContextLengthError,
  ApiRateLimitError,
  AuthenticationError,
  ModelNotFoundError
} from '@openagents/core/src/chat/errors';

/**
 * Format an error for SSE streaming
 */
export function formatErrorForStream(error: unknown): string {
  // Convert unknown errors to ChatError first
  const chatError = error instanceof ChatError 
    ? error 
    : transformUnknownError(error);
  
  // Return stream format
  return chatError.toStreamFormat();
}

/**
 * Format an error for JSON response
 */
export function formatErrorForJsonResponse(error: unknown): {
  error: string;
  status: number;
  details?: unknown;
} {
  // Convert unknown errors to ChatError first
  const chatError = error instanceof ChatError 
    ? error 
    : transformUnknownError(error);
  
  // Determine HTTP status code based on error type
  let status = 500;
  
  if (chatError instanceof AuthenticationError) {
    status = 401;
  } else if (chatError instanceof ApiRateLimitError) {
    status = 429;
  } else if (chatError instanceof ModelNotFoundError) {
    status = 404;
  } else if (chatError.category === 'validation') {
    status = 400;
  }
  
  return {
    error: chatError.userMessage,
    status,
    details: process.env.NODE_ENV === 'development' ? chatError.message : undefined
  };
}

/**
 * Create a user-friendly error message
 * This is a wrapper around the ChatError's userMessage that adds specific formatting
 */
export function createUserFriendlyErrorMessage(error: unknown): string {
  // Convert unknown errors to ChatError first
  const chatError = error instanceof ChatError 
    ? error 
    : transformUnknownError(error);
  
  // For tool errors, provide more specific guidance
  if (chatError instanceof ToolError) {
    return `Error with tool '${chatError.toolName}': ${chatError.userMessage}`;
  }
  
  // For context length errors, provide more specific guidance
  if (chatError instanceof ContextLengthError) {
    let message = chatError.userMessage;
    
    // Add recommendation to use a model with larger context
    if (!message.includes('larger context')) {
      message += ' Try starting a new conversation or using a model with a larger context window.';
    }
    
    return message;
  }
  
  // Return the standard user message
  return chatError.userMessage;
}

/**
 * Format tool execution error for tool result
 */
export function formatToolExecutionErrorForResult(error: unknown, toolName: string): string {
  // Convert unknown errors to ChatError first
  const chatError = error instanceof ChatError 
    ? error 
    : transformUnknownError(error);
  
  // If it's already a tool error, use its formatting
  if (chatError instanceof ToolError) {
    return chatError.toToolResult();
  }
  
  // Format as a generic tool error
  return `Error executing tool ${toolName}: ${chatError.userMessage}`;
}