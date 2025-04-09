# GitHub Token Handling in Tool Context

## Overview
This document explains how GitHub token handling is implemented in the Coder agent using AsyncLocalStorage for tool context management.

## Implementation Details

### Tool Context Setup
```typescript
export const toolContext = new AsyncLocalStorage<Coder>();
```

The tool context is implemented using Node.js's `AsyncLocalStorage`, which allows us to store and access the Coder instance throughout the async execution context.

### Token Storage
The GitHub token is stored as a protected property on the Coder class:

```typescript
export class Coder extends AIChatAgent<Env> {
  protected githubToken?: string;
  // ...
}
```

### Token Extraction and Storage Process
When a message is received, the token is extracted and stored in the following flow:

1. The `extractToken` method processes incoming messages
2. For chat requests (`cf_agent_use_chat_request`), it:
   - Extracts the GitHub token from the request body
   - Uses `toolContext.run()` to store the token in the current context
   - Makes the token available throughout the execution context

Example implementation:
```typescript
if (data.type === "cf_agent_use_chat_request" && data.init.method === "POST") {
  const body = data.init.body
  const requestData = JSON.parse(body as string)
  const githubToken = requestData.githubToken

  return toolContext.run(this, async () => {
    this.githubToken = githubToken;
    return githubToken;
  });
}
```

## Benefits
- Type-safe implementation with proper TypeScript typing
- Token is scoped to the current request context
- Protected access to prevent unauthorized modifications
- Seamless integration with the existing agent architecture

## Usage
The GitHub token is now accessible within any method of the Coder class during the request lifecycle, enabling authenticated GitHub API calls when needed.
