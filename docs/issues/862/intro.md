# GitHub Token Integration - Issue #862

## Problem Statement

GitHub integration currently has token handling issues where:
- Direct GitHub API calls use `agent.env.GITHUB_TOKEN` 
- MCP GitHub tools don't pass tokens to the MCP server
- The API Keys page doesn't include GitHub as a provider
- There's no consistent pattern for handling authentication errors

## Solution Requirements

1. Add GitHub as a provider in the API Keys page
2. Update the GitHub plugin to use a consistent method for token access
3. Ensure all GitHub API calls include proper token handling and error messages
4. Synchronize GitHub token to MCP clients when token is set or changed
5. Create documentation for implementation and usage

## Implementation Benefits

- Users can add their GitHub tokens through the standard API Keys interface
- All GitHub operations will use the same token handling logic
- Authentication errors will provide clear guidance to users
- MCP GitHub operations will have proper authentication
- Future GitHub tool additions will have a clear pattern to follow

## Files to Update

1. `/apps/coder/src/pages/settings/ApiKeysPage.tsx`
2. `/apps/coder/src/providers/ApiKeyProvider.tsx`
3. `/packages/agents/src/plugins/github-plugin.ts`
4. `/packages/agents/src/plugins/direct-github-tools.ts`
5. `/apps/coder/src/server/mcp-clients.ts`
6. `/apps/coder/src/server.ts`

See the implementation document for detailed changes.