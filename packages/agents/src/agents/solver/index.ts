import { type Connection, type WSMessage } from "agents";
import { AsyncLocalStorage } from "node:async_hooks";
import type { SolverState } from "./types";
import { OpenAgent } from "../../common/open-agent";
import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";

export const solverContext = new AsyncLocalStorage<Solver>();

/**
 * Solver Agent that handles issue resolution in OpenAgents Projects
 */
import { getSolverSystemPrompt } from "./prompts";

export class Solver extends OpenAgent<SolverState> {
  // Initialize state by extending the base state with solver-specific properties
  initialState: SolverState = {
    ...this.getBaseInitialState(), // Type-safe access to base state
    // Add solver-specific initial state properties here
  };
  
  // Debug counter for state mutations
  private stateUpdateCounter = 0;
  
  /**
   * Logs the current state of the agent with a custom tag for debugging
   */
  private logState(tag: string) {
    console.debug(`State [${tag}]:`, {
      hasIssue: !!this.state.currentIssue,
      hasProject: !!this.state.currentProject,
      hasTeam: !!this.state.currentTeam,
      issueId: this.state.currentIssue?.id,
    });
  }
  
  /**
   * Overridden setState to better handle the complex state we need to store
   * and ensure proper serialization of non-primitive objects
   */
  override async setState(partialState: Partial<SolverState>) {
    // Increment counter for tracking updates
    this.stateUpdateCounter++;
    
    try {
      // Create a complete state object that includes all required properties
      const fullState: SolverState = {
        ...this.state
      };
      
      // Apply updates from partial state with deep cloning
      Object.keys(partialState).forEach(key => {
        const value = partialState[key as keyof SolverState];
        if (value !== undefined) {
          // Deep clone objects to avoid reference issues
          (fullState as any)[key] = typeof value === 'object' && value !== null
            ? JSON.parse(JSON.stringify(value))
            : value;
        }
      });
      
      // Ensure messages array always exists (required by type)
      if (!fullState.messages) {
        fullState.messages = [];
      }
      
      // Call parent setState with our complete state
      await super.setState(fullState);
    } catch (error) {
      console.error(`Error updating state:`, error);
    }
  }

