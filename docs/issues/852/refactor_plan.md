# Server Refactoring and Error Handling Plan for Issue #852

## Problem Analysis

After a thorough analysis of the current implementation in `apps/coder/src/server/server.ts`, the following problems have been identified:

1. **Monolithic File Structure**: The current server file is over 1100 lines long and handles multiple concerns, including:
   - HTTP server and route configuration
   - API provider configuration and client initialization
   - Message validation and transformation
   - Streaming response handling
   - Error detection, classification, and reporting
   - Tool execution and error handling
   - Recovery mechanisms for failed requests

2. **Error Handling Issues**:
   - String-based error pattern matching is fragile and error-prone
   - Error handling logic is duplicated across different sections of the code
   - Error propagation is inconsistent, with transformations that can lose context
   - Global variables like `(global as any).__lastToolExecutionError` create state management issues
   - Provider-specific error handling is intermixed with general error logic
   - Recovery mechanisms are tightly coupled with error handling
   - Errors are not properly typed, making it difficult to ensure consistent handling

3. **Testability Issues**:
   - The monolithic structure makes unit testing almost impossible
   - Dependencies are not injectable, complicating test environment setup
   - Error simulation cannot be done in isolation from the HTTP layer

## Proposed New File Structure

We'll organize the codebase into a more modular structure, separating concerns and improving testability:

```
apps/coder/src/server/
├── index.ts                  # Main server entry point (exports app)
├── server.ts                 # Simplified server setup (imports modules below)
├── routes/
│   ├── index.ts              # Exports all routes
│   ├── chat.ts               # Chat endpoint implementation
│   └── mcp.ts                # MCP routing (refactored from mcp-api.ts)
├── providers/
│   ├── index.ts              # Provider factory and registry
│   ├── types.ts              # Provider interface definitions
│   ├── anthropic.ts          # Anthropic-specific implementation
│   ├── google.ts             # Google-specific implementation
│   ├── openrouter.ts         # OpenRouter-specific implementation
│   └── ollama.ts             # Ollama-specific implementation
├── streaming/
│   ├── index.ts              # Streaming utilities
│   ├── stream-manager.ts     # Stream creation and error handling
│   └── response-formatter.ts # Format different response types for streaming
├── tools/
│   ├── index.ts              # Tool registration and management
│   ├── shell-command.ts      # Shell command tool implementation
│   └── mcp-tools.ts          # MCP tool integration (refactored from mcp-clients.ts)
├── utils/
│   ├── validation.ts         # Message and input validation
│   ├── api-keys.ts           # API key management
│   └── model-helpers.ts      # Model detection and configuration
└── errors/
    ├── index.ts              # Error handling exports
    ├── error-formatter.ts    # Error message formatting for responses
    └── error-recovery.ts     # Error recovery mechanisms
```

Additionally, we'll create a centralized error class hierarchy in the core package:

```
packages/core/src/chat/errors/
├── index.ts                  # Error class exports
├── base-error.ts             # Base error class with common functionality
├── provider-errors.ts        # Provider-specific error classes
├── validation-errors.ts      # Validation error classes
├── tool-errors.ts            # Tool execution error classes
├── network-errors.ts         # Network related error classes
├── limit-errors.ts           # Rate limit and context limit errors
└── transformers.ts           # Error transformation utilities
```

## Error Class Hierarchy

We'll implement a proper error class hierarchy for better typing and error handling:

```typescript
// Base error class for all chat-related errors
export class ChatError extends Error {
  public readonly category: ErrorCategory;
  public readonly severity: ErrorSeverity;
  public readonly userMessage: string;
  public readonly originalError?: unknown;
  
  constructor(options: ChatErrorOptions) {
    super(options.message);
    this.name = this.constructor.name;
    this.category = options.category;
    this.severity = options.severity || 'error';
    this.userMessage = options.userMessage || options.message;
    this.originalError = options.originalError;
  }
  
  // Format for client consumption
  toClientFormat(): ErrorResponse {
    return {
      error: true,
      category: this.category,
      message: this.userMessage,
      details: this.message,
      severity: this.severity,
      timestamp: Date.now()
    };
  }
}

// Provider-specific errors
export class ProviderError extends ChatError {
  public readonly provider: string;
  public readonly statusCode?: number;
  
  constructor(options: ProviderErrorOptions) {
    super({
      ...options,
      category: 'provider',
    });
    this.provider = options.provider;
    this.statusCode = options.statusCode;
  }
}

// More specific provider errors
export class AuthenticationError extends ProviderError {...}
export class RateLimitError extends ProviderError {...}
export class ModelNotFoundError extends ProviderError {...}

// Validation errors
export class ValidationError extends ChatError {...}
export class MessageValidationError extends ValidationError {...}
export class ModelValidationError extends ValidationError {...}

// Tool errors
export class ToolExecutionError extends ChatError {
  public readonly toolName: string;
  public readonly arguments?: Record<string, unknown>;
  
  constructor(options: ToolExecutionErrorOptions) {
    super({
      ...options,
      category: 'tool',
    });
    this.toolName = options.toolName;
    this.arguments = options.arguments;
  }
}

// Context limit errors
export class ContextLimitError extends ChatError {...}

// Network errors
export class NetworkError extends ChatError {...}
```

