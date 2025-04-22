# Minimal Effect-based Solver Agent

This is a minimal implementation of an Effect-based agent backend with a simple React frontend. The goal is to establish a clean foundation where:

1. The frontend sends a message
2. The backend (using Effect) receives it
3. The backend updates its message state
4. The frontend reflects that state change

## Architecture

### Backend (`src/agents/solver/index.ts`)

The minimal backend uses Effect.js for functional, type-safe message handling. It:

- Defines a minimal state type with just a `messages` array
- Uses Effect.gen for generator-based effect composition
- Handles only simple `chat_message` type messages
- Responds with an echo of the message
- Updates the state which is automatically broadcasted to connected clients

```typescript
// Message handling workflow in Effect.js
const handleMessageEffect = Effect.gen(this, function*(self) {
    // 1. Parse the message
    const parsedMessage = yield* Effect.try(/*...*/);

    // 2. Process chat_message type
    if (parsedMessage.type === 'chat_message' && parsedMessage.content) {
        // Create message objects...
        
        // 3. Update State (triggers broadcast to clients)
        yield* Effect.tryPromise({
            try: () => this.setState({
                messages: [...(this.state.messages || []), userMessage, assistantResponse]
            }),
            catch: (unknown) => new StateUpdateError({ cause: unknown })
        });
    }
});
```

### Frontend Hook (`src/hooks/useOpenAgent_Minimal.ts`)

A minimal React hook that:

- Connects to the agent backend via the Cloudflare Agents SDK
- Tracks connection status
- Provides a simple `sendMessage` function for the UI
- Receives state updates from the backend

```typescript
export function useOpenAgent_Minimal(agentId, agentType) {
  // Initialize state
  const [messages, setMessages] = useState([]);
  const [connectionStatus, setConnectionStatus] = useState('disconnected');
  
  // Connect to agent backend
  const cloudflareAgent = useCloudflareAgent({
    name: agentId,
    agent: agentType,
    onStateUpdate: (newState) => {
      // Update local state when backend state changes
      if (newState.messages) {
        setMessages(newState.messages);
      }
    }
  });
  
  // Send a message to the backend
  const sendMessage = useCallback((content) => {
    cloudflareAgent.send(JSON.stringify({
      type: "chat_message",
      content: content
    }));
  }, [cloudflareAgent]);
  
  // Return the hook interface
  return {
    connectionStatus,
    messages,
    sendMessage,
    disconnect: cloudflareAgent.close
  };
}
```

### UI Components

- `MinimalSolverConnector.tsx`: Displays messages and provides input field
- `MinimalSolverControls.tsx`: Shows connection status and connect/disconnect buttons
- `MinimalSolverExample.tsx`: Demonstrates how to use the components together

## Usage

```tsx
import { MinimalSolverExample } from "./components/agent/MinimalSolverExample";

function App() {
  return (
    <div className="h-screen">
      <MinimalSolverExample agentId="my-agent-id" />
    </div>
  );
}
```

## Message Flow

1. User types message and clicks Send
2. Frontend sends `{ type: "chat_message", content: "..." }` to backend
3. Backend processes message with Effect.js
4. Backend updates its state with new messages
5. State update is automatically broadcasted to all connected clients
6. Frontend receives state update via `onStateUpdate` callback
7. UI reflects the new messages

## Building On This Foundation

This minimal implementation can be extended by:

1. Adding AI model integration to generate actual responses
2. Implementing tools and tool execution
3. Adding context storage for multi-turn conversations
4. Supporting more message types beyond basic chat
5. Adding authentication and user-specific state

The goal is to have a clean, understandable foundation before adding more complex features.