  /**
   * Handles incoming WebSocket messages
   */
  async onMessage(connection: Connection, message: WSMessage) {
    try {
      const parsedMessage = JSON.parse(message as string);

      // Create a safe copy for logging that redacts sensitive information
      const safeMessageForLogging = { ...parsedMessage };
      if (safeMessageForLogging.githubToken) {
        safeMessageForLogging.githubToken = "[REDACTED]";
      }

      console.log("ON MESSAGE RECEIVED:", safeMessageForLogging);
      console.log("CURRENT STATE before handling message:", JSON.stringify({
        hasIssue: !!this.state.currentIssue,
        hasProject: !!this.state.currentProject,
        hasTeam: !!this.state.currentTeam
      }));
      
      // Handle specific message types
      switch (parsedMessage.type) {
        case "get_system_prompt":
          // Handle system prompt request
          const requestId = parsedMessage.requestId;
          console.log(`Handling system prompt request with ID ${requestId}`);
          console.log("STATE CHECK in get_system_prompt:", JSON.stringify({
            hasIssue: !!this.state.currentIssue,
            hasProject: !!this.state.currentProject,
            hasTeam: !!this.state.currentTeam,
            issueDetails: this.state.currentIssue ? {
              id: this.state.currentIssue.id,
              title: this.state.currentIssue.title,
              number: this.state.currentIssue.number,
              source: this.state.currentIssue.source
            } : null
          }));
          
          // If context is missing but we have it in message history, try to recover it
          if (!this.state.currentIssue) {
            console.log("CONTEXT RECOVERY ATTEMPT: Looking for context in message history...");
            
            // Look for context messages in history
            const contextMessages = this.state.messages.filter(msg => 
              msg.content && typeof msg.content === 'string' && 
              (msg.content.includes("issue") && msg.content.includes("context")));
            
            console.log(`Found ${contextMessages.length} potential context messages in history`);
            
            if (contextMessages.length > 0) {
              console.log("First context message:", contextMessages[0].content.substring(0, 100) + "...");
            }
          }
          
          // Get the system prompt
          const systemPrompt = this.getSystemPrompt();
          
          // Analyze the system prompt
          console.log("SYSTEM PROMPT ANALYSIS:", {
            length: systemPrompt.length,
            hasIssueContext: systemPrompt.includes("CURRENT ISSUE"),
            hasProjectContext: systemPrompt.includes("PROJECT CONTEXT"),
            hasTeamContext: systemPrompt.includes("TEAM CONTEXT"),
            firstFewWords: systemPrompt.substring(0, 50) + "..."
          });
          
          // Send the system prompt back to the client
          connection.send(JSON.stringify({
            type: "prompt_response",
            requestId: requestId,
            prompt: systemPrompt,
            timestamp: new Date().toISOString()
          }));
          console.log(`System prompt sent back for request ${requestId}`);
          break;
          
        case "set_context":
          // Handle context setting message with issue, project and team data
          console.log("SET_CONTEXT: Received context data from client");
          this.logState("Before set_context");
          
          // Log the received context data
          console.log("RECEIVED CONTEXT DATA:", JSON.stringify({
            issue: parsedMessage.issue ? {
              id: parsedMessage.issue.id,
              title: parsedMessage.issue.title,
              number: parsedMessage.issue.number,
              source: parsedMessage.issue.source
            } : null,
            project: parsedMessage.project ? {
              id: parsedMessage.project.id,
              name: parsedMessage.project.name
            } : null,
            team: parsedMessage.team ? {
              id: parsedMessage.team.id,
              name: parsedMessage.team.name
            } : null
          }));
          
          try {
            // Basic validation
            if (parsedMessage.issue && (!parsedMessage.issue.id || !parsedMessage.issue.title)) {
              console.error("SET_CONTEXT: Validation error - missing required fields");
              connection.send(JSON.stringify({
                type: "context_error",
                error: "Issue data is missing required fields",
                timestamp: new Date().toISOString()
              }));
              break;
            } 
            
            // Ensure source is set to "openagents" if not specified
            if (parsedMessage.issue && !parsedMessage.issue.source) {
              console.log("SET_CONTEXT: Adding default source 'openagents' to issue");
              parsedMessage.issue.source = "openagents";
            }
            
            // Update state with minimal set of properties - avoid using ...this.state to prevent issues
            await this.setState({
              currentIssue: parsedMessage.issue,
              currentProject: parsedMessage.project,
              currentTeam: parsedMessage.team
            });
            
            console.log("SET_CONTEXT: Context updated successfully");
            this.logState("After set_context");
            
            // Generate a test system prompt to confirm context is included
            const testPrompt = this.getSystemPrompt();
            console.log("SET_CONTEXT: System prompt validation:", {
              hasIssue: testPrompt.includes("CURRENT ISSUE"),
              hasProject: testPrompt.includes("PROJECT CONTEXT"),
              hasTeam: testPrompt.includes("TEAM CONTEXT"),
              includesTitle: testPrompt.includes(parsedMessage.issue.title)
            });
            
            // Inform client of success
            connection.send(JSON.stringify({
              type: "context_set",
              success: true,
              timestamp: new Date().toISOString()
            }));
          } catch (error) {
            console.error("SET_CONTEXT: Error setting context:", error);
            
            // Report error to client
            connection.send(JSON.stringify({
              type: "context_error",
              error: error instanceof Error ? error.message : String(error),
              timestamp: new Date().toISOString()
            }));
          }
          break;
          
        case "observation":
          // Handle observation message
          console.log("Adding observation from client:", parsedMessage.content);
          this.addAgentObservation(parsedMessage.content);
          break;
          
        case "status_update":
          // Handle status update message
          console.log("Status update received:", parsedMessage.content);
          // Implement status update handling as needed
          break;
          
        case "command":
          // Handle command message
          console.log(`Command received: ${parsedMessage.command}`, parsedMessage.params);
          
          // Command handler based on command type
          switch (parsedMessage.command) {
            case "verify_token":
              // Verify GitHub token exists and report status
              const hasToken = !!this.state.githubToken && this.state.githubToken.length > 0;
              console.log("TOKEN VERIFICATION:", {
                hasToken: hasToken,
                tokenLength: this.state.githubToken ? this.state.githubToken.length : 0,
                hasContext: !!this.state.currentIssue && !!this.state.currentProject
              });
              
              // Send response back to client
              connection.send(JSON.stringify({
                type: "command_response",
                command: "verify_token",
                success: hasToken,
                details: {
                  hasToken: hasToken,
                  tokenLength: this.state.githubToken ? this.state.githubToken.length : 0,
                  contextStatus: {
                    hasIssue: !!this.state.currentIssue,
                    hasProject: !!this.state.currentProject
                  }
                },
                timestamp: new Date().toISOString()
              }));
              
              // If token is missing, also log an observation
              if (!hasToken) {
                this.addAgentObservation("Token verification failed: GitHub token is missing or empty.");
              }
              break;
              
            default:
              console.log(`Unknown command: ${parsedMessage.command}`);
              connection.send(JSON.stringify({
                type: "command_error",
                command: parsedMessage.command,
                error: "Unknown command",
                timestamp: new Date().toISOString()
              }));
              break;
          }
          break;
          
        case "set_github_token":
          // New message type specifically for setting GitHub token
          console.log("Setting GitHub token via dedicated message");
          
          try {
            const token = parsedMessage.token;
            if (token && typeof token === 'string') {
              console.log(`Setting GitHub token from message (length: ${token.length})`);
              const result = this.setGithubToken(token);
              console.log("✓ GitHub token set result:", result);
              
              // Send success response back to client
              connection.send(JSON.stringify({
                type: "token_response",
                success: true,
                message: "GitHub token set successfully",
                timestamp: new Date().toISOString()
              }));
            } else {
              console.error("No valid token provided in set_github_token message");
              connection.send(JSON.stringify({
                type: "token_response",
                success: false,
                message: "No valid token provided",
                timestamp: new Date().toISOString()
              }));
            }
          } catch (tokenError) {
            console.error("✗ Failed to set GitHub token from message:", tokenError);
            connection.send(JSON.stringify({
              type: "token_response",
              success: false,
              message: "Failed to set token: " + String(tokenError),
              timestamp: new Date().toISOString()
            }));
          }
          break;
          
        case "shared_infer":
          // Handle inference request
          console.log("Inference request received");
          console.log("STATE CHECK before inference:", JSON.stringify({
            hasIssue: !!this.state.currentIssue,
            hasProject: !!this.state.currentProject,
            hasTeam: !!this.state.currentTeam,
            issueDetails: this.state.currentIssue ? {
              id: this.state.currentIssue.id,
              title: this.state.currentIssue.title,
              number: this.state.currentIssue.number,
              source: this.state.currentIssue.source
            } : null
          }));
          
          try {
            const inferProps = parsedMessage.params;
            
            // CRITICAL FIX: Check for GitHub token in params - but don't call setGithubToken directly
            // Instead use it directly for this operation but recommend client use set_github_token message
            if (inferProps.githubToken && typeof inferProps.githubToken === 'string') {
              console.log(`Using GitHub token from inference params (length: ${inferProps.githubToken.length})`);
              
              // Update the state directly without calling setGithubToken method
              this.updateState({
                githubToken: inferProps.githubToken
              } as Partial<SolverState>);
              
              console.log("✓ GitHub token applied to state");
              
              // Remove the token from the params to avoid leaking it in logs
              delete inferProps.githubToken;
              
              // Add a note to encourage using the proper message type
              this.addAgentObservation("Note: For more reliable token setting, use 'set_github_token' message type instead of parameter passing");
            } else {
              console.warn("No GitHub token found in inference params");
            }
            
            // Check for context data in message
            if (parsedMessage.context) {
              const missingContext = !this.state.currentIssue || !this.state.currentProject || !this.state.currentTeam;
              const differentIssue = this.state.currentIssue && 
                                    this.state.currentIssue.id !== parsedMessage.context.issue.id;
              
              if (missingContext || differentIssue) {
                console.log("Context needs to be updated from inference message");
                this.logState("Before context update");
                
                try {
                  // Update the state with context from the message
                  await this.setState({
                    currentIssue: parsedMessage.context.issue,
                    currentProject: parsedMessage.context.project,
                    currentTeam: parsedMessage.context.team
                  });
                  
                  console.log("Context updated from inference message");
                  this.logState("After context update");
                } catch (contextError) {
                  console.error("Error updating context from message:", contextError);
                }
              } else {
                console.log("Context already matches message context, no update needed");
              }
            } else {
              console.log("No context provided in inference message");
            }
            
            // Always check whether we have the context we need after potential updates
            this.logState("Before system prompt generation");
            
            // Use the state information for the system prompt
            if (!inferProps.system || !inferProps.system.includes("CURRENT ISSUE")) {
              console.log("Generating system prompt for inference");
              try {
                // Generate a system prompt with the current state
                const systemPrompt = this.getSystemPrompt();
                
                console.log("System prompt analysis:", {
                  length: systemPrompt.length,
                  hasIssue: systemPrompt.includes("CURRENT ISSUE"),
                  hasProject: systemPrompt.includes("PROJECT CONTEXT"),
                  hasTeam: systemPrompt.includes("TEAM CONTEXT"),
                  issueTitle: this.state.currentIssue?.title || "none"
                });
                
                // Update the system prompt in the inference properties
                inferProps.system = systemPrompt;
              } catch (promptError) {
                console.error("Error generating system prompt:", promptError);
              }
            } else {
              console.log("Using provided system prompt");
            }
            
            // Run the inference
            const result = await this.sharedInfer(inferProps);
            console.log("INFERENCE RESULT:", JSON.stringify({
              id: result.id,
              content: result.content?.substring(0, 50) + "..." || "No content",
              role: result.role,
              hasValidResponse: !!result.id && !!result.content
            }));
            
            // Add the response to our messages array if it's not already there
            const existingMessageIndex = this.state.messages.findIndex(msg => msg.id === result.id);
            if (existingMessageIndex === -1 && result.id && result.content) {
              console.log("Adding assistant response to agent state");
              
              // Format the assistant message with proper typing
              const assistantMessage: any = {
                id: result.id,
                role: 'assistant' as const, // Use const assertion for literal type
                content: result.content,
                parts: [{
                  type: 'text' as const, // Use const assertion for literal type
                  text: result.content
                }]
              };
              
              // Update the messages array in state
              await this.setState({
                messages: [...this.state.messages, assistantMessage]
              });
              
              // Verify the message was added
              console.log("Messages array updated, now contains", this.state.messages.length, "messages");
            } else if (existingMessageIndex !== -1) {
              console.log("Response already exists in messages array at index", existingMessageIndex);
            }
            
            // Send the inference result back to the client
            connection.send(JSON.stringify({
              type: "infer_response",
              requestId: parsedMessage.requestId,
              result: result,
              timestamp: new Date().toISOString()
            }));
            console.log(`Inference result sent back for request ${parsedMessage.requestId}`);
          } catch (error) {
            console.error("Error performing inference:", error);
            connection.send(JSON.stringify({
              type: "infer_response",
              requestId: parsedMessage.requestId,
              error: error instanceof Error ? error.message : String(error),
              timestamp: new Date().toISOString()
            }));
          }
          break;
          
        default:
          console.log("Unhandled message type:", parsedMessage.type);
      }
    } catch (error) {
      console.error("Error parsing message:", error);
    }
  }
  
