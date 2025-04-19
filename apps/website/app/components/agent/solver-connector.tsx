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
import { ScrollArea } from "../ui/scroll-area";
import type { TextUIPart, UIMessage } from "@ai-sdk/ui-utils";

interface SolverConnectorProps {
  issue: any;  // The issue object from the parent component
  agent: any;  // The agent instance passed from parent
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
  agent,  // Now receiving the agent instance from parent
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

  // Agent instance is now passed from parent
  // No need to create a new hook instance

  // Sync connection state from the agent hook
  useEffect(() => {
    // Use the agent's connection status directly
    console.log("Agent connection status:", agent.connectionStatus);
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
          // IMPORTANT: Don't call setGithubToken directly - use message passing
          agent.sendRawMessage({
            type: "set_github_token",
            token: githubToken
          }),
          timeoutPromise
        ]);
        console.log("✓ GitHub token set message sent successfully");
      } catch (error) {
        console.error("✗ Failed to send GitHub token message:", error);
        throw new Error(`Failed to send GitHub token: ${error instanceof Error ? error.message : "Unknown error"}`);
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

  // Log connection status only
  useEffect(() => {
    if (connectionState === 'connected') {
      console.log("SolverConnector: Connected to agent");
    }
  }, [connectionState]);

  // Create a message container that starts scrolled to the bottom
  const MessageContainer = ({ children }: { children: React.ReactNode }) => {
    const viewportRef = useRef<HTMLDivElement>(null);
    const isInitialRender = useRef(true);

    // Position scroll at bottom after DOM is updated
    useLayoutEffect(() => {
      const viewport = viewportRef.current?.querySelector('[data-slot="scroll-area-viewport"]');
      if (viewport instanceof HTMLDivElement) {
        // Set scroll to bottom on render
        viewport.scrollTop = viewport.scrollHeight;

        if (isInitialRender.current) {
          // If it's the first render, also schedule another scroll after images/content load
          setTimeout(() => {
            if (viewport instanceof HTMLDivElement) {
              viewport.scrollTop = viewport.scrollHeight;
            }
          }, 100);
          isInitialRender.current = false;
        }
      }
    }, [agent.messages.length]);

    // When new messages arrive, scroll to bottom if already near bottom
    useEffect(() => {
      if (!isInitialRender.current) {
        const viewport = viewportRef.current?.querySelector('[data-slot="scroll-area-viewport"]');
        if (viewport instanceof HTMLDivElement) {
          const isNearBottom =
            viewport.scrollHeight - viewport.clientHeight <=
            viewport.scrollTop + 100; // Within 100px of bottom

          if (isNearBottom) {
            // Immediately jump to bottom without animation
            viewport.scrollTop = viewport.scrollHeight;
          }
        }
      }
    }, [agent.messages]);

    return (
      <ScrollArea
        ref={viewportRef}
        className="flex-1 px-4 min-h-0"
        style={{ overscrollBehavior: 'contain' }}
      >
        {children}
      </ScrollArea>
    );
  };

  // No connection monitoring needed - handled by parent component
  // This component just shows the UI based on the agent's state

  return (
    <Card className={cn("h-full flex flex-col py-0 overflow-hidden", className)}>
      <CardContent className="flex-1 flex flex-col overflow-hidden p-0 pt-0">
        {connectionState === 'disconnected' && (
          <div className="flex flex-col items-center justify-center h-full overflow-auto">
            <Terminal className="h-8 w-8 mb-2 text-muted-foreground" />
            <p className="text-muted-foreground mb-2 text-center px-4 text-sm">
              Please connect the agent using the controls in the sidebar.
            </p>
          </div>
        )}

        {connectionState === 'connecting' && (
          <div className="flex flex-col items-center justify-center h-full overflow-auto">
            <Spinner className="h-8 w-8 mb-2" />
            <p className="text-muted-foreground text-center px-4 text-sm">
              Connecting to agent...
            </p>
          </div>
        )}

        {connectionState === 'error' && (
          <div className="flex flex-col items-center justify-center h-full overflow-auto">
            <AlertTriangle className="h-8 w-8 mb-2 text-destructive" />
            <p className="text-muted-foreground mb-2 text-center px-4 text-sm">
              {errorMessage || "Connection error. Check the sidebar for options."}
            </p>
          </div>
        )}

        {connectionState === 'connected' && (
          <div className="h-full flex flex-col">
            <div className="flex-1 flex flex-col overflow-hidden">
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
                    // Store the input value before clearing
                    const inputValue = input.value;

                    // Clear the input field immediately
                    input.value = '';

                    // Create a user message
                    const userMessage = {
                      id: generateId(),
                      role: 'user' as const,
                      content: inputValue,
                      parts: [{
                        type: 'text' as const,
                        text: inputValue
                      }]
                    };

                    // Add the user message to the agent
                    agent.setMessages([...agent.messages, userMessage]);

                    try {
                      // First ensure we have context
                      const contextMissing = !agent.state?.currentIssue || !agent.state?.currentProject || !agent.state?.currentTeam;

                      // Set context if needed
                      if (contextMissing) {
                        // Send context to ensure agent has issue information
                        const contextMessage = {
                          type: "set_context",
                          issue: formattedIssue,
                          project: formattedProject,
                          team: formattedTeam,
                          timestamp: new Date().toISOString()
                        };

                        agent.sendRawMessage(contextMessage);

                        // Wait a moment for context to be processed
                        await new Promise(resolve => setTimeout(resolve, 300));
                      }

                      // Get all messages for the inference
                      const allMessages = [...agent.messages, userMessage].map(message => ({
                        id: message.id,
                        role: message.role as UIMessage['role'],
                        content: message.content || '',
                        parts: [{
                          type: 'text' as const,
                          text: message.content || ''
                        }] as TextUIPart[]
                      }));

                      // Get system prompt to ensure context is included
                      let systemPrompt = null;
                      try {
                        // Use message passing instead of direct RPC call
                        const promptRequestId = "prompt_req_" + Date.now() + "_" + Math.random().toString(36).substring(2, 10);
                        
                        // Send the request via message
                        const promptResponse = await agent.sendRawMessage({
                          type: "get_system_prompt",
                          requestId: promptRequestId
                        });
                        
                        // Extract the prompt from the response
                        systemPrompt = promptResponse?.prompt;
                        
                        if (!systemPrompt) {
                          console.warn("No system prompt returned from agent");
                        }
                      } catch (promptError) {
                        console.error("Error fetching system prompt:", promptError);
                      }

                      // Run shared inference with full message history
                      // Send the inference request
                      const requestId = generateId();
                      
                      // CRITICAL FIX: Re-set the GitHub token to ensure it's available
                      // This adds redundancy to prevent token loss
                      if (githubToken) {
                        try {
                          console.log("Re-setting GitHub token before inference (length:", githubToken.length, ")");
                          // IMPORTANT: Don't call setGithubToken directly - use message passing
                          agent.sendRawMessage({
                            type: "set_github_token",
                            token: githubToken
                          });
                        } catch (tokenError) {
                          console.error("Error re-setting GitHub token:", tokenError);
                        }
                      } else {
                        console.warn("No GitHub token available to set before inference!");
                      }

                      // Send the request with context data for reliability
                      const response = await agent.sendRawMessage({
                        type: "shared_infer",
                        requestId: requestId,
                        params: {
                          model: "anthropic/claude-3.5-sonnet", // Using OpenRouter model
                          messages: allMessages,
                          system: systemPrompt,
                          temperature: 0.7,
                          max_tokens: 1000,
                          githubToken: githubToken, // Explicitly pass token as parameter
                        },
                        context: {
                          issue: formattedIssue,
                          project: formattedProject,
                          team: formattedTeam
                        },
                        timestamp: new Date().toISOString()
                      });

                      // For async responses, just return and let the WebSocket handler update UI
                      if (!response?.result || !response.result.id || !response.result.content) {
                        return;
                      }

                      const result = response.result;

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
                      console.error("Error with shared inference:", error);

                      // Fallback to standard handleSubmit if shared inference fails
                      console.log("Falling back to standard handleSubmit...");
                      agent.handleSubmit(inputValue)
                        .then(() => {
                          console.log("Message sent to agent via standard method");
                        })
                        .catch(fallbackError => {
                          console.error("Error with fallback message send:", fallbackError);
                        });
                    }

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
