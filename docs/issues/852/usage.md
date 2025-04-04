# Using the New Error Handling System

This guide explains how to use the new error handling system implemented as part of issue #852.

## Core Concepts

The new error handling system is built on a hierarchy of typed error classes, with the base `ChatError` class providing common functionality. Errors are categorized by type (provider, validation, tool, etc.) and include both technical details and user-friendly messages.

## Throwing Errors

### Basic Error Usage

```typescript
import { ValidationError } from '@openagents/core/src/chat/errors';

function validateInput(input: any) {
  if (!input.model) {
    throw new ValidationError({
      message: 'Model ID is required',
      userMessage: 'Please specify a model for your request',
      field: 'model'
    });
  }
}
```

### Provider-Specific Errors

For provider-specific errors, use the appropriate error class:

```typescript
import { AuthenticationError } from '@openagents/core/src/chat/errors';

function validateApiKey(provider: string, apiKey?: string) {
  if (!apiKey || apiKey.length < 10) {
    throw new AuthenticationError({
      message: `Invalid API key for ${provider} provider`,
      provider: provider as any,
      userMessage: `Please add a valid API key for ${provider} in Settings`
    });
  }
}
```

### Tool Execution Errors

For tool-related errors, include the tool name and any relevant arguments:

```typescript
import { ToolExecutionError } from '@openagents/core/src/chat/errors';

async function executeTool(toolName: string, args: any) {
  try {
    // Tool execution logic
  } catch (error) {
    throw new ToolExecutionError({
      message: `Failed to execute tool ${toolName}: ${error.message}`,
      toolName,
      arguments: args,
      userMessage: `The tool ${toolName} encountered an error. Please check your input and try again.`
    });
  }
}
```

## Handling Errors

### Transforming Raw Errors

The system includes transformers for converting raw errors to our typed format:

```typescript
import { transformUnknownError } from '@openagents/core/src/chat/errors';

try {
  // Some operation that might throw
} catch (error) {
  // Transform to our error type
  const chatError = transformUnknownError(error);
  
  // Now you can use the structured error
  console.error(`Error category: ${chatError.category}`);
  console.error(`User message: ${chatError.userMessage}`);
}
```

### Provider-Specific Transformers

For provider API errors, use the dedicated transformers:

```typescript
import { transformAnthropicError } from '@openagents/core/src/chat/errors';

try {
  // Call to Anthropic API
} catch (error) {
  // Transform to a properly typed Anthropic error
  const anthropicError = transformAnthropicError(error);
  
  // Now you can handle specific error types
  if (anthropicError instanceof AuthenticationError) {
    // Handle auth errors
  } else if (anthropicError instanceof RateLimitError) {
    // Handle rate limits
  }
}
```

## Error Formatting

### For API Responses

```typescript
import { formatErrorForJsonResponse } from '@openagents/core/src/server/errors';

app.get('/api/some-endpoint', (c) => {
  try {
    // Endpoint logic
  } catch (error) {
    // Format for JSON response
    const { error: errorMessage, status, details } = formatErrorForJsonResponse(error);
    return c.json({ error: errorMessage, details }, status);
  }
});
```

### For SSE Streams

```typescript
import { formatErrorForStream } from '@openagents/core/src/server/errors';

app.get('/api/stream', (c) => {
  try {
    // Stream setup logic
  } catch (error) {
    // Format for SSE stream
    const errorResponse = formatErrorForStream(error);
    return new Response(errorResponse);
  }
});
```

## Error Recovery

### Cleaning Up Failed Messages

```typescript
import { cleanupMessagesWithFailedToolCalls } from '@openagents/core/src/server/errors';

try {
  // Streaming logic with tool calls
} catch (error) {
  // Clean up messages with failed tool calls
  const cleanedMessages = await cleanupMessagesWithFailedToolCalls(
    messages,
    error,
    { addAssistantMessage: true }
  );
  
  // Continue with cleaned messages
  const recoveryStream = await createStream(cleanedMessages);
}
```

## Advanced Usage: Stream Manager

The `streamManager` component provides a unified way to handle streams and errors:

```typescript
import { streamManager } from '@openagents/core/src/server/streaming';

app.post('/api/chat', async (c) => {
  try {
    // Create the stream
    const streamResult = await streamManager.createStream(provider, messages, options);
    
    // Return the stream response
    return streamManager.createStreamResponse(c, streamResult);
  } catch (error) {
    // Handle errors with proper formatting
    return streamManager.handleStreamError(error, c);
  }
});
```

## Best Practices

1. **Use specific error classes** rather than generic ones when possible
2. **Include both technical and user-friendly messages** in errors
3. **Add recovery suggestions** when appropriate
4. **Categorize errors properly** to enable consistent handling
5. **Transform errors early** in the call stack to get typed benefits
6. **Include relevant context** like provider names, model IDs, etc.
7. **Log technical details** for debugging while showing user-friendly messages in the UI