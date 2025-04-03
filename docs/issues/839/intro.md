# Implementing MCP Client Configuration in Settings UI

## Current Understanding

Currently, the OpenAgents Coder application has hardcoded Model Context Protocol (MCP) client configurations in the server startup code. These MCP clients provide tool functionality to AI models via the Model Context Protocol, allowing them to perform actions like file operations, GitHub interactions, and shell commands.

The current implementation in `apps/coder/src/server/mcp-clients.ts` initializes three types of MCP clients at server startup:
1. A remote MCP client connecting to "https://mcp-github.openagents.com/sse"
2. A local GitHub MCP client that uses stdio transport
3. A local Shell MCP client that uses stdio transport

All these clients are initialized with hardcoded values during app startup in `main.ts`, and the server code in `server.ts` retrieves these clients via the `getMCPClients()` function.

## Required Changes

Based on the GitHub issue #839 and the MCP documentation, we need to:

1. Create a user-configurable settings interface for MCP clients
2. Enable users to add, edit, and remove MCP client configurations
3. Allow enabling/disabling specific MCP clients
4. Dynamically initialize MCP clients based on user configurations
5. Show connection status for each client

## Implementation Plan

1. **Data Model Updates**:
   - Create a schema for MCP client configuration in the settings repository
   - Design the data structure for storing client configurations

2. **MCP Client Module Changes**:
   - Modify `mcp-clients.ts` to load configurations from settings repository
   - Implement dynamic initialization of clients based on stored configurations
   - Add functions to reinitialize clients when configurations change

3. **Settings UI Implementation**:
   - Create a new `MCPModelsPage.tsx` component for the settings UI
   - Add UI elements for adding, editing, and removing client configurations
   - Implement connection status indicators for each client
   - Add form validation for client configurations

4. **Route and Navigation Updates**:
   - Add a new route for the MCP settings page in `routes.tsx`
   - Update the settings layout to include a navigation item for MCP clients

5. **User Experience Enhancements**:
   - Implement error handling and validation for configuration inputs
   - Add a "Test Connection" feature for clients
   - Provide helpful documentation and examples for common configurations

6. **Security Considerations**:
   - Implement validation for command configurations
   - Ensure secure storage of sensitive configuration data
   - Add warnings for potentially unsafe configurations

By implementing these changes, we'll move from a hardcoded MCP client approach to a flexible, user-configurable system that allows users to tailor their MCP client setup to their specific needs.