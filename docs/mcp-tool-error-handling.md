# MCP Tool Error Handling Implementation

## Overview

This document describes the approach taken to improve error handling in the OpenAgents Coder application, particularly focusing on MCP (Model Context Protocol) tool execution errors. The implementation ensures that errors like "Error executing tool get_file_contents: Authentication Failed: Bad credentials" are properly displayed to users rather than being replaced with generic or hardcoded error messages.

## Problem Statement

The application was experiencing several issues with error handling:

1. Tool execution errors were being replaced with a generic "An error occurred" message
2. A hardcoded context overflow error message was being displayed for all generic errors
3. The error extraction logic wasn't properly identifying and displaying specific errors 
4. Error formatting was inconsistent, sometimes adding redundant "Error:" prefixes

## Implementation Strategy

The solution involved a comprehensive approach across both server-side and client-side components:

### Server-Side Improvements

1. **Enhanced Error Logging**
   - Added detailed logging of all error properties
   - Improved the extraction of contextual information from error objects
   - Implemented special logging for tool execution errors

2. **Error Detection and Classification**
   - Added robust patterns to identify tool execution errors
   - Improved authentication error detection
   - Created separate handling for different error types

3. **Error Propagation**
   - Ensured tool execution errors are passed through without modification
   - Implemented extraction of the most relevant error message from complex error objects
   - Added fallbacks for different error types

### Client-Side Improvements

1. **Error Extraction Enhancements**
   - Added comprehensive error property inspection
   - Implemented extraction of error messages from error causes and stacks
   - Added special handling for nested error information

2. **Error Message Processing**
   - Removed hardcoded context overflow error replacement
   - Implemented smart prefix handling to avoid redundant "Error:" prefixes
   - Added special case handling for tool execution errors

3. **Error Display in UI**
   - Updated system message rendering to properly display special error formats
   - Enhanced error message logging for debugging
   - Added special formatting for different error types

## Key Components Modified

1. **ChatStateProvider.tsx**
   - Updated error handling in `onError` callback
   - Improved error message extraction and formatting
   - Added comprehensive error property logging

2. **server.ts**
   - Enhanced `onError` handler in stream options
   - Added detailed logging of error information
   - Improved error classification and propagation

3. **chat-message.tsx**
   - Updated system message rendering for special error types
   - Added better error content formatting
   - Improved logging of error message content

## Technical Details

### Error Type Detection

We implemented several patterns to detect different types of errors:

```typescript
// Tool execution error detection
const isToolExecutionError = 
  errorMessage.includes('Error executing tool') || 
  errorMessage.includes('AI_ToolExecutionError') ||
  errorMessage.includes('Authentication Failed') ||
  (event.error instanceof Error && 
    ((event.error as any).name === 'AI_ToolExecutionError' || 
     ((event.error as any).cause && 
      (event.error as any).cause.includes?.('Error executing tool'))));
```

### Error Message Extraction

For complex error objects, we extract the most relevant message:

```typescript
// Extract tool error message from error object
if (event.error instanceof Error && (event.error as any).cause) {
  const cause = (event.error as any).cause;
  if (typeof cause === 'string' && cause.includes('Error executing tool')) {
    console.log("SERVER: USING ERROR CAUSE AS MESSAGE:", cause);
    throw new Error(cause);
  }
}

// Extract from error message with regex
const toolErrorMatch = errorMessage.match(/Error executing tool[^:]*:(.*?)(\n|$)/);
if (toolErrorMatch && toolErrorMatch[1]) {
  const toolError = `Error executing tool${toolErrorMatch[1]}`;
  console.log("SERVER: EXTRACTED TOOL ERROR:", toolError);
  throw new Error(toolError);
}
```

### Smart Error Formatting

We implemented smart formatting to avoid redundant prefixes:

```typescript
// Avoid adding redundant prefixes
if (userFriendlyError.startsWith('Error') || 
    userFriendlyError.startsWith('⚠️') || 
    userFriendlyError.startsWith('MODEL_ERROR')) {
  // Don't add a prefix if one already exists
  errorContent = userFriendlyError;
} else {
  // Add a prefix for errors that don't have one
  errorContent = `⚠️ Error: ${userFriendlyError}`;
}
```

## Lessons Learned

1. **Error Object Complexity**: Error objects can contain nested information in various properties like `cause`, `message`, `stack`, etc. A comprehensive approach is needed to extract the most relevant information.

2. **Error Propagation Challenges**: Errors can be transformed or lost as they pass through different layers of the application. Careful handling at each layer is necessary to preserve the original error information.

3. **Consistent Error Formatting**: Maintaining consistent error formatting while avoiding redundant prefixes requires careful implementation of formatting logic.

4. **Debugging Importance**: Comprehensive logging is essential for diagnosing error handling issues, especially for errors that occur in production environments.

## Future Improvements

1. **Structured Error Handling**: Implement a more structured approach to error handling with specific error types and classes.

2. **UI Error Components**: Create dedicated UI components for different types of errors with appropriate styling and actions.

3. **Error Telemetry**: Add error telemetry to collect information about error frequencies and types.

4. **Error Recovery Options**: Provide users with actionable recovery options for common errors.

## Conclusion

The implemented changes significantly improve the error handling in the OpenAgents Coder application, ensuring that users receive accurate and helpful error messages when tool execution fails. This enhancement contributes to a better user experience by providing clear information about what went wrong, particularly for authentication and permission-related errors in MCP tool execution.