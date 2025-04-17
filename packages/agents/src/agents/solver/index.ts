import { type Connection, type WSMessage } from "agents";
import { AsyncLocalStorage } from "node:async_hooks";
import type { SolverState } from "./types";
import { OpenAgent } from "../../common/types";

export const solverContext = new AsyncLocalStorage<Solver>();

/**
 * Solver Agent that handles issue resolution in OpenAgents Projects
 */
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

    } catch (error) {
      console.error("Error parsing message:", error);
    }
  }
}
