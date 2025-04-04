import { useState, useEffect, useCallback } from 'react';
// import { SSEClientTransport } from './mcp/sse'
// import { Client } from "@modelcontextprotocol/sdk/client/index"
// import { type JSONRPCMessage } from './mcp/types';

// Chat module exports
export * from './chat/types'
// export * from './chat/useChat'  // Removed
export * from './chat/usePersistentChat'
export * from './chat/useThreads'
export * from './chat/useSettings'
export * from './chat/errorHandler'
export { MODELS } from './chat/MODELS'
export * from './chat/constants'
// Database module exports
export * from './db'
// Selectively re-export from MCP to avoid duplicate exports
export type { Transport } from './mcp/transport'
export * from './mcp/schema'
export * from './mcp/sse'
export * from './utils/commandExecutor'
export * from './utils/commandParser'
export * from './utils/setupElectronCommandExecutor'
export * from './utils/reactCompatibility'
export * from './utils/logManager'

// import { SSEClientTransport } from "@modelcontextprotocol/sdk/client/sse.js"

/*
interface MCPState {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  error?: Error;
  result?: string;
  serverUrl?: string;
}

interface ToolResponse {
  content: Array<{ type: string; text: string }>;
}

declare global {
  interface Window {
    electron: {
      mcpInvoke: (channel: string, ...args: any[]) => Promise<any>;
      mcpGetUrl: () => Promise<string>;
    };
  }
}

export function useMCP() {
  const [state, setState] = useState<MCPState>({ status: 'connecting' });

  useEffect(() => {
    const init = async () => {
      try {
        const url = await window.electron.mcpGetUrl();
        setState(prev => ({ ...prev, status: 'connected', serverUrl: url }));
      } catch (error) {
        setState(prev => ({ ...prev, status: 'error', error: error as Error }));
      }
    };
    void init();
  }, []);

  const callTool = useCallback(async (name: string, args: Record<string, any>) => {
    try {
      const result = await window.electron.mcpInvoke('mcp:call', name, args);
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
  }, []);

  return {
    ...state,
    callTool
  };
}

let currentUrl: string;

export async function connectToServer() {
  currentUrl = "https://mcp-github.openagents.com/sse";
  // currentUrl = "http://localhost:8787/sse";
  const transport = new SSEClientTransport(new URL(currentUrl));
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
  console.log("Connected to MCP server:", currentUrl);

  return client;
}

export function getCurrentUrl() {
  return currentUrl;
}
*/

// Export React compatibility utilities
export * from './utils/reactCompatibility';

// Export tools-related functionality
export * from './tools/TOOLS';
