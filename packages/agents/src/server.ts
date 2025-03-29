import { routeAgentRequest } from "agents";
import { AsyncLocalStorage } from "node:async_hooks";

// Import our CoderAgent
import { CoderAgent } from "./coder-agent";

// We use ALS to expose the agent context to the tools
export const agentContext = new AsyncLocalStorage<CoderAgent>();

// Export the CoderAgent class for the Durable Object
export { CoderAgent };

// Create and export the default fetch handler as an ES module
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Route the request to the appropriate agent handler
    return (await routeAgentRequest(request, env)) || 
           new Response("Agent not found", { status: 404 });
  },
};