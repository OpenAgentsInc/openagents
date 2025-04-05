# MCP Tool Grouping and Selection Issues

## Overview of the Problem

We're experiencing issues with MCP tools not appearing in the tool selection dropdown, even though they show as enabled in the Tools settings page. Additionally, toggling tools on/off in the UI doesn't seem to persist their enabled/disabled state correctly.

## Current Implementation Issues

### 1. Browser Compatibility Issues

The first major issue we identified was browser incompatibility with Node.js `require()` calls:

- The code in `ToolsPage.tsx` and `tool-select.tsx` was using Node.js-style `require()` calls to import modules dynamically
- In browser environments (as opposed to Electron), `require()` is not defined, causing errors
- This was preventing MCP tools from being loaded in browser environments

### 2. Tool Visibility Issues

Even after fixing the browser compatibility issues, we still have issues with tools not appearing in the dropdown:

- When an MCP tool is enabled in the Tools settings page, it's not showing in the tool selection dropdown
- The tool selection dropdown filters tools based on what's in the `enabledToolIds` array from the settings database
- There seems to be a disconnection between:
  1. The UI that enables/disables tools in the Tools settings page
  2. The settings database that stores which tools are enabled/disabled
  3. The tool selection dropdown that filters tools based on the enabled status

### 3. Toggling Tool Status

The UI for toggling tools on/off in either the Tools page or the dropdown doesn't seem to correctly update the tool's enabled status:

- When clicking to enable/disable a tool, the UI changes but the state doesn't persist
- This suggests issues with the `toggleToolEnabled`, `enableTool`, and `disableTool` functions in the `useSettings` hook
- The database operations might not be completing successfully

## Technical Deep Dive

### Settings Repository and Tools Management

The following components are responsible for tool management:

1. **`useSettings` hook**:
   - Provides methods like `toggleToolEnabled`, `enableTool`, `disableTool`, and `getEnabledToolIds`
   - These methods delegate to the `settingsRepository`

2. **`settingsRepository`**:
   - Stores and retrieves settings from the database, including enabled tool IDs
   - The `enabledToolIds` array in the settings object tracks which tools are enabled
   - Default only includes `shell_command`

3. **Tool Selection UI**:
   - `ToolSelect` component in `tool-select.tsx` displays a dropdown of available tools
   - Only shows tools that are in the `enabledToolIds` array
   - Uses `getEnabledToolIds()` to fetch the list of enabled tools

4. **MCP Client Integration**:
   - MCP tools are fetched from MCP clients via `getMCPClients()` and `refreshTools()`
   - Tools are added to the system via `extendWithMCPTools()`
   - When associating tools with providers, there's a check to match tool IDs with provider IDs

### Attempted Fixes

1. **Fixing Browser Compatibility**:
   - Replaced Node.js `require()` calls with proper ES Module imports
   - Updated imports in both `ToolsPage.tsx` and `tool-select.tsx`
   - This resolved the "require is not defined" error

2. **Debug Logging**:
   - Added detailed logging to track:
     - What tools are available in the system
     - Which tools are enabled according to settings
     - What tools are being shown in the dropdown after filtering

3. **Temporary Fix for Dropdown Visibility**:
   - Modified `tool-select.tsx` to show all tools regardless of enabled status
   - This helps verify if MCP tools are being properly loaded but just not showing due to filtering

## Current Status

As of the temporary fix, we can see:

1. Built-in tools (shell_command) appear correctly
2. Mock MCP tools (in development mode) appear correctly
3. Real MCP tools from connected clients do appear in the tool list, but:
   - They don't show in the filtered dropdown without our temporary fix
   - Toggling their enabled status doesn't seem to work properly

## Next Steps for Resolution

1. **Investigate Toggling Logic**:
   - Debug the `toggleToolEnabled`, `enableTool`, and `disableTool` functions
   - Add logging to trace the full path from UI action to database update

2. **Fix Tool Persistence**:
   - Ensure that enabling a tool properly adds it to the `enabledToolIds` array in settings
   - Verify that database operations are completing successfully

3. **Fix Tool Selection Dropdown**:
   - Either continue with the temporary fix to show all tools
   - Or fix the enablement process so tools properly appear after being enabled

4. **Testing Across Environments**:
   - Test in both Electron and browser environments
   - Verify that the fixed solution works consistently

5. **Document the Final Solution**:
   - Update the documentation with the complete solution
   - Include information on how tool enablement works and how to debug issues

## Possible Solutions

1. **Fix Database Operations**:
   - Ensure that `enableTool` and `disableTool` correctly update the database
   - Add better error handling and user feedback when operations fail

2. **Alternative Filtering Approach**:
   - Consider changing the dropdown to show all tools but disable selection of tools that aren't enabled
   - This would make it clearer which tools are available but need to be enabled

3. **Simplified Tool Management**:
   - Consider a more direct approach to tool management that doesn't rely on complex database operations
   - Possibly use localStorage as a fallback when database operations fail