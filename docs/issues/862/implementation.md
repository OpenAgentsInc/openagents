# GitHub Token Integration Implementation

## Overview

This document provides technical details of how GitHub token handling was implemented in the OpenAgents codebase to address Issue #862. The implementation ensures proper handling of GitHub tokens throughout the application, from UI settings to agent environment to MCP integration.

## Components Modified

### UI Changes

1. **API Keys Page**
   - Added GitHub as a provider in `ApiKeysPage.tsx`
   - Updated provider descriptions to include GitHub
   - Ensured GitHub tokens can be saved and managed alongside other API keys

2. **API Key Provider**
   - Updated to load GitHub token from settings
   - Added GitHub to the list of valid key providers

### GitHub Plugin Updates

1. **Centralized Token Access**
   - Added `getGitHubToken()` method to ensure consistent token access across tools
   - Method prioritizes explicitly passed tokens, then falls back to environment variables

2. **Tool Schema Updates**
   - Updated all GitHub tool schemas to include optional token parameters
   - Modified each tool's execute method to use the centralized token method
   - Improved error handling for authentication issues

3. **MCP Tool Integration**
   - Improved `callMCPTool` method to handle authentication errors correctly
   - Added specific error messages for 401, 403, and 429 status codes

### Agent Integration

1. **Coder Agent Constructor**
   - Updated to set GitHub token from API keys into the agent environment
   - Ensured token is available to GitHub plugin through `agent.env.GITHUB_TOKEN`

2. **MCP Integration**
   - Created new `mcp-github-token.ts` module to handle token synchronization with MCP
   - Implemented methods to update tokens in MCP client
   - Added event listeners to update MCP when token changes in settings

## Token Flow

1. User adds GitHub token in API Keys page
2. Token is saved to settings repository
3. On agent initialization:
   - API Key Provider loads token from settings
   - Coder agent constructor sets token in environment
   - MCP GitHub token sync initializes
4. When GitHub plugin methods are called:
   - Direct API calls use `getGitHubToken()` to retrieve token
   - MCP tool calls include token in parameters

## Error Handling

- Authentication errors (401/403) generate clear messages about token issues
- Rate limit errors (429) include information about reset time
- Network errors include suggestions to check connectivity

## Testing

- To test this implementation:
  1. Add a GitHub token in the API Keys page
  2. Try GitHub operations in a chat conversation
  3. Check logs for token usage and any error messages
  4. Verify MCP tool calls include the token

## Security Considerations

- Tokens are stored securely in the settings repository
- Tokens are never logged or exposed in client-side code
- Error messages don't include token values
- Expired or invalid tokens generate user-friendly error messages
