# MCP Client Configuration

The OpenAgents Coder application provides a flexible system for configuring Model Context Protocol (MCP) clients. This allows AI models to access tools for interacting with the file system, GitHub, shell commands, and more.

## Understanding MCP Clients

MCP clients serve as bridges between AI models and external systems or functionality. They provide tools that models can use to perform actions through the Model Context Protocol. The Coder application supports two types of MCP clients:

1. **SSE (Server-Sent Events) Clients**: Connect to remote MCP servers via HTTP
2. **stdio (Standard I/O) Clients**: Spawn local processes and communicate via stdin/stdout

## Configuration Interface

MCP clients can be configured through the Settings UI:

1. Open the Coder application
2. Go to Settings → MCP Clients
3. Use the interface to add, edit, enable/disable, or delete client configurations

## Configuration Options

### SSE Clients

- **Name**: A descriptive name for the client
- **URL**: The SSE endpoint URL (e.g., https://example.com/mcp/sse)
- **Headers**: HTTP headers for authentication or customization (stored as environment variables)
- **Enabled**: Whether the client should be initialized at startup

### stdio Clients

- **Name**: A descriptive name for the client
- **Command**: The command to execute (e.g., npx)
- **Arguments**: Command-line arguments (e.g., -y @modelcontextprotocol/server-github)
- **Environment Variables**: Variables to pass to the process
- **Enabled**: Whether the client should be initialized at startup

## Default Configurations

The application comes with predefined configurations for common MCP clients:

1. **Remote GitHub MCP**: Connects to the OpenAgents GitHub MCP server
2. **Local GitHub MCP**: Runs a local GitHub MCP client using npx
3. **Local Shell MCP**: Runs a local shell MCP client using uvx

These defaults can be modified or disabled through the settings interface.

## Client Status

Each client displays its connection status:

- **Connected**: The client is initialized and responding
- **Disconnected**: The client is not initialized (either disabled or not started)
- **Error**: The client failed to initialize (with an error message)

## Refreshing Clients

Clients can be refreshed (reinitialized) without restarting the application:

- Use the refresh button next to a specific client to reinitialize just that client
- Use the "Refresh All" button to reinitialize all enabled clients

## Security Considerations

When configuring MCP clients, keep these security considerations in mind:

1. **Command Execution**: stdio clients can execute commands on your system. Be careful what commands you configure.
2. **Authentication Tokens**: When providing authentication tokens (like GitHub tokens), ensure they have the minimum required permissions.
3. **Network Access**: SSE clients establish network connections. Ensure you're connecting to trusted servers.

## Implementation Details

Under the hood, MCP client configurations are stored in the settings repository and loaded at application startup. The implementation includes:

1. **Data Model**: `MCPClientConfig` interface in the settings schema
2. **Storage**: Client configurations stored in the RxDB database
3. **Dynamic Initialization**: Clients initialized based on stored configurations
4. **Status Tracking**: Connection status stored and updated in settings
5. **API Layer**: RESTful endpoints for client operations

For more details, see the [MCP Client Implementation](./issues/839/implementation.md) document.