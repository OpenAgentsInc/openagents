# Structure Refactoring for useChat Hook

Based on the detailed analysis of the current implementation, I'll now focus on making the initial structural changes to refactor the `useChat` hook in `packages/core/src/chat/useChat.ts`.

## 1. Confirming Unconditional Hook Calls

First, I've verified that the hook already calls the three necessary hooks unconditionally at the top level:

```typescript
// Always call useAgent with the same parameters (React hooks must be called unconditionally)
// Create options object first, then add the headers property if needed
// Always normalize agent ID to lowercase to prevent reconnection loops
const normalizedAgentId = agentId?.toLowerCase() || 'coderagent';

// ... [some code omitted] ...

const agentOptions1 = {
  agent: normalizedAgentId, // Ensure agent ID is always lowercase
  name: agentName || agentOptions?.agentName || 'default',
  host: agentServerUrl || agentOptions?.serverUrl || 'https://agents.openagents.com',
  onStateUpdate: agentOptions?.onStateUpdate,
};

// Add headers through type assertion if auth token is provided
const agentOptionsWithHeaders = agentAuthToken 
  ? { ...agentOptions1, headers: { Authorization: `Bearer ${agentAuthToken}` } } as any
  : agentOptions1;
  
const agent = useAgent(agentOptionsWithHeaders);

// Always call useAgentChat (React hooks must be called unconditionally)
const agentChat = useAgentChat({
  agent, // Always pass the agent returned by useAgent
  initialMessages: chatOptions.initialMessages,
  // Disable the automatic fetch of initial messages that causes CORS errors
  getInitialMessages: null,
  // The connection will only be used if shouldUseAgent is true (checked in useEffect)
});

// Track the original useChat instance
const vercelChat = vercelUseChat({
  ...chatOptions,
  maxSteps: 15,
  api: shouldUseAgent ? undefined : "https://chat.openagents.com", // Don't use API if using agent
  onError: (error) => {
    console.error('Chat error:', error);
    chatOptions.onError?.(error);
  },
});
```

These hooks are already called unconditionally at the top level, which is good. The `useAgentChat` call already has `getInitialMessages: null` to disable automatic fetching, which we'll maintain.

## 2. Defining `isAgentActive`

Next, I'll add the `isAgentActive` constant after the hook calls to simplify conditional logic throughout the hook:

```typescript
// ... existing hook calls ...

// Flag to determine if agent mode is active (both should use agent AND connection is established)
const isAgentActive = shouldUseAgent && agentConnection.isConnected;

// ... rest of the hook ...
```

This flag will help simplify our conditional logic throughout the hook, providing a single source of truth for whether we're using the agent or not.

## 3. Refactoring `returnValue` (Initial Draft)

Now I'll restructure the `returnValue` object by defining `activeChat` and using it to determine which chat implementation to use for core properties:

```typescript
// Define activeChat based on whether agent is active
const activeChat = isAgentActive ? agentChat : vercelChat;

// Prepare return value with proper typing
const returnValue = {
  // Core chat properties from the active chat implementation
  messages: isAgentActive ? agentMessages : processedMessages,
  isLoading: activeChat.isLoading,
  error: activeChat.error,
  input: activeChat.input,
  handleInputChange: activeChat.handleInputChange,
  setMessages: activeChat.setMessages,
  reload: activeChat.reload,
  stop: activeChat.stop,
  
  // Temporarily comment out these as they will be redefined later
  // append: ...,
  // handleSubmit: ...,
  
  // Agent connection info
  agentConnection: {
    isConnected: agentConnection.isConnected,
    client: agentConnection.client
  },
  
  // Utilities specific to this implementation
  testCommandExecution,
  fetchMessages,
  
  // Combined command execution capability
  executeCommand: isAgentActive
    ? executeAgentCommand // Use agent execution when agent is active
    : (command: string) => {
        if (localCommandExecution) {
            return safeExecuteCommand(command, commandOptions);
        } else {
            console.error("❌ USECHAT: Cannot execute command. Agent not connected and local execution disabled.");
            return Promise.reject("Command execution not available.");
        }
      },
  executeAgentCommand, // Also keep the specific agent command function for explicit agent calls
};

// Add debugging properties
Object.defineProperties(returnValue, {
  localCommandExecution: {
    enumerable: true,
    value: localCommandExecution
  },
  isCommandExecutionEnabled: {
    enumerable: true,
    value: Boolean(localCommandExecution)
  },
  isAgentConnected: {
    enumerable: true,
    value: agentConnection.isConnected
  },
  isUsingAgent: {
    enumerable: true,
    value: isAgentActive // Use the new isAgentActive flag here
  }
});
```

### Notes on Changes Made:

1. **Added `isAgentActive` Flag**: This simplifies our conditional logic by providing a single flag that indicates whether the agent is active and connected.

2. **Defined `activeChat`**: This provides a clean way to access the active chat implementation's properties and methods.

3. **Restructured `returnValue`**: The core chat properties now come directly from `activeChat`, simplifying the code and making it clearer which implementation is being used.

4. **Temporarily Commented Out Append/HandleSubmit**: As instructed, I've temporarily stubbed these as they'll need more complex refactoring to handle command execution correctly.

5. **Used `isAgentActive` Flag**: I've updated all the conditional logic to use the new `isAgentActive` flag for consistency.

These initial changes provide a solid foundation for further refactoring. The next steps would involve:

1. Refactoring the `append` and `handleSubmit` functions to use the `isAgentActive` flag
2. Updating the command execution logic to work with both implementations
3. Simplifying the message fetching logic
4. Final cleanup and testing