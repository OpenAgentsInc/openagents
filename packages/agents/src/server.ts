import { routeAgentRequest } from "agents";
import type { Env } from "./types";
import type { ExecutionContext } from '@cloudflare/workers-types';

// Export the agents so they can be used as Durable Objects
export { Coder } from './agents/coder';
export { Solver } from './agents/solver';

/**
 * Worker entry point that routes incoming requests to the appropriate agent
 * Uses the official Cloudflare Agents SDK's routeAgentRequest helper
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    console.log(`[AGENT SERVER] Request path: ${new URL(request.url).pathname}`);

    // Route the request to our agent via the Agents SDK
    // This automatically handles routing to the correct agent (coder or solver)
    // and manages WebSocket connections properly
    return (
      (await routeAgentRequest(request, env, { cors: true })) ||
      new Response("Agent not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
