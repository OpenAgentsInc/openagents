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
  const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'error' | 'closed'>('connecting');
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [rawState, setRawState] = useState<any>(null);
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
    host: 'agents.openagents.com',
    path: 'agents',
    debug: true,

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
      setRawState(state);
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

  if (!mounted) {
    return null;
  }

  return (
    <div className="flex flex-col h-full">
      {children}
      
      <Card className="mb-4">
        <CardHeader>
          <CardTitle>Connection Status: {connectionStatus}</CardTitle>
          {connectionError && (
            <CardDescription className="text-red-500">
              Error: {connectionError}
            </CardDescription>
          )}
        </CardHeader>
      </Card>
      
      <Card className="mb-4">
        <CardHeader>
          <CardTitle>Agent State</CardTitle>
        </CardHeader>
        <CardContent>
          <pre className="bg-muted text-foreground p-4 overflow-auto whitespace-pre-wrap rounded-md">
            {JSON.stringify(rawState, null, 2)}
          </pre>
        </CardContent>
      </Card>
      
      <Card>
        <CardHeader>
          <CardTitle>Messages</CardTitle>
        </CardHeader>
        <CardContent>
          <pre className="bg-muted text-foreground p-4 overflow-auto whitespace-pre-wrap rounded-md">
            {JSON.stringify(messages, null, 2)}
          </pre>
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
      <main className="w-full mx-auto p-4">
        <ClientOnly agentId={agentId || ""} githubToken={githubToken}>
          <AgentContent agentId={agentId || ""} />
        </ClientOnly>
      </main>
    </>
  );
}

function AgentContent({ agentId }: { agentId: string }) {
  const [loading, setLoading] = useState(true);
  const [agent, setAgent] = useState<Agent | null>(null);
  const agentStore = useAgentStore();

  useEffect(() => {
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

  if (loading) {
    return <Card><CardContent>Loading agent data...</CardContent></Card>;
  }

  if (!agent) {
    return <Card><CardContent>Agent not found: {agentId}</CardContent></Card>;
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Agent Raw Data</CardTitle>
      </CardHeader>
      <CardContent>
        <pre className="bg-muted text-foreground p-4 overflow-auto whitespace-pre-wrap rounded-md">
          {JSON.stringify(agent, null, 2)}
        </pre>
      </CardContent>
    </Card>
  );
}
