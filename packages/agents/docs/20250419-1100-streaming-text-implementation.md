# Streaming Text Implementation for Solver Agent

**Date**: April 19, 2025  
**Author**: Claude  
**Last Updated**: April 19, 2025 (TypeScript fixes)

## Overview

This document outlines the implementation of real-time text streaming for the Solver agent, replacing the previous approach that waited for complete responses before displaying them to users. The implementation leverages Vercel AI SDK's `streamText` function combined with Cloudflare Durable Objects' state synchronization capabilities to provide a more interactive and responsive user experience.

## Problem Statement

Previously, the Solver agent used the `generateText` function from Vercel AI SDK, which only returns a response after the entire text generation is complete. This approach resulted in:

1. Long wait times for users, especially for lengthy responses
2. No visibility into the agent's thought process during generation
3. Reduced interactivity and engagement
4. Potential perception of system sluggishness

## Solution Architecture

The implementation follows a state-based synchronization approach using Cloudflare Durable Objects, rather than traditional WebSocket event callbacks. This architecture provides several advantages:

1. **Resilience to Client Disconnections**: If a client disconnects and reconnects, they immediately see the current state without special handling
2. **Automatic Multi-Client Synchronization**: All connected clients automatically receive state updates through DO's built-in synchronization
3. **State Persistence**: Streaming state persists until explicitly removed or modified
4. **Simplified Client Implementation**: The frontend doesn't need to maintain event listeners for different streaming events

### Components Modified

1. **Backend (Agent)**:
   - Added `streamingInfer` method to `OpenAgent` class
   - Updated WebSocket message handler to support streaming requests
   - Implemented state-based token streaming
   - Enhanced tool call execution within streaming context

2. **Frontend (UI)**:
   - Updated chat message component to show streaming indicators
   - Enhanced message types to support streaming states
   - Added placeholder message before streaming begins
   - Improved error handling and state transitions

### Key Files Modified

1. `/packages/agents/src/common/open-agent.ts`
   - Added `streamingInfer` method
   - Updated imports to include `streamText`
   - Added types for streaming response parts

2. `/packages/agents/src/agents/solver/index.ts`
   - Enhanced WebSocket message handler for streaming requests
   - Added branching logic for streaming vs. non-streaming requests

3. `/apps/website/app/components/agent/solver-connector.tsx`
   - Updated form submission to support streaming requests
   - Added streaming placeholder message
   - Improved handling of streaming state updates

4. `/apps/website/app/components/ui/chat-message.tsx`
   - Added visual indicators for streaming state
   - Added support for tool processing indicators
   - Enhanced error display for failed responses

5. `/apps/website/app/lib/types.ts`
   - Added streaming-related message properties
   - Enhanced message interface with streaming states

## Implementation Details

### 1. Streaming Inference Method

The core of the implementation is the new `streamingInfer` method in the `OpenAgent` class:

```typescript
async streamingInfer(props: InferProps): Promise<{ requestId: string, messageId: string }> {
  // Create a streaming placeholder message
  const messageId = generateId();
  const streamingMessage = {
    id: messageId,
    role: 'assistant',
    content: '',
    isStreaming: true,
    timestamp: new Date().toISOString()
  };

  // Add placeholder to state (triggers UI update for all clients)
  this.updateState({
    messages: [...this.state.messages, streamingMessage]
  });

  // Process streaming in background
  (async () => {
    // Use Vercel AI SDK streamText function
    const { textStream, fullStream } = await streamText({
      model: openrouter(model),
      messages: formattedMessages,
      tools: solverTools,
      toolChoice: 'auto',
      temperature,
      maxTokens: max_tokens,
      topP: top_p
    });

    // Process stream chunks and update state for each
    for await (const chunk of fullStream) {
      if (chunk.type === 'text-delta') {
        // Update accumulated text with new token
        accumulatedText += chunk.textDelta;
        
        // Update message in state with new accumulated text
        // This automatically syncs to all connected clients
        this.setState({
          ...this.state,
          messages: this.state.messages.map(msg => 
            msg.id === messageId 
              ? { ...msg, content: accumulatedText, isStreaming: true } 
              : msg
          )
        });
      }
      else if (chunk.type === 'tool-call') {
        // Process tool calls...
      }
    }

    // Process tool calls if any
    if (toolCalls.length > 0) {
      // Update state to show tool processing
      // Execute tools and update state with results
    }
    
    // Mark streaming as complete in state
    this.setState({
      ...this.state,
      messages: this.state.messages.map(msg => 
        msg.id === messageId 
          ? { ...msg, content: accumulatedText, isStreaming: false } 
          : msg
      )
    });
  })();

  // Return IDs for client tracking
  return { requestId, messageId };
}
```

