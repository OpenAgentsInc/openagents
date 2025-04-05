# Tool Selection API Fix

## Issue Summary

The tool selection component was displaying tools correctly, but when selected tools were passed to the chat API, they weren't being correctly included in the final stream options. This created a situation where users could select tools in the UI, but those selections had no effect on the actual chat request.

## Root Cause Analysis

After investigating the problem, I discovered several issues in how the tool selection data was being passed through the API chain:

1. **Inconsistent Request Structure**: 
   - The ChatInputArea was passing `selectedToolIds` inside a nested `body` property
   - The server-side chat route was expecting `selectedToolIds` directly in the request body
   - This mismatch caused the server to ignore the selected tools

2. **Data Path Differences**:
   - The vercel/ai SDK has a different expectation for how tool data is passed
   - Our custom server implementation had its own assumptions
   - These two systems weren't aligned, causing the tool selection to be lost

3. **Inadequate Debug Information**:
   - Without detailed logging, it was difficult to see where in the chain the tool selection was being lost
   - The request format being sent vs. expected wasn't clearly visible

## Solution Implemented

1. **Multiple Placement Strategy**:
   - Modified ChatInputArea to place `selectedToolIds` in both the root of options and in options.body
   - Updated usePersistentChat to ensure `selectedToolIds` is included in both places
   - This ensures compatibility with both our server and the vercel/ai SDK

2. **Enhanced Server-Side Extraction**:
   - Implemented a robust extraction function in the server to look for `selectedToolIds` in multiple places
   - Added deep traversal of the request body to find `selectedToolIds` wherever it might be
   - Made the server more flexible in accepting different request formats

3. **Comprehensive Debug Logging**:
   - Added detailed logging throughout the request chain
   - Implemented visual markers (emojis) to make important log entries stand out
   - Made the tool inclusion/exclusion process more transparent in logs

## Implementation Details

### 1. Client-Side (ChatInputArea.tsx)
```typescript
const submissionOptions = {
  // Place directly in options for direct server access
  selectedToolIds: toolsToUse,
  
  // Also place in body for vercel/ai SDK
  body: {
    selectedToolIds: toolsToUse,
    debug_tool_selection: true
  }
};
```

### 2. Middleware (usePersistentChat.ts)
```typescript
// Set in both locations for maximum compatibility
submissionOptions.selectedToolIds = selectedToolIds;
submissionOptions.body = {
  ...submissionOptions.body,
  selectedToolIds: selectedToolIds,
};
```

### 3. Server-Side (chat.ts)
```typescript
// Robust extraction that checks multiple locations
let selectedToolIds: string[] = [];

if (Array.isArray(body.selectedToolIds)) {
  selectedToolIds = body.selectedToolIds;
} 
else if (body.options?.body?.selectedToolIds) {
  selectedToolIds = body.options.body.selectedToolIds;
}
else {
  // Deep traversal to find selectedToolIds anywhere in the request
  // (implementation details omitted for brevity)
}
```

## Verification

The fix can be verified by:

1. Opening the tool selection dropdown
2. Selecting one or more tools
3. Sending a message
4. Observing server logs to confirm that:
   - The selected tools were correctly extracted from the request
   - Only the selected tools are included in the final stream options
   - The model can use those tools in its responses

This fix ensures that the entire tool selection flow works correctly from UI to server to model output.