# Issue #854: Implement Configurable Tool Selection

## Overview
This issue involves creating a comprehensive tool management system similar to the existing model selection mechanism, allowing users to:
1. Globally enable/disable tools in the application settings
2. Select a subset of tools for each individual chat request

## Implementation Requirements

### 1. Central Tool Definition
- Create `packages/core/src/tools/TOOLS.ts` to define all available tools
- Implement a `ToolDefinition` interface with:
  - `id`: Unique identifier for the tool
  - `name`: Human-readable name
  - `description`: Explanation of the tool's purpose
  - `type`: Either 'builtin' or 'mcp'
  - `schema`: JSON schema defining the tool's parameters/structure
  - `serverIdentifier`: ID used when communicating with server

### 2. Settings Management
- Extend `useSettings` hook to manage:
  - `enabledToolIds`: Array of enabled tool IDs
  - `toggleToolEnabled()`: Method to enable/disable a tool
  - `getEnabledToolIds()`: Method to get currently enabled tools

### 3. UI Components
- Create `ToolsPage.tsx` for global tool management in settings
- Create `ToolSelect.tsx` component for per-request tool selection
- Update routing and settings layout to include the new pages/components

### 4. API Integration
- Modify chat endpoint to:
  - Accept selected tool IDs as part of chat requests
  - Verify tools are globally enabled
  - Filter and pass appropriate tools to the language model

### 5. MCP Tool Handling
- Discover available MCP tools via `tools/list` endpoint
- Handle dynamic tool updates
- Implement robust error handling for tool execution

## Implementation Plan
1. Create the central tools definition file
2. Extend settings hooks for tool management
3. Implement UI components for tool selection
4. Modify API endpoints to handle tool filtering
5. Test and integrate MCP tool discovery

This implementation will follow the pattern established by the model selection mechanism, ensuring a consistent user experience while providing powerful customization options for tool usage.