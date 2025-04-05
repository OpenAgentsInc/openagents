# Issue #854: Configurable Tool Selection - Final Solution

## Overview

I've implemented a comprehensive tool management system that allows users to both globally enable/disable tools in the application settings and select a subset of tools for each individual chat request, similar to how model selection works.

## Key Components Implemented

### 1. Core Tool Definition

Created the central tool definition in `packages/core/src/tools/TOOLS.ts`:
- Defined the `ToolDefinition` interface with necessary fields
- Implemented static definitions for built-in tools
- Added utility functions to extend with dynamic MCP tools

### 2. Settings Management

Extended the settings system to handle tool enablement:
- Updated `Settings` interface to include `enabledToolIds`
- Added methods to `SettingsRepository` for tool management:
  - `toggleToolEnabled()`
  - `enableTool()`
  - `disableTool()`
  - `getEnabledToolIds()`
- Added corresponding methods to the `useSettings` hook

### 3. UI Components

Created new UI components for tool management:
- `ToolsPage.tsx` for global tool configuration
- `tool-select.tsx` for per-request tool selection
- Updated routing in `routes.tsx` to include the Tools page
- Added the Tools page link to the settings sidebar

### 4. Chat Integration

Modified the chat system to handle tool filtering:
- Updated the chat endpoint to accept selected tool IDs
- Implemented global enablement verification
- Added filtering based on user selection

## User Flow

1. Global Tool Management
   - Users navigate to Settings → Tools
   - They can enable/disable tools globally
   - These settings persist across chat sessions

2. Per-Request Tool Selection
   - In the chat interface, users can select which tools to use for a specific request
   - Only globally enabled tools appear in this selector
   - Selected tools are sent with the chat request

3. Tool Filtering
   - The server checks if requested tools are globally enabled
   - It filters the tools based on user selection
   - Only the selected and enabled tools are passed to the language model

## Future Enhancements

1. **Tool Discovery**: Implement automatic discovery of tools from MCP clients
2. **Tool Categories**: Group tools by category for easier management
3. **Model-Tool Compatibility**: Define which tools work with which models
4. **Tool Usage Analytics**: Track which tools are used most frequently

## Conclusion

This implementation provides a flexible and user-friendly way to manage AI tools, giving users control over which capabilities are available globally and for specific chat requests. The architecture follows the existing patterns for model selection, ensuring a consistent user experience.