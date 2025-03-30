# Replacing Custom SDK Bridge with Official Cloudflare Agents SDK

## Summary
In this PR, we replaced the custom agent-sdk-bridge implementation with the official Cloudflare Agents SDK hooks and libraries. This change significantly improves the reliability and maintainability of our WebSocket connections to Cloudflare Agents while reducing code complexity.

## Key Changes

1. **Custom WebSocket Bridge Removed**:
   - Deleted the custom WebSocket implementation in `/packages/core/src/mcp/agent-sdk-bridge.ts`
   - This removes approximately 700 lines of complex WebSocket management code

2. **Official SDK Integration**:
   - Added direct imports for `useAgent` from 'agents/react'
   - Added direct imports for `useAgentChat` from 'agents/ai-react'
   - Converted agent-connection.ts to a minimal type wrapper around the official SDK

3. **Simplified useChat Implementation**:
   - Modified useChat to use the official SDK hooks directly
   - Removed the extra utility layer between hooks and SDK
   - Direct agent.call() usage for RPC methods instead of going through wrappers
   - Better message persistence with official message synchronization 

4. **Configuration Updates**:
   - Updated dependencies in package.json
   - Configured moduleResolution to "bundler" for better ESM support

## Benefits

1. **Simplified Codebase**:
   - Removed complex custom WebSocket management code
   - Removed redundant wrapper functions that added no value
   - Better alignment with official SDK documentation and examples

2. **Improved Reliability**:
   - Fixed WebSocket connection issues with direct SDK usage
   - Fixed case sensitivity problems in agent names
   - Better message persistence between sessions
   - Fixed RPC method timeout issues by using SDK directly

3. **Better Debugging**:
   - Official SDK provides better error messages and connection status
   - Improved logging for connection and message handling

4. **Future Compatibility**:
   - Easier to upgrade as the Agents SDK evolves
   - Direct access to new features as they are added to the SDK

## Implementation Details

1. **useChat.ts Changes**:
   - Now uses useAgent and useAgentChat hooks directly
   - Directly calls agent.call('method', [...args]) instead of going through utils
   - Uses agent.close() for disconnection
   - Preserves local command execution functionality
   - Simplifies project context setting

2. **agent-connection.ts Changes**:
   - Converted to a minimal type interface and re-export layer
   - Removed all redundant functions like createAgentUtils
   - Simply re-exports the official SDK hooks and types

3. **Type Handling**:
   - Updated tsconfig.json moduleResolution to "bundler"
   - Added necessary type casts to handle conflicts between SDK types and local types
   - Added StepStartUIPart support to Message types

## Testing

To verify these changes:
1. Test WebSocket connections to coderagent and other agents
2. Verify message persistence between sessions
3. Test RPC method calls like setProjectContext and getMessages
4. Test command execution through the agent