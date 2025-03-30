# Fixing Method Timeout Issues in CoderAgent

## Problem

After successfully resolving the WebSocket connection and message persistence issues, we encountered a problem with client-to-agent RPC calls timing out:

```
Failed to set project context: Error: Request timed out after 30s when calling method 'setProjectContext' on agent CoderAgent/default
```

```
Failed to fetch agent messages: Error: Request timed out after 30s when calling method 'getMessages' on agent CoderAgent/default
```

These timeouts prevented proper initialization of the agent and retrieval of message history.

## Root Cause

The RPC methods being called by the client (`setProjectContext` and `getMessages`) were defined in the CoderAgent class but were not properly marked as callable methods for RPC. The Agents SDK requires methods to be explicitly marked with the `@unstable_callable` decorator to expose them for remote procedure calls.

## Solution

Added the `@unstable_callable` decorator to the necessary methods in the CoderAgent class:

```typescript
import { unstable_callable } from "agents";

export class CoderAgent extends AIChatAgent<Env> {
  // ...
  
  /**
   * Set the project context for this agent
   * This method is marked as callable by clients through RPC
   */
  @unstable_callable({
    description: "Set the repository context for the coding agent"
  })
  setProjectContext(context: {
    repoOwner?: string;
    repoName?: string;
    branch?: string;
    path?: string;
  }) {
    console.log("📁 Setting project context:", context);
    this.projectContext = { ...this.projectContext, ...context };
    console.log("🔄 Updated project context:", this.projectContext);
    // Return the updated context to confirm success
    return { 
      success: true, 
      context: this.projectContext 
    };
  }
  
  /**
   * Get messages for this agent
   * This method is marked as callable by clients through RPC
   */
  @unstable_callable({
    description: "Get the message history for this agent"
  })
  getMessages() {
    console.log(`📋 Getting ${this.messages.length} messages from agent`);
    return this.messages;
  }
  
  // ...
}
```

## Technical Details

The Agents SDK uses a remote procedure call (RPC) mechanism for client-server communication. Methods that should be accessible via RPC must be:

1. Explicitly marked with the `@unstable_callable` decorator
2. Have serializable parameters and return values
3. Be implemented to handle asynchronous execution

Without the decorator, the method exists on the class but isn't registered with the SDK's internal dispatcher, leading to timeouts when clients attempt to call these methods.

## Testing

To verify this fix:

1. Deploy the updated code
2. Connect to the CoderAgent through WebSocket
3. Verify that `setProjectContext` calls complete successfully
4. Verify that `getMessages` calls return message history without timing out
5. Confirm that the agent initialization process completes properly

This fix ensures that the agent and client can properly communicate via RPC methods, completing the full solution for issue #804.