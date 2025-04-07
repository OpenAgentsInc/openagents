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
   - Improved logging for token discovery and usage

2. **Standardized MCP Tool Usage**
   - Replaced all direct GitHub API calls with MCP tool calls
   - Ensured consistent parameter naming and approach across all GitHub tools
   - Added detailed logging for all MCP tool calls

3. **Error Handling and Reliability**
   - Implemented retry mechanism for network errors and Cloudflare timeouts
   - Added specific error messages for different HTTP status codes
   - Improved error reporting with detailed diagnostics and user-friendly messages
   - Enhanced logging throughout the GitHub plugin for better debugging

4. **MCP Tool Integration**
   - Improved `callMCPTool` method with retry logic for transient errors
   - Added specific error messages for common GitHub API errors (401, 403, 404, 429)
   - Added specific handling for Cloudflare errors (522, 524)
   - Enhanced logging for request parameters and responses

### Agent Integration

1. **Coder Agent Constructor**
   - Updated to set GitHub token from API keys into the agent environment
   - Added comprehensive environment logging
   - Ensured token is available to GitHub plugin through `agent.env.GITHUB_TOKEN`

2. **MCP Integration**
   - Created new `mcp-github-token.ts` module to handle token synchronization with MCP
   - Made token handling work in both browser and Node.js environments
   - Implemented methods to update tokens in MCP client
   - Added event listeners to update MCP when token changes in settings

3. **Environment Support**
   - Enhanced Agent and Logger to work in both browser and Node.js environments
   - Added fallback mechanisms when window is not available
   - Fixed token propagation between browser and server environments

## Token Flow

1. User adds GitHub token in API Keys page
2. Token is saved to settings repository
3. On agent initialization:
   - API Key Provider loads token from settings
   - Coder agent constructor sets token in environment
   - MCP GitHub token sync initializes
4. When GitHub plugin methods are called:
   - GitHub plugin gets token from agent environment
   - MCP tool calls include token in parameters
   - Error handling with retries occurs if connection issues arise

## Error Handling

- Authentication errors (401/403) generate clear messages about token issues
- Rate limit errors (429) include information about reset time
- Repository not found errors (404) provide guidance on checking repository name
- Connection timeouts (522/524) automatically retry and suggest waiting
- Server errors (500-599) provide meaningful context and next steps
- Network errors include retry mechanism and suggestions to check connectivity

## Reliability Improvements

- **Retry Mechanism**: Automatically retries up to 2 times on connection errors
- **Timeout Handling**: Adds 2-second delay between retries for recovery
- **Network Error Detection**: Identifies and handles various network failure types
- **Consistent Error Format**: All errors return JSON with standardized error structure
- **Detailed Logging**: Every step of token retrieval and API call has comprehensive logging

## Testing

- To test this implementation:
  1. Add a GitHub token in the API Keys page
  2. Try GitHub operations in a chat conversation
  3. Check logs for token usage and any error messages
  4. Verify MCP tool calls include the token

## Security Considerations

- Tokens are stored securely in the settings repository
- Tokens are never logged or exposed in client-side code, only token lengths are logged
- Error messages don't include token values
- Expired or invalid tokens generate user-friendly error messages

## Future Improvements

- Consider implementing token validation on save
- Add refresh token support if GitHub API changes to require it
- Cache repository validation results to reduce API calls
- Implement more granular permission checking based on token scopes
- Add telemetry for most common GitHub operations to optimize performance
- Consider implementing local caching for frequently accessed repositories