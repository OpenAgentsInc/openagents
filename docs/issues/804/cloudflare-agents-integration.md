# Cloudflare Agents Integration for OpenAgents

## Overview

This document outlines the integration between OpenAgents and Cloudflare Workers-based AI agents. The integration allows clients to connect directly to Cloudflare Workers running the Agents SDK through WebSockets, enabling real-time AI assistance with specialized capabilities.

## Architecture

The architecture consists of three main components:

1. **Client-Side SDK Bridge**: A WebSocket-based implementation that allows browser-based clients to connect directly to Cloudflare Workers without requiring Node.js.

2. **Cloudflare Worker Agent**: A Durable Object-based implementation running on Cloudflare Workers, providing specialized AI capabilities through tools and API integrations.

3. **Integration Layer (useChat)**: The integration with the existing UI components through an enhanced version of the useChat hook.

## WebSocket Protocol

The WebSocket connection follows this protocol:

1. **Connection**: Client establishes a WebSocket connection to `/agents/api/:agentId/:agentName`
2. **Handshake**: Client sends a handshake message with agent information
3. **Method Calls**: Client sends method calls with a unique ID, the agent processes them and responds
4. **State Updates**: Server can push state updates to the client at any time
5. **Reconnection**: Client handles reconnection with exponential backoff on connection failures

### Message Format

```json
// Method call from client to server
{
  "id": "1647347511111-0",
  "type": "call",
  "method": "getMessages",
  "args": []
}

// Response from server to client
{
  "id": "1647347511111-0",
  "type": "response",
  "result": [{"id": "msg1", "role": "assistant", "content": "Hello!"}]
}

// State update from server to client
{
  "type": "state",
  "state": {"context": {"repoOwner": "example", "repoName": "project"}}
}
```

## Client Implementation

The client implementation includes:

1. **Agent SDK Bridge**: A WebSocket-based client that implements the same interface as the official Cloudflare Agents SDK.
2. **Agent Connection**: Utilities for managing connections, message routing, and command execution.
3. **Enhanced useChat Hook**: Integration with the existing chat UI and command execution system.

## Durable Object Agent

The Durable Object agent provides:

1. **State Persistence**: Messages and context are stored in the Durable Object's state.
2. **Tool Integration**: Access to specialized tools for tasks like repository management.
3. **LLM Integration**: Connection to LLMs through providers like OpenRouter.

## Example Usage

```tsx
// Using the agent-enabled useChat hook
const chat = useChat({
  // Agent configuration
  agentId: 'coder-agent',
  agentName: 'my-instance',
  agentServerUrl: 'https://agents.openagents.com',
  
  // Project context for the agent
  agentOptions: {
    projectContext: {
      repoOwner: 'OpenAgentsInc',
      repoName: 'openagents',
      branch: 'main'
    }
  },
  
  // Enable local fallback
  localCommandExecution: true
});

// Send a message to the agent
chat.append({
  role: 'user',
  content: 'What files are in the src directory?'
});

// Execute a command through the agent
const result = await chat.executeCommand('ls -la src');
```

## Connection Flow

1. Client creates a WebSocket connection to the server
2. Server instantiates or retrieves the appropriate Durable Object for the requested agent
3. Client sends a handshake message to identify its agent type and instance
4. After connection is established, client can call methods on the agent
5. Agent responses and state updates are streamed back to the client

## Benefits

1. **Real-time AI Assistance**: Provides a streaming, interactive experience with specialized AI capabilities.
2. **Scalability**: Leverages Cloudflare's globally distributed network to ensure low-latency responses.
3. **Persistence**: Maintains conversation history and context using Durable Objects.
4. **Specialization**: Different agent types can provide domain-specific capabilities.
5. **Cross-Platform**: Works in browser, desktop, and mobile environments.

## Future Enhancements

1. **Multiple Agent Types**: Support for different specialized agents beyond the CoderAgent.
2. **Authentication**: Integration with OAuth for GitHub and other services.
3. **Tool Extensions**: Adding more specialized tools for different domains.
4. **Offline Support**: Caching and offline operation capabilities.
5. **Multi-User Collaborations**: Enabling shared agent sessions between multiple users.

## Implementation Details

The implementation ensures proper error handling, reconnection logic, and progressive enhancement. It gracefully degrades to local functionality when agent connections cannot be established, ensuring users always have a working experience.