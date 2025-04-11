import { useEffect, useState } from "react";
import { useLoaderData, useParams } from "react-router";
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
export async function loader({ params, context }: LoaderFunctionArgs) {
  const { agentId } = params;
  const { env } = context.cloudflare;

  // For security, don't try to load agents on the server
  // Just return the ID and let client-side handle data lookup
  return { id: agentId, githubToken: env.GITHUB_TOKEN };
}

function ClientOnly({ agentId, children, githubToken }: { agentId: string, children: React.ReactNode, githubToken: string }) {
  const [mounted, setMounted] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'error' | 'closed'>('connecting');
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const agentStore = useAgentStore();

  // Set up component and log initialization only once
  useEffect(() => {
    setMounted(true);
    console.log("Initializing agent with WebSocket connection for agent:", agentId);
  }, [agentId]);

  // Standard agent configuration with clear debugging
  const agent = useAgent({
    name: agentId,
    agent: 'coder',
    host: 'agents.openagents.com', // Standard format without protocol prefix
    path: 'agents', // Must NOT start with a slash according to PartySocket requirements
    // room: agentId, // Required by PartySocket for room identification
    debug: true, // Enable verbose logging

    // WebSocket event handlers with improved logging
    onMessage: (message) => {
      console.log("WebSocket message received:", message.data);
      try {
        const data = JSON.parse(message.data);
        console.log("Parsed message data:", data);
      } catch (e) {
        console.log("Raw message (not JSON):", message.data);
      }
    },

    onOpen: () => {
      console.log("🎉 WebSocket connection established successfully");
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
      let errorMessage = 'Unknown error';

      if (error) {
        if (error.message) errorMessage = error.message;
        if (error.code) errorMessage += ` (Code: ${error.code})`;
      }

      console.error("Connection details:", {
        errorMessage,
        agentId,
        host: 'agents.openagents.com',
        path: 'agents',
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

  // Connection timeout check
  useEffect(() => {
    if (!agent) return;

    const timer = setTimeout(() => {
      if (connectionStatus === 'connecting') {
        console.log("Connection timeout after 5 seconds");
        setConnectionStatus('error');
        setConnectionError('Connection timeout - WebSocket connection not established after 5 seconds');
      }
    }, 5000);

    return () => clearTimeout(timer);
  }, [agent, connectionStatus]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || !agent) return;

    // Add user message with proper timestamps and ID
    const userMessage = {
      role: 'user' as const,
      content: input.trim(),
      id: `user-${Date.now()}`,
      createdAt: Date.now(),
    };

    // Update local state immediately for responsive UI
    setMessages(prev => [...prev, userMessage]);
    setInput("");

    try {
      console.log("Sending message to agent:", userMessage);

      // Update agent state with new message
      agent.setState({
        messages: [...messages, userMessage]
      });

      console.log("Message sent successfully");

      // Use the githubToken prop passed from the loader
      console.log("Using GitHub token:", githubToken ? "Token present" : "No token");

      await agent.call('infer', [githubToken])

      console.log('called infer')

      // Optional: Add loading state here if needed
      // setMessages(prev => [...prev, { role: 'assistant', content: '...', id: 'loading', createdAt: Date.now() }]);

    } catch (error) {
      console.error("Error sending message:", error);

      // Show error in UI
      setConnectionError(`Failed to send message: ${error.message || 'Unknown error'}`);

      // Optionally revert the message if it failed to send
      // setMessages(prev => prev.filter(msg => msg.id !== userMessage.id));
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
              <div className={`inline-flex items-center px-2 py-1 rounded text-xs font-medium ${connectionStatus === 'connected'
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
                <div className="grid grid-cols-2 gap-1">
                  <span>WebSocket Host:</span>
                  <span className="font-medium">agents.openagents.com</span>

                  <span>WebSocket Path:</span>
                  <span className="font-medium">agents</span>

                  <span>Agent ID:</span>
                  <span className="font-medium">{agentId}</span>

                  <span>Agent Type:</span>
                  <span className="font-medium">coder</span>
                </div>
              </div>
            </div>
          </CardDescription>

          {connectionError && (
            <div className="mt-2 p-2 bg-red-50 text-red-700 text-sm rounded-md flex justify-between items-center">
              <span>{connectionError}</span>
              {(connectionStatus === 'error' || connectionStatus === 'closed') && (
                <button
                  onClick={() => {
                    console.log("Reloading page to reconnect");
                    window.location.reload();
                  }}
                  className="ml-2 px-2 py-1 bg-primary text-primary-foreground rounded text-xs"
                >
                  Reload to Reconnect
                </button>
              )}
            </div>
          )}
        </CardHeader>

        <CardContent>
          <div className="h-[400px] overflow-y-auto mb-4 space-y-4">
            {messages.length === 0 && (
              <div className="h-full flex items-center justify-center text-muted-foreground">
                <div className="text-center">
                  <p>No messages yet</p>
                  <p className="text-xs mt-1">Start by sending a message below</p>
                </div>
              </div>
            )}

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
                  {message.createdAt && (
                    <div className="text-xs opacity-70 mt-1">
                      {new Date(message.createdAt).toLocaleTimeString()}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>

          <form onSubmit={handleSubmit} className="flex gap-2">
            <input
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder={
                connectionStatus === 'connected'
                  ? 'Type your message...'
                  : 'Waiting for connection...'
              }
              className="flex-1 px-3 py-2 rounded-md border bg-background"
              disabled={connectionStatus !== 'connected'}
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
  const { githubToken } = useLoaderData<typeof loader>();

  return (
    <>
      <Header />

      <main className="w-full max-w-2xl mx-auto p-8 pt-24">
        <ClientOnly agentId={agentId || ""} githubToken={githubToken}>
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
