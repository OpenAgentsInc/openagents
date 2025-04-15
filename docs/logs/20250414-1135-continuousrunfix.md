# WebSocket Command Implementation for Continuous Run

## Problem Analysis

Two key issues were identified:

1. **Method Access Issue**: The frontend client obtained from `useAgent` doesn't automatically expose methods decorated with `@unstable_callable`. The error message `TypeError: agent.startContinuousRun is not a function` confirmed this limitation.

2. **Tool Usage Issue**: After fixing the first issue, a new error occurred where the LLM was trying to use the `scheduleTask` tool during continuous run mode with invalid arguments, causing an `AI_InvalidToolArgumentsError`. The LLM was misinterpreting its role during continuous run and trying to self-schedule using the wrong mechanism.

## Implementation Strategy

To address both issues:

1. **For Method Access Issue**:
   - Replace direct method calls with a command pattern using WebSocket messages
   - Enhance the `onMessage` handler to process structured command messages
   - Remove unnecessary `@unstable_callable` decorators

2. **For Tool Usage Issue**:
   - Add clear guidance in the system prompt to prevent the LLM from using the `scheduleTask` tool during continuous runs
   - Ensure `continueInfer` continues to use the direct `this.schedule` method for self-scheduling

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

### 3. System Prompt Changes (prompts.ts)

Added explicit guidance to the system prompt to prevent the LLM from trying to use the `scheduleTask` tool for continuous run rescheduling:

```diff
// Add usage guidelines
systemPrompt += `\n\nGUIDELINES:
1. USE TOOLS to gather information before suggesting or implementing changes
2. CREATE DETAILED PLANS for complex tasks, breaking them into smaller steps
3. FOLLOW CODING CONVENTIONS and patterns in the existing codebase
4. PRESERVE existing functionality when adding new features
5. MAINTAIN ERROR HANDLING and proper testing
6. BE AUTONOMOUS - solve problems independently when possible
7. REPORT DETAILED PROGRESS and explain your decisions
8. EXECUTE TASKS systematically and step-by-step
+9. CONTINUOUS RUN: When operating in continuous run mode (triggered by startContinuousRun), focus on performing the exploration/analysis steps for the current cycle. The rescheduling of the next cycle is handled automatically by the continueInfer function; you do not need to use the scheduleTask tool for self-rescheduling during a continuous run.`;
```

### 4. Verification of continueInfer Method

Confirmed that the `continueInfer` method in server.ts correctly uses `this.schedule` for self-scheduling:

```typescript
public async continueInfer(payload?: any) {
  console.log(`[continueInfer] Agent waking up. Payload: ${JSON.stringify(payload)}`);
  try {
    // Perform main thinking loop
    await this.infer();

    // Check if we should continue running (based on state flag)
    if (this.state.isContinuousRunActive) {
      const delayInSeconds = 60; // Run again in 60 seconds
      console.log(`[continueInfer] Rescheduling self in ${delayInSeconds} seconds.`);
      await this.schedule(delayInSeconds, 'continueInfer', { reason: 'continuous execution' });
    } else {
      console.log(`[continueInfer] Not rescheduling - continuous run inactive.`);
    }
  } catch (error) {
    console.error("[continueInfer] Error during inference or rescheduling:", error);
    // Consider rescheduling even on error, maybe with backoff?
    if (this.state.isContinuousRunActive) {
      const delayInSeconds = 300; // Reschedule after 5 mins on error
      console.log(`[continueInfer] Rescheduling self after error in ${delayInSeconds} seconds.`);
      await this.schedule(delayInSeconds, 'continueInfer', { reason: 'error recovery' });
    }
  }
}
```

## Advantages of This Approach

1. **Standard Communication Pattern**: Uses a WebSocket message pattern that's already supported by the `useAgent` hook rather than trying to use RPC
2. **Clear Role Separation**: Explicit guidance prevents the LLM from confusing its role during continuous run mode
3. **Error Prevention**: Avoids invalid tool calls by directing the LLM away from using `scheduleTask` during continuous run cycles
4. **Extensible**: Framework supports adding more command types in the future

## Testing Considerations

1. **Functional Test**: Verify that clicking the play/pause button in the UI properly toggles the continuous run state
2. **Error Prevention Test**: Confirm the LLM no longer attempts to use the `scheduleTask` tool during continuous run cycles
3. **Cycle Completion Test**: Verify that multiple continuous run cycles complete successfully without errors

## Next Steps

1. Consider adding more commands to the system using this pattern for other agent control functions
2. Add TypeScript interfaces for message types to ensure type safety
3. Enhance error logging and visualization in the UI for better debugging