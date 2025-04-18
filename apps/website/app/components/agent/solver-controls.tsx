import React, { useState, useEffect, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Spinner } from "@/components/ui/spinner";
import { generateId } from 'ai'
import { BotIcon, PlugZap, CheckCircle, AlertTriangle, BookOpen, FileCode2 } from "lucide-react";
import { useOpenAgent } from "@openagents/core";
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

interface SolverControlsProps {
  issue: any;  // The issue object from the parent component
  agent: any;  // The agent instance passed from parent
  githubToken: string;
}

// Connection states for the Solver agent
type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'error';

export function SolverControls({ issue, agent, githubToken }: SolverControlsProps) {
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

  // Agent instance is now passed from parent
  // No need to create a new hook instance

  // Sync connection state from the agent hook
  useEffect(() => {
    // Use the agent's connection status directly
    console.log("Agent connection status in controls:", agent.connectionStatus);
    setConnectionState(agent.connectionStatus);

    // If there's an error, set an error message
    if (agent.connectionStatus === 'error') {
      setErrorMessage("Connection to Solver agent failed or was lost.");
    }
  }, [agent.connectionStatus]);

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

  return (
    <Card className="mb-4">
      <CardHeader className="pb-2">
        <CardTitle className="text-sm flex items-center">
          <BotIcon className="h-4 w-4 mr-2" />
          <span>AI Agent Controls</span>
        </CardTitle>
      </CardHeader>

      <CardContent className="space-y-4">
        {connectionState === 'disconnected' && (
          <div>
            <p className="text-xs text-muted-foreground mb-2">
              Connect the AI agent to help solve this issue. The agent will analyze the issue and suggest solutions.
            </p>
            {errorMessage && (
              <div className="p-2 rounded-md bg-destructive/10 text-destructive text-xs mb-2">
                {errorMessage}
              </div>
            )}
          </div>
        )}

        {connectionState === 'connecting' && (
          <div className="flex items-center gap-2">
            <Spinner className="h-4 w-4" />
            <p className="text-xs">Connecting to agent...</p>
          </div>
        )}

        {connectionState === 'error' && (
          <div>
            <div className="p-2 rounded-md bg-destructive/10 text-destructive text-xs mb-2">
              {errorMessage || "Error connecting to agent"}
            </div>
            {retryCount < maxRetries && (
              <Button
                variant="outline"
                size="sm"
                onClick={retryConnection}
                className="w-full mt-2"
              >
                Retry Connection
              </Button>
            )}
          </div>
        )}

        {/* Connected state summary */}
        {connectionState === 'connected' && (
          <div>
            <div className="flex items-center gap-2 mb-2">
              <CheckCircle className="h-4 w-4 text-green-500" />
              <p className="text-xs font-medium">Agent Connected & Active</p>
            </div>
            <p className="text-xs text-muted-foreground">
              The agent is analyzing the issue and providing suggestions in the main panel.
            </p>
          </div>
        )}
      </CardContent>

      <CardFooter className="pt-0 flex flex-col gap-2">
        {/* Connect button */}
        <div className="w-full">
          {(!isClient || !isHydrated) ? (
            // Default state for SSR
            <Button variant="default" suppressHydrationWarning className="w-full">
              <PlugZap className="h-4 w-4 mr-2" />
              Connect Agent
            </Button>
          ) : connectionState === 'disconnected' ? (
            // Client-side rendering for disconnected state
            <Button
              variant={isConnectButtonDisabled ? "secondary" : "default"}
              className={`w-full ${isConnectButtonDisabled ? "opacity-50 cursor-not-allowed" : ""}`}
              onClick={isConnectButtonDisabled ? undefined : connectToSolver}
              suppressHydrationWarning
            >
              <PlugZap className="h-4 w-4 mr-2" />
              Connect Agent
            </Button>
          ) : (
            // Connecting or error state
            <Button
              variant="outline"
              className="w-full opacity-50 cursor-not-allowed"
              disabled
              suppressHydrationWarning
            >
              <BotIcon className="h-4 w-4 mr-2" />
              {connectionState === 'connecting' ? "Connecting..." : connectionState === 'error' ? "Error" : "Connected"}
            </Button>
          )}
        </div>

        {/* Demo buttons only shown when connected */}
        {connectionState === 'connected' && (
          <>
            {/* Send test message */}
            <Button
              variant="outline"
              size="sm"
              className="w-full text-xs"
              onClick={async () => {
                try {
                  // Create a user message
                  const testMessage = {
                    id: generateId(),
                    role: 'user' as const,
                    content: `What do you know about this issue? Please summarize the current context.`,
                    parts: [{
                      type: 'text' as const,
                      text: `What do you know about this issue? Please summarize the current context.`
                    }]
                  };

                  // Add the user message to the agent
                  agent.setMessages([...agent.messages, testMessage]);

                  // Send the message to the agent
                  agent.handleSubmit(testMessage.content)
                    .then(() => {
                      console.log("Test message sent to agent");
                    })
                    .catch(error => {
                      console.error("Error sending test message to agent:", error);
                    });
                } catch (error) {
                  console.error("Error with test message:", error);
                }
              }}
            >
              Send Test Message
            </Button>

            {/* Test shared inference */}
            <Button
              variant="outline"
              size="sm"
              className="w-full text-xs"
              onClick={async () => {
                try {
                  // Create a test message with proper typing
                  const testMessage = {
                    id: generateId(),
                    role: 'user' as const,
                    content: `Please detail everything you know about the current issue, project, and team context.`,
                    parts: [{
                      type: 'text' as const,
                      text: `Please detail everything you know about the current issue, project, and team context.`
                    }]
                  };

                  // Add the message to the chat immediately for UI responsiveness
                  agent.setMessages([...agent.messages, testMessage]);
                  
                  // Get all messages for the inference (full chat history)
                  const allMessages = [...agent.messages].map(message => ({
                    id: message.id,
                    role: message.role as 'user' | 'assistant' | 'system',
                    content: message.content || '',
                    parts: [{
                      type: 'text' as const,
                      text: message.content || ''
                    }]
                  }));
                  
                  // Check if context is present
                  console.log("AGENT STATE BEFORE INFERENCE:", JSON.stringify({
                    hasIssue: !!agent.state?.currentIssue,
                    hasProject: !!agent.state?.currentProject,
                    hasTeam: !!agent.state?.currentTeam,
                    issueDetails: agent.state?.currentIssue ? {
                      id: agent.state.currentIssue.id,
                      title: agent.state.currentIssue.title,
                      source: agent.state.currentIssue.source
                    } : null
                  }));
                  
                  // Make sure context is set
                  const contextMissing = !agent.state?.currentIssue || !agent.state?.currentProject || !agent.state?.currentTeam;
                  if (contextMissing) {
                    console.log("Context missing, setting it now...");
                    
                    // Set context
                    const contextMessage = {
                      type: "set_context",
                      issue: formattedIssue,
                      project: formattedProject,
                      team: formattedTeam,
                      timestamp: new Date().toISOString()
                    };
                    
                    console.log("MANUALLY SENDING CONTEXT BEFORE INFERENCE:", JSON.stringify(contextMessage));
                    agent.sendRawMessage(contextMessage);
                    
                    // Wait for context to be processed
                    await new Promise(resolve => setTimeout(resolve, 500));
                    
                    console.log("STATE AFTER CONTEXT UPDATE:", JSON.stringify({
                      hasIssue: !!agent.state?.currentIssue,
                      hasProject: !!agent.state?.currentProject,
                      hasTeam: !!agent.state?.currentTeam
                    }));
                  }
                
                  // Get system prompt explicitly
                  console.log("Fetching system prompt for inference...");
                  let systemPrompt = null;
                  try {
                    systemPrompt = await agent.getSystemPrompt();
                    console.log("SYSTEM PROMPT RETRIEVED:", systemPrompt.substring(0, 200) + "...");
                    console.log("SYSTEM PROMPT CONTENT CHECK:", {
                      hasIssue: systemPrompt.includes("CURRENT ISSUE"),
                      hasProject: systemPrompt.includes("PROJECT CONTEXT"),
                      hasTeam: systemPrompt.includes("TEAM CONTEXT")
                    });
                  } catch (promptError) {
                    console.error("Error fetching system prompt:", promptError);
                  }
                  
                  // Run shared inference with full message history and explicit system prompt
                  console.log("Running shared inference with explicit system prompt...");
                  const requestId = generateId();
                  console.log(`Sending inference request with ID ${requestId}`);
                  
                  const response = await agent.sendRawMessage({
                    type: "shared_infer",
                    requestId: requestId,
                    params: {
                      model: "@cf/meta/llama-4-scout-17b-16e-instruct",
                      messages: allMessages,
                      system: systemPrompt, // Explicitly include system prompt
                      temperature: 0.7,
                      max_tokens: 1000,
                      stream: true
                    },
                    // Also include context data in case agent needs to restore it
                    context: {
                      issue: formattedIssue,
                      project: formattedProject,
                      team: formattedTeam
                    },
                    timestamp: new Date().toISOString()
                  });
                  
                  console.log("Shared inference response:", response);
                  
                  // Extract the result from the response
                  const result = response?.result;
                  
                  // Log raw result for debugging
                  console.log("Result object:", result);
                  
                  // Don't show an error message immediately - WebSocket responses are asynchronous
                  if (!result || !result.id || !result.content) {
                    console.log("Waiting for async inference result via WebSocket...");
                    // The server will add the result to messages state when available
                    // We'll rely on useOpenAgent's internal WebSocket handler to update messages
                    return;
                  }
                  
                  console.log("Shared inference completed successfully");
                  
                  // Add the assistant's response to the message history if not already added
                  if (result && result.id && result.content) {
                    // Check if this response is already in the messages
                    const responseExists = agent.messages.some(msg => msg.id === result.id);
                    
                    if (!responseExists) {
                      console.log("Adding assistant response to message history:", result.content.substring(0, 50) + "...");
                      
                      // Create a proper assistant message
                      const assistantMessage = {
                        id: result.id,
                        role: 'assistant' as const,
                        content: result.content,
                        parts: [{
                          type: 'text' as const,
                          text: result.content
                        }]
                      };
                      
                      // Update the messages with the new assistant response
                      agent.setMessages([...agent.messages, assistantMessage]);
                    } else {
                      console.log("Assistant response already exists in message history");
                    }
                  } else {
                    console.warn("Inference result is missing id or content, cannot add to history", result);
                  }
                } catch (error) {
                  console.error("Error running shared inference:", error);
                }
              }}
            >
              Test Shared Inference
            </Button>

            {/* View System Prompt */}
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
                  console.log("ProjectData exists:", !!agent.state?.currentProject);
                  console.log("TeamData exists:", !!agent.state?.currentTeam);

                  // Only set context if it's missing from the agent state
                  if ((!agent.state?.currentIssue || !agent.state?.currentProject || !agent.state?.currentTeam) &&
                    formattedIssue && formattedProject && formattedTeam) {
                    console.log("Context missing in agent state, setting before fetching prompt");

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
                      console.log("Context data sent successfully before fetching prompt");

                      // Wait a short time for the agent to process the context
                      setTimeout(() => {
                        fetchSystemPrompt();
                      }, 300);
                    } catch (err) {
                      console.error("Failed to send context data:", err);
                      fetchSystemPrompt();
                    }
                  } else {
                    console.log("Context data already exists in agent state, fetching prompt directly");
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
                <Button
                  variant="outline"
                  size="sm"
                  className="w-full text-xs"
                >
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

            {/* Send status update */}
            <Button
              variant="outline"
              size="sm"
              className="w-full text-xs"
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

            {/* Send command */}
            <Button
              variant="outline"
              size="sm"
              className="w-full text-xs"
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
          </>
        )}
      </CardFooter>
    </Card>
  );
}
