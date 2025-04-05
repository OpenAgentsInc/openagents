# Tool Selection Fix - Issue #854

## Problem Description

There was a critical disconnect in the tool selection system between the user interface and the server handling chat requests. This resulted in:

1. Tools disabled in the settings still being available to AI models
2. Tools selected in the message input not being respected in chat requests 
3. No proper end-to-end propagation of tool selections

## Root Causes Identified

1. **Server-Side Override**: The server's `getEnabledToolIds()` function automatically included all MCP tools regardless of user settings
2. **Missing Parameter Propagation**: The `selectedToolIds` parameter wasn't being properly forwarded to API requests
3. **Body Options Missing**: The `body` options in the Vercel AI SDK weren't being updated with selected tools

## Changes Made

### 1. ChatInputArea Component

Updated the message submission to include proper tool selection and debugging information:

```typescript
// Include the selected tools in the submission
const submissionOptions = {
  // Pass the tools that have been explicitly selected in the UI
  // The server should use these specific tool IDs rather than all available tools
  selectedToolIds: selectedToolIds,
  // Add a debug flag to trace the tool selection issues
  debug_tool_selection: true
};

console.log('[ChatInputArea] Submitting with explicitly selected tools:', selectedToolIds);
```

### 2. usePersistentChat Hook

Enhanced the submission handling to ensure selected tools are forwarded to the API:

```typescript
// If tools are explicitly selected, add them to the body options for the API call
if (Array.isArray(selectedToolIds)) {
  console.log('[usePersistentChat] Forwarding selected tool IDs to API:', selectedToolIds);

  // Create a copy of the current options
  const currentOptions = { ...customOptions };
  
  // Modify the body to include selectedToolIds
  currentOptions.body = {
    ...currentOptions.body,
    selectedToolIds: selectedToolIds,
  };
  
  // Create a temporary useChat instance with the modified options
  vercelChatState.body = currentOptions.body;
} 
```

### 3. Server-Side API Changes

Improved the server's handling of tool filtering with better logging and explicit filtering:

```typescript
// Filter tools based on user selection for this request
if (selectedToolIds.length > 0) {
  console.log('[Server] Filtering tools based on explicit user selection:', selectedToolIds);
  
  const filteredTools: Record<string, any> = {};
  for (const toolId of selectedToolIds) {
    if (combinedTools[toolId]) {
      console.log(`[Server] Including selected tool: ${toolId}`);
      filteredTools[toolId] = combinedTools[toolId];
    } else {
      console.log(`[Server] Selected tool not available: ${toolId}`);
    }
  }
  
  // This is the important part - ONLY use the tools explicitly selected by the user
  // rather than all enabled tools
  combinedTools = filteredTools;
  
  console.log(`[Server] Final tool selection: ${Object.keys(combinedTools).join(', ')}`);
}
```

## Testing and Verification

These changes ensure that:

1. Only tools enabled in settings are available to the AI
2. Only tools explicitly selected in the message input are used in a given chat request
3. The entire pipeline from UI to server properly respects user tool selections
4. Debug logging is available at every step to troubleshoot any issues

## Future Recommendations

1. **UI Feedback**: Add more visible feedback when tools are being enabled/disabled
2. **Default Tool Selection**: Provide better guidance for users about which tools to select
3. **Caching**: Add caching of enabled tool IDs to improve performance
4. **Error Handling**: Add specific error messages for tool selection issues

The implemented solution ensures proper control over tool usage while maintaining backward compatibility with existing code.