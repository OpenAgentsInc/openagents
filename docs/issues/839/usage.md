# MCP Client Configuration Usage Guide

This document explains how to use the new MCP client configuration system in the Coder application.

## What are MCP Clients?

Model Context Protocol (MCP) clients provide tools to AI models, allowing them to perform actions like file operations, GitHub interactions, and shell commands. The Coder application can connect to both remote MCP servers (via HTTP Server-Sent Events) and local MCP processes (via standard I/O).

## Accessing MCP Client Settings

1. Open the Coder application
2. Click on the settings icon or navigate to the settings page
3. Select "MCP Clients" from the settings menu

## Managing MCP Clients

### Viewing Clients

The MCP Clients page displays all configured clients with their:
- Name and type
- Current connection status
- Configuration details
- Environment variables (with sensitive values masked)

### Adding a New Client

To add a new MCP client:

1. Click the "Add Client" button
2. Select the client type:
   - **SSE (Remote)**: For connecting to remote MCP servers via HTTP
   - **stdio (Local)**: For spawning local processes
3. Fill in the required details:
   - For SSE clients:
     - Name: A descriptive name
     - URL: The SSE endpoint URL (e.g., https://example.com/mcp/sse)
     - Headers: Any HTTP headers needed for authentication
   - For stdio clients:
     - Name: A descriptive name
     - Command: The command to execute (e.g., npx)
     - Arguments: Command-line arguments (e.g., -y @modelcontextprotocol/server-github)
     - Environment Variables: Variables passed to the process
4. Enable or disable the client using the toggle
5. Click "Create Client"

### Editing a Client

To edit an existing client:

1. Click the edit (pencil) icon for the client you want to modify
2. Update the configuration as needed
3. Click "Save Changes"

Changes take effect immediately - the client will be reinitialized with the new configuration if relevant fields are changed.

### Enabling/Disabling Clients

Each client has a toggle switch that allows you to enable or disable it without deleting the configuration. Disabled clients will not be initialized at startup.

### Refreshing Clients

To reconnect a client:

1. Click the refresh icon for the specific client you want to reconnect
2. The client status will update to reflect the new connection state

To reconnect all clients:

1. Click the "Refresh All" button at the top of the page
2. All enabled clients will be reinitialized

### Deleting Clients

To remove a client configuration:

1. Click the delete (trash) icon for the client you want to remove
2. Confirm the deletion when prompted

## Example Configurations

### Remote GitHub MCP Client

- **Name**: GitHub MCP
- **Type**: SSE (Remote)
- **URL**: https://mcp-github.openagents.com/sse

### Local GitHub MCP Client

- **Name**: Local GitHub MCP
- **Type**: stdio (Local)
- **Command**: npx
- **Arguments**: -y @modelcontextprotocol/server-github
- **Environment Variables**:
  - GITHUB_PERSONAL_ACCESS_TOKEN: your_github_token

### Local Shell MCP Client

- **Name**: Local Shell MCP
- **Type**: stdio (Local)
- **Command**: uvx
- **Arguments**: mcp-shell-server
- **Environment Variables**:
  - ALLOW_COMMANDS: ls,cat,pwd,echo,grep,find,ps,wc

## Troubleshooting

If a client fails to connect:

1. Check the client's status message for error details
2. Verify that the configuration is correct
3. For SSE clients, ensure the URL is accessible from your network
4. For stdio clients, ensure the command is installed and executable
5. Check environment variables for any required authentication tokens
6. Try refreshing the client

If tools aren't appearing in the AI model:

1. Make sure the client is enabled and connected
2. Check that the model supports tools
3. Refresh the client to ensure tools are loaded
4. Check the console logs for any error messages

## Security Considerations

- For security reasons, be cautious when configuring stdio clients that execute commands
- Use the minimum required permissions for GitHub tokens
- Be careful with environment variables containing sensitive information
- For shell MCP clients, restrict allowed commands to the minimum necessary