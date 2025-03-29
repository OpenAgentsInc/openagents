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
    try {
      // Check if this is a WebSocket upgrade request
      const upgradeHeader = request.headers.get('Upgrade');
      if (upgradeHeader === 'websocket') {
        console.log(`📶 WebSocket upgrade request received: ${request.url}`);
      }
      
      // CRITICAL ERROR: Check if CoderAgent binding exists
      if (!env.CoderAgent) {
        console.error("🚨 CRITICAL ERROR: CoderAgent Durable Object binding is missing in the worker environment!");
        
        // For WebSocket upgrade requests, don't return a 500 error as it breaks the WebSocket handshake
        if (upgradeHeader === 'websocket') {
          // Use 101 status code for WebSocket upgrade requests to properly handle the handshake
          return new Response("Server configuration error: CoderAgent Durable Object binding is missing.", { 
            status: 101, // Switching Protocols - needed for WebSocket
            headers: { 
              'Content-Type': 'text/plain',
              'Connection': 'upgrade',
              'Upgrade': 'websocket'
            }
          });
        } else {
          // For regular requests, 500 is appropriate
          return new Response("Server configuration error: CoderAgent Durable Object binding is missing.", { 
            status: 500,
            headers: { 'Content-Type': 'text/plain' }
          });
        }
      }
      
      // Check for required API keys
      if (!env.OPENROUTER_API_KEY && request.url.includes('/agents/coderagent')) {
        console.error("🚨 OPENROUTER_API_KEY is not set in environment variables!");
        // Still proceed with the request - we'll handle the missing API key in the agent
      }
      
      // Make sure the request URL includes necessary path components for routing
      const url = new URL(request.url);
      if (url.pathname === '/' || url.pathname === '') {
        // If someone hits the root path, redirect to a proper agent path
        return new Response("No agent specified. Use /agents/coderagent/default to connect to the CoderAgent.", { 
          status: 400,
          headers: { 'Content-Type': 'text/plain' }
        });
      }
      
      // Extract path components for more detailed debugging
      const pathParts = url.pathname.split('/').filter(Boolean);
      console.log(`🛣️ Request path: ${url.pathname}, parts:`, pathParts);
      
      // Custom handling for the known correct path pattern
      if (pathParts[0] === 'agents' && pathParts[1] === 'coderagent') {
        const instanceName = pathParts[2] || 'default';
        console.log(`🎯 Direct routing to CoderAgent with instance name: ${instanceName}`);
        
        try {
          // Manually route to the CoderAgent Durable Object
          const id = env.CoderAgent.idFromName(instanceName);
          const agent = env.CoderAgent.get(id);
          
          // Forward the request to the Durable Object
          return agent.fetch(request);
        } catch (err) {
          console.error(`🚨 Error routing to CoderAgent/${instanceName}:`, err);
          // For WebSocket connections, send WebSocket compatible response
          if (upgradeHeader === 'websocket') {
            return new Response(`Agent error: ${err instanceof Error ? err.message : String(err)}`, {
              status: 101, // Switching Protocols - needed for WebSocket
              headers: { 
                'Content-Type': 'text/plain',
                'Connection': 'upgrade',
                'Upgrade': 'websocket'
              }
            });
          } else {
            return new Response(`Agent error: ${err instanceof Error ? err.message : String(err)}`, {
              status: 500,
              headers: { 'Content-Type': 'text/plain' }
            });
          }
        }
      }
      
      // Use standard routing for other paths
      console.log(`🔄 Using standard routeAgentRequest for path: ${url.pathname}`);
      const response = await routeAgentRequest(request, env);
      
      if (response) {
        return response;
      } else {
        // If no agent route matched
        return new Response(`Agent not found at path: ${url.pathname}. Try /agents/coderagent/default`, { 
          status: 404,
          headers: { 'Content-Type': 'text/plain' }
        });
      }
    } catch (error) {
      console.error("🚨 Unhandled error in server handler:", error);
      
      // For WebSocket connections, use WebSocket compatible response
      const upgradeHeader = request.headers.get('Upgrade');
      if (upgradeHeader === 'websocket') {
        return new Response(`Server error: ${error instanceof Error ? error.message : String(error)}`, {
          status: 101, // Switching Protocols - needed for WebSocket
          headers: { 
            'Content-Type': 'text/plain',
            'Connection': 'upgrade',
            'Upgrade': 'websocket'
          }
        });
      }
      
      return new Response(`Server error: ${error instanceof Error ? error.message : String(error)}`, {
        status: 500,
        headers: { 'Content-Type': 'text/plain' }
      });
    }
  },
};