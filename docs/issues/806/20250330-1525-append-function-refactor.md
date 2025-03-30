# Refactoring the `append` Function in useChat Hook

The `append` function is a crucial part of the `useChat` hook as it handles sending messages and processing commands. In the current implementation, it has complex logic to route messages to either the agent or the local chat API, and also handles command execution.

## Current Implementation of `append`

```typescript
// Custom append function that checks for commands or routes to agent
const append = useCallback(async (message: any) => { // TODO: Type compatibility issues between UIMessage and Message, using any as workaround
  // If using agent and connected, send message to agent via agentChat
  if (shouldUseAgent && agentConnection.isConnected && agentChat) {
    try {
      console.log('📤 USECHAT: Sending message to agent via official SDK:', message.role);
      
      // Use the official SDK to send the message
      // Ensure message format is compatible with agentChat.append
      const result = await agentChat.append(message); // Pass message directly
      console.log('✅ USECHAT: Message sent to agent successfully');
      
      return result;
    } catch (error) {
      console.error('❌ USECHAT: Failed to send message to agent:', error);
      // Call onError and don't fall back to originalAppend as that goes to a different API
      chatOptions.onError?.(error instanceof Error ? error : new Error(String(error)));
      // Return null to match originalAppend's potential void return
      return null;
    }
  }
  
  // If not using agent, use the original append function
  console.log('📤 USECHAT: Sending message via vercelUseChat');
  const result = await originalAppend(message); // Pass the potentially UIMessage
  
  // Skip command execution if it's not enabled
  if (!localCommandExecution) {
    return result;
  }
  
  // Check if this is a user message and parse commands (Local command execution part)
  if (message.role === 'user' && typeof message.content === 'string') {
    const commands = parseCommandsFromMessage(message.content);
    
    if (commands.length > 0 && result) {
      // Store commands for processing after the response is received
      pendingCommandsRef.current = {
        messageId: typeof result === 'object' && result !== null && 'id' in result ? (result as any).id || 'unknown' : 'unknown',
        commands,
        isProcessing: false
      };
    }
  }
  
  return result;
}, [
  shouldUseAgent, 
  agentConnection.isConnected,
  agentChat,
  localCommandExecution, 
  originalAppend,
  chatOptions.onError
]);
```

## Proposed Refactored `append` Function

In the refactored hook, we'll simplify the `append` function by using the `isAgentActive` flag and delegating most of the work to the active chat implementation. We'll still need to handle command execution logic for both agent and non-agent modes.

```typescript
// Custom append function that handles message sending and command detection
const append = useCallback(async (message: any) => { // Type compatibility issues remain for now
  try {
    // Log which implementation we're using
    console.log(`📤 USECHAT: Sending message via ${isAgentActive ? 'agent chat' : 'local chat'}`);
    
    // Use the active chat implementation's append method
    const result = await activeChat.append(message);
    
    // If agent is active or command execution is disabled, we're done
    if (isAgentActive || !localCommandExecution) {
      return result;
    }
    
    // --- LOCAL COMMAND EXECUTION LOGIC (only for non-agent mode) ---
    
    // Check if this is a user message and parse commands
    if (message.role === 'user' && typeof message.content === 'string') {
      const commands = parseCommandsFromMessage(message.content);
      
      if (commands.length > 0 && result) {
        // Store commands for processing after the response is received
        pendingCommandsRef.current = {
          messageId: typeof result === 'object' && result !== null && 'id' in result ? (result as any).id || 'unknown' : 'unknown',
          commands,
          isProcessing: false
        };
      }
    }
    
    return result;
  } catch (error) {
    console.error('❌ USECHAT: Failed to send message:', error);
    chatOptions.onError?.(error instanceof Error ? error : new Error(String(error)));
    return null;
  }
}, [
  isAgentActive, // Depend on isAgentActive instead of separate flags
  activeChat,    // Depend on activeChat which contains the active implementation
  localCommandExecution,
  chatOptions.onError
]);
```

## Refactoring Command Execution Logic

The command execution logic is spread across multiple effects and callbacks in the current implementation. In the refactored hook, we should maintain this functionality while making it clearer which parts apply to which mode.

```typescript
// Process local commands in assistant messages - only needed for non-agent mode
useEffect(() => {
  // Skip if command execution is disabled, no messages, or agent is active
  if (!localCommandExecution || messages.length === 0 || isAgentActive) {
    return;
  }
  
  // The rest of the command execution logic remains the same...
  // [Rest of the existing command execution logic]
  
}, [
  messages, 
  localCommandExecution, 
  isAgentActive, // Add isAgentActive dependency
  commandOptions, 
  onCommandStart, 
  onCommandComplete, 
  updateMessage, 
  originalAppend
]);
```

## Refactoring `handleSubmit` Function

We'll also need to refactor the `handleSubmit` function to use the active chat implementation:

```typescript
// Custom handleSubmit function to use the active chat's implementation
const handleSubmit = useCallback((e?: FormEvent<HTMLFormElement>) => {
  // Prevent default form submission behavior if event is provided
  e?.preventDefault();
  
  // Use the active chat implementation's handleSubmit method
  return activeChat.handleSubmit(e);
}, [activeChat]);
```

## Adding Refactored Functions to `returnValue`

Now we can update the `returnValue` object to include our refactored functions:

```typescript
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
  
  // Add our refactored functions
  append,
  handleSubmit,
  
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
    ? executeAgentCommand 
    : (command: string) => {
        if (localCommandExecution) {
            return safeExecuteCommand(command, commandOptions);
        } else {
            console.error("❌ USECHAT: Cannot execute command. Agent not connected and local execution disabled.");
            return Promise.reject("Command execution not available.");
        }
      },
  executeAgentCommand,
};
```

## Summary of Changes

1. **Simplified `append` Function**: Uses `activeChat` and `isAgentActive` to determine which implementation to use.

2. **Refactored Command Execution Logic**: Added a condition to skip command execution when agent is active, as agent handles commands differently.

3. **Added `handleSubmit` Function**: Uses the active chat implementation's `handleSubmit` method.

4. **Updated `returnValue`**: Added refactored functions to the return value.

These changes maintain all the functionality of the original implementation while making the code clearer and more maintainable. The refactored hook will still handle commands in both agent and non-agent modes, but with cleaner separation of concerns.