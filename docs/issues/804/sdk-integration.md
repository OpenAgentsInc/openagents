# Issue #804: WebSocket Connection to Cloudflare Agents

## SDK Integration

This PR replaces the custom agent-sdk-bridge with the official Cloudflare Agents SDK hooks and libraries. This change simplifies our codebase and provides better compatibility with the Agents SDK.

### Changes Made

1. **Removed Custom Implementation**:
   - Deleted the custom WebSocket bridge (`/packages/core/src/mcp/agent-sdk-bridge.ts`)
   - Updated `agent-connection.ts` to act as a thin wrapper around the official SDK

2. **Added Official SDK Hooks**:
   - Added direct imports for `useAgent` and `useAgentChat` from the official SDK
   - Updated the `useChat` hook to use these hooks directly

3. **Updated Configuration**:
   - Added proper dependencies to package.json
   - Updated tsconfig.json to work with ESM modules

4. **Enhanced Type Support**:
   - Added support for StepStartUIPart to our Message types
   - Fixed type incompatibilities between our definitions and SDK definitions

### Benefits

1. **Simplified Codebase**:
   - Removed ~700 lines of custom WebSocket implementation
   - Better maintainability by leveraging the official SDK

2. **Improved Reliability**:
   - Fixed connection and timeout issues
   - Better message persistence with official SDK support

3. **Better Integration**:
   - Direct compatibility with latest Cloudflare Agents features
   - Easier to upgrade as the SDK evolves

### Remaining Work

1. Some type definition issues that need to be resolved by configuring the module resolution properly
2. Integration testing for verifying functionality

## Testing Instructions

1. Test WebSocket connections to coderagent
2. Test message persistence between sessions
3. Test RPC method calls (setProjectContext, getMessages, etc.)
4. Test command execution through the agent