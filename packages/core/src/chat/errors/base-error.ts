/**
 * Base error class for all chat-related errors
 */

// Error categories
export type ErrorCategory = 
  | 'provider'     // Provider-specific errors (authentication, rate limits, etc.)
  | 'validation'   // Input validation errors
  | 'tool'         // Tool execution errors
  | 'network'      // Network and connectivity errors
  | 'limit'        // Context window and rate limit errors
  | 'system'       // Internal system errors
  | 'unknown';     // Unknown or uncategorized errors

// Error severity levels
export type ErrorSeverity = 
  | 'fatal'        // Unrecoverable errors that terminate the request
  | 'error'        // Standard errors that should be reported to the user
  | 'warning'      // Issues that don't prevent operation but should be noted
  | 'info';        // Informational messages about non-critical issues

// Options for creating chat errors
export interface ChatErrorOptions {
  message: string;          // Technical error message (for logs)
  userMessage?: string;     // User-friendly message (for display)
  category?: ErrorCategory; // Error category
  severity?: ErrorSeverity; // Error severity
  originalError?: unknown;  // Original error that caused this one
  metadata?: Record<string, unknown>; // Additional error metadata
}

// Client-facing error response format
export interface ErrorResponse {
  error: true;
  category: ErrorCategory;
  message: string;
  details: string;
  severity: ErrorSeverity;
  timestamp: number;
  metadata?: Record<string, unknown>;
}

/**
 * Base error class for all chat-related errors in the system
 */
export class ChatError extends Error {
  public readonly category: ErrorCategory;
  public readonly severity: ErrorSeverity;
  public readonly userMessage: string;
  public readonly originalError?: unknown;
  public readonly metadata: Record<string, unknown>;
  
  constructor(options: ChatErrorOptions) {
    super(options.message);
    this.name = this.constructor.name;
    this.category = options.category || 'unknown';
    this.severity = options.severity || 'error';
    this.userMessage = options.userMessage || options.message;
    this.originalError = options.originalError;
    this.metadata = options.metadata || {};
    
    // Maintain proper stack trace
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, this.constructor);
    }
  }
  
  /**
   * Format the error for client consumption
   */
  toClientFormat(): ErrorResponse {
    return {
      error: true,
      category: this.category,
      message: this.userMessage,
      details: this.message,
      severity: this.severity,
      timestamp: Date.now(),
      metadata: Object.keys(this.metadata).length > 0 ? this.metadata : undefined
    };
  }
  
  /**
   * Format the error for error stream in the SSE format
   */
  toStreamFormat(): string {
    return `data: error:${JSON.stringify(this.toClientFormat())}\n\n`;
  }
  
  /**
   * Format the error for legacy compatibility
   * This maintains backward compatibility with the old error format
   */
  toLegacyFormat(): { error: boolean; message: string; details: string } {
    return {
      error: true,
      message: this.userMessage,
      details: this.message
    };
  }
}

/**
 * System error for internal application errors
 */
export class SystemError extends ChatError {
  constructor(options: Omit<ChatErrorOptions, 'category'>) {
    super({
      ...options,
      category: 'system',
    });
    this.name = 'SystemError';
  }
}

/**
 * Unknown error for unclassified errors
 */
export class UnknownError extends ChatError {
  constructor(options: Omit<ChatErrorOptions, 'category'>) {
    super({
      ...options,
      category: 'unknown',
    });
    this.name = 'UnknownError';
  }
}