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
          console.log("State before generating system prompt:", JSON.stringify({
            hasIssue: !!this.state.currentIssue,
            hasProject: !!this.state.currentProject,
            hasTeam: !!this.state.currentTeam
          }));
          
          // Get the system prompt
          const systemPrompt = this.getSystemPrompt();
          
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
          console.log("Received context data from client");
          console.log("Issue data:", parsedMessage.issue ? `ID: ${parsedMessage.issue.id}` : 'None');
          console.log("Project data:", parsedMessage.project ? `ID: ${parsedMessage.project.id}` : 'None');
          console.log("Team data:", parsedMessage.team ? `ID: ${parsedMessage.team.id}` : 'None');
          
          try {
            // Update the agent's state with the new context
            await this.setState({
              ...this.state,
              currentIssue: parsedMessage.issue,
              currentProject: parsedMessage.project,
              currentTeam: parsedMessage.team
            });
            
            console.log("Context set successfully");
            console.log("State after context update:", JSON.stringify({
              hasIssue: !!this.state.currentIssue,
              hasProject: !!this.state.currentProject,
              hasTeam: !!this.state.currentTeam
            }));
          } catch (error) {
            console.error("Error setting context:", error);
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
          // Implement command handling logic as needed
          break;
          
        case "shared_infer":
          // Handle inference request
          console.log("Inference request received");
          try {
            const inferProps = parsedMessage.params;
            const result = await this.sharedInfer(inferProps);
            
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
    console.log("SOLVER AGENT: getSystemPrompt called - debug state", JSON.stringify({
      hasIssue: !!this.state.currentIssue,
      hasProject: !!this.state.currentProject,
      hasTeam: !!this.state.currentTeam,
      projectDetails: this.state.currentProject ? {
        id: this.state.currentProject.id,
        name: this.state.currentProject.name,
        color: this.state.currentProject.color
      } : null,
      teamDetails: this.state.currentTeam ? {
        id: this.state.currentTeam.id,
        name: this.state.currentTeam.name,
        key: this.state.currentTeam.key
      } : null
    }));
    
    const systemPrompt = getSolverSystemPrompt({ 
      state: this.state,
      temperature: 0.7
    });
    
    console.log("SOLVER AGENT: Generated system prompt begins with:", systemPrompt.substring(0, 200) + "...");
    
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
