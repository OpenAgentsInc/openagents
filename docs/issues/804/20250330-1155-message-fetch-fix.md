# Agent Message Fetching Fix

## Problem Description

After switching to the official Cloudflare Agents SDK, we encountered an issue with message history not being fetched and displayed after the agent connection was established. This happened because:

1. In the official SDK, the `useAgentChat` hook has a built-in HTTP fetch mechanism to get initial messages through `getInitialMessages` option
2. We disabled this option (`getInitialMessages: null`) because it was causing CORS errors
3. However, we didn't implement a replacement mechanism to fetch messages using the non-CORS prone RPC method

## Solution

We implemented a new approach to fetch messages after the agent connection is established, using the agent's RPC `getMessages` method:

1. Added a `initialMessagesFetchedRef` to track whether messages have been fetched for the current connection
2. Created a new useEffect hook that runs when the agent connection status changes
3. Inside this effect, if the agent is connected and messages haven't been fetched yet, we call `agent.call('getMessages')`
4. We then use `agentChat.setMessages()` to populate the chat state with the fetched messages
5. Added proper error handling and fallback logic if the fetch fails

## Implementation Details

### Tracking Fetch Status

We added a ref to track the fetch status:

```typescript
// Ref to track if initial messages have been fetched from agent
const initialMessagesFetchedRef = useRef(false);
```

### Message Fetch Effect

We implemented a new effect to fetch messages after connection:

```typescript
// New Effect to Fetch Initial Messages
useEffect(() => {
  // Only fetch if using agent, connected, agentChat is ready, and not already fetched
  if (
      shouldUseAgent &&
      agentConnection.isConnected &&
      agent && 
      agentChat.setMessages && 
      !initialMessagesFetchedRef.current 
     )
  {
    console.log('📄 USECHAT: Agent connected, attempting to fetch initial messages...');
    initialMessagesFetchedRef.current = true; // Set flag immediately to prevent re-fetch attempts

    agent.call('getMessages')
      .then((fetchedMessages: any) => {
        if (fetchedMessages && Array.isArray(fetchedMessages) && fetchedMessages.length > 0) {
          console.log(`✅ USECHAT: Fetched ${fetchedMessages.length} initial messages from agent.`);
          // Use setMessages from useAgentChat to populate the history
          agentChat.setMessages(fetchedMessages);
        } else {
          console.log('ℹ️ USECHAT: No initial messages found on agent or fetch returned empty/invalid.');
           // Fallback to initial messages if provided
           if (chatOptions.initialMessages && chatOptions.initialMessages.length > 0) {
              console.log('ℹ️ USECHAT: Falling back to chatOptions.initialMessages (if any).');
              agentChat.setMessages(chatOptions.initialMessages);
           } else {
              agentChat.setMessages([]); // Ensure it's at least an empty array
           }
        }
      })
      .catch((error: Error) => {
        console.error('❌ USECHAT: Failed to fetch initial messages from agent:', error);
        // Potentially fall back to chatOptions.initialMessages on error
        if (chatOptions.initialMessages && chatOptions.initialMessages.length > 0) {
           console.log('ℹ️ USECHAT: Falling back to chatOptions.initialMessages due to fetch error.');
           agentChat.setMessages(chatOptions.initialMessages);
        }
      });
  }
}, [
    shouldUseAgent,
    agentConnection.isConnected,
    agent,
    agentChat.setMessages,
    chatOptions.initialMessages 
]);
```

### Reset Logic

We also added logic to reset the fetch flag when the agent disconnects or the component unmounts:

```typescript
// Reset fetch status if agent is deselected
if (!shouldUseAgent) {
  initialMessagesFetchedRef.current = false;
}

// In cleanup function
initialMessagesFetchedRef.current = false; // Reset fetch flag on unmount
```

## Benefits

This implementation:

1. Properly fetches message history after agent connection is established
2. Avoids the CORS issues that occurred with the default HTTP fetch mechanism
3. Maintains proper state across component rerenders and connection changes
4. Includes fallback logic for error cases
5. Doesn't interfere with the existing connection management logic

## Related Changes

In addition to implementing the message fetch mechanism, we made several improvements to the agent connection management:

1. Improved type definitions for better type safety
2. Enhanced the agent connection effect with better connection/disconnection handling
3. Added more robust error handling and fallback logic
4. Improved the public interface to pass through more SDK functionality
5. Added additional safeguards around reconnection logic to prevent state issues