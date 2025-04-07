# MCP GitHub Tools Integration

## Overview

This document explains the integration of Model Context Protocol (MCP) GitHub tools with the Coder agent. This integration enables the Coder agent to perform GitHub operations using the MCP protocol.

## Architecture

The integration follows a client-server architecture:

1. **MCP GitHub Server**: Hosts GitHub tools and exposes them via the MCP protocol
   - Located at: `https://mcp-github.openagents.com/sse`
   - Implemented in: `apps/mcp-github-server/src/index.ts`

2. **Coder Agent**: Connects to the MCP server and exposes GitHub tools to users
   - Plugin implementation: `packages/agents/src/plugins/github-plugin.ts`
   - Uses: `@modelcontextprotocol/sdk/client`

## Implementation Details

### MCP Client Integration

The GitHub plugin creates and manages an MCP client:

```typescript
// Initialize MCP client
this.mcpClient = new Client(
  {
    name: "coder-agent",
    version: "1.0.0"
  },
  {
    capabilities: {
      tools: {}
    }
  }
);

// Connect to MCP GitHub server
await this.mcpClient?.connect({
  url: "https://mcp-github.openagents.com/sse"
});
```

### Tool Mapping

The plugin maps MCP tools to AI SDK tools, providing a consistent interface for the Coder agent:

```typescript
githubGetFile: tool({
  description: "Get the contents of a file from a GitHub repository",
  parameters: z.object({
    owner: z.string().describe("The repository owner (username or org)"),
    repo: z.string().describe("The repository name"),
    path: z.string().describe("Path to the file in the repository"),
    branch: z.string().optional().describe("Branch name, defaults to main/master")
  }),
  execute: async ({ owner, repo, path, branch }) => {
    try {
      return await this.callMCPTool('get_file_contents', { owner, repo, path, branch });
    } catch (error) {
      console.error("Error getting file contents:", error);
      return `Error: ${error instanceof Error ? error.message : String(error)}`;
    }
  }
})
```

### Tool Execution

The plugin includes a unified method for MCP tool execution with error handling:

```typescript
private async callMCPTool(toolName: string, params: any): Promise<string> {
  if (!this.mcpClient || !this.connected) {
    throw new Error("MCP client not initialized or not connected");
  }

  try {
    console.log(`Calling MCP tool ${toolName} with params:`, params);
    
    // Call the MCP tool
    const result = await this.mcpClient.callTool({
      name: toolName,
      arguments: params
    });
    
    // Extract the text from the first content item
    if (result?.content?.[0]?.type === 'text') {
      return result.content[0].text;
    }
    
    // Fallback to JSON stringifying the entire result
    return JSON.stringify(result);
  } catch (error) {
    // Error handling and reconnection logic
    // ...
  }
}
```

### Error Handling

The implementation includes robust error handling with automatic reconnection:

1. **Connection Errors**: If the MCP connection fails, the plugin attempts to reconnect
2. **Tool Execution Errors**: Errors from the MCP server are caught and formatted
3. **Graceful Degradation**: If MCP services are unavailable, informative errors are provided

## Available GitHub Tools

The integration provides a comprehensive set of GitHub tools:

### Repository Operations
- `githubGetFile`: Get file contents
- `githubPushFiles`: Push multiple files in a single commit
- `githubCreateRepository`: Create a new repository
- `githubCreateBranch`: Create a new branch

### Issue Operations
- `githubListIssues`: List repository issues
- `githubCreateIssue`: Create a new issue
- `githubGetIssue`: Get issue details
- `githubUpdateIssue`: Update an existing issue

### Pull Request Operations
- `githubListPullRequests`: List pull requests
- `githubCreatePullRequest`: Create a new PR
- `githubGetPullRequest`: Get PR details

### Code Operations
- `githubSearchCode`: Search for code across repositories
- `githubListCommits`: List repository commits

## Security Considerations

1. **GitHub Tokens**: The integration supports passing GitHub tokens to authenticate requests
2. **Rate Limiting**: Includes reconnection logic to handle rate limiting errors
3. **Error Obfuscation**: Sensitive information is removed from error messages

## Testing

To test the integration:

1. Start the Coder agent
2. Verify that the GitHub tools are available
3. Test various GitHub operations:
   - Fetching file contents
   - Listing issues
   - Creating pull requests
   - Searching for code

## Future Improvements

1. **Token Management**: Add a secure way to manage GitHub tokens
2. **Tool Discovery**: Dynamically update available tools based on MCP server capabilities
3. **Caching**: Add caching for frequently used operations
4. **Authentication UI**: Add a user interface for GitHub authentication