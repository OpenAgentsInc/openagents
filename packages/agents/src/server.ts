import { AsyncLocalStorage } from "node:async_hooks";
import { routeAgentRequest } from "agents";

// Import our CoderAgent
import { CoderAgent } from "./coder-agent";

// We use ALS to expose the agent context to the tools
export const agentContext = new AsyncLocalStorage<CoderAgent>();

// Export the CoderAgent class for the Durable Object
export { CoderAgent };

/**
 * Worker entry point that routes incoming requests to the appropriate handler
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    if (!env.OPENROUTER_API_KEY) {
      console.error(
        "OPENROUTER_API_KEY is not set, don't forget to set it using 'wrangler secret put OPENROUTER_API_KEY'"
      );
      return new Response("OPENROUTER_API_KEY is not set", { status: 500 });
    }
    
    return (
      // Route the request to our agent or return 404 if not found
      (await routeAgentRequest(request, env)) ||
      new Response("Not found", { status: 404 })
    );
  },
};