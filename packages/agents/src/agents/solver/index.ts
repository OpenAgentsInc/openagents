import { type Connection, type WSMessage } from "agents";
import { AsyncLocalStorage } from "node:async_hooks";
import type { SolverState } from "./types";
import { OpenAgent } from "../../common/types";

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
      
      // Handle specific message types
      switch (parsedMessage.type) {
        case "get_system_prompt":
          // Handle system prompt request
          const requestId = parsedMessage.requestId;
          console.log(`Handling system prompt request with ID ${requestId}`);
          
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
    return getSolverSystemPrompt({ 
      state: this.state,
      temperature: 0.7
    });
  }
}
