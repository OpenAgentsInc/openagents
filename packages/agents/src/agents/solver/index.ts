import { Agent, type Connection, type WSMessage } from "agents";
import type { UIMessage } from "ai";

type SolverState = {
  messages: UIMessage[];
}

export class Solver extends Agent<Env, SolverState> {
  /**
   * Handles incoming WebSocket messages
   */
  async onMessage(connection: Connection, message: WSMessage) {
    try {
      const parsedMessage = JSON.parse(message as string);
      console.log("ON MESSAGE RECEIVED:", parsedMessage);
    } catch (error) {
      console.error("Error parsing message:", error);
    }
  }
}
