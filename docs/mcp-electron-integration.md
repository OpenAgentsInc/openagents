# MCP Integration with Electron

This document details the implementation of Model Context Protocol (MCP) in our Electron-based desktop application, focusing on the architecture and communication flow between the frontend and backend.

## Architecture Overview

The integration follows a three-layer architecture:

1. **Frontend Layer**: React components using the `useMCP` hook
2. **IPC Layer**: Electron's IPC system for secure communication
3. **Backend Layer**: MCP client running in the main process

```
┌─────────────────┐         ┌─────────────────┐         ┌─────────────────┐
│   Frontend      │         │   IPC Bridge    │         │    Backend      │
│                 │         │                 │         │                 │
│  React Components│   IPC   │  Preload Script │   IPC   │   Main Process  │
│  useMCP Hook    │ ───────>│  Context Bridge │ ───────>│   MCP Client   │
│                 │         │                 │         │                 │
└─────────────────┘         └─────────────────┘         └─────────────────┘
```

## Implementation Details

### 1. Frontend Implementation (`packages/core/src/index.ts`)

The `useMCP` hook provides a React interface for MCP functionality:

```typescript
interface MCPState {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  error?: Error;
  result?: string;
}

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

### 2. IPC Layer

#### Context Bridge (`apps/coder/src/helpers/ipc/mcp/mcp-context.ts`)

Exposes MCP functionality to the renderer process:

```typescript
export function exposeMcpContext() {
  contextBridge.exposeInMainWorld("electron", {
    mcpInvoke: (channel: string, ...args: any[]) => {
      return ipcRenderer.invoke(channel, ...args);
    },
  });
}
```

#### IPC Listeners (`apps/coder/src/helpers/ipc/mcp/mcp-listeners.ts`)

Handles MCP requests in the main process:

```typescript
export function addMcpEventListeners() {
  ipcMain.handle('mcp:add', async (_, a: number, b: number) => {
    if (!mcpClient) {
      throw new Error('MCP client not connected');
    }

    try {
      const result = await mcpClient.callTool({
        name: 'add',
        arguments: { a, b }
      });
      return result;
    } catch (error) {
      console.error('Error calling add tool:', error);
      throw error;
    }
  });
}
```

### 3. Backend Implementation

#### MCP Client Setup (`apps/coder/src/main.ts`)

Initializes the MCP client in the main process:

```typescript
async function setupMcp() {
  try {
    const client = await connectToServer();
    setMcpClient(client);
    console.log("MCP client connected successfully");
  } catch (error) {
    console.error("Failed to connect MCP client:", error);
  }
}

app.whenReady()
  .then(createWindow)
  .then(installExtensions)
  .then(setupMcp);
```

## Communication Flow

1. **Frontend Initialization**:
   - React component mounts
   - `useMCP` hook initializes and calls `mcpInvoke`

2. **IPC Communication**:
   - Request goes through context bridge
   - Main process receives request via IPC
   - MCP client executes tool call
   - Result returns through IPC chain

3. **State Management**:
   - Frontend maintains connection state
   - Updates UI based on results/errors
   - Handles disconnections gracefully

## Security Considerations

1. **Context Isolation**:
   - Enabled in Electron webPreferences
   - Prevents direct access to Node.js/Electron APIs

2. **IPC Validation**:
   - Channels are explicitly defined
   - Arguments are validated before use

3. **Error Handling**:
   - Graceful handling of connection failures
   - Clear error messages to users

## Known Issues

1. **Type Resolution**:
   - Different versions of MCP SDK causing type conflicts
   - Currently using type assertions as workaround
   - Need to align package versions across workspace

## Future Improvements

1. **Connection Management**:
   - Implement reconnection logic
   - Add connection status indicators
   - Handle cleanup on app close

2. **Tool Registry**:
   - Create a central registry of available tools
   - Dynamic tool discovery and registration
   - Tool capability negotiation

3. **Error Recovery**:
   - Implement retry mechanisms
   - Add circuit breakers for failing tools
   - Better error reporting to users

## References

- [Model Context Protocol Documentation](docs/mcp.md)
- [Electron Security Guidelines](https://www.electronjs.org/docs/latest/tutorial/security)
- [IPC Best Practices](https://www.electronjs.org/docs/latest/tutorial/ipc)
