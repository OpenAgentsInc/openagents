import { useEffect, useState } from "react";
import { useLoaderData, useParams } from "react-router";
import type { LoaderFunctionArgs } from "react-router";
import type { Route } from "./+types/agent";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "~/components/ui/card";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "~/components/ui/collapsible";
import { ChevronDown, ChevronUp, AlertCircle, CheckCircle } from "lucide-react";
import { useAgentStore } from "~/lib/store";
import { useAgent } from "agents/react";
import { Label } from "~/components/ui/label";
import { ClientOnlyMessageList } from "~/components/ui/client-only-message-list";
import { AgentList } from "~/components/agent-list";

// Message type definition 
interface Message {
  role: 'user' | 'assistant';
  content: string;
  id: string;
  createdAt?: Date;
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
  const [agentData, setAgentData] = useState<Agent | null>(null);
  const agentStore = useAgentStore();

  // Set up component and log initialization only once
  useEffect(() => {
    setMounted(true);
    console.log("Initializing agent with WebSocket connection for agent:", agentId);
    
    // Get agent from Zustand store
    const foundAgent = agentStore.getAgent(agentId);
    if (foundAgent) {
      setAgentData(foundAgent);
    }
    
    // Clean up function to handle component unmounting
    return () => {
      // Reset states when navigating away
      setMessages([]);
      setRawState(null);
      setConnectionStatus('connecting');
      setConnectionError(null);
    };
  }, [agentId, agentStore]);

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
        setMessages(state.messages.map(msg => ({
          ...msg,
          id: msg.id || `msg-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`
        })));
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
    <div className="flex h-full">
      {/* Left Sidebar */}
      <div className="w-80 border-r overflow-y-auto flex flex-col">
        {/* Connection status bar */}
        <div className="flex flex-col">
          {/* Status indicator with background */}
          <div className={`px-4 py-2 flex items-center gap-2 ${
            connectionStatus === 'connected' 
              ? 'bg-green-100 dark:bg-green-900/20 text-green-800 dark:text-green-400' 
              : connectionStatus === 'error' 
                ? 'bg-red-100 dark:bg-red-900/20 text-red-800 dark:text-red-400' 
                : 'bg-amber-100 dark:bg-amber-900/20 text-amber-800 dark:text-amber-400'
          }`}>
            {connectionStatus === 'connected' ? (
              <CheckCircle className="h-4 w-4" />
            ) : (
              <AlertCircle className="h-4 w-4" />
            )}
            <Label className="font-medium">
              {connectionStatus === 'connected' ? 'Connected' : 
               connectionStatus === 'connecting' ? 'Connecting...' : 
               connectionStatus === 'closed' ? 'Disconnected' : 'Connection Error'}
            </Label>
          </div>
          
          {/* Agent details in normal background */}
          {agentData && connectionStatus === 'connected' && (
            <div className="px-4 py-2 text-xs text-foreground opacity-80 font-mono border-b">
              <div>ID: {agentData.id}</div>
              <div>Purpose: {agentData.purpose}</div>
              <div>Created: {new Date(agentData.createdAt).toLocaleString()}</div>
              
              <Collapsible className="w-full mt-1">
                <CollapsibleTrigger className="text-xs flex items-center gap-1 text-foreground underline opacity-70 hover:opacity-100">
                  <ChevronDown className="h-3 w-3" /> View agent state
                </CollapsibleTrigger>
                <CollapsibleContent>
                  <div className="pt-2">
                    <pre className="bg-muted text-foreground p-2 overflow-auto whitespace-pre-wrap rounded-md max-h-96 text-xs mt-1">
                      {JSON.stringify(rawState, null, 2)}
                    </pre>
                  </div>
                </CollapsibleContent>
              </Collapsible>
            </div>
          )}
        </div>
        
        <div className="p-4">
          {/* Connection error details */}
          {connectionError && (
            <div className="mb-4 p-3 text-xs rounded border border-red-200 bg-red-50 dark:bg-red-900/10 dark:border-red-800 text-red-800 dark:text-red-400">
              <div className="font-semibold mb-1">Error Details:</div>
              <div>{connectionError}</div>
            </div>
          )}
          
          {/* Agent list */}
          <AgentList currentAgentId={agentId} />
        </div>
      </div>
      
      {/* Main Message Area */}
      <div className="flex-1 p-4 overflow-y-auto">
        <div className="max-w-4xl mx-auto">
          {/* Messages with proper client-only handling */}
          {messages.length > 0 ? (
            <ClientOnlyMessageList
              messages={messages.map(msg => ({
                ...msg,
                createdAt: msg.createdAt ? new Date(msg.createdAt) : undefined
              }))}
              showTimeStamps={false}
              isTyping={connectionStatus === 'connecting'} 
            />
          ) : connectionStatus === 'connecting' ? (
            <div className="p-12 text-muted-foreground text-center border rounded-lg">
              <div className="flex items-center justify-center space-x-2">
                <div className="animate-bounce">•</div>
                <div className="animate-bounce animation-delay-150">•</div>
                <div className="animate-bounce animation-delay-300">•</div>
              </div>
              <div className="mt-2">Connecting...</div>
            </div>
          ) : (
            <div className="p-12 text-muted-foreground text-center border rounded-lg">
              No messages yet
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default function AgentDetails() {
  // Get agent ID from URL
  const { agentId } = useParams();
  const { githubToken } = useLoaderData<typeof loader>();

  return (
    <>
      <main className="w-full mx-auto h-screen">
        <ClientOnly agentId={agentId || ""} githubToken={githubToken} />
      </main>
    </>
  );
}
