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
  }, []);

  // Create and configure agent - this should trigger a WebSocket connection
  const agent = useAgent({
    name: agentId,
    agent: 'coder', // The type of agent to connect to
    host: 'agents.openagents.com', // WebSocket server address

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
      setConnectionStatus('error');
      setConnectionError(error.message || 'Unknown error');
    },

    // Agent state update handler
    onStateUpdate: (state: AgentState) => {
      console.log("Agent state updated:", state);
      if (state.messages) {
        setMessages(state.messages);
      }
    }
  });

  useEffect(() => {
    console.log("Agent created:", agent);

    // Check if agent.connection exists and log its state
    if (agent && agent.connection) {
      console.log("WebSocket connection state:", agent.connection.readyState);
    } else {
      console.warn("No WebSocket connection available on agent object");
    }
  }, [agent]);

  // Effect to check connection status after component mounts
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

  // Separate effect for pinging to avoid race conditions
  useEffect(() => {
    if (!agent || connectionStatus !== 'connected') return;

    // We only want to ping once when the connection is first established
    const pingTimeout = setTimeout(() => {
      try {
        if (typeof agent.ping === 'function') {
          console.log("Sending ping to agent");
          agent.ping();
        } else {
          console.log("No ping method available on agent");
        }
      } catch (error) {
        console.error("Error sending ping:", error);
      }
    }, 500); // Short delay to ensure everything is initialized

    return () => clearTimeout(pingTimeout);
  }, [agent, connectionStatus]);

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
            {connectionStatus && (
              <span className={`ml-2 inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${connectionStatus === 'connected'
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
              </span>
            )}
          </CardDescription>
          {connectionError && (
            <div className="mt-2 p-2 bg-red-50 text-red-700 text-sm rounded-md flex justify-between items-center">
              <span>{connectionError}</span>
              {(connectionStatus === 'error' || connectionStatus === 'closed') && (
                <button
                  onClick={() => {
                    // Reset connection status
                    setConnectionStatus('connecting');
                    setConnectionError(null);

                    // Force page reload to reconnect
                    window.location.reload();
                  }}
                  className="ml-2 px-2 py-1 bg-primary text-primary-foreground rounded text-xs"
                >
                  Reconnect
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
