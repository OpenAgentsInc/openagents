# MCP Tools Display Fix

## Issue Summary

After fixing the tool selection component's toggling functionality, there's another issue where registered MCP server tools aren't appearing in the tool selection dropdown. The UI can select and deselect tools, but it's not showing all available MCP tools from configured clients.

## Root Cause Analysis

After examining the code, I found several interconnected issues:

1. **Server-Side Tool Initialization**:
   - The MCP clients initialization is running server-side only (as it should)
   - When tools are refreshed, they're stored in a server-side `mcpClients.allTools` object
   - This works fine for actual API requests, but this data isn't exposed to the client side

2. **Client-Side Tool Fetching**:
   - The ToolSelect component tries to call `getMCPClients()` directly from the browser
   - In browser environments, this returns empty or mock data due to environment checks
   - There's no API endpoint to retrieve the actual MCP tools from the server

3. **Environment Mismatch**:
   - The `mcp-clients.ts` file has browser environment detection that prevents actual tool loading
   - The server-side tools and client-side tool representation are out of sync

## Solution Required

To fix this issue, we need to:

1. **Create a dedicated API endpoint** for fetching MCP tools:
   - Add a route at `/api/mcp/tools` that returns the actual server-side MCP tools
   - This endpoint should use the server's `getMCPTools()` function to get real data

2. **Modify the ToolSelect component** to fetch tools from the API:
   - Instead of directly calling `getMCPClients()`, make an API request to `/api/mcp/tools`
   - Parse the response and convert it to the ToolDefinition format

3. **Add a function to fetch tools from the server**:
   - Create a client-side utility that fetches tools from the API
   - Handle error cases and caching appropriately

4. **Ensure tool IDs match between client and server**:
   - Verify that the tool IDs used in the UI match those expected by the server
   - Use the same format for tool identifiers in both places

## Implementation Steps

1. Create a new MCP tools API endpoint in the server
2. Update the ToolSelect component to fetch tools via API
3. Add comprehensive error handling for network failures
4. Add tool refresh capability to the ToolSelect component

This approach will ensure that the tool selection component displays all available tools from the server, regardless of which environment (client or server) the code is running in.