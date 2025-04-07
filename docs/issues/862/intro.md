# GitHub Token Handling - Issue #862

## Overview

Issue #862 addresses inconsistent GitHub token handling in the existing implementation. The current implementation has different approaches for accessing GitHub tokens across different API and tool calls:

1. Direct API tools try to access tokens via `agent.env.GITHUB_TOKEN`
2. MCP tool calls don't pass tokens at all
3. MCP GitHub server expects tokens to be provided in tool parameters

The goal is to create a unified approach to GitHub token management that connects the UI settings with the agent and MCP server.

## Current Implementation Analysis

### GitHub Plugin Implementation

The GitHub plugin (`OpenAIAgentPlugin`) in `packages/agents/src/plugins/github-plugin.ts` has inconsistent token handling:

- **Direct API Calls**: Access token from `this.agent?.env?.GITHUB_TOKEN`
  ```typescript
  const token = this.agent?.env?.GITHUB_TOKEN;
  return await directGitHubTools.getFileContents(owner, repo, path, branch, token);
  ```

- **MCP Tool Calls**: Don't pass tokens at all
  ```typescript
  return await this.callMCPTool('create_repository', { name, description, private: isPrivate });
  // No token is passed to MCP tools
  ```

### Settings Management

- API keys are managed through `ApiKeysPage.tsx` using the `useSettings` hook
- The settings repository (`settingsRepository`) in `packages/core/src/db/repositories/settings-repository.ts` handles storing API keys
- GitHub token is not currently included in the API keys interface
- API keys are available via the `apiKeys` state in `ApiKeyProvider.tsx`, but not passed to the agent or MCP server

### MCP Client Configuration

- MCP GitHub client is defined in `mcp-clients.ts` with a placeholder for GitHub token:
  ```typescript
  {
    env: {
      GITHUB_PERSONAL_ACCESS_TOKEN: '<TOKEN_REQUIRED>'
    }
  }
  ```
- No mechanism currently exists to inject user-provided GitHub tokens into this configuration

### Direct GitHub Tools

- The direct GitHub tools module (`direct-github-tools.ts`) accepts tokens for API calls
- It correctly utilizes tokens when provided, adding proper Authorization headers:
  ```typescript
  if (token) {
    headers['Authorization'] = `token ${token}`;
  }
  ```

## Required Changes

1. **UI Changes**:
   - Add GitHub as a provider in the API Keys page
   - Update `ApiKeysPage.tsx` to include GitHub token management
   - Implement proper storage/retrieval of GitHub token using `settingsRepository`

2. **Token Propagation**:
   - Pass GitHub token from settings to the Coder agent environment
   - Update agent initialization to include GitHub token from settings
   - Create a consistent `getGitHubToken()` method in the GitHub plugin

3. **Tool Schema Updates**:
   - Update authentication-required tool schemas to include optional token parameter
   - Modify MCP tool calls to pass the token parameter
   - Ensure consistent error handling for authentication failures

4. **MCP Integration**:
   - Update MCP GitHub client configuration to use the token from settings
   - Add mechanism to reinitialize MCP client when token changes
   - Handle token refresh and validation

## Integration Flow

1. User enters GitHub token in the API Keys page
2. Token is stored in the settings repository
3. On agent initialization, token is loaded from settings and added to agent environment
4. GitHub plugin uses the token for direct API calls
5. MCP GitHub client is initialized with the token from settings
6. Tools consistently pass the token when making authenticated requests

## Security Considerations

- GitHub tokens should never be logged or exposed in responses
- Token validation should be implemented before use
- Appropriate error messages for authentication failures should be provided (without revealing sensitive information)
- Token expiration should be handled gracefully

## Next Steps

1. Implement GitHub token management in `ApiKeysPage.tsx`
2. Create consistent token access method in GitHub plugin
3. Update tool schemas to include token parameters
4. Implement token propagation from settings to agent environment
5. Update MCP client initialization to use token from settings
6. Add comprehensive error handling for authentication failures