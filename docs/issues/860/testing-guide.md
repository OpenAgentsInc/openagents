# Testing Guide for MCP GitHub Tool Integration

This guide provides instructions for testing the integration of MCP GitHub tools with the Coder agent.

## Prerequisites

1. A running OpenAgents instance
2. GitHub Personal Access Token with appropriate permissions
3. An MCP GitHub server properly configured with the token

## Configuration Setup

1. Configure the MCP GitHub client:

```bash
# In the OpenAgents API service
curl -X POST http://localhost:3000/api/mcp/clients -d '{
  "name": "GitHub MCP Client",
  "type": "stdio",
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-github"],
  "env": {
    "GITHUB_PERSONAL_ACCESS_TOKEN": "your-github-token"
  },
  "enabled": true
}'
```

2. Refresh the MCP tools:

```bash
curl -X POST http://localhost:3000/api/mcp/tools/refresh
```

## Test Cases

### 1. Basic Connectivity Test

Test that the Coder agent can connect to the MCP GitHub server through the bridge API.

**User Query**: "Can you list my GitHub repositories?"

**Expected Response**: The agent should use the `githubListRepos` tool and return a list of repositories.

### 2. Repository Information Test

Test that the agent can fetch detailed information about a GitHub repository.

**User Query**: "Get information about the repository 'username/repo-name'"

**Expected Response**: The agent should use the `githubGetRepo` tool and return details about the specified repository.

### 3. Issue Management Test

Test that the agent can list and create issues in a GitHub repository.

**User Query**: "List open issues in the repository 'username/repo-name'"

**Expected Response**: The agent should use the `githubListIssues` tool and return a list of open issues.

**User Query**: "Create a new issue in 'username/repo-name' titled 'Test Issue' with description 'This is a test issue created by the agent'"

**Expected Response**: The agent should use the `githubCreateIssue` tool and create a new issue in the specified repository.

### 4. File Content Test

Test that the agent can fetch file contents from a GitHub repository.

**User Query**: "Show me the content of the README.md file in the 'username/repo-name' repository"

**Expected Response**: The agent should use the `githubGetFileContents` tool and return the content of the README.md file.

### 5. Error Handling Test

Test that the agent handles errors gracefully.

**User Query**: "Get information about a non-existent repository 'username/non-existent-repo'"

**Expected Response**: The agent should attempt to use the `githubGetRepo` tool, but handle the error gracefully and inform the user that the repository does not exist.

## Troubleshooting

If tests fail, check the following:

1. **MCP Bridge API**: Ensure the MCP bridge API endpoint is correctly set in the GitHub plugin.

2. **GitHub Token**: Verify that the GitHub token has the necessary permissions.

3. **MCP Client**: Check that the MCP GitHub client is running and properly configured.

4. **Network Connectivity**: Ensure that the Cloudflare Worker can reach the MCP bridge API.

5. **Error Logs**: Check both the agent logs and the MCP service logs for detailed error messages.

## Validation Checklist

- [ ] All GitHub tools are correctly registered with the agent
- [ ] The agent can execute all GitHub tools successfully
- [ ] Errors are handled gracefully and meaningful messages are provided to users
- [ ] The integration performs well with minimal latency
- [ ] GitHub token is securely managed and not exposed in logs