# Debugging GitHub Token Handling (Issue #862)

## Problem Overview

The GitHub token handling in OpenAgents is not working as expected. While read operations to the GitHub API appear to function correctly, write operations (like commenting on issues) are failing with "Requires authentication" errors.

This document outlines the enhanced logging and debugging steps implemented to diagnose the issue.

## Diagnostic Logging Additions

### 1. Tool Execution Logging

Enhanced the tool execution code to provide detailed token information:

```typescript
// Make sure to include the GitHub token in the arguments if available
const githubToken = coderAgent.githubToken;

// Debug token information safely
if (githubToken) {
  const tokenPrefix = githubToken.substring(0, 8);
  const tokenLength = githubToken.length;
  console.log(`CODER_TOKEN_DEBUG: Token present for ${name}. Token starts with: ${tokenPrefix}..., length: ${tokenLength}`);
} else {
  console.log(`CODER_TOKEN_DEBUG: No token available for ${name}`);
}

// Add token to arguments
const argsWithToken = githubToken ? { ...args, token: githubToken } : args;

// Log full arguments structure without token value (for security)
const argsForLogging = {...args};
if (argsWithToken.token) {
  argsForLogging._hasToken = true;
  argsForLogging._tokenLength = argsWithToken.token.length;
}
console.log(`Executing MCP tool ${name} with args:`, JSON.stringify(argsForLogging));
```

This provides:
- Safe logging of token presence (first few characters only)
- Token length information
- Confirmation that token is being added to args
- Full argument structure without exposing the token value

### 2. Token Update Method Logging

Enhanced the `updateGitHubToken` method to verify token format and persistence:

```typescript
async updateGitHubToken(token: string) {
  // Verify token format and log safely
  if (token && typeof token === 'string') {
    const tokenPrefix = token.substring(0, 8);
    const tokenLength = token.length;
    console.log(`TOKEN_UPDATE: Valid token received. Starts with: ${tokenPrefix}..., length: ${tokenLength}`);
    
    // Set the token on the instance
    this.githubToken = token;
    console.log("GitHub token stored in agent instance");
  } else {
    console.log(`TOKEN_UPDATE: Invalid token received: ${token ? "non-string value" : "null or undefined"}`);
    return;
  }
  
  // ... [MCP client initialization code] ...
  
  // Verify that token is available in the instance after refresh
  if (this.githubToken) {
    const tokenPrefix = this.githubToken.substring(0, 8);
    console.log(`TOKEN_VERIFY: After refresh, token is still present. Prefix: ${tokenPrefix}...`);
  } else {
    console.log(`TOKEN_VERIFY: WARNING - Token is no longer present after refresh!`);
  }
}
```

This provides:
- Validation that the token is a non-empty string
- Logging of token prefix and length for debugging
- Verification that token persists after MCP client refresh

### 3. WebSocket Message Handling Logging

Enhanced the `onMessage` method to provide detailed logging about incoming messages:

```typescript
override async onMessage(connection: Connection, message: WSMessage) {
  console.log(`INCOMING_MESSAGE: Received message of type: ${typeof message}`);
  
  // Call the parent method first
  await super.onMessage(connection, message);
  
  if (typeof message === 'object' && message !== null) {
    const chatMessage = message as any;
    console.log(`MESSAGE_CONTENT: Message has type: ${chatMessage.type}, has data: ${!!chatMessage.data}`);
    
    if (chatMessage.type === "cf_agent_use_chat_request" && chatMessage.data) {
      const data = chatMessage.data;
      
      // Check token in data
      console.log(`TOKEN_CHECK: Message data has githubToken: ${!!data.githubToken}`);
      console.log(`TOKEN_CHECK: Message data structure: ${JSON.stringify(Object.keys(data))}`);
      
      // Check API keys section if present
      if (data.apiKeys) {
        console.log(`API_KEYS: Message has apiKeys section with keys: ${JSON.stringify(Object.keys(data.apiKeys))}`);
        console.log(`API_KEYS: Has GitHub key: ${!!data.apiKeys.github}`);
        
        // Try apiKeys.github first
        if (data.apiKeys.github) {
          console.log(`API_KEYS: Using GitHub token from apiKeys.github`);
          await this.updateGitHubToken(data.apiKeys.github);
          return;
        }
      }
      
      // Fall back to direct githubToken property
      if (data.githubToken) {
        console.log(`TOKEN_FOUND: Using GitHub token from data.githubToken`);
        await this.updateGitHubToken(data.githubToken);
      } else {
        console.log(`TOKEN_MISSING: No GitHub token found in message data or apiKeys`);
      }
    }
  } else {
    console.log(`UNEXPECTED_MESSAGE: Message is not an object or is null. Type: ${typeof message}`);
  }
}
```

This provides:
- Message type and structure verification
- Data property inspection
- Multiple token source checking (both `data.githubToken` and `data.apiKeys.github`)
- Clear logging of token presence or absence

## Possible Issues and Solutions

Based on the enhanced logging, several possible issues could be diagnosed:

1. **Token Not Received**: The logs will show if no token is being received in the WebSocket messages
   - Solution: Check client-side code that should send the token

2. **Token Not Stored**: The logs will show if the token is received but not properly stored
   - Solution: Fix the assignment in `updateGitHubToken`

3. **Token Not Passed to Tools**: The logs will show if the token is stored but not passed to tool execution
   - Solution: Ensure `argsWithToken` properly includes the token

4. **Token Rejected by MCP Server**: If all client-side handling is correct but the server rejects the token
   - Solution: Check MCP server code, token format, or permissions

5. **Wrong Token Format**: The token might be in an unexpected format
   - Solution: Check token structure and ensure it's passed as expected

## Next Steps After Diagnosis

Once the specific issue is identified through the logs:

1. If the token is not being received:
   - Investigate the client-side code that should be sending the token

2. If the token is received but not persisting:
   - Check for any code that might be overwriting the `githubToken` property

3. If the token is not being passed to the MCP tool:
   - Verify the tool execution and argument passing logic

4. If the MCP server is not using the token correctly:
   - Review the MCP GitHub server implementation

## Testing Authentication

Once fixes are implemented, test with these operations:
1. Read operation: `get_file_contents` on a public repository (should work without token)
2. Read operation: `get_file_contents` on a private repository (requires valid token)
3. Write operation: `add_issue_comment` (requires valid token with correct scopes)
4. Write operation: `create_or_update_file` (requires valid token with correct scopes)

## Note on Token Scopes

The GitHub token must have the correct scopes for the operations being performed:
- For read operations on public repos: No token needed
- For read operations on private repos: `repo` scope
- For commenting on issues: `repo` scope
- For creating/updating files: `repo` scope

Ensure the token provided has adequate scopes for the intended operations.