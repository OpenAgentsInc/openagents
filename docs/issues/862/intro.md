# GitHub Token Handling Implementation (Issue #862)

## Overview

This document outlines the implementation of GitHub token handling in the OpenAgents system, specifically addressing [Issue #862](https://github.com/OpenAgentsInc/openagents/issues/862). The solution focuses on ensuring consistent and secure token handling across both direct API calls and Model Context Protocol (MCP) tool executions.

## Background

The OpenAgents system uses GitHub integration for various operations through two mechanisms:
1. MCP (Model Context Protocol) tools for GitHub operations
2. Direct API calls for certain operations

Previously, there were inconsistencies in how GitHub tokens were handled:
- Direct API calls attempted to get tokens from `agent.env.GITHUB_TOKEN`
- MCP tool calls weren't receiving tokens consistently
- Token initialization timing issues caused authentication failures

## Current Implementation

The current implementation in `packages/agents/src/server.ts` handles GitHub tokens through several key components:

### 1. Token Reception
```typescript
override async onMessage(connection: Connection, message: WSMessage) {
  // Extract token from WebSocket messages
  if (data.type === "cf_agent_use_chat_request") {
    const { githubToken } = requestData;
    this.githubToken = githubToken;
  }
}
```

### 2. Token Storage
The token is stored in the agent context using AsyncLocalStorage:
```typescript
export const agentContext = new AsyncLocalStorage<Coder>();
```

### 3. Token Usage in MCP Tools
```typescript
executions: {
  ...executions,
  ...Object.fromEntries(
    this.state.tools.map(tool => [
      tool.name,
      async (args: any) => {
        return this.mcp.callTool({
          serverId: tool.serverId,
          name: tool.name,
          args
        }, CallToolResultSchema, {});
      }
    ])
  ),
  getGithubToken: async () => context?.githubToken
}
```

## Key Features

1. **Consistent Token Handling**
   - Single source of truth for GitHub tokens
   - Proper token propagation through the system
   - Clear token lifecycle management

2. **Security**
   - Tokens are never logged in full
   - Token validation before use
   - Proper error handling for missing/invalid tokens

3. **Error Handling**
   - Clear error messages for authentication failures
   - Proper handling of rate limits
   - Detailed logging for debugging

4. **Integration Points**
   - WebSocket message handling
   - MCP tool execution
   - Environment variable access
   - Request header processing

## Usage

The GitHub token should be provided in one of these ways:
1. Through the WebSocket message body in `cf_agent_use_chat_request`
2. Via API headers (`x-github-token` or `x-api-key`)
3. Through the request body in `data.apiKeys.github`

## Testing

To verify the token handling:
1. Check token extraction from WebSocket messages
2. Verify token propagation to MCP tools
3. Test authentication error scenarios
4. Validate token security measures

## Future Considerations

1. Token refresh handling
2. Additional authentication methods
3. Enhanced error reporting
4. Performance optimization for token handling

## Related Changes

The implementation has gone through several iterations, each addressing specific aspects:
- Token extraction from various sources
- Proper initialization timing
- Error handling improvements
- Documentation updates

## References

- [Issue #862](https://github.com/OpenAgentsInc/openagents/issues/862)
- Related commits:
  - Token initialization sequence (8a40218)
  - Cloudflare worker context fixes (bfce8f5)
  - WebSocket message handling (ad09326)
  - TypeScript improvements (e193390)