### 2. WebSocket Message Handler Changes

The WebSocket message handler in the Solver class was updated to support streaming requests:

```typescript
// Check if this is a streaming request
if (parsedMessage.streaming === true) {
  // Use streamText implementation
  const { requestId, messageId } = await this.streamingInfer({
    ...inferProps,
    requestId: parsedMessage.requestId
  });
  
  // Send acknowledgment message
  connection.send(JSON.stringify({
    type: "stream_started",
    requestId: parsedMessage.requestId || requestId,
    messageId,
    timestamp: new Date().toISOString()
  }));
  
  // Actual content will be streamed via state updates
} else {
  // Use standard non-streaming implementation
  const result = await this.sharedInfer(inferProps);
  // ...existing code...
}
```

### 3. Frontend Changes

The frontend was updated to send streaming requests and display streaming status:

```typescript
// Create a placeholder for streaming responses
const assistantPlaceholder = {
  id: assistantMessageId,
  role: 'assistant',
  content: '',
  isStreaming: true
};

// Add placeholder to UI immediately
agent.setMessages([...agent.messages, assistantPlaceholder]);

// Send the request with streaming enabled
await agent.sendRawMessage({
  type: "shared_infer",
  requestId: requestId,
  streaming: true,
  params: {
    // Parameter details...
  }
});

// State updates come automatically via the Durable Object
```

### 4. UI Enhancements

The ChatMessage component was enhanced to display streaming status:

```tsx
{isStreaming && (
  <div className="flex items-center px-3 py-1">
    <div className="animate-pulse flex space-x-1">
      <span className="inline-block w-2 h-2 bg-primary rounded-full animate-pulse"></span>
      <span className="inline-block w-2 h-2 bg-primary rounded-full animate-pulse delay-75"></span>
      <span className="inline-block w-2 h-2 bg-primary rounded-full animate-pulse delay-150"></span>
    </div>
    <span className="text-xs text-muted-foreground ml-2">Generating...</span>
  </div>
)}

{isProcessingTools && (
  <div className="flex items-center px-3 py-1">
    <div className="animate-pulse mr-2">
      <span className="inline-block w-4 h-4 rounded-full border-2 border-transparent border-t-primary animate-spin"></span>
    </div>
    <span className="text-xs text-muted-foreground">Processing tool calls...</span>
  </div>
)}
```

## Benefits of the Implementation

1. **Improved User Experience**
   - Users see text as it's generated in real-time
   - Reduced perceived latency, especially for longer responses
   - More interactive and engaging interface
   - Visibility into tool execution progress

2. **Technical Advantages**
   - Better utilization of Cloudflare Durable Objects' capabilities
   - More resilient to client disconnections and refreshes
   - Simplified state management and synchronization
   - Consistent state update pattern across all agent operations

3. **Developer Experience**
   - Clear separation of streaming and non-streaming paths
   - Type-safe message properties
   - Simplified client-side event handling
   - Direct use of DO's state synchronization

## Future Improvements

1. **Token Streaming Rate Limiting**
   - Consider implementing debouncing for state updates to reduce the number of DO state mutations

2. **Client-Side Rendering Optimization**
   - Enhance text rendering to minimize layout shifts during streaming

3. **Tool Streaming**
   - Stream tool execution results as they become available rather than waiting for all tools to complete

4. **Streaming Control**
   - Add the ability for users to pause or cancel streaming responses

5. **Analytics**
   - Add metrics for streaming performance and user engagement

## Conclusion

The streaming text implementation significantly improves the Solver agent's user experience by providing real-time feedback during text generation. By leveraging Cloudflare Durable Objects' state synchronization capabilities, the solution is both robust and maintainable, with minimal client-side complexity.

This approach aligns with modern expectations for AI assistant interfaces, where users expect to see responses form in real-time rather than waiting for complete outputs. The implementation maintains all existing functionality while adding streaming capabilities, ensuring backward compatibility with systems not designed for streaming responses.