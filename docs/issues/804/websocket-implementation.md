# WebSocket Implementation for Cloudflare Agents

## Overview

This document describes the implementation of direct WebSocket connections to Cloudflare Agents SDK. This approach allows browser-based clients to connect directly to Cloudflare Workers running the Agents SDK without requiring any intermediary services.

## WebSocket Client Implementation

The implementation consists of two primary components:

1. **Agent SDK Bridge**: A client-side implementation that matches the Cloudflare Agents SDK interface
2. **Direct WebSocket Communication**: Uses native WebSockets to communicate with the agent server

## Core Implementation Details

### 1. WebSocket Connection Establishment

The WebSocket connection is initialized with a URL that follows the pattern:
```
wss://agents.openagents.com/api/agent/{agentId}/{instanceName}
```

This URL must exactly match the route pattern defined in the Cloudflare Worker's route handler.

### 2. Connection State Management

The implementation maintains a connection state and provides automatic reconnection:

- Tracks connection state with `connected` flag
- Handles WebSocket lifecycle events (open, message, error, close)
- Implements exponential backoff for reconnection attempts

### 3. Message Protocol

The communication protocol uses a simple JSON-based RPC format:

```typescript
// Client -> Server: Method call
{
  id: string;
  type: 'call';
  method: string;
  args: unknown[];
}

// Server -> Client: Method response
{
  id: string;
  type: 'response';
  result?: unknown;
  error?: string;
}

// Server -> Client: State update
{
  type: 'state';
  state: unknown;
}
```

### 4. Error Handling

The implementation includes comprehensive error handling:

- Connection failure detection
- Request timeouts (30 seconds)
- WebSocket event error handling
- Message parsing error handling

## Usage

The WebSocket client is used through the `useChat` hook with agent-specific options:

```typescript
const chat = useChat({
  agentId: 'CoderAgent',
  agentName: 'default-instance',
  agentServerUrl: 'https://agents.openagents.com',
  
  // Additional options such as project context
  agentOptions: {
    projectContext: {
      repoOwner: 'OpenAgentsInc',
      repoName: 'openagents',
      branch: 'main'
    }
  }
});
```

## Agent Methods

The WebSocket client provides the following methods that match the Cloudflare Agents SDK:

1. **call**: Executes a method on the agent
2. **setState**: Updates the agent's state
3. **close**: Terminates the connection

These methods are wrapped by utility functions to provide domain-specific functionality:

- **getMessages**: Fetches chat history
- **sendMessage**: Sends a new message to the agent
- **executeCommand**: Runs a command through the agent
- **setProjectContext**: Updates the project context
- **getProjectContext**: Retrieves the current project context

## Integration with useChat

The connection to the agent is managed through the `useChat` hook:

1. The hook establishes a connection when agent options are provided
2. It routes messages to the agent when connected
3. It handles connection status changes
4. It manages message history and state

## Security Considerations

The implementation includes several security measures:

1. **HTTPS Only**: WebSocket connections are only established over secure connections (WSS)
2. **Auth Token Support**: Optional authentication token can be provided
3. **Error Isolation**: Error handling prevents cascading failures

## Production Readiness

The implementation is production-ready with the following features:

1. **Reconnection Logic**: Automatic reconnection with exponential backoff
2. **Timeout Handling**: Prevents indefinite waiting for responses
3. **Proper Cleanup**: Resources are properly released on disconnection
4. **Detailed Logging**: Important events are logged for debugging