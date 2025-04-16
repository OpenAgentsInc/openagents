import { Agent } from "agents";
import type { Env } from "./types";
import type { ExecutionContext } from '@cloudflare/workers-types';
import { Coder } from './agents/coder';
import { Solver } from './agents/solver';

export { Coder } from './agents/coder';
export { Solver } from './agents/solver';

/**
 * Worker entry point that routes incoming requests to the appropriate agent
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    const url = new URL(request.url);
    const pathParts = url.pathname.split('/').filter(Boolean);
    
    // Check if the request is for an agent
    if (pathParts[0] === 'agent' && pathParts.length > 1) {
      const agentType = pathParts[1];
      
      // Map agent type to Durable Object
      if (agentType === 'coder') {
        // Generate a unique ID for this agent instance
        const agentId = url.searchParams.get('id') || 'default';
        const id = env.Coder.idFromName(agentId);
        const obj = env.Coder.get(id);
        
        // Forward the request to the Durable Object
        return obj.fetch(request);
      } 
      else if (agentType === 'solver') {
        // Generate a unique ID for this agent instance
        const agentId = url.searchParams.get('id') || 'default';
        const id = env.Solver.idFromName(agentId);
        const obj = env.Solver.get(id);
        
        // Forward the request to the Durable Object
        return obj.fetch(request);
      }
    }
    
    // Handle OPTIONS request for CORS
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        headers: {
          'Access-Control-Allow-Origin': '*',
          'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
          'Access-Control-Allow-Headers': 'Content-Type, X-GitHub-Token'
        }
      });
    }
    
    // If no matching agent or path, return 404
    return new Response("Agent not found", { status: 404 });
  },
} satisfies ExportedHandler<Env>;