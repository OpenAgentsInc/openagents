import { useState } from 'react';
import { SSEClientTransport } from './mcp/sse'
import { Client } from "@modelcontextprotocol/sdk/client/index.js"
// import { SSEClientTransport } from "@modelcontextprotocol/sdk/client/sse.js"

interface MCPState {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  error?: Error;
}

export function useMCP() {
  const [state, setState] = useState<MCPState>({ status: 'connecting' });

  // const eventSourceRef = useRef<EventSource | null>(null);

  // useEffect(() => {
  //   const connectToMCP = async () => {
  //     try {
  //       // Create EventSource for SSE connection
  //       const eventSource = new EventSource('http://localhost:8787');
  //       eventSourceRef.current = eventSource;

  //       // Handle connection open
  //       eventSource.onopen = () => {
  //         // Send initialize request
  //         const initializeRequest = {
  //           jsonrpc: "2.0",
  //           id: 1,
  //           method: "initialize",
  //           params: {
  //             protocolVersion: LATEST_PROTOCOL_VERSION,
  //             clientInfo: {
  //               name: "OpenAgents MCP Client",
  //               version: "1.0.0"
  //             },
  //             capabilities: {
  //               sampling: {},
  //               roots: {
  //                 listChanged: true
  //               }
  //             }
  //           }
  //         };

  //         fetch('http://localhost:8787', {
  //           method: 'POST',
  //           headers: {
  //             'Content-Type': 'application/json',
  //           },
  //           body: JSON.stringify(initializeRequest)
  //         });

  //         setState({ status: 'connected' });
  //       };

  //       // Handle messages
  //       eventSource.onmessage = (event) => {
  //         const message = JSON.parse(event.data);
  //         // Handle different message types
  //         console.log('Received message:', message);
  //       };

  //       // Handle errors
  //       eventSource.onerror = (error) => {
  //         setState({ status: 'error', error: new Error('Connection failed') });
  //         eventSource.close();
  //       };

  //     } catch (error) {
  //       setState({ status: 'error', error: error as Error });
  //     }
  //   };

  //   connectToMCP();

  //   // Cleanup
  //   return () => {
  //     if (eventSourceRef.current) {
  //       eventSourceRef.current.close();
  //     }
  //   };
  // }, []);

  return state;
}

export async function connectToServer() {
  const transport = new SSEClientTransport(new URL("http://localhost:8787/sse"))
  console.log("Created transport:", transport)

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
  )

  console.log("Created client:", client)

  const connected = await client.connect(transport)
  console.log("CONNECTED?", connected)
}