## Error Transformation Strategy

We'll implement consistent error transformation functions:

1. **Provider-specific transformers**:
   - `transformAnthropicError`: Converts Anthropic API errors to our error classes
   - `transformGoogleError`: Converts Google API errors to our error classes
   - `transformOpenRouterError`: Converts OpenRouter errors to our error classes
   - `transformOllamaError`: Converts Ollama errors to our error classes

2. **Generic transformers**:
   - `transformUnknownError`: Converts any unknown error to an appropriate error class
   - `transformNetworkError`: Handles network-related errors consistently
   - `transformValidationError`: Processes validation errors

3. **Stream formatting**:
   - `formatErrorForStream`: Consistently formats any error for SSE streaming

## Refactoring Implementation Plan

### Phase 1: Set Up Core Error System

1. Create the core error classes in `packages/core/src/chat/errors/`:
   - Implement base error classes and hierarchy
   - Develop transformation utilities
   - Create error response formatting functions

2. Create utility functions for error handling:
   - Error classification
   - User-friendly message generation
   - Severity determination

### Phase 2: Break Down Server.ts

1. Extract routing logic:
   - Move the chat endpoint to `routes/chat.ts`
   - Move MCP API endpoints to `routes/mcp.ts`

2. Extract provider initialization:
   - Create provider factory in `providers/index.ts`
   - Implement provider-specific modules

3. Extract streaming logic:
   - Create stream manager in `streaming/stream-manager.ts`
   - Implement response formatters

4. Extract validation logic:
   - Move message validation to `utils/validation.ts`
   - Implement input schema validation

5. Extract tool handling:
   - Move shell command tool to `tools/shell-command.ts`
   - Refactor MCP tool integration

### Phase 3: Implement New Error Handling

1. Replace string-based error detection with proper error instances
2. Implement provider-specific error transformers
3. Create consistent error propagation through the stream
4. Implement centralized error recovery mechanisms
5. Add typed error responses for the client

### Phase 4: Create Simplified Server Entry Point

1. Create a new simplified `server.ts` that uses the modular components
2. Ensure backward compatibility with existing API endpoints
3. Add comprehensive logging for error tracking

## Integration with Client-side Error Handling

We'll ensure the new error system integrates with the existing client-side handling by:

1. Maintaining SSE stream format compatibility
2. Keeping the existing error message format alongside the new typed format
3. Gradually enhancing client-side code to leverage additional error information

## Testing Strategy

The modular structure will allow better testing:

1. Unit tests for individual components:
   - Provider client initialization
   - Error transformation
   - Message validation

2. Integration tests for complete flows:
   - Chat endpoint with mocked provider responses
   - Error handling with simulated errors

3. End-to-end tests for real-world scenarios:
   - Conversations with tool use
   - Error recovery mechanisms

## Implementation Timeline

1. **Day 1-2**: Set up core error system
   - Create error class hierarchy
   - Implement transformation utilities
   - Write basic tests

2. **Day 3-4**: Break down server.ts
   - Extract routing, providers, and tools
   - Create modular file structure
   - Ensure basic functionality works

3. **Day 5-6**: Implement new error handling
   - Replace string matching with proper error classes
   - Implement error propagation
   - Add recovery mechanisms

4. **Day 7**: Integration and testing
   - Ensure all components work together
   - Verify error handling works as expected
   - Document the new architecture

## Benefits of the Refactoring

1. **Improved Maintainability**:
   - Smaller, focused modules are easier to understand and modify
   - Clear separation of concerns makes responsibilities explicit
   - Reduced duplication improves consistency

2. **Better Error Handling**:
   - Typed errors provide compile-time safety
   - Consistent error propagation improves debugging
   - Better user feedback through specific error messages

3. **Enhanced Testability**:
   - Isolated components are easier to test
   - Mocking dependencies becomes simpler
   - Error simulation can be done at various levels

4. **Future Extensibility**:
   - Adding new providers becomes straightforward
   - Error handling can evolve independently of business logic
   - New tools can be integrated more easily