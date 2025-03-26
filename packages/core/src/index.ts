import { useState, useEffect, useRef } from 'react';
import { LATEST_PROTOCOL_VERSION } from './mcp/schema';

interface MCPState {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  error?: Error;
}

export function useMCP() {
  const [state, setState] = useState<MCPState>({ status: 'connecting' });
  const eventSourceRef = useRef<EventSource | null>(null);

  useEffect(() => {
    const connectToMCP = async () => {
      try {
        // Create EventSource for SSE connection
        const eventSource = new EventSource('http://localhost:8787');
        eventSourceRef.current = eventSource;

        // Handle connection open
        eventSource.onopen = () => {
          // Send initialize request
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

          fetch('http://localhost:8787', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
            },
            body: JSON.stringify(initializeRequest)
          });

          setState({ status: 'connected' });
        };

        // Handle messages
        eventSource.onmessage = (event) => {
          const message = JSON.parse(event.data);
          // Handle different message types
          console.log('Received message:', message);
        };

        // Handle errors
        eventSource.onerror = (error) => {
          setState({ status: 'error', error: new Error('Connection failed') });
          eventSource.close();
        };

      } catch (error) {
        setState({ status: 'error', error: error as Error });
      }
    };

    connectToMCP();

    // Cleanup
    return () => {
      if (eventSourceRef.current) {
        eventSourceRef.current.close();
      }
    };
  }, []);

  return state;
}
