import { useState, useEffect, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Spinner } from "@/components/ui/spinner";
import { Terminal, Columns, BotIcon, PlugZap, Power, CheckCircle, AlertTriangle, BookOpen, FileCode2 } from "lucide-react";
import { useOpenAgent } from "@openagents/core";
import { generateId } from "ai";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

// Add special client directive to address hydration issues
const isClient = typeof window !== "undefined";

interface SolverConnectorProps {
  issue: any;  // The issue object from the parent component
  githubToken: string;
}

// Connection states for the Solver agent
type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'error';

export function SolverConnector({ issue, githubToken }: SolverConnectorProps) {
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
    <Card className="mb-6">
      <CardHeader className="pb-2">
        <div className="flex justify-between items-center">
          <CardTitle className="text-lg flex items-center">
            <BotIcon className="h-5 w-5 mr-2" />
            Solver Agent
          </CardTitle>
          <Badge
            variant={
              connectionState === 'connected' ? "success" :
                connectionState === 'connecting' ? "warning" :
                  connectionState === 'error' ? "destructive" : "secondary"
            }
          >
            {connectionState === 'connected' ? "Connected" :
              connectionState === 'connecting' ? "Connecting..." :
                connectionState === 'error' ? "Error" : "Disconnected"}
          </Badge>
        </div>
      </CardHeader>

      <CardContent>
        {connectionState === 'disconnected' && (
          <div className="text-center py-6">
            <Terminal className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
            <h3 className="text-lg font-medium mb-2">Solver Agent Disconnected</h3>
            <p className="text-muted-foreground mb-4">
              The Solver agent can analyze this issue and help implement a solution.
              It will create a structured plan and guide you through fixing the issue.
            </p>
          </div>
        )}

        {connectionState === 'connecting' && (
          <div className="text-center py-6">
            <Spinner className="h-12 w-12 mx-auto mb-4" />
            <h3 className="text-lg font-medium mb-2">Connecting to Solver Agent</h3>
            <p className="text-muted-foreground">
              Establishing connection and analyzing issue #{issue.identifier}...
            </p>
          </div>
        )}

        {connectionState === 'error' && (
          <div className="text-center py-6">
            <AlertTriangle className="h-12 w-12 mx-auto mb-4 text-destructive" />
            <h3 className="text-lg font-medium mb-2">Connection Error</h3>
            <p className="text-muted-foreground mb-4">
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
          <div className="py-2">
            <div className="flex items-center">
              <CheckCircle className="h-5 w-5 mr-2 text-green-500" />
              <span className="font-medium">Solver Agent Connected</span>
            </div>

            {agent.messages.length > 1 && (
              <div className="border rounded-md p-3 mt-4 bg-muted/50">
                <h4 className="font-medium mb-2">Latest Update:</h4>
                <p className="text-sm">
                  {agent.messages[agent.messages.length - 1].content.substring(0, 150)}
                  {agent.messages[agent.messages.length - 1].content.length > 150 ? '...' : ''}
                </p>
              </div>
            )}
          </div>
        )}
      </CardContent>

      <CardFooter className="flex justify-between">
        {connectionState === 'connected' && (
          <div className="flex gap-2 flex-wrap">
            <Button
              variant="outline"
              onClick={() => {
                // Create a user message to send to the agent
                const testMessage = `Test message sent at ${new Date().toISOString()}`;
                console.log("Sending test message:", testMessage);

                // Use the handleSubmit method to send a user message
                agent.handleSubmit(testMessage)
                  .then(() => {
                    console.log("Test message submitted successfully");
                  })
                  .catch(error => {
                    console.error("Error submitting test message:", error);
                  });
              }}
            >
              Send Test Message
            </Button>

            <Button
              variant="outline"
              onClick={() => {
                // Use the raw message sending capability
                const observation = `UI Observation at ${new Date().toISOString()}`;
                console.log("Sending raw observation message to agent:", observation);

                // Format the message as a direct WebSocket message
                // This should match how the agent's onMessage handler expects it
                const message = {
                  type: "observation",
                  content: observation,
                  timestamp: new Date().toISOString(),
                  issueId: issue.id
                };

                // Send the raw message directly via WebSocket
                agent.sendRawMessage(message);
              }}
            >
              Add Observation
            </Button>

            <Button
              variant="outline"
              onClick={() => {
                // Send a status update message
                const status = `Status update at ${new Date().toISOString()}`;
                console.log("Sending status update message:", status);

                const message = {
                  type: "status_update",
                  content: status,
                  timestamp: new Date().toISOString(),
                  issueId: issue.id
                };

                agent.sendRawMessage(message);
              }}
            >
              Send Status Update
            </Button>

            <Button
              variant="outline"
              onClick={() => {
                // Send a custom command message that the agent might understand
                console.log("Sending custom command message");

                const message = {
                  type: "command",
                  command: "analyze_issue",
                  timestamp: new Date().toISOString(),
                  issueId: issue.id,
                  params: {
                    priority: "high",
                    context: "ui-testing"
                  }
                };

                agent.sendRawMessage(message);
              }}
            >
              Send Command
            </Button>

            <Button
              variant="outline"
              onClick={async () => {
                try {
                  // Create a test message with proper UIMessage typing including required parts array
                  const testMessage = {
                    id: generateId(),
                    role: 'user' as const,
                    content: `This is a test message. Can you summarize what you know about the current issue?`,
                    parts: [{
                      type: 'text' as const,
                      text: `This is a test message. Can you summarize what you know about the current issue?`
                    }]
                  };

                  // Run shared inference with just the test message
                  // The agent will use its own system prompt automatically
                  console.log("Running shared inference...");
                  const result = await agent.sharedInfer({
                    model: "@cf/meta/llama-4-scout-17b-16e-instruct",
                    messages: [testMessage],
                    temperature: 0.7,
                    max_tokens: 500
                  });

                  // Log the result
                  console.log("Shared inference result:", result);

                  // Add the result to the messages
                  agent.setMessages([
                    ...agent.messages,
                    {
                      id: result.id,
                      role: 'assistant',
                      content: result.content
                    }
                  ]);

                } catch (error) {
                  console.error("Error running shared inference:", error);
                }
              }}
            >
              Test Shared Inference
            </Button>

            <Dialog
              open={promptDialogOpen}
              onOpenChange={(open) => {
                setPromptDialogOpen(open);

                // When opening the dialog, fetch the system prompt
                if (open) {
                  setIsLoadingPrompt(true);
                  setSystemPrompt('Loading system prompt...');

                  console.log("====== FETCHING SYSTEM PROMPT ======");
                  console.log("Agent state:", agent.state);
                  console.log("ProjectData exists:", !!agent.state.currentProject);
                  console.log("TeamData exists:", !!agent.state.currentTeam);

                  // Let's try setting the project and team data again before fetching the prompt
                  if (formattedProject && formattedTeam) {
                    console.log("ATTEMPT TO RE-SET PROJECT AND TEAM BEFORE GETTING PROMPT");

                    try {
                      // Use raw message sending directly - this matches how the other buttons work
                      const contextMessage = {
                        type: "set_context",
                        issue: formattedIssue,
                        project: formattedProject,
                        team: formattedTeam,
                        timestamp: new Date().toISOString()
                      };

                      // Send the raw message
                      agent.sendRawMessage(contextMessage);
                      console.log("Context data re-sent successfully before fetching prompt");

                      // Wait a short time for the agent to process the context
                      setTimeout(() => {
                        fetchSystemPrompt();
                      }, 300);
                    } catch (err) {
                      console.error("Failed to re-send context data:", err);
                      fetchSystemPrompt();
                    }
                  } else {
                    fetchSystemPrompt();
                  }

                  function fetchSystemPrompt() {
                    // Fetch the system prompt
                    agent.getSystemPrompt()
                      .then(prompt => {
                        console.log("System prompt received from agent:", prompt.substring(0, 100) + "...");

                        // Check if it has our project/team sections
                        const hasProjectSection = prompt.includes("PROJECT CONTEXT:");
                        const hasTeamSection = prompt.includes("TEAM CONTEXT:");

                        console.log("Prompt analysis:", {
                          hasProjectSection,
                          hasTeamSection,
                          length: prompt.length
                        });

                        if (!hasProjectSection && !hasTeamSection) {
                          console.warn("WARNING: System prompt does not contain project or team sections");
                          console.warn("This may indicate the agent server is not using the updated code");

                          // Create a manually enhanced prompt for demonstration
                          if (formattedProject || formattedTeam) {
                            let enhancedPrompt = prompt;

                            if (formattedProject) {
                              enhancedPrompt += `\n\nPROJECT CONTEXT (CLIENT-SIDE FALLBACK):
Name: ${formattedProject.name}
ID: ${formattedProject.id}
${formattedProject.color ? `Color: ${formattedProject.color}` : ''}
${formattedProject.icon ? `Icon: ${formattedProject.icon}` : ''}`;
                            }

                            if (formattedTeam) {
                              enhancedPrompt += `\n\nTEAM CONTEXT (CLIENT-SIDE FALLBACK):
Name: ${formattedTeam.name}
ID: ${formattedTeam.id}
Key: ${formattedTeam.key || 'N/A'}`;
                            }

                            console.log("Created enhanced prompt with client-side fallback");
                            setSystemPrompt(enhancedPrompt);
                          } else {
                            setSystemPrompt(prompt);
                          }
                        } else {
                          setSystemPrompt(prompt);
                        }

                        setIsLoadingPrompt(false);
                      })
                      .catch(error => {
                        console.error("Error fetching system prompt:", error);
                        setSystemPrompt(`Failed to load system prompt: ${error.message || 'Unknown error'}`);
                        setIsLoadingPrompt(false);
                      });
                  }
                }
              }}
            >
              <DialogTrigger asChild>
                <Button variant="outline">
                  <BookOpen className="h-4 w-4 mr-2" />
                  View System Prompt
                </Button>
              </DialogTrigger>
              <DialogContent className="max-w-4xl max-h-[80vh] overflow-y-auto">
                <DialogHeader>
                  <DialogTitle className="flex items-center">
                    <FileCode2 className="mr-2 h-5 w-5" />
                    Solver Agent System Prompt
                  </DialogTitle>
                  <DialogDescription>
                    This is the system prompt that guides the agent's behavior and capabilities.
                  </DialogDescription>
                </DialogHeader>
                <div className="mt-4">
                  {isLoadingPrompt ? (
                    <div className="flex justify-center py-8">
                      <Spinner className="h-8 w-8" />
                    </div>
                  ) : (
                    <div className="p-4 bg-muted rounded-md overflow-auto">
                      <pre className="text-sm whitespace-pre-wrap font-mono" style={{ maxHeight: '60vh' }}>
                        {systemPrompt}
                      </pre>
                    </div>
                  )}
                </div>
              </DialogContent>
            </Dialog>
          </div>
        )}

        <div>
          {/* Server-side rendering always shows initial state - Client will rehydrate */}
          {(!isClient || !isHydrated) ? (
            // Default state for SSR
            <Button variant="default" suppressHydrationWarning>
              <PlugZap className="h-4 w-4 mr-2" />
              Connect to Solver
            </Button>
          ) : connectionState === 'disconnected' ? (
            // Client-side rendering for disconnected state
            <Button
              variant={isConnectButtonDisabled ? "secondary" : "default"}
              className={isConnectButtonDisabled ? "opacity-50 cursor-not-allowed" : ""}
              onClick={isConnectButtonDisabled ? undefined : connectToSolver}
              suppressHydrationWarning
            >
              <PlugZap className="h-4 w-4 mr-2" />
              Connect to Solver
            </Button>
          ) : (
            // Client has connected or error state - no disconnect button needed due to autoreconnect
            <></>
          )}
        </div>
      </CardFooter>
    </Card>
  );
}
