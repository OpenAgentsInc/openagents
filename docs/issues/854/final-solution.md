# Issue #854: Configurable Tool Selection - Final Solution

## Overview

I've implemented a comprehensive tool management system that allows users to both globally enable/disable tools in the application settings and select a subset of tools for each individual chat request, similar to how model selection works. Additionally, tools are now organized by their providers (MCP clients), making it easier to manage groups of related tools.

## Key Components Implemented

### 1. Core Tool Definition

Created the central tool definition in `packages/core/src/tools/TOOLS.ts`:
- Defined the `ToolDefinition` interface with necessary fields
- Implemented static definitions for built-in tools
- Added utility functions to extend with dynamic MCP tools
- Added provider tracking capabilities (providerId, providerName)

### 2. Settings Management

Extended the settings system to handle tool enablement:
- Updated `Settings` interface to include `enabledToolIds`
- Added methods to `SettingsRepository` for tool management:
  - `toggleToolEnabled()`
  - `enableTool()`
  - `disableTool()`
  - `getEnabledToolIds()`
- Added corresponding methods to the `useSettings` hook

### 3. MCP Tool Tracking

Modified MCP clients management to track tools by provider:
- Added `clientTools` to the `MCPClients` interface to track which tools belong to which client
- Enhanced the `refreshTools()` function to maintain the mapping
- Updated the `extendWithMCPTools()` utility to incorporate provider information into tool definitions

### 4. UI Components

Created new UI components for tool management:
- `ToolsPage.tsx` for global tool configuration:
  - Organized tools by provider in collapsible groups
  - Added provider-level enable/disable all functionality
  - Visual indicators for enabled status at the provider level
  - Search functionality across all tools
  
- `tool-select.tsx` for per-request tool selection:
  - Grouped tools by provider in the dropdown
  - Added provider-level selection actions (Select All / Clear)
  - Visual indicators for selection state
  - Improved search that works with provider groups
  
- Updated routing in `routes.tsx` to include the Tools page
- Added the Tools page link to the settings sidebar

### 5. Chat Integration

Modified the chat system to handle tool filtering:
- Updated the chat endpoint to accept selected tool IDs
- Implemented global enablement verification
- Added filtering based on user selection

## User Flow

### 1. Global Tool Management
   - Users navigate to Settings → Tools
   - They see tools organized by provider in collapsible sections
   - They can enable/disable individual tools or all tools from a provider at once
   - These settings persist across chat sessions

### 2. Per-Request Tool Selection
   - In the chat interface, users can select which tools to use for a specific request
   - Tools are organized by provider in the dropdown
   - Only globally enabled tools appear in this selector
   - Users can select/deselect all tools from a provider at once
   - Selected tools are sent with the chat request

### 3. Tool Filtering
   - The server checks if requested tools are globally enabled
   - It filters the tools based on user selection
   - Only the selected and enabled tools are passed to the language model

## Key Features

### Provider Grouping
- Tools are grouped by their source (built-in or specific MCP client)
- Visual distinction between built-in and MCP tools
- Provider-level actions for bulk enabling/disabling tools
- Collapsible sections for better organization

### Visual Indicators
- Badge showing how many tools are selected/enabled
- Color coding for different tool types
- Provider status indicators (All Enabled, Some Enabled)
- Tool counts for each provider

### Improved Search
- Search across all tools regardless of provider
- Filter view that shows only matching tools
- Provider information retained in search results

## Migration Handling

Implemented proper database schema migration:
- Updated schema version from 2 to 3
- Added migration strategies to handle existing databases
- Provided sensible defaults for new fields
- Graceful degradation for older databases

## Future Enhancements

1. **Tool Categories**: Further categorize tools by functionality (file operations, search, etc.)
2. **Model-Tool Compatibility**: Define which tools work with which models
3. **Tool Usage Analytics**: Track which tools are used most frequently
4. **User Presets**: Allow saving combinations of tools for quick selection

## Conclusion

This implementation provides a flexible and user-friendly way to manage AI tools, giving users control over which capabilities are available globally and for specific chat requests. The addition of provider grouping significantly improves the organization and usability of tools, especially as more MCP clients are added to the system.