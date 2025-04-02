# Error Handling in OpenAgents

This document provides a guide to error handling in the OpenAgents application, specifically focusing on handling errors from LLM API calls and displaying them to users.

## Overview

The OpenAgents application is designed to gracefully handle errors from various sources, particularly from LLM API calls (like OpenAI, Anthropic, OpenRouter, and local models like LMStudio and Ollama). When errors occur, they are:

1. Logged to the console for debugging
2. Processed into user-friendly messages
3. Displayed directly in the chat interface
4. Optionally stored in the database for persistence

## Error Types

Common error types include:

- **Context length exceeded**: When the conversation becomes too long for the model's context window
- **Rate limit exceeded**: When API rate limits are reached
- **Authentication errors**: When API keys are missing or invalid
- **Model unavailability**: When a selected model doesn't exist or is unavailable
- **Network errors**: When network connectivity issues prevent API calls
- **Service errors**: When the model service returns an error

## Implementation

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

#### 2. Error Message Formatting

The `createUserFriendlyErrorMessage` utility function (in `packages/core/src/chat/errorHandler.ts`) transforms technical error messages into user-friendly text:

```typescript
export function createUserFriendlyErrorMessage(error: unknown): string {
  const errorMessage = error instanceof Error 
    ? error.message 
    : typeof error === 'object' && error !== null 
      ? JSON.stringify(error) 
      : String(error);
  
  // Extract useful information from error message if possible
  if (errorMessage.includes("context length of only")) {
    return "This conversation is too long for the model's context window. Try starting a new chat or using a model with a larger context size.";
  } 
  
  if (errorMessage.includes("rate limit")) {
    return "Rate limit exceeded. Please wait a moment before sending another message.";
  } 
  
  // More error pattern matching...
  
  // Return original message if no specific error was matched
  return errorMessage;
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

### Server-Side Error Handling

#### 1. Streaming API Errors

The server (`apps/coder/src/server/server.ts`) handles errors in the streaming API:

```typescript
// In the onError callback
onError: (event: { error: unknown }) => {
  const errorMessage = event.error instanceof Error
    ? `${event.error.message}\n${event.error.stack}`
    : String(event.error);
    
  console.error("💥 streamText onError callback:", errorMessage);

  // Throw error to be caught by stream handler
  throw new Error(`MODEL_ERROR: ${errorMessage}`);
},
```

#### 2. Error Formatting for Client

The server formats errors into a special format for the client to parse:

```typescript
try {
  // Format error message as a special event
  const errorObject = {
    error: true,
    message: userFriendlyError,
    details: errorMessage
  };
  
  // Send error to client
  await responseStream.write(`data: error:${JSON.stringify(errorObject)}\n\n`);
} catch (writeError) {
  console.error("Failed to write error message to stream");
}
```

## Special Error Handling for Model Types

### LMStudio Errors

For LMStudio models, special error handling includes:

1. **Context length issues**: When a model has a smaller context window than required
2. **Connection errors**: When the LMStudio server is not running
3. **Dynamic model discovery**: When a model referenced is not found in the discovered models

### OpenRouter/Cloud Model Errors

For cloud models, error handling focuses on:

1. **API key validation**: Checking if API keys are present and valid
2. **Rate limiting**: Detecting and explaining rate limit errors
3. **Model availability**: Checking if the requested model exists

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

1. **Always use append()**: When adding error messages to the chat, use the `append()` function from `usePersistentChat` to ensure proper persistence
2. **Be specific but concise**: Error messages should explain what went wrong without technical jargon
3. **Suggest solutions**: When possible, include a suggestion for how to fix the issue
4. **Maintain consistency**: Use the system role and warning style for all error messages
5. **Log for debugging**: Always include console.error logs with detailed error information

## Testing Error Scenarios

To test error handling:
- Set an invalid API key
- Try a conversation that exceeds the model's context length
- Request a non-existent model
- Disconnect from the network while generating
- Use a model with a smaller context window than needed