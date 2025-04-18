import { useEffect, useState, useRef, useMemo } from "react";
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
import { Button } from "~/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "~/components/ui/collapsible";
import { ChevronDown, AlertCircle, CheckCircle, ArrowUp, Play, Pause, ListTodo } from "lucide-react";
import { useAgentStore } from "~/lib/store";
import { useAgent } from "agents/react";
import { Label } from "~/components/ui/label";
import { ClientOnlyMessageList } from "~/components/ui/client-only-message-list";
import { AgentList } from "~/components/agent-list";
import { GitHubTokenInput } from "~/components/github-token-input";
import { CopyButton } from "~/components/ui/copy-button";

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

const TOKEN_STORAGE_KEY = "github_token";

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

function ClientOnly({ agentId, children }: { agentId: string, children?: React.ReactNode }) {
  const [mounted, setMounted] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'error' | 'closed'>('connecting');
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [rawState, setRawState] = useState<any>(null);
  const [agentData, setAgentData] = useState<Agent | null>(null);
  const [githubToken, setGithubToken] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const agentStore = useAgentStore();
  
  // Calculate task counts from agent state
  const taskCounts = useMemo(() => {
    const tasks = rawState?.tasks || [];
    return {
      pending: tasks.filter((t: any) => t.status === 'pending').length,
      inProgress: tasks.filter((t: any) => t.status === 'in-progress').length,
      completed: tasks.filter((t: any) => t.status === 'completed').length,
      failed: tasks.filter((t: any) => t.status === 'failed').length,
      cancelled: tasks.filter((t: any) => t.status === 'cancelled').length,
      total: tasks.length,
    };
  }, [rawState?.tasks]);

  // Set up component and log initialization only once
  useEffect(() => {
    setMounted(true);
    console.log("Initializing agent with WebSocket connection for agent:", agentId);

    // Get agent from Zustand store
    const foundAgent = agentStore.getAgent(agentId);
    if (foundAgent) {
      setAgentData(foundAgent);
    }

    // Load GitHub token from localStorage
    const storedToken = localStorage.getItem(TOKEN_STORAGE_KEY);
    if (storedToken) {
      setGithubToken(storedToken);
    }

    // Listen for GitHub token changes
    const handleTokenChange = () => {
      console.log("GitHub token changed, updating token");
      const updatedToken = localStorage.getItem(TOKEN_STORAGE_KEY);
      setGithubToken(updatedToken);
    };

    window.addEventListener('github-token-changed', handleTokenChange);

    // Clean up function to handle component unmounting
    return () => {
      // Reset states when navigating away
      setMessages([]);
      setRawState(null);
      setConnectionStatus('connecting');
      setConnectionError(null);
      window.removeEventListener('github-token-changed', handleTokenChange);
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

  // Handle toggling continuous run
  const handleToggleContinuousRun = () => { // Make it non-async, send is usually fire-and-forget
    if (!agent || connectionStatus !== 'connected') return;

    const currentlyActive = rawState?.isContinuousRunActive || false;
    const command = currentlyActive ? 'stopContinuousRun' : 'startContinuousRun';
    console.log(`Sending command: ${command}`);

    try {
      // Send a structured command message via WebSocket
      agent.send(JSON.stringify({
        type: 'command',
        command: command,
      }));
      console.log(`Sent ${command} command via WebSocket`);
      // State update will come via onStateUpdate, no need to set locally here
    } catch (error) {
      console.error(`Error sending ${command} command:`, error);
      setConnectionError(`Failed to send ${command} command: ${error.message || 'Unknown error'}`);
    }
  };

  // Handle form submission
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || !agent) return;

    // Add user message with proper timestamps and ID
    const userMessage = {
      role: 'user' as const,
      content: input.trim(),
      id: `user-${Date.now()}`,
      createdAt: new Date(),
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

      agent.send(JSON.stringify({
        githubToken: githubToken,
        userMessage: userMessage
      }));

      console.log("Message sent successfully");
    } catch (error) {
      console.error("Error sending message:", error);

      // Show error in UI
      setConnectionError(`Failed to send message: ${error.message || 'Unknown error'}`);
    }
  };

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

  // Focus input when connected
  useEffect(() => {
    if (connectionStatus === 'connected' && inputRef.current) {
      inputRef.current.focus();
    }
  }, [connectionStatus, mounted]);

  if (!mounted) {
    return null;
  }

  const missingToken = !githubToken;

  return (
    <div className="flex h-full">
      {/* Left Sidebar */}
      <div className="w-80 border-r overflow-y-auto flex flex-col">
        {/* Connection status bar */}
        <div className="flex flex-col">
          {/* Status indicator with background */}
          <div className={`px-4 py-2 flex items-center gap-2 ${connectionStatus === 'connected'
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
                    <pre className="bg-muted text-foreground p-2 overflow-auto whitespace-pre-wrap rounded-md max-h-96 text-xs mt-1 relative">
                      <div className="absolute top-2 right-2">
                        <CopyButton content={JSON.stringify(rawState, null, 2)} />
                      </div>
                      {JSON.stringify(rawState, null, 2)}
                    </pre>
                  </div>
                </CollapsibleContent>
              </Collapsible>
              
              {/* Task counts and controls */}
              <div className="mt-3 pt-3 border-t border-border/50">
                <Label className="text-xs font-semibold text-muted-foreground flex items-center gap-1.5 mb-2">
                  <ListTodo className="w-3.5 h-3.5" /> Agent Tasks
                </Label>
                <div className="grid grid-cols-2 gap-x-2 gap-y-1 text-xs mb-3">
                  {(taskCounts.pending > 0) && <div>Pending: {taskCounts.pending}</div>}
                  {(taskCounts.inProgress > 0) && <div>In Progress: {taskCounts.inProgress}</div>}
                  {(taskCounts.completed > 0) && <div>Completed: {taskCounts.completed}</div>}
                  {(taskCounts.failed > 0) && <div className="text-red-600 dark:text-red-500">Failed: {taskCounts.failed}</div>}
                  {(taskCounts.cancelled > 0) && <div>Cancelled: {taskCounts.cancelled}</div>}
                  <div className="col-span-2 mt-1 pt-1 border-t border-border/20">Total: {taskCounts.total}</div>
                </div>

                <Label className="text-xs font-semibold text-muted-foreground block mb-2">
                  Continuous Run
                </Label>
                <Button
                  variant="outline"
                  size="sm"
                  className="w-full h-8 text-xs"
                  onClick={handleToggleContinuousRun}
                  disabled={connectionStatus !== 'connected'} // Disable if not connected
                >
                  {rawState?.isContinuousRunActive ? (
                    <><Pause className="w-3 h-3 mr-2" /> Pause Run</>
                  ) : (
                    <><Play className="w-3 h-3 mr-2" /> Start Run</>
                  )}
                </Button>
              </div>
            </div>
          )}
        </div>

        <div className="p-4 mt-auto"> {/* Added mt-auto to push this section down */}
          {/* GitHub Token Input */}
          <GitHubTokenInput />

          {/* Token missing warning */}
          {missingToken && (
            <div className="mb-4 p-3 text-xs rounded border border-amber-200 bg-amber-50 dark:bg-amber-900/10 dark:border-amber-800 text-amber-800 dark:text-amber-400">
              <div className="font-semibold mb-1">GitHub Token Required</div>
              <div>Please add your GitHub token above to use GitHub tools with this agent.</div>
            </div>
          )}

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

      {/* Main Chat Container */}
      <div className="flex-1 grid grid-rows-[1fr_auto] max-h-full">
        {/* Messages Area with Auto-scroll */}
        <div className="overflow-y-auto">
          <div className="px-4 py-4 max-w-4xl mx-auto w-full h-full">
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

        {/* Fixed Input Area */}
        <div className="border-t bg-background py-3 px-4">
          <div className="max-w-4xl mx-auto w-full">
            <form onSubmit={handleSubmit} className="flex items-center gap-2">
              <input
                ref={inputRef}
                type="text"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                placeholder={!githubToken ? "Add GitHub token to chat" : "Type a message..."}
                disabled={connectionStatus !== 'connected' || !githubToken}
                className="flex-1 px-4 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary bg-background"
              />
              <button
                type="submit"
                disabled={connectionStatus !== 'connected' || !input.trim() || !githubToken}
                className="p-2 rounded-lg bg-primary text-primary-foreground disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <ArrowUp className="h-5 w-5" />
              </button>
            </form>
          </div>
        </div>
      </div>
    </div>
  );
}

export default function AgentDetails() {
  // Get agent ID from URL
  const { agentId } = useParams();

  return (
    <>
      <main className="w-full mx-auto h-screen">
        <ClientOnly agentId={agentId || ""} />
      </main>
    </>
  );
}
