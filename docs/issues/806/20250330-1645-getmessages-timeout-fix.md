# Fixing getMessages Timeout Issue in useChat

## Problem Identified

After implementing the refactored `useChat` hook, we encountered an issue where the initial messages were not being loaded properly when connecting to an agent. The code was getting stuck at the point where it calls `agent.call('getMessages')`, showing the log message "📄 USECHAT: Agent active, attempting to fetch initial messages..." but never proceeding further.

This indicated that the promise returned by `agent.call('getMessages')` was never resolving or rejecting, leaving the UI in a hanging state.

## Root Cause Analysis

After investigating the codebase, we found potential issues in the `getMessages` RPC method implementation in the `CoderAgent` class (in `/packages/agents/src/coder-agent.ts`):

1. **Database Query Issues**:
   - The SQL query to fetch messages doesn't have a timeout parameter
   - There's no limit specified, so it could be trying to fetch a large number of messages

2. **Missing Error Propagation**:
   - Errors are caught and logged, but not propagated to the client
   - The method returns an empty array on error, making it difficult to diagnose issues

3. **No Internal Timeout**:
   - The method doesn't have an internal timeout mechanism
   - If the database query hangs, the entire method will hang indefinitely

## Solution Implemented

To address this issue, we implemented a client-side timeout mechanism in the `useChat` hook:

1. **Added Promise Timeout Wrapper**:
   - Created a `withTimeout` utility function that wraps promises with a timeout
   - Rejects the promise with a descriptive error if the timeout is exceeded

2. **Two-Stage Testing**:
   - First tests with `executeCommand` (3-second timeout) to verify basic RPC connectivity
   - Then attempts `getMessages` (5-second timeout) if the test succeeds

3. **Comprehensive Fallbacks**:
   - Added fallbacks at every stage to ensure the UI never hangs
   - Even if all RPC calls fail, the hook will:
     - Use `initialMessages` if available
     - Fall back to an empty array if not

4. **Improved Logging**:
   - Added more detailed logs to help diagnose issues
   - Clearly indicates timeouts vs. regular rejections
   - Provides information about the stage at which any failure occurs

## Code Implementation

The key part of the implementation is the `withTimeout` utility function:

```typescript
const withTimeout = <T>(promise: Promise<T>, timeoutMs: number, operation: string): Promise<T> => {
  return new Promise<T>((resolve, reject) => {
    // Set up timeout
    const timeoutId = setTimeout(() => {
      reject(new Error(`Timeout (${timeoutMs}ms) exceeded for ${operation}`));
    }, timeoutMs);
    
    // Execute original promise
    promise
      .then(result => {
        clearTimeout(timeoutId);
        resolve(result);
      })
      .catch(error => {
        clearTimeout(timeoutId);
        reject(error);
      });
  });
};
```

This function is then used to wrap the RPC calls:

```typescript
withTimeout(agent.call('getMessages'), 5000, "getMessages call")
  .then(...)
  .catch(...);
```

## Benefits of this Approach

1. **Prevents UI Hanging**: The UI will never get stuck in a loading state, as all promises now have timeouts
2. **Graceful Degradation**: Falls back to default behavior when RPC calls fail
3. **Diagnostic Information**: Provides clear logs to help diagnose issues
4. **Backward Compatible**: Continues to work with the existing agent implementation

## Future Improvements

While this client-side fix addresses the immediate issue, the root cause should be fixed in the agent implementation:

1. **Add Database Query Limits**: Modify the `getMessages` RPC method to include a LIMIT clause
2. **Add Server-Side Timeout**: Implement a timeout in the agent's database query
3. **Improve Error Handling**: Propagate errors to the client rather than swallowing them
4. **Consider Pagination**: For large message histories, implement pagination to avoid timeouts

These server-side improvements would be a more robust long-term solution.