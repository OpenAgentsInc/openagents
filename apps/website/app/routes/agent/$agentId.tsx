import { useEffect } from "react";
import { useParams } from "react-router";
import type { LoaderFunctionArgs } from "react-router";
import type { Route } from "./+types/agent";
import { Header } from "~/components/header";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "~/components/ui/card";
import { useState } from "react";
import { useAgentStore } from "~/lib/store";
import { useAgent } from "agents/react";

// Message type definition
interface Message {
  role: 'user' | 'assistant';
  content: string;
  id?: string;
  createdAt?: number;
}

// Agent state type
interface AgentState {
  messages?: Message[];
  [key: string]: any;
}

// Use the Agent type locally to avoid import issues
interface Agent {
  id: string;
  purpose: string;
  createdAt: number;
}

export function meta({ params }: Route.MetaArgs) {
  return [
    { title: `Agent: ${params.agentId}` },
    { name: "description", content: "View agent details" },
  ];
}

// Load agent data - server-side only returns ID for safety
export async function loader({ params }: LoaderFunctionArgs) {
  const { agentId } = params;

  // For security, don't try to load agents on the server
  // Just return the ID and let client-side handle data lookup
  return { id: agentId };
}

function ClientOnly({ agentId, children }: { agentId: string, children: React.ReactNode }) {
  const [mounted, setMounted] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'error' | 'closed'>('connecting');
  const [connectionError, setConnectionError] = useState<string | null>(null);

  useEffect(() => {
    setMounted(true);

    // Check if WebSocket is supported in this environment
    if (typeof WebSocket !== 'undefined') {
      console.log("WebSocket is supported in this environment");
      try {
        // Try creating a test WebSocket to verify connectivity
        const testSocket = new WebSocket('wss://agents.openagents.com');

        testSocket.onopen = () => {
          console.log("Test WebSocket connection successful");
          testSocket.close();
        };

        testSocket.onerror = (error) => {
          console.error("Test WebSocket connection failed:", error);
        };
      } catch (error) {
        console.error("Error creating test WebSocket:", error);
      }
    } else {
      console.error("WebSocket is not supported in this environment");
    }
  }, []);

  // Create and configure agent with WebSocket connection
  console.log("Initializing agent with WebSocket connection");
  
  // The issue might be with the protocol in PartySocket, so let's try different formats
  // According to Cloudflare's docs, the WebSocket URL should be based on the worker route
  // Use the route pattern from wrangler.jsonc: "agents.openagents.com"
  const agent = useAgent({
    name: agentId,
    agent: 'coder',
    // Try with no protocol prefix - let PartySocket add it
    host: 'agents.openagents.com',
    // Add more parameters that might be expected by the Agents API
    room: agentId, // Required by PartySocket
    query: { clientId: agentId }, // Add query params
    debug: true, // Enable debug mode for connection diagnostics

    // WebSocket event handlers
    onMessage: (message) => {
      console.log("WebSocket message received:", message.data);
      try {
        // Try to parse the message data as JSON
        const data = JSON.parse(message.data);
        console.log("Parsed WebSocket message:", data);
      } catch (e) {
        console.log("Raw message (not JSON):", message.data);
      }
    },

    onOpen: () => {
      console.log("WebSocket connection established successfully");
      setConnectionStatus('connected');
      setConnectionError(null);
    },

    onClose: (event) => {
      console.log("WebSocket connection closed", event.code, event.reason);
      setConnectionStatus('closed');
      setConnectionError(`Connection closed: ${event.reason || 'Unknown reason'} (code: ${event.code})`);
    },

    onError: (error) => {
      console.error("WebSocket connection error:", error);

      // Get more detailed error information
      let errorMessage = 'Unknown error';
      if (error) {
        if (error.message) errorMessage = error.message;
        if (error.code) errorMessage += ` (Code: ${error.code})`;
      }

      console.error("Detailed error:", {
        errorObject: error,
        errorMessage,
        agentId,
        host: 'agents.openagents.com',
        room: agentId
      });

      setConnectionStatus('error');
      setConnectionError(errorMessage);
    },

    // Agent state update handler
    onStateUpdate: (state: AgentState) => {
      console.log("Agent state updated:", state);
      if (state.messages) {
        setMessages(state.messages);
      }
    }
  });

  console.log("Agent created:", agent);

  // Check connection status after component mounts
  useEffect(() => {
    if (!agent) return;
    
    // Check connection status after a short delay
    const timer = setTimeout(() => {
      if (connectionStatus === 'connecting') {
        console.log("WebSocket connection is still pending after timeout");
        setConnectionStatus('error');
        setConnectionError('Connection timeout - unable to establish WebSocket connection');
      }
    }, 5000);
    
    return () => clearTimeout(timer);
  }, [agent, connectionStatus]);
  
  // Add a fallback approach if connection fails
  useEffect(() => {
    // If connection fails with error status, try alternative connection approach
    if (connectionStatus === 'error' && agent) {
      const fallbackTimer = setTimeout(() => {
        console.log("Connection failed, trying alternative approaches");
        
        // Try multiple WebSocket URL formats to find one that works
        const attemptConnection = async () => {
          const urlFormats = [
            'wss://agents.openagents.com',
            'wss://agents.openagents.com/',
            'wss://agents.openagents.com/ws',
            'wss://agents.openagents.com/agent',
            'wss://agents.openagents.com/connect'
          ];
          
          console.log("Testing these WebSocket URL formats:", urlFormats);
          
          for (const url of urlFormats) {
            try {
              console.log(`Trying WebSocket connection to: ${url}`);
              const socket = new WebSocket(url);
              
              // Create a promise to handle the connection attempt
              const result = await new Promise((resolve) => {
                // Set a timeout to avoid waiting too long
                const timeout = setTimeout(() => {
                  socket.close();
                  resolve({ success: false, url, reason: 'timeout' });
                }, 3000);
                
                socket.onopen = () => {
                  clearTimeout(timeout);
                  socket.close();
                  resolve({ success: true, url });
                };
                
                socket.onerror = () => {
                  clearTimeout(timeout);
                  resolve({ success: false, url, reason: 'error' });
                };
              });
              
              if (result.success) {
                console.log(`🎉 Successful WebSocket connection to: ${result.url}`);
                setConnectionError(`Direct WebSocket works with: ${result.url}, but agent connection still failing. Check developer console for details.`);
                
                // Try to reconnect the agent with this URL format
                if (agent.updateProperties && agent.reconnect) {
                  console.log("Attempting to reconnect agent with the working URL");
                  try {
                    const parsedUrl = new URL(result.url);
                    agent.updateProperties({
                      host: parsedUrl.host,
                      path: parsedUrl.pathname || '/',
                    });
                    agent.reconnect();
                  } catch (e) {
                    console.error("Failed to update agent properties:", e);
                  }
                }
                
                return;
              } else {
                console.log(`❌ Failed WebSocket connection to: ${result.url} (${result.reason})`);
              }
            } catch (error) {
              console.error(`Error testing WebSocket URL ${url}:`, error);
            }
          }
          
          console.error("All WebSocket connection attempts failed");
          setConnectionError("All WebSocket connection attempts failed. The server may be unavailable.");
        };
        
        attemptConnection();
      }, 3000);
      
      return () => clearTimeout(fallbackTimer);
    }
  }, [agent, connectionStatus, agentId]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || !agent) return;

    // Add user message
    const userMessage = {
      role: 'user' as const,
      content: input,
      id: Date.now().toString(),
      createdAt: Date.now(),
    };

    setMessages(prev => [...prev, userMessage]);
    setInput("");

    try {
      console.log("Updating agent state with message:", userMessage);
      // Update agent state
      await agent.setState({
        messages: [...messages, userMessage]
      });
      console.log("Agent state updated successfully");
    } catch (error) {
      console.error("Error updating agent state:", error);
    }
  };

  if (!mounted) {
    return null;
  }

  return (
    <div className="flex flex-col h-full">
      {children}

      <Card className="mt-6">
        <CardHeader>
          <CardTitle>Chat with Agent</CardTitle>
          <CardDescription>
            Ask your agent questions about code
            <div className="mt-2">
              <div className={`inline-flex items-center px-2 py-1 rounded text-xs font-medium ${
                connectionStatus === 'connected' 
                  ? 'bg-green-100 text-green-800' 
                  : connectionStatus === 'connecting'
                  ? 'bg-blue-100 text-blue-800'
                  : 'bg-red-100 text-red-800'
              }`}>
                {connectionStatus === 'connected' 
                  ? '● Connected' 
                  : connectionStatus === 'connecting'
                  ? '● Connecting...'
                  : '● Connection Error'}
              </div>
              
              <div className="text-xs text-muted-foreground mt-2">
                WebSocket Host: agents.openagents.com<br/>
                Agent ID: {agentId}<br/>
                Agent Type: coder
              </div>
            </div>
          </CardDescription>
          {connectionError && (
            <div className="mt-2 p-2 bg-red-50 text-red-700 text-sm rounded-md flex justify-between items-center">
              <span>{connectionError}</span>
              {(connectionStatus === 'error' || connectionStatus === 'closed') && (
                <button
                  onClick={() => {
                    // Reset connection status
                    setConnectionStatus('connecting');
                    setConnectionError('Attempting to reconnect...');
                    
                    // Try to directly reconnect without page reload
                    if (agent && agent.reconnect) {
                      console.log("Attempting direct reconnection without reload");
                      agent.reconnect();
                      
                      // Set a timeout to reload the page if reconnection fails
                      setTimeout(() => {
                        if (connectionStatus !== 'connected') {
                          console.log("Reconnection still not successful, reloading page");
                          window.location.reload();
                        }
                      }, 5000);
                    } else {
                      // Force page reload to reconnect if reconnect method not available
                      window.location.reload();
                    }
                  }}
                  className="ml-2 px-2 py-1 bg-primary text-primary-foreground rounded text-xs"
                >
                  Try Reconnect
                </button>
              )}
            </div>
          )}
        </CardHeader>
        <CardContent>
          <div className="h-[400px] overflow-y-auto mb-4 space-y-4">
            {messages.map((message, index) => (
              <div
                key={message.id || index}
                className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                <div
                  className={`max-w-[80%] rounded-lg p-3 ${message.role === 'user'
                    ? 'bg-primary text-primary-foreground ml-auto'
                    : 'bg-muted'
                    }`}
                >
                  <p className="whitespace-pre-wrap text-sm">{message.content}</p>
                </div>
              </div>
            ))}
          </div>

          <form onSubmit={handleSubmit} className="flex gap-2">
            <input
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Type your message..."
              className="flex-1 px-3 py-2 rounded-md border bg-background"
            />
            <button
              type="submit"
              className="px-4 py-2 bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
              disabled={!input.trim() || connectionStatus !== 'connected'}
              title={
                connectionStatus !== 'connected'
                  ? 'Cannot send message: WebSocket connection not established'
                  : 'Send message'
              }
            >
              Send
            </button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

export default function AgentDetails() {
  // Get agent ID from URL
  const { agentId } = useParams();

  return (
    <>
      <Header />

      <main className="w-full max-w-2xl mx-auto p-8 pt-24">
        <ClientOnly agentId={agentId || ""}>
          <AgentContent agentId={agentId || ""} />
        </ClientOnly>
      </main>
    </>
  );
}

function AgentContent({ agentId }: { agentId: string }) {
  // Move all client-side code here
  const [loading, setLoading] = useState(true);
  const [agent, setAgent] = useState<Agent | null>(null);
  const agentStore = useAgentStore();

  useEffect(() => {
    // Initialize agent here
    const initAgent = async () => {
      try {
        console.log('Loading agent with ID:', agentId);
        console.log('Available agents:', agentStore.agents);

        // Get agent from Zustand store
        const foundAgent = agentStore.getAgent(agentId);
        console.log('Found agent:', foundAgent);

        if (foundAgent) {
          setAgent(foundAgent);
        }

        setLoading(false);
      } catch (error) {
        console.error('Failed to initialize agent:', error);
        setLoading(false);
      }
    };

    initAgent();
  }, [agentId, agentStore]);

  // Agent not found view
  const NotFoundView = () => (
    <div className="text-center">
      <h1 className="text-3xl font-bold mb-6">Agent Not Found</h1>
      <p className="mb-8">We couldn't find an agent with ID: {agentId}</p>
      <a
        href="/spawn"
        className="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs hover:bg-primary/90"
      >
        Spawn a New Agent
      </a>
    </div>
  );

  // Loading view
  if (loading) {
    return <h1 className="text-3xl font-bold mb-6 text-center">Loading...</h1>;
  }

  // If no agent is found, show not found message
  if (!agent) {
    return <NotFoundView />;
  }

  // Format date from timestamp - on client only
  const formattedDate = new Date(agent.createdAt || 0).toLocaleString();

  return (
    <div className="space-y-6">
      <div className="mb-8">
        <h1 className="text-3xl font-bold mb-2">Agent: {agent.id}</h1>
        <p className="text-muted-foreground">Created on {formattedDate}</p>
      </div>

      <div className="space-y-6">
        <Card>
          <CardHeader>
            <CardTitle>Agent Purpose</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="whitespace-pre-wrap">{agent.purpose}</p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