  /**
   * Gets the system prompt for the Solver agent
   * Overrides the base implementation to use the Solver-specific system prompt
   */
  getSystemPrompt() {
    this.logState("getSystemPrompt called");
    
    // Create a snapshot of the current state to use for system prompt generation
    // This avoids any potential race conditions or state changes during prompt generation
    const stateSnapshot: SolverState = {
      ...this.state,
      currentIssue: this.state.currentIssue ? { ...this.state.currentIssue } : undefined,
      currentProject: this.state.currentProject ? { ...this.state.currentProject } : undefined,
      currentTeam: this.state.currentTeam ? { ...this.state.currentTeam } : undefined
    };
    
    // Log state snapshot for debugging
    console.log("SYSTEM PROMPT: State snapshot:", JSON.stringify({
      hasIssue: !!stateSnapshot.currentIssue,
      hasProject: !!stateSnapshot.currentProject,
      hasTeam: !!stateSnapshot.currentTeam,
      issueDetails: stateSnapshot.currentIssue ? {
        id: stateSnapshot.currentIssue.id,
        title: stateSnapshot.currentIssue.title,
        source: stateSnapshot.currentIssue.source
      } : null
    }));
    
    // Generate the system prompt using the state snapshot
    const systemPrompt = getSolverSystemPrompt({ 
      state: stateSnapshot,
      temperature: 0.7
    });
    
    // Log first part of generated prompt for debugging
    console.log("SYSTEM PROMPT: Generated prompt begins with:", systemPrompt.substring(0, 200) + "...");
    
    // Log key sections present in the prompt
    console.log("SYSTEM PROMPT: Key sections check:", {
      hasIssueSection: systemPrompt.includes("CURRENT ISSUE"),
      hasProjectSection: systemPrompt.includes("PROJECT CONTEXT"),
      hasTeamSection: systemPrompt.includes("TEAM CONTEXT"),
      includesIssueTitle: stateSnapshot.currentIssue ? 
        systemPrompt.includes(stateSnapshot.currentIssue.title) : false
    });
    
    return systemPrompt;
  }
  
