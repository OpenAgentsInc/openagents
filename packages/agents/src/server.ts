import { routeAgentRequest } from "agents";
import type { Env } from "./types";
import type { ExecutionContext } from '@cloudflare/workers-types';
import { Coder } from './agents/coder';
import { Solver } from './agents/solver';

/**
 * Worker entry point that routes incoming requests to the appropriate agent
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Configure agent routing
    const agentConfig = {
      agents: {
        'coder': Coder, // Map path '/agent/coder' to Coder class
        'solver': Solver // Map path '/agent/solver' to Solver class
      },
      // Enable CORS for cross-origin requests
      cors: true
    };

    // Route the request to the appropriate agent or return 404 if not found
    return (
      (await routeAgentRequest(request, env, agentConfig)) ||
      new Response("Agent not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;