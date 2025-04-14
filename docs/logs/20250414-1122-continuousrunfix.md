# WebSocket Command Implementation for Continuous Run

## Problem Analysis

The continuous run functionality was implemented with methods in the `Coder` agent class decorated with `@unstable_callable`:

```typescript
@unstable_callable({
  description: "Start continuous agent execution that persists until explicitly stopped"
})
async startContinuousRun() { /* implementation */ }
```

However, the frontend client obtained from `useAgent` doesn't automatically expose these methods. The error message `TypeError: agent.startContinuousRun is not a function` confirmed that the client-side code couldn't directly call these methods.

## Implementation Strategy

The solution replaces direct method calls with a command pattern using WebSocket messages:

1. **Backend (server.ts)**: Enhanced the `onMessage` handler to process structured command messages
2. **Frontend (website route)**: Modified the toggle function to send commands via WebSocket instead of trying to call methods directly
3. **Cleanup**: Removed unnecessary `@unstable_callable` decorators that were causing confusion

## Changes Made

### 1. Backend Changes (server.ts)

a) Enhanced the `onMessage` handler to process command messages:

```typescript
onMessage(connection: Connection, message: WSMessage) {
  try {
    const parsedMessage = JSON.parse(message as string);
    console.log("ON MESSAGE RECEIVED:", parsedMessage);

    // --- Command Handling ---
    if (parsedMessage.type === 'command' && parsedMessage.command) {
      console.log(`Processing command: ${parsedMessage.command}`);
      switch (parsedMessage.command) {
        case 'startContinuousRun':
          this.startContinuousRun().catch(e => console.error("Error starting continuous run from command:", e));
          break;
        case 'stopContinuousRun':
          this.stopContinuousRun().catch(e => console.error("Error stopping continuous run from command:", e));
          break;
        default:
          console.warn(`Received unknown command: ${parsedMessage.command}`);
      }
      return; // Exit after processing command
    }

    // --- Existing GitHub Token Logic ---
    if (parsedMessage.githubToken) {
      console.log("Processing githubToken update...");
      const githubToken = parsedMessage.githubToken;
      this.updateState({ githubToken });
      this.infer();
      return;
    }

    console.warn("Received unhandled message structure:", parsedMessage);
  } catch (error) {
    console.error("Error processing received message:", error);
    console.error("Raw message data:", message);
  }
}
```

b) Removed `@unstable_callable` decorators from the continuous run methods:

```typescript
/**
 * Starts continuous agent execution
 */
async startContinuousRun() {
  // existing implementation unchanged
}

/**
 * Stops continuous agent execution
 */
async stopContinuousRun() {
  // existing implementation unchanged
}
```

### 2. Frontend Changes (apps/website/app/routes/agent/$agentId.tsx)

Updated the `handleToggleContinuousRun` function to use WebSocket messaging:

```typescript
const handleToggleContinuousRun = () => { // Make it non-async, send is usually fire-and-forget
  if (!agent || connectionStatus !== 'connected') return;

  const currentlyActive = rawState?.isContinuousRunActive || false;
  const command = currentlyActive ? 'stopContinuousRun' : 'startContinuousRun';
  console.log(`Sending command: ${command}`);

  try {
    // Send a structured command message via WebSocket
    agent.send(JSON.stringify({
      type: 'command',
      command: command,
    }));
    console.log(`Sent ${command} command via WebSocket`);
    // State update will come via onStateUpdate
  } catch (error) {
    console.error(`Error sending ${command} command:`, error);
    setConnectionError(`Failed to send ${command} command: ${error.message || 'Unknown error'}`);
  }
};
```

## Advantages of This Approach

1. **Standard Communication Pattern**: Uses a WebSocket message pattern that's already supported by the `useAgent` hook rather than trying to use RPC
2. **Reduced Confusion**: No longer trying to call methods that aren't directly exposed on the client object
3. **Explicit Command Structure**: Messages have a clear `type: 'command'` identifier to distinguish them from other message types
4. **Extensible**: New commands can be easily added to the switch statement in the future

## Testing Considerations

1. **Functional Test**: Verify that clicking the play/pause button in the UI properly toggles the continuous run state
2. **State Update Test**: Confirm that the button's appearance updates correctly after state changes
3. **Error Handling**: Verify that connection errors are properly reported to the user

## Security Implications

The command-based approach doesn't introduce new security risks, as:
1. The commands are processed through the existing authenticated WebSocket connection
2. Only predefined commands are accepted (whitelist approach in the server switch statement)
3. No user input is directly passed to command execution

## Next Steps

1. Consider adding more commands to the system using this pattern for other agent control functions
2. Add TypeScript interfaces for message types to ensure type safety
3. Consider implementing command acknowledgments for more robust error handling
4. Document this pattern for other developers working with agent functionality