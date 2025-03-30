# Replacing Custom SDK Bridge with Official Cloudflare Agents SDK

## Summary
In this PR, we replaced the custom agent-sdk-bridge implementation with the official Cloudflare Agents SDK hooks and libraries. This change significantly improves the reliability and maintainability of our WebSocket connections to Cloudflare Agents.

## Key Changes

1. **Custom WebSocket Bridge Removed**:
   - Deleted the custom WebSocket implementation in `/packages/core/src/mcp/agent-sdk-bridge.ts`
   - This removes approximately 700 lines of complex WebSocket management code

2. **Official SDK Integration**:
   - Added direct imports for `useAgent` from 'agents/react'
   - Added direct imports for `useAgentChat` from 'agents/ai-react'
   - Updated agent-connection.ts to be a thin wrapper around the official SDK

3. **Improved useChat Implementation**:
   - Modified useChat to use the official SDK hooks directly
   - Better message persistence with official message synchronization 
   - Fixed RPC method timeout issues

4. **Configuration Updates**:
   - Updated dependencies in package.json
   - Added proper module configuration

## Benefits

1. **Simplified Codebase**:
   - Removed complex custom code that was difficult to maintain
   - Better alignment with official SDK documentation and examples

2. **Improved Reliability**:
   - Fixed WebSocket connection issues
   - Fixed case sensitivity problems in agent names
   - Better message persistence between sessions

3. **Better Debugging**:
   - Official SDK provides better error messages and connection status
   - Improved logging for connection and message handling

4. **Future Compatibility**:
   - Easier to upgrade as the Agents SDK evolves
   - Direct access to new features as they are added to the SDK

## Testing

To verify these changes:
1. Test WebSocket connections to coderagent and other agents
2. Verify message persistence between sessions
3. Test RPC method calls like setProjectContext and getMessages
4. Test command execution through the agent