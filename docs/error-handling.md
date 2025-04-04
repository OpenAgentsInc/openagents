# Error Handling in OpenAgents

This document provides a guide to error handling in the OpenAgents application, specifically focusing on handling errors from LLM API calls and displaying them to users.

## Overview

The OpenAgents application is designed to gracefully handle errors from various sources, particularly from LLM API calls (like OpenAI, Anthropic, OpenRouter, and local models like Ollama). When errors occur, they are:

1. Logged to the console for debugging
2. Processed into user-friendly messages
3. Displayed directly in the chat interface
4. Optionally stored in the database for persistence

## Error Types

The application has a comprehensive error type system with the following categories:

- **Provider Errors**: Issues related to external API providers
  - `AuthenticationError`: When API keys are missing or invalid
  - `RateLimitError`: When API rate limits are reached
  - `ModelNotFoundError`: When a requested model doesn't exist
  - `ServiceUnavailableError`: When the provider service is down
  - `ResourceExhaustedError`: When provider quota is exceeded

- **Validation Errors**: Issues with input validation
  - `MessageValidationError`: When messages have invalid format
  - `ApiKeyValidationError`: When API keys are missing
  - `ModelValidationError`: When model settings are invalid
  - `SchemaValidationError`: When request schema is invalid

- **Tool Errors**: Issues with tool execution
  - `ToolExecutionError`: When tool execution fails
  - `ToolAuthenticationError`: When tool authentication fails
  - `ToolArgumentError`: When tool arguments are invalid
  - `ToolTimeoutError`: When tool execution times out

- **Network Errors**: Connection and communication issues
  - `ConnectionTimeoutError`: When connections time out
  - `ServerUnreachableError`: When servers cannot be reached
  - `HttpStatusError`: When HTTP requests fail with status codes

- **Limit Errors**: Constraints and quota issues
  - `ContextLengthError`: When the conversation exceeds context length
  - `ApiRateLimitError`: When API rate limits are exceeded
  - `TokenQuotaError`: When token quotas are exceeded

## Implementation

### Backend Error Handling

#### 1. Core Error System

The core error system is implemented in `packages/core/src/chat/errors/` with a class hierarchy:

```typescript
// Base error class for all chat-related errors
export class ChatError extends Error {
  public readonly category: ErrorCategory;
  public readonly severity: ErrorSeverity;
  public readonly userMessage: string;
  
  // Format error for client consumption
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
  
  // Format for SSE streams
  toStreamFormat(): string {
    return `data: error:${JSON.stringify(this.toClientFormat())}\n\n`;
  }
}
```

#### 2. Error Transformation

Provider-specific errors are transformed into our typed system using dedicated transformers:

```typescript
// Transform Anthropic-specific errors
export function transformAnthropicError(error: unknown): ProviderError {
  // Extract error details from Anthropic format
  
  // Map to appropriate error types
  if (statusCode === 401) {
    return new AuthenticationError({
      message: errorMessage,
      provider: 'anthropic',
      statusCode
    });
  }
  
  // Handle other error types...
}
```

#### 3. Stream Error Handling

Errors in the chat stream are handled with a consistent pattern:

```typescript
try {
  // Create and process the stream
  const streamResult = await streamManager.createStream(provider, messages, options);
  return streamManager.createStreamResponse(c, streamResult);
} catch (error) {
  // Transform error and return error stream
  const chatError = error instanceof ChatError 
    ? error 
    : transformUnknownError(error);
  return streamManager.handleStreamError(chatError, c);
}
```

#### 4. Error Recovery

For certain errors, recovery mechanisms attempt to continue the conversation:

```typescript
// Clean up messages with failed tool calls
export async function cleanupMessagesWithFailedToolCalls(
  messagesWithToolCalls: Message[],
  error?: unknown
): Promise<Message[]> {
  // Remove problematic messages and add error explanation
  // Create a modified system message with error information
  // Return cleaned messages for continued conversation
}
```

### Client-Side Error Handling

#### 1. Error Display in Chat UI

Errors are displayed directly in the chat interface as system messages with a yellow warning style:

```tsx
// Display error in chat as system message
const errorSystemMessage = {
  id: `error-${Date.now()}`,
  role: 'system',
  content: `⚠️ Error: ${userFriendlyError}`,
  createdAt: new Date(),
};

// Add the error message to the chat
append(errorSystemMessage);
```

#### 2. Error Parsing from SSE Stream

The client parses error messages from the server's SSE stream:

```typescript
// Check if this is our special error format
if (chunk.startsWith("data: error:")) {
  const errorData = JSON.parse(chunk.substring("data: error:".length));
  
  // Display error message to user
  const errorSystemMessage = {
    id: `error-${Date.now()}`,
    role: 'system',
    content: `⚠️ Error: ${errorData.message}`,
    createdAt: new Date(),
  };
  
  append(errorSystemMessage);
  return;
}
```

#### 3. Error Handling in Components

In page components (like `HomePage.tsx`), the error handling is set up in the `usePersistentChat` hook's options:

```typescript
const {
  messages,
  append,
  // other properties...
} = usePersistentChat({
  // Configuration...
  onError: (error) => {
    console.error('Chat hook onError:', error);
    
    // Get user-friendly error message
    const userFriendlyError = createUserFriendlyErrorMessage(error);
    
    // Display in chat
    const errorSystemMessage = {
      id: `error-${Date.now()}`,
      role: 'system',
      content: `⚠️ Error: ${userFriendlyError}`,
      createdAt: new Date(),
    };
    
    append(errorSystemMessage);
  }
});
```

## UI Components for Error Display

The `ChatMessage` component (`apps/coder/src/components/ui/chat-message.tsx`) includes special styling for system/error messages:

```typescript
// Style variant for system messages
isSystem: {
  true: "border-yellow-500 border bg-yellow-50 dark:bg-yellow-900/20 text-yellow-800 dark:text-yellow-200 w-full",
  false: "",
},

// System message rendering
if (isSystem) {
  return (
    <div className="flex flex-col items-center w-full">
      <div className={cn(chatBubbleVariants({ isUser: false, isSystem: true, animation }))}>
        <MarkdownRenderer>{content}</MarkdownRenderer>
      </div>
      {/* Timestamp if needed */}
    </div>
  )
}
```

## Best Practices

1. **Use typed errors**: Always use the proper error class for the situation
2. **Include user-friendly messages**: Provide clear guidance about what went wrong
3. **Add recovery suggestions**: When possible, suggest how to fix the issue
4. **Consistent formatting**: Use system role and warning style for all error messages
5. **Log for debugging**: Include detailed technical information in console logs
6. **Handle provider-specific errors**: Use dedicated transformers for different providers
7. **Implement recovery mechanisms**: Try to recover from non-fatal errors

## Testing Error Scenarios

To test error handling, try these scenarios:

- Set an invalid API key in Settings
- Try a conversation that exceeds the model's context length
- Request a non-existent model
- Disconnect from the network while generating
- Use a model with smaller context window than needed
- Try using a tool with invalid permissions
- Execute a shell command that produces an error