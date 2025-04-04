# Tool Error Handling in OpenAgents

## Problem

There was an issue with how tool execution errors were being handled in the chat stream. When the LLM attempted to use a tool (like GitHub API) and encountered a "not found" error for a file or resource, the entire conversation would stop with a generic error message: "Error: No response from LLM. Check your API key."

This is problematic because:
1. The error message is misleading (API key is fine, the problem is the resource doesn't exist)
2. The LLM should be able to adapt to the fact that a file doesn't exist and continue the conversation
3. The current behavior breaks the entire conversation rather than allowing it to continue

## Solution Approach

We've implemented a new error handling strategy that distinguishes between "fatal" and "non-fatal" tool errors:

1. **Non-fatal errors**: These errors should be communicated back to the LLM as results of the tool execution, not as system-level errors. The LLM should be able to adapt and continue the conversation. Examples include:
   - File or resource not found
   - Invalid arguments provided to a tool
   - Timeout when executing a tool
   - Tool not found

2. **Fatal errors**: These errors represent systemic issues that the LLM cannot reasonably work around. Examples include:
   - Authentication failures (API keys)
   - Permission denied errors
   - Network connectivity issues

## Implementation Details

### 1. Added `nonFatal` Flag to Tool Errors

We've enhanced the `ToolError` class hierarchy to include a `nonFatal` boolean flag:

```typescript
export interface ToolErrorOptions extends Omit<ChatErrorOptions, 'category'> {
  toolName: string;
  toolType?: string;
  arguments?: Record<string, unknown>;
  invocationId?: string;
  // When true, the error is non-fatal and should be returned as a result to the model
  nonFatal?: boolean;
}
```

Each error type is configured with a sensible default for this flag:
- `ResourceNotFoundError`: `nonFatal: true`
- `ToolNotFoundError`: `nonFatal: true`
- `ToolArgumentError`: `nonFatal: true`
- `ToolTimeoutError`: `nonFatal: true`
- `ToolAuthenticationError`: `nonFatal: false`
- `ToolPermissionError`: `nonFatal: false`
- `ToolExecutionError`: `nonFatal: false` (generic fallback)

### 2. Created `ResourceNotFoundError` for File/Resource Not Found Cases

We've added a specific error class for the "resource not found" case:

```typescript
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
  }
}
```

### 3. Enhanced Error Transformation Logic

We've improved our error transformers to better detect and categorize "not found" errors:

```typescript
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
```

### 4. Modified Stream Error Handling

We've updated the stream manager to handle non-fatal tool errors differently:

```typescript
// Check if this is a non-fatal tool error that should be returned as a result
if (chatError instanceof ToolError && chatError.nonFatal) {
  // For non-fatal errors, pass the error message as a result back to the model
  console.log("Non-fatal tool error detected. Converting to result:", chatError.userMessage);
  // Set a special property that the AI SDK stream processor will recognize
  (chatError as any).shouldConvertToToolResult = true;
  (chatError as any).toolResult = chatError.toToolResult();
}
```

### 5. Added Resource Path Extraction

To provide more useful error messages, we extract the file path or resource identifier from error messages:

```typescript
function extractResourcePathFromError(errorMessage: string): string | undefined {
  // Try different patterns to extract file or resource path
  
  // Check for quotes around path
  const quotedMatch = errorMessage.match(/['"]([^'"]+\.[\w]+)['"]/) || 
                      errorMessage.match(/['"]([\/\w\d\-_.]+)['"]/) ||
                      errorMessage.match(/No such file or directory: ['"]([^'"]+)['"]/);
  
  if (quotedMatch && quotedMatch[1]) {
    return quotedMatch[1];
  }
  
  // More extraction patterns...
}
```

## Results

With these changes, when a tool execution encounters a "not found" error:

1. The error is properly classified as a non-fatal `ResourceNotFoundError`
2. The error is converted to a tool result that the LLM can process
3. The conversation continues with the LLM aware that the resource wasn't found
4. The user sees a specific error message about the missing resource, not a generic "API key error"

For example, when trying to access a non-existent file like `packages/core/src/chat/errors/ChatError.ts`, the LLM will receive a message like:

```
Error executing tool get_file_contents: Resource not found: packages/core/src/chat/errors/ChatError.ts
```

Instead of terminating the conversation, the LLM can acknowledge the file doesn't exist and suggest an alternative approach or file location.

## Testing

To test this fix:
1. Ask the LLM to look at a file that doesn't exist
2. Verify the LLM receives the error as a tool result
3. Confirm the conversation continues with the LLM acknowledging the file doesn't exist
4. Check the error message is specific and helpful