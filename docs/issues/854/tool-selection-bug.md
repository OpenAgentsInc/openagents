# Tool Selection Bug in MCP Implementation

## Problem Description

There is a critical disconnect between the tool selection UI and the actual tools being made available to agents. The issue manifests in several ways:

1. **UI-Server Mismatch**: Tools that appear to be disabled in the settings UI are still being made available to the language models
2. **Control Ineffectiveness**: The tool selection control in the message input area is not functioning properly 
3. **Inconsistent State**: Different tool states appear in the settings page versus the message composition area

## Root Causes

After investigation, the following root causes have been identified:

1. **Server Override**: The server-side implementation in `chat.ts` is overriding the user's tool selections by automatically including all detected MCP tools
2. **Missing State Synchronization**: The tool selection UI is not properly communicating selections to the chat request
3. **Tool Handler Inconsistency**: The tools page in settings and the tool-select component are operating on different data sources

## Specific Implementation Issues

1. In `apps/coder/src/server/routes/chat.ts`:
   - The `getEnabledToolIds()` function is incorrectly ignoring the user's settings and returning all MCP tools by default:
   ```typescript
   // Always including all MCP tools regardless of user selection
   if (Object.keys(mcpTools).length > 0) {
     console.log('[Server] Including MCP tools in enabled tools list');
     const allTools = [...baseTools, ...Object.keys(mcpTools)];
     console.log('[Server] Enabled tool IDs:', allTools);
     return allTools;
   }
   ```

2. In `apps/coder/src/components/ui/tool-select.tsx`:
   - The component is rendering properly but event handlers for tool selection aren't propagating correctly

3. In the chat functionality:
   - Selected tools aren't being properly included in the chat request
   - No validation is happening to check if user selections match server availability

## Impact

These issues create a confusing and frustrating user experience:

1. Users believe they are controlling which tools are available, but their selections have no effect
2. The UI suggests control that doesn't actually exist
3. Tools that should be disabled are still being accessed by language models
4. Inconsistent behavior between settings and actual chat usage

## Fix Requirements

1. **Server-Side Fix**: Modify `getEnabledToolIds()` to respect user settings rather than including all tools
2. **Client-Side Fix**: Ensure tool-select component properly updates and communicates selections
3. **Chat Request Fix**: Validate that the chat request includes only the tools the user has selected
4. **UI Consistency**: Ensure the same set of enabled tools appears in both settings and message composition

The fix should maintain the ability to use MCP tools while giving users proper control over which tools are enabled and which ones are selected for each specific chat request.