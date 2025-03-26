# @openagents/core

Core functionality for OpenAgents applications.

## MCP Client Implementation

The core package provides a React hook `useMCP` that implements a client for the Model Context Protocol (MCP). This hook enables applications to connect to MCP servers and access their tools, resources, and capabilities.

### Usage

```typescript
import { useMCP } from '@openagents/core';

function MyComponent() {
  const { status, error } = useMCP();

  useEffect(() => {
    console.log('MCP Connection Status:', status);
  }, [status]);

  // Handle different connection states
  if (status === 'error') {
    return <div>Error connecting to MCP server: {error?.message}</div>;
  }

  if (status === 'connecting') {
    return <div>Connecting to MCP server...</div>;
  }

  return <div>Connected to MCP server!</div>;
}
```

### How It Works

The `useMCP` hook manages a connection to an MCP server using Server-Sent Events (SSE) and follows this flow:

1. **Connection Initialization**
   - Creates an EventSource connection to the MCP server (default: http://localhost:8787)
   - Manages connection state through React state
   - Handles connection errors and cleanup

2. **Protocol Implementation**
   - Sends an initialize request when connection is established
   - Follows the MCP 2.0 protocol specification
   - Supports standard capabilities (sampling, roots)

3. **State Management**
   The hook maintains a connection state with the following possible values:
   - `connecting`: Initial state while establishing connection
   - `connected`: Successfully connected to the MCP server
   - `disconnected`: Connection was closed
   - `error`: Connection failed with an error

### Connection States

```typescript
interface MCPState {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  error?: Error;
}
```

### Implementation Details

1. **EventSource Connection**
   ```typescript
   const eventSource = new EventSource('http://localhost:8787');
   ```

2. **Initialize Request**
   ```typescript
   const initializeRequest = {
     jsonrpc: "2.0",
     id: 1,
     method: "initialize",
     params: {
       protocolVersion: LATEST_PROTOCOL_VERSION,
       clientInfo: {
         name: "OpenAgents MCP Client",
         version: "1.0.0"
       },
       capabilities: {
         sampling: {},
         roots: {
           listChanged: true
         }
       }
     }
   };
   ```

3. **Message Handling**
   - Listens for server messages via EventSource
   - Parses and processes JSON-RPC messages
   - Updates state based on message content

4. **Error Handling**
   - Catches connection errors
   - Updates state with error information
   - Closes connection on error
   - Provides error information to consuming components

5. **Cleanup**
   - Automatically closes EventSource connection when component unmounts
   - Prevents memory leaks and dangling connections

### Future Enhancements

The current implementation provides basic MCP connectivity. Future enhancements may include:

1. Tool Invocation
   - Methods to call server-provided tools
   - Tool response handling
   - Progress notifications

2. Resource Management
   - Resource subscription
   - Resource update notifications
   - Resource content access

3. Authentication
   - OAuth integration
   - Token management
   - Secure connection handling

4. Advanced Features
   - Batch requests
   - Custom capabilities
   - Logging control
   - Completion suggestions

## Contributing

Please see the main repository's CONTRIBUTING.md for guidelines on how to contribute to this package.
