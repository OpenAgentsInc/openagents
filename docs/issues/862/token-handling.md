# GitHub Token Handling Implementation (Issue #862)

## Overview

This document explains the implementation of GitHub token handling in the OpenAgents system, specifically focusing on the correct passing of tokens from the UI to the agent environment and then to MCP (Model Context Protocol) tools.

## Problem Statement

GitHub tokens were not reliably passed from the user interface to the MCP tools, causing GitHub operations to fail with 522 timeout errors, even after tokens were configured in the API Keys settings page.

## Root Cause

1. **Initialization Timing Issue**: The MCP GitHub client was being initialized before the token was loaded from settings.
2. **Inconsistent Token Access Paths**: Multiple code paths were used to access the token, with no clear precedence.
3. **Missing Header Extraction**: Tokens passed via HTTP headers were not being extracted.
4. **Token Fallback Mechanism**: The use of a placeholder debug token was masking the actual issue.

## Implementation

### 1. Token Flow Architecture

GitHub tokens now flow through the system in this sequence:

```
UI Settings → HTTP Headers & Request Data → Agent Environment → MCP Tools
```

### 2. UI Components (ChatPage.tsx)

The token is retrieved from settings and passed in two ways:
- As HTTP headers (`x-api-key` and `x-github-token`) during agent initialization
- As a data property (`apiKeys.github`) in the agent chat hook

```typescript
// --- API Keys ---
const { apiKeys } = useApiKeyContext();

// --- Initialize Agent ---
const agent = useAgent({
  agent: "coder",
  headers: {
    "x-api-key": apiKeys.github || "",
    "x-github-token": apiKeys.github || "",
  },
});

// --- Agent Chat Hook ---
const { /* ... */ } = useAgentChat({
  data: {
    apiKeys,
  },
  agent,
  // ...
});
```

### 3. Server Token Initialization (server.ts)

The server extracts tokens from both HTTP headers and request body, ensuring they're available before MCP client creation:

```typescript
async function initializeGitHubToken(env: Env, request?: Request): Promise<void> {
  // Extract GitHub token from request headers first
  if (request) {
    const apiKeyHeader = request.headers.get('x-api-key');
    const githubTokenHeader = request.headers.get('x-github-token');
    
    if (githubTokenHeader && githubTokenHeader.trim() !== '') {
      // Set token in all possible locations
      env.apiKeys = env.apiKeys || {};
      env.apiKeys.github = githubTokenHeader;
      env.GITHUB_TOKEN = githubTokenHeader;
      env.GITHUB_PERSONAL_ACCESS_TOKEN = githubTokenHeader;
    } else if (apiKeyHeader && apiKeyHeader.trim() !== '') {
      // Similar handling for x-api-key header
      // ...
    } else {
      // Try to extract token from request body if it's POST
      if (request.method === 'POST') {
        // Clone the request before reading the body
        const clonedRequest = request.clone();
        const contentType = request.headers.get('content-type') || '';
        
        if (contentType.includes('application/json')) {
          const body = await clonedRequest.json();
          
          // Look for token in data.apiKeys.github (matches the React hook structure)
          if (body && body.data && body.data.apiKeys && body.data.apiKeys.github) {
            const githubToken = body.data.apiKeys.github;
            
            // Set token in all possible locations
            env.apiKeys = env.apiKeys || {};
            env.apiKeys.github = githubToken;
            env.GITHUB_TOKEN = githubToken;
            env.GITHUB_PERSONAL_ACCESS_TOKEN = githubToken;
          }
        }
      }
    }
  }
  
  // If token is already in apiKeys object from request data, ensure it's in all locations
  if (env.apiKeys && env.apiKeys.github) {
    env.GITHUB_TOKEN = env.apiKeys.github;
    env.GITHUB_PERSONAL_ACCESS_TOKEN = env.apiKeys.github;
  }
}
```

### 4. GitHub Plugin Token Access (github-plugin.ts)

The plugin now has a clear precedence for token access:

```typescript
private getGitHubToken(paramToken?: string): string | undefined {
  // 1. Prioritize token passed directly to the tool method
  if (paramToken && paramToken.trim() !== '') {
    return paramToken;
  }
  
  // 2. Attempt to use the token from the agent environment
  const env = this.getAgentEnv();
  if (!env) return undefined;
  
  // Check in standard locations, in order of precedence
  if (env.GITHUB_TOKEN && typeof env.GITHUB_TOKEN === 'string') {
    return env.GITHUB_TOKEN;
  }
  
  if (env.GITHUB_PERSONAL_ACCESS_TOKEN && typeof env.GITHUB_PERSONAL_ACCESS_TOKEN === 'string') {
    return env.GITHUB_PERSONAL_ACCESS_TOKEN;
  }
  
  // Check apiKeys as final option
  if (env.apiKeys && env.apiKeys.github && typeof env.apiKeys.github === 'string') {
    return env.apiKeys.github;
  }
  
  // No token found
  return undefined; // No more placeholder tokens
}
```

### 5. Key Improvements

1. **Removed Fallback Tokens**: Eliminated placeholder debug tokens to ensure failures are visible ("fail fast" approach).
2. **Centralized Token Extraction**: All token handling happens in the `initializeGitHubToken` function.
3. **Clear Prioritization**: Established a clear order of precedence for token sources.
4. **Improved Logging**: Enhanced logging to track token flow and presence.
5. **Multiple Passing Mechanisms**: Support for both header-based and data-based token passing.

## Error Handling

Improved error messages now guide users to the proper resolution (adding a token in Settings):

```typescript
if (!tokenToSend) {
  console.warn(`⚠️ No GitHub token is being sent for MCP tool ${toolName}.`);
  console.warn(`⚠️ This operation may fail if accessing private repositories.`);
  console.warn(`Please add a GitHub token in Settings > API Keys to enable full GitHub functionality.`);
}
```

For authentication failures (HTTP 401/403):
```typescript
if (status === 401 || status === 403) {
  return JSON.stringify({
    error: `Authentication error (${status}) from GitHub. A valid token with correct permissions is required. Please add a GitHub token in Settings > API Keys.`
  });
}
```

## Testing

To test the GitHub token handling:

1. Configure a token in the API Keys settings page
2. Use a GitHub tool like `githubGetFile` to access a repository
3. Check logs for token presence at each stage:
   - Request headers extraction
   - Agent environment initialization
   - MCP tool invocation
   - API request

## Troubleshooting

If GitHub operations fail:

1. Check logs for "No GitHub token found" warnings
2. Verify token is configured in API Keys settings
3. Confirm token has appropriate permissions for the operation
4. Look for 401/403 errors that indicate authentication issues
5. Check for 522/524 errors that may indicate connection issues