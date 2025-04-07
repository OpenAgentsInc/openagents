import { AsyncLocalStorage } from "node:async_hooks";
import { routeAgentRequest } from "agents";

// Import our CoderAgent and the agent context
import { CoderAgent, agentContext } from "./coder-agent";

// Export the CoderAgent class for the Durable Object
// IMPORTANT: This export name MUST match the class_name in wrangler.jsonc
export { CoderAgent, agentContext };

/**
 * Worker entry point that routes incoming requests to the appropriate handler
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    if (!env.AI) {
      console.error(
        "AI binding is not available. Make sure you have configured the AI binding in your wrangler.jsonc file."
      );
      return new Response("AI binding is not available", { status: 500 });
    }
    
    return (
      // Route the request to our agent or return 404 if not found
      (await routeAgentRequest(request, env)) ||
      new Response("Not found", { status: 404 })
    );
  },
};