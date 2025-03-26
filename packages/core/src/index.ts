import { useState, useEffect } from 'react';
import { SSEClientTransport } from './mcp/sse'
import { Client } from "@modelcontextprotocol/sdk/client/index.js"
import { type JSONRPCMessage } from './mcp/types';
// import { SSEClientTransport } from "@modelcontextprotocol/sdk/client/sse.js"

interface MCPState {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  error?: Error;
  result?: string;
}

interface ToolResponse {
  content: Array<{ type: string; text: string }>;
}

declare global {
  interface Window {
    electron: {
      mcpInvoke: (channel: string, ...args: any[]) => Promise<any>;
    };
  }
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
  console.log("Connected to MCP server.");

  return client;
}