  /**
   * Sets the current issue with project and team context
   * @param issue The current issue being worked on
   * @param project Optional project context
   * @param team Optional team context
   * @returns Promise resolving to true if successful
   */
  async setCurrentIssue(issue: BaseIssue, project?: BaseProject, team?: BaseTeam) {
    console.log("SOLVER AGENT: Setting current issue with projects and teams");
    console.log("Issue ID:", issue.id);
    console.log("Issue title:", issue.title);
    console.log("Project data:", project ? JSON.stringify(project) : 'undefined');
    console.log("Team data:", team ? JSON.stringify(team) : 'undefined');
    
    try {
      // Log current state before update
      console.log("Current state before update:", JSON.stringify({
        hasIssue: !!this.state.currentIssue,
        hasProject: !!this.state.currentProject,
        hasTeam: !!this.state.currentTeam
      }));
      
      await this.setState({
        ...this.state,
        currentIssue: issue,
        currentProject: project,
        currentTeam: team
      });
      
      // Log state after update to verify data was stored
      console.log("State after update:", JSON.stringify({
        hasIssue: !!this.state.currentIssue,
        hasProject: !!this.state.currentProject,
        hasTeam: !!this.state.currentTeam
      }));
      
      return true;
    } catch (error) {
      console.error("Error setting current issue:", error);
      console.error("Error details:", JSON.stringify(error));
      throw error;
    }
  }
}
