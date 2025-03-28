# @openagents/core

Core functionality for OpenAgents applications.

## Command Execution

The command execution feature allows running shell commands in the Electron environment.

### Setting Up Command Execution in Electron

To enable command execution in your Electron app, follow these steps:

1. In your Electron main process, set up the command execution handler:

```typescript
// In your Electron main process
import { setupElectronCommandExecutor } from '@openagents/core';

// Set up command execution for Electron
setupElectronCommandExecutor();
```

This sets up the necessary IPC handlers for executing commands from renderer processes.

### Using the useChat Hook with Command Execution

The `useChat` hook supports command execution for AI conversations:

```tsx
import { useChat } from '@openagents/core';

function ChatComponent() {
  const { messages, append } = useChat({
    // Enable command execution
    localCommandExecution: true,
    
    // Optional command options
    commandOptions: {
      // Working directory for commands
      cwd: '/path/to/working/directory',
      
      // Timeout in milliseconds (default: 30000)
      timeout: 60000,
      
      // Additional environment variables
      env: { NODE_ENV: 'development' }
    },
    
    // Optional callbacks
    onCommandStart: (command) => {
      console.log(`Starting command: ${command}`);
    },
    onCommandComplete: (command, result) => {
      console.log(`Command completed: ${command}`);
      console.log('Result:', result);
    }
  });
  
  // ... rest of your component
}
```

### Command Format

Commands are executed when the AI sends messages with the following format:

```
You can use the <execute-command>echo "Hello World"</execute-command> syntax to run shell commands.
```

The command will be executed and the output will replace the command tag in the message.

### Security Considerations

- Command execution is disabled by default and must be explicitly enabled
- Commands are executed with the same permissions as the Node.js/Electron process
- A blacklist of potentially dangerous commands is implemented
- Timeout limits prevent long-running commands
- Command execution is restricted to Node.js and Electron environments

## MCP Client Implementation

The core package provides a React hook `useMCP` that implements a client for the Model Context Protocol (MCP). This hook enables applications to connect to MCP servers and access their tools, resources, and capabilities through Electron's IPC system.

### Usage

```typescript
import { useMCP } from '@openagents/core';

function MyComponent() {
  const { status, result, error } = useMCP();

  // Handle different connection states
  if (status === 'error') {
    return <div>Error connecting to MCP server: {error?.message}</div>;
  }

  if (status === 'connecting') {
    return <div>Connecting to MCP server...</div>;
  }

  return (
    <div>
      <p>Connected to MCP server!</p>
      {result && <p>Tool Result: {result}</p>}
    </div>
  );
}
```

### How It Works

The MCP integration follows a three-layer architecture:

1. **Frontend Layer (`useMCP` Hook)**
   - Provides React interface for MCP functionality
   - Manages connection state
   - Handles tool invocation through IPC

2. **IPC Layer**
   - Uses Electron's context bridge for security
   - Provides type-safe IPC communication
   - Handles message serialization/deserialization

3. **Backend Layer**
   - Runs MCP client in Electron main process
   - Connects to MCP server using SSE
   - Executes tool calls and returns results

### Connection States

```typescript
interface MCPState {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  error?: Error;
  result?: string;
}
```

### Implementation Details

1. **Frontend Hook**
   ```typescript
   export function useMCP() {
     const [state, setState] = useState<MCPState>({ status: 'connecting' });

     useEffect(() => {
       const callAddTool = async () => {
         try {
           const result = await window.electron.mcpInvoke('mcp:add', 5, 3);
           setState(prev => ({
             ...prev,
             status: 'connected',
             result: result.content[0].text
           }));
         } catch (error) {
           setState(prev => ({
             ...prev,
             status: 'error',
             error: error as Error
           }));
         }
       };

       void callAddTool();
     }, []);

     return state;
   }
   ```

2. **Backend Connection**
   ```typescript
   export async function connectToServer() {
     const transport = new SSEClientTransport(new URL("http://localhost:8787/sse"));
     const client = new Client(
       { name: 'client', version: '0.0.1' },
       {
         capabilities: {
           sampling: {},
           roots: {
             listChanged: true
           }
         }
       }
     );

     await client.connect(transport);
     return client;
   }
   ```

### Security Considerations

1. **Context Isolation**
   - MCP client runs only in the main process
   - Frontend communicates through secure IPC channels
   - No direct access to Node.js or Electron APIs

2. **Type Safety**
   - TypeScript interfaces for all IPC messages
   - Runtime validation of tool arguments
   - Error handling across process boundaries

### Current Features

1. **Tool Invocation**
   - Secure tool calls through IPC
   - Typed responses with error handling
   - Async/await interface

2. **Connection Management**
   - Automatic connection on app start
   - Status tracking and error reporting
   - Clean disconnection on app close

3. **State Management**
   - React state integration
   - Real-time status updates
   - Error state propagation

### Future Enhancements

1. **Tool Registry**
   - Dynamic tool discovery
   - Tool capability negotiation
   - Tool documentation

2. **Resource Management**
   - Resource subscription
   - Resource update notifications
   - Resource content access

3. **Advanced Features**
   - Batch requests
   - Progress notifications
   - Custom capabilities
   - Completion suggestions

## Contributing

Please see the main repository's CONTRIBUTING.md for guidelines on how to contribute to this package.

## Documentation

For detailed implementation documentation, see:
- [MCP Documentation](../../docs/mcp.md)
- [MCP Electron Integration](../../docs/mcp-electron-integration.md)
