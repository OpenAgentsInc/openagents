# GitHub Token Authentication in MCP Server

This document explains how GitHub token authentication works in the MCP (Model Context Protocol) server implementation.

## Overview

The system uses a Bearer token authentication mechanism to pass GitHub tokens from the client through the MCP server to GitHub API calls. This allows authenticated access to GitHub's API while maintaining security and proper token handling.

## Flow Diagram

```mermaid
sequenceDiagram
    participant Client
    participant MCP Server
    participant GitHub API

    Client->>MCP Server: Request with Bearer Token
    Note over MCP Server: Extract token from headers
    MCP Server->>MCP Server: Wrap GitHub requests with token
    MCP Server->>GitHub API: API calls with token
    GitHub API->>MCP Server: Response
    MCP Server->>Client: Tool execution result
```

## Implementation Details

### 1. Client-Side Token Passing

The client must include the GitHub token in the Authorization header when making requests to the MCP server:

```typescript
headers: {
  'Authorization': 'Bearer ghp_your_github_token_here'
}
```

### 2. Token Extraction (Server-Side)

The token extraction happens at two levels:

#### a. Initial Request Handler
```typescript
// In the main fetch handler
const authHeader = request.headers.get('Authorization');
const token = authHeader?.startsWith('Bearer ') ? authHeader.substring(7) : undefined;
```

#### b. Tool Execution Context
```typescript
// In each tool handler
const authHeader = extra?.request?.headers?.get('Authorization');
const token = authHeader?.startsWith('Bearer ') ? authHeader.substring(7) : undefined;
```

### 3. Token Usage in GitHub Requests

The system uses a token-aware request wrapper to include the token in all GitHub API calls:

```typescript
const withToken = (token?: string) => {
  return (url: string, options: RequestOptions = {}) => {
    return githubRequest(url, { ...options, token });
  };
};
```

### 4. Logging and Debugging

The system includes comprehensive logging to track token presence and usage:

```typescript
// Initial request logging
console.log("Incoming request to MCP server");
console.log("Authorization header present:", !!authHeader);

// Tool execution logging
console.log('Received auth header:', authHeader ? 'present' : 'not present');
console.log('Successfully extracted token:', token.substring(0, 8) + '...');
console.log(`🔑 GitHub token present: ${!!token}`);
```

## Security Considerations

1. **Token Masking**: When logging, only the first 8 characters of the token are shown for security.
2. **Bearer Format**: Only tokens with the 'Bearer ' prefix are accepted.
3. **Error Handling**: Token extraction errors are caught and logged without exposing sensitive information.
4. **Temporary Context**: The token is only available during the tool execution context.

## Error Handling

The system includes robust error handling at multiple levels:

1. **Token Extraction**:
```typescript
try {
  const authHeader = extra?.request?.headers?.get('Authorization');
  // ... token extraction
} catch (e) {
  console.error('Error accessing request headers:', e);
}
```

2. **GitHub API Calls**:
```typescript
try {
  globalThis.githubRequest = withToken(context.token);
  const result = await tool.handler(validatedParams);
  // ... handle success
} catch (error) {
  // ... handle error
} finally {
  globalThis.githubRequest = originalRequest;
}
```

## CORS Configuration

The server is configured to accept cross-origin requests with authentication headers:

```typescript
corsOptions: {
  origin: "*",
  methods: "GET,POST",
  headers: "*"
}
```

## Testing

To test the authentication:

1. Make a request to the MCP server with a GitHub token:
```bash
curl -H "Authorization: Bearer ghp_your_token" https://mcp-github.openagents.com/sse
```

2. Check the logs for token presence:
```
Incoming request to MCP server
Authorization header present: true
Token received: ghp_1234...
```

3. Verify GitHub API access in tool execution:
```
🔧 Executing GitHub tool: create_branch
🔑 GitHub token present: true
✅ Tool execution successful
```

## Common Issues

1. **Missing Token**: If no Authorization header is present, operations will run unauthenticated
2. **Invalid Token Format**: Tokens must start with 'Bearer '
3. **Token Extraction Failures**: Check logs for 'Error accessing request headers'

## Future Improvements

1. Add token validation before GitHub API calls
2. Implement token refresh mechanism
3. Add rate limiting based on token presence
4. Add token scopes verification for different operations
