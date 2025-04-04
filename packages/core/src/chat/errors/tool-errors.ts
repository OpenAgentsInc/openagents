/**
 * Tool execution error classes
 */

import { ChatError, ChatErrorOptions } from './base-error';

// Tool error options
export interface ToolErrorOptions extends Omit<ChatErrorOptions, 'category'> {
  toolName: string;
  toolType?: string;
  arguments?: Record<string, unknown>;
  invocationId?: string;
  // When true, the error is non-fatal and should be returned as a result to the model
  nonFatal?: boolean;
}

/**
 * Base class for tool execution errors
 */
export class ToolError extends ChatError {
  public readonly toolName: string;
  public readonly toolType?: string;
  public readonly arguments?: Record<string, unknown>;
  public readonly invocationId?: string;
  public readonly nonFatal: boolean;
  
  constructor(options: ToolErrorOptions) {
    super({
      ...options,
      category: 'tool',
    });
    this.name = 'ToolError';
    this.toolName = options.toolName;
    this.toolType = options.toolType;
    this.arguments = options.arguments;
    this.invocationId = options.invocationId;
    this.nonFatal = options.nonFatal || false;
    
    // Add tool info to metadata
    this.metadata.toolName = options.toolName;
    if (options.toolType) this.metadata.toolType = options.toolType;
    if (options.invocationId) this.metadata.invocationId = options.invocationId;
    if (options.nonFatal) this.metadata.nonFatal = options.nonFatal;
    
    // Only include arguments if they're safe to serialize
    if (options.arguments) {
      try {
        // Test if we can stringify the arguments
        JSON.stringify(options.arguments);
        this.metadata.arguments = options.arguments;
      } catch (e) {
        // If not, just note that arguments were provided but not serializable
        this.metadata.arguments = { _note: "Arguments provided but not serializable" };
      }
    }
  }
  
  /**
   * Format the error as a tool invocation result for AI models
   */
  toToolResult(): string {
    return `Error executing tool ${this.toolName}: ${this.userMessage}`;
  }
}

/**
 * Tool not found errors
 */
export class ToolNotFoundError extends ToolError {
  constructor(options: ToolErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Tool "${options.toolName}" not found or unavailable. The model attempted to use a tool that doesn't exist.`,
      // Resource not found errors should be non-fatal by default
      nonFatal: options.nonFatal !== undefined ? options.nonFatal : true
    });
    this.name = 'ToolNotFoundError';
  }
}

/**
 * Resource not found errors (file not found, repo not found, etc.)
 */
export class ResourceNotFoundError extends ToolError {
  public readonly resourcePath?: string;
  
  constructor(options: ToolErrorOptions & { resourcePath?: string }) {
    super({
      ...options,
      userMessage: options.userMessage || 
        options.resourcePath
          ? `Resource not found: ${options.resourcePath}`
          : `Resource not found`,
      // Resource not found errors should be non-fatal by default
      nonFatal: options.nonFatal !== undefined ? options.nonFatal : true
    });
    this.name = 'ResourceNotFoundError';
    this.resourcePath = options.resourcePath;
    if (options.resourcePath) this.metadata.resourcePath = options.resourcePath;
  }
}

/**
 * Tool authentication errors
 */
export class ToolAuthenticationError extends ToolError {
  constructor(options: ToolErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Authentication failed for tool "${options.toolName}". Please check your credentials in Settings.`,
      // Auth errors are fatal by default as they likely affect all tool calls
      nonFatal: options.nonFatal !== undefined ? options.nonFatal : false
    });
    this.name = 'ToolAuthenticationError';
  }
}

/**
 * Tool argument validation errors
 */
export class ToolArgumentError extends ToolError {
  public readonly invalidArgument?: string;
  
  constructor(options: ToolErrorOptions & { invalidArgument?: string }) {
    super({
      ...options,
      userMessage: options.userMessage || 
        options.invalidArgument
          ? `Invalid argument "${options.invalidArgument}" for tool "${options.toolName}".`
          : `Invalid arguments for tool "${options.toolName}". Please check the tool requirements and try again.`,
      // Argument errors are non-fatal by default as the model can retry with different args
      nonFatal: options.nonFatal !== undefined ? options.nonFatal : true
    });
    this.name = 'ToolArgumentError';
    this.invalidArgument = options.invalidArgument;
    if (options.invalidArgument) this.metadata.invalidArgument = options.invalidArgument;
  }
}

/**
 * Tool execution timeout errors
 */
export class ToolTimeoutError extends ToolError {
  public readonly timeoutMs?: number;
  
  constructor(options: ToolErrorOptions & { timeoutMs?: number }) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Tool "${options.toolName}" execution timed out${options.timeoutMs ? ` after ${options.timeoutMs}ms` : ''}.`,
      // Timeout errors are non-fatal by default
      nonFatal: options.nonFatal !== undefined ? options.nonFatal : true
    });
    this.name = 'ToolTimeoutError';
    this.timeoutMs = options.timeoutMs;
    if (options.timeoutMs) this.metadata.timeoutMs = options.timeoutMs;
  }
}

/**
 * Tool permission errors
 */
export class ToolPermissionError extends ToolError {
  constructor(options: ToolErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Permission denied for tool "${options.toolName}". You may not have the necessary permissions to use this tool.`,
      // Permission errors are usually fatal for that specific tool
      nonFatal: options.nonFatal !== undefined ? options.nonFatal : false
    });
    this.name = 'ToolPermissionError';
  }
}

/**
 * Tool execution errors (general execution failures)
 */
export class ToolExecutionError extends ToolError {
  constructor(options: ToolErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Error executing tool "${options.toolName}". The tool encountered an error during execution.`,
      // General execution errors could be either fatal or non-fatal
      nonFatal: options.nonFatal !== undefined ? options.nonFatal : false
    });
    this.name = 'ToolExecutionError';
  }
}

/**
 * MCP client errors (specific to MCP tool execution)
 */
export class MCPClientError extends ToolError {
  public readonly clientId?: string;
  
  constructor(options: ToolErrorOptions & { clientId?: string }) {
    super({
      ...options,
      userMessage: options.userMessage || 
        options.clientId
          ? `MCP client "${options.clientId}" error while executing tool "${options.toolName}".`
          : `MCP client error while executing tool "${options.toolName}".`,
      // Client errors are handled based on error specifics
      nonFatal: options.nonFatal !== undefined ? options.nonFatal : false
    });
    this.name = 'MCPClientError';
    this.clientId = options.clientId;
    if (options.clientId) this.metadata.clientId = options.clientId;
  }
}