import React, { useState, useEffect, useCallback, useRef, useLayoutEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Spinner } from "@/components/ui/spinner";
import { Terminal, Columns, BotIcon, PlugZap, Power, CheckCircle, AlertTriangle, BookOpen, FileCode2 } from "lucide-react";
import { useOpenAgent, type Message as AgentMessage } from "@openagents/core";
import { generateId } from "ai";
import { MessageList } from "@/components/ui/message-list";
import { cn } from "@/lib/utils";
import { Input } from "../ui/input";
import type { TextUIPart, UIMessage } from "@ai-sdk/ui-utils";

interface SolverConnectorProps {
  issue: any;  // The issue object from the parent component
  githubToken: string;
  className?: string;
}

// Connection states for the Solver agent
type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'error';

// Convert agent message to UI message
const toUIMessage = (msg: AgentMessage): UIMessage => ({
  ...msg,
  parts: msg.parts?.map(part => ({
    ...part,
    type: 'text'
  })) as TextUIPart[] || []
});

export function SolverConnector({
  issue,
  githubToken,
  className = "",
}: SolverConnectorProps & { className?: string }) {
  // Use default state for server-side rendering, then update on client
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [isStartingSolver, setIsStartingSolver] = useState(false);

  // For consistent server/client rendering, force hydration after mount
  const [isHydrated, setIsHydrated] = useState(false);

  // State for system prompt dialog
  const [systemPrompt, setSystemPrompt] = useState<string>('Loading...');
  const [isLoadingPrompt, setIsLoadingPrompt] = useState(false);
  const [promptDialogOpen, setPromptDialogOpen] = useState(false);

  useEffect(() => {
    setIsHydrated(true);
  }, []);

  // Create a formatted issue object to send to the Solver agent
  const formattedIssue = {
    id: issue.id,
    number: parseInt(issue.identifier.replace(/[^\d]/g, '')),
    title: issue.title,
    description: issue.description || "",
    source: "openagents", // Using our own source identifier
    status: issue.status.type === 'done' ? 'closed' : 'open',
    labels: issue.labels?.map((label: any) => label.name) || [],
    assignee: issue.assignee?.name,
    created: new Date(issue.createdAt),
    updated: issue.updatedAt ? new Date(issue.updatedAt) : undefined
  };

  // Create formatted project object if available - using BaseProject interface
  const formattedProject = issue.project ? {
    id: issue.project.id,
    name: issue.project.name,
    color: issue.project.color,
    icon: issue.project.icon
  } : undefined;

  // Create formatted team object if available - using BaseTeam interface
  const formattedTeam = issue.team ? {
    id: issue.team.id,
    name: issue.team.name,
    key: issue.team.key || 'default'
  } : undefined;

  // Extract repository context from issue or use defaults
  const repoInfo = {
    owner: "openagentsinc", // Replace with dynamic value if available
    repo: "openagents",  // Replace with dynamic value if available
    branch: "main"      // Replace with dynamic value if available
  };

  // Use the OpenAgent hook to connect to the Solver agent
  // Don't add "solver-" prefix here since useOpenAgent already adds it
  const agent = useOpenAgent(issue.id, "solver");

  // Sync connection state from the agent hook
  useEffect(() => {
    // Use the agent's connection status directly
    console.log("Agent connection status:", agent.connectionStatus);
    setConnectionState(agent.connectionStatus);

    // If there's an error, set an error message
    if (agent.connectionStatus === 'error') {
      setErrorMessage("Connection to Solver agent failed or was lost.");
    }

    // Use the agent-specific event names
    const agentName = `solver-${issue.id}`;
    const connectedEventName = `agent:${agentName}:connected`;
    const disconnectedEventName = `agent:${agentName}:disconnected`;
    const errorEventName = `agent:${agentName}:error`;

    // Add event listeners to track WebSocket connection status
    const handleConnected = (event: Event) => {
      console.log("Solver UI: Received connection event", event);
      setConnectionState('connected');
    };

    const handleDisconnected = (event: Event) => {
      console.log("Solver UI: Received disconnection event", event);
      setConnectionState('disconnected');
    };

    const handleError = (event: Event) => {
      console.log("Solver UI: Received connection error event", event);
      setConnectionState('error');
      setErrorMessage("Connection to Solver agent failed or was lost.");
    };

    // Listen to agent-specific events to avoid recursion
    window.addEventListener(connectedEventName, handleConnected);
    window.addEventListener(disconnectedEventName, handleDisconnected);
    window.addEventListener(errorEventName, handleError);

    return () => {
      window.removeEventListener(connectedEventName, handleConnected);
      window.removeEventListener(disconnectedEventName, handleDisconnected);
      window.removeEventListener(errorEventName, handleError);
    };
  }, [agent.connectionStatus, agent.state, issue.id]);

  // Handle connection to the Solver agent
  const connectToSolver = async () => {
    console.log("===== DEEP DEBUG START =====");
    console.log("Issue data:", issue);
    console.log("Formatted issue:", formattedIssue);
    console.log("Formatted project:", formattedProject);
    console.log("Formatted team:", formattedTeam);
    if (!githubToken) {
      setErrorMessage("GitHub token is required. Please set it in your account settings.");
      setConnectionState('error');
      return;
    }

    setConnectionState('connecting');
    setIsStartingSolver(true);

    try {
      // Log connection information
      console.log("=== CONNECTING TO SOLVER AGENT ===");
      console.log("Issue ID:", issue.id);
      console.log("Issue Identifier:", issue.identifier);
      console.log("Token (first 10 chars):", githubToken.substring(0, 10) + "...");

      // Add timeout promise to detect stalled connections
      const timeoutPromise = new Promise((_, reject) => {
        setTimeout(() => reject(new Error("Connection timed out. The agent server may be unreachable.")), 30000);
      });

      // Basic step 1: Set GitHub token
      console.log("Step 1: Setting GitHub token...");

      try {
        await Promise.race([
          agent.setGithubToken(githubToken),
          timeoutPromise
        ]);
        console.log("✓ GitHub token set successfully");
      } catch (error) {
        console.error("✗ Failed to set GitHub token:", error);
        throw new Error(`Failed to set GitHub token: ${error instanceof Error ? error.message : "Unknown error"}`);
      }

      // Basic step 2: Set repository context
      if (agent.setRepositoryContext) {
        console.log("Step 2: Setting repository context...");
        try {
          await agent.setRepositoryContext(repoInfo.owner, repoInfo.repo, repoInfo.branch);
          console.log("✓ Repository context set successfully");
        } catch (error) {
          console.error("✗ Failed to set repository context:", error);
          throw new Error(`Failed to set repository context: ${error instanceof Error ? error.message : "Unknown error"}`);
        }
      }

      // Basic step 3: Set current issue with project and team context
      console.log("Step 3: Setting current issue with project and team context...");
      console.log("Project data being passed:", formattedProject ? JSON.stringify(formattedProject) : 'undefined');
      console.log("Team data being passed:", formattedTeam ? JSON.stringify(formattedTeam) : 'undefined');

      try {
        // Use raw message sending directly - this matches how the other agent buttons work
        console.log("Sending issue, project and team context via raw message...");

        // Format in the same way the agent expects to receive state updates
        const contextMessage = {
          type: "set_context",
          issue: formattedIssue,
          project: formattedProject,
          team: formattedTeam,
          timestamp: new Date().toISOString()
        };

        // Send the raw message
        agent.sendRawMessage(contextMessage);
        console.log("✓ Context message sent successfully");
      } catch (error) {
        console.error("✗ Failed to send context message:", error);
        throw new Error(`Failed to send context: ${error instanceof Error ? error.message : "Unknown error"}`);
      }

      // Basic step 4: Submit initial prompt
      console.log("Step 4: Sending initial prompt to agent...");
      const initialPrompt = `I need help with issue ${issue.identifier}: "${issue.title}". Please analyze this issue and suggest a plan to solve it.`;
      console.log("Initial prompt:", initialPrompt);

      try {
        await agent.handleSubmit(initialPrompt);
        console.log("✓ Initial prompt sent successfully");
      } catch (error) {
        console.error("✗ Failed to send initial prompt:", error);
        throw new Error(`Failed to send initial prompt: ${error instanceof Error ? error.message : "Unknown error"}`);
      }

      // Basic step 5: Start inference
      console.log("Step 5: Starting agent inference...");
      try {
        await agent.infer(githubToken);
        console.log("✓ Agent inference started successfully");
      } catch (error) {
        console.error("✗ Failed to start agent inference:", error);
        throw new Error(`Failed to start agent inference: ${error instanceof Error ? error.message : "Unknown error"}`);
      }

      console.log("=== AGENT CONNECTION PROCESS COMPLETE ===");
      setConnectionState('connected');
    } catch (err) {
      console.error("=== ERROR CONNECTING TO SOLVER AGENT ===", err);
      setErrorMessage(err instanceof Error ? err.message : "Failed to connect to the Solver agent. Please try again later.");
      setConnectionState('error');
    } finally {
      setIsStartingSolver(false);
    }
  };

  // This can be used to determine if the button should be disabled
  const isConnectButtonDisabled = isStartingSolver || !githubToken;

  // Add a retry mechanism
  const [retryCount, setRetryCount] = useState(0);
  const maxRetries = 3;

  const retryConnection = useCallback(() => {
    if (retryCount < maxRetries) {
      setRetryCount(prev => prev + 1);
      setConnectionState('disconnected');
      setErrorMessage('');
      setIsStartingSolver(false);
      console.log(`Retry attempt ${retryCount + 1}/${maxRetries}`);
    } else {
      setErrorMessage(`Failed after ${maxRetries} attempts. The agent server may be down or unreachable.`);
    }
  }, [retryCount, maxRetries]);

  // Disconnect from the Solver agent
  const disconnectFromSolver = () => {
    console.log("Disconnecting from Solver agent...");

    // Use the proper disconnect method to close the WebSocket
    agent.disconnect();

    // Also update local UI state
    setConnectionState('disconnected');

    console.log("Successfully disconnected from Solver agent");
  };

  // Update connection state based on agent messages
  useEffect(() => {
    // If we get messages when disconnected, we may be connected
    if (agent.messages.length > 0 && connectionState === 'disconnected') {
      console.log("Agent has messages, connection might be working");
    }
  }, [agent.messages, connectionState]);

  // Create ref at the component level, not inside the effect
  const contextSetRef = React.useRef(false);

  // Add debug logging for context state
  useEffect(() => {
    if (connectionState === 'connected' && agent.state) {
      console.log("Context state debug:", {
        hasIssue: !!agent.state?.currentIssue,
        hasProject: !!agent.state?.currentProject,
        hasTeam: !!agent.state?.currentTeam,
        contextSetRef: contextSetRef.current
      });
    }
  }, [connectionState, agent.state]);

  // Ensure context is set when connected
  useEffect(() => {
    // Only proceed if connected
    if (connectionState !== 'connected') {
      // Reset the ref when disconnected so we can set context on next connection
      contextSetRef.current = false;
      return;
    }

    // Check if we need to set context (either ref is false or state is missing context)
    const needsContextSet = !contextSetRef.current ||
      !agent.state?.currentIssue ||
      !agent.state?.currentProject ||
      !agent.state?.currentTeam;

    if (needsContextSet) {
      // Mark as set to prevent infinite loop
      contextSetRef.current = true;

      // Wait a brief moment to let the connection stabilize
      setTimeout(() => {
        // Send context data to ensure it's always set
        console.log("Auto-sending context data on connection...");
        try {
          // Format in the same way the agent expects to receive state updates
          const contextMessage = {
            type: "set_context",
            issue: formattedIssue,
            project: formattedProject,
            team: formattedTeam,
            timestamp: new Date().toISOString()
          };

          // Send the raw message
          agent.sendRawMessage(contextMessage);
          console.log("✓ Context auto-set on connection");
        } catch (error) {
          console.error("Failed to auto-set context:", error);
        }
      }, 500);
    } else {
      console.log("Context already set, skipping auto-set");
    }
  }, [connectionState, formattedIssue, formattedProject, formattedTeam, agent, agent.state]);

  // Create a message container that starts scrolled to the bottom
  const MessageContainer = ({ children }: { children: React.ReactNode }) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const isInitialRender = useRef(true);

    // Position scroll at bottom after DOM is updated
    useLayoutEffect(() => {
      if (containerRef.current) {
        // Set scroll to bottom on render
        containerRef.current.scrollTop = containerRef.current.scrollHeight;

        if (isInitialRender.current) {
          // If it's the first render, also schedule another scroll after images/content load
          setTimeout(() => {
            if (containerRef.current) {
              containerRef.current.scrollTop = containerRef.current.scrollHeight;
            }
          }, 100);
          isInitialRender.current = false;
        }
      }
    }, [agent.messages.length]);

    // When new messages arrive, scroll to bottom if already near bottom
    useEffect(() => {
      if (!isInitialRender.current && containerRef.current) {
        const container = containerRef.current;
        const isNearBottom =
          container.scrollHeight - container.clientHeight <=
          container.scrollTop + 100; // Within 100px of bottom

        if (isNearBottom) {
          // Immediately jump to bottom without animation
          container.scrollTop = container.scrollHeight;
        }
      }
    }, [agent.messages]);

    return (
      <div
        ref={containerRef}
        className="flex-1 overflow-y-auto px-4 py-2 min-h-0"
        style={{ overscrollBehavior: 'contain', maxHeight: 'calc(100% - 60px)' }} // Prevent scroll chaining and set max height
      >
        {children}
      </div>
    );
  };

  // Monitor the connection status
  useEffect(() => {
    let connectionStartTime: number | null = null;
    const connectionTimeout = 15000; // 15 seconds timeout

    if (connectionState === 'connecting') {
      connectionStartTime = Date.now();
    }

    // Check connection status periodically
    const intervalId = setInterval(() => {
      if (connectionState === 'connecting' && connectionStartTime) {
        const elapsedTime = Date.now() - connectionStartTime;
        console.log(`Still connecting... (${Math.round(elapsedTime / 1000)}s elapsed)`);

        // If we've been connecting for more than the timeout, consider it an error
        if (elapsedTime > connectionTimeout) {
          console.error("Connection timeout exceeded");
          setConnectionState('error');
          setErrorMessage("Connection timed out. The agent server may be unreachable.");
        }
      } else if (connectionState === 'connected') {
        // Periodically check if our connection is still valid by checking for agent state
        if (!agent.state || Object.keys(agent.state).length === 0) {
          console.warn("Connected but no agent state - connection may be stale");
        } else {
          console.log("Connection confirmed with valid agent state");
        }
      }
    }, 3000);

    // Cleanup interval
    return () => clearInterval(intervalId);
  }, [connectionState, agent.state]);

  return (
    <Card className={cn("py-0 h-full flex flex-col", className)}>
      <CardContent className="flex-1 flex flex-col overflow-hidden p-0 pt-0">
        {connectionState === 'disconnected' && (
          <div className="text-center py-6 overflow-auto flex-1">
            <Terminal className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
            <h3 className="text-lg font-medium mb-2">Solver Agent Disconnected</h3>
            <p className="text-muted-foreground mb-4 px-4">
              The Solver agent can analyze this issue and help implement a solution.
              It will create a structured plan and guide you through fixing the issue.
            </p>
          </div>
        )}

        {connectionState === 'connecting' && (
          <div className="text-center py-6 overflow-auto flex-1">
            <Spinner className="h-12 w-12 mx-auto mb-4" />
            <h3 className="text-lg font-medium mb-2">Connecting to Solver Agent</h3>
            <p className="text-muted-foreground px-4">
              Establishing connection and analyzing issue #{issue.identifier}...
            </p>
          </div>
        )}

        {connectionState === 'error' && (
          <div className="text-center py-6 overflow-auto flex-1">
            <AlertTriangle className="h-12 w-12 mx-auto mb-4 text-destructive" />
            <h3 className="text-lg font-medium mb-2">Connection Error</h3>
            <p className="text-muted-foreground mb-4 px-4">
              {errorMessage || "Failed to connect to the Solver agent. Please try again."}
            </p>
            {retryCount < maxRetries && (
              <Button
                variant="outline"
                size="sm"
                onClick={retryConnection}
                className="mt-2"
              >
                Retry Connection
              </Button>
            )}
          </div>
        )}

        {connectionState === 'connected' && (
          <div className="h-full flex flex-col">
            <div className="rounded-md flex-1 flex flex-col overflow-hidden">
              {/* Use our auto-scrolling container */}
              <MessageContainer>
                {/* Convert agent messages to MessageList format */}
                <MessageList
                  messages={agent.messages.map(message => ({
                    id: message.id,
                    role: message.role as UIMessage['role'],
                    content: message.content || '',
                    parts: [{
                      type: 'text' as const,
                      text: message.content || ''
                    }] as TextUIPart[]
                  }))}
                  showTimeStamps={true}
                />
              </MessageContainer>

              {/* Add message input */}
              <div className="px-4 py-3 border-t flex-shrink-0">
                <form onSubmit={async (e) => {
                  e.preventDefault();
                  const input = e.currentTarget.elements.namedItem('message') as HTMLInputElement;
                  if (input && input.value.trim()) {
                    // Create a user message
                    const userMessage = {
                      id: generateId(),
                      role: 'user' as const,
                      content: input.value,
                      parts: [{
                        type: 'text' as const,
                        text: input.value
                      }]
                    };

                    // Add the user message to the agent
                    agent.setMessages([...agent.messages, userMessage]);

                    agent.sharedInfer({
                      model: "@cf/meta/llama-4-scout-17b-16e-instruct",
                      messages: [...agent.messages.map(message => ({
                        id: message.id,
                        role: message.role as UIMessage['role'],
                        content: message.content || '',
                        parts: [{
                          type: 'text' as const,
                          text: message.content || ''
                        }] as TextUIPart[]
                      })), userMessage],
                      stream: true
                    });

                    // Send the message to the agent and get a response
                    agent.handleSubmit(input.value)
                      .then(() => {
                        console.log("Message sent to agent");
                      })
                      .catch(error => {
                        console.error("Error sending message to agent:", error);
                      });

                    // Clear the input
                    input.value = '';
                  }
                }} className="flex flex-row items-center">
                  <Input
                    type="text"
                    name="message"
                    autoFocus
                    autoComplete="off"
                    placeholder="Send a message..."
                    className="flex-1 min-w-0 bg-background border border-input rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                  />
                  <Button type="submit" size="sm" className="ml-2" variant="outline">
                    Send
                  </Button>
                </form>
              </div>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
