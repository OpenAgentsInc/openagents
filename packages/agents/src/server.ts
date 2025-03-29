import { routeAgentRequest } from "agents";
import { AsyncLocalStorage } from "node:async_hooks";

// Import our CoderAgent
import { CoderAgent } from "./coder-agent";

// We use ALS to expose the agent context to the tools
export const agentContext = new AsyncLocalStorage<CoderAgent>();

// Export the CoderAgent class for the Durable Object
export { CoderAgent };

/**
 * Debugging function to analyze the agent environment
 */
/**
 * Interface for the environment object with Durable Object bindings
 */
interface DurableObjectEnv extends Env {
  CoderAgent?: {
    idFromName(name: string): any;
    get(id: any): any;
  };
}

/**
 * Analyze the agent environment to help diagnose binding issues
 */
function analyzeAgentEnvironment(env: DurableObjectEnv) {
  console.log(`📊 Analyzing Agent Environment:`);
  
  // Check for CoderAgent Durable Object binding
  console.log(`🔍 CoderAgent binding exists: ${!!env.CoderAgent}`);
  
  // List all available bindings in the environment
  const envKeys = Object.keys(env);
  console.log(`📚 Available environment bindings: ${envKeys.join(', ')}`);
  
  // Check if the CoderAgent binding has expected methods
  if (env.CoderAgent) {
    const methods = Object.getOwnPropertyNames(Object.getPrototypeOf(env.CoderAgent));
    console.log(`🔧 CoderAgent methods: ${methods.join(', ')}`);
    
    // Specifically check for idFromName method
    console.log(`🔑 idFromName method exists: ${methods.includes('idFromName')}`);
  }
  
  // Check for OPENROUTER_API_KEY
  console.log(`🔑 OPENROUTER_API_KEY is set: ${!!env.OPENROUTER_API_KEY}`);
}

// Create and export the default fetch handler as an ES module
export default {
  async fetch(request: Request, env: DurableObjectEnv, ctx: ExecutionContext) {
    try {
      // Check if this is a WebSocket upgrade request
      const upgradeHeader = request.headers.get('Upgrade');
      if (upgradeHeader === 'websocket') {
        console.log(`📶 WebSocket upgrade request received: ${request.url}`);
      }
      
      // Run environment analysis to help diagnose the issue
      analyzeAgentEnvironment(env);
      
      // CRITICAL ERROR: Check if CoderAgent binding exists
      if (!env.CoderAgent) {
        console.error("🚨 CRITICAL ERROR: CoderAgent Durable Object binding is missing in the worker environment!");
        return new Response("Server configuration error: CoderAgent Durable Object binding is missing.", { 
          status: 500,
          headers: { 'Content-Type': 'text/plain' }
        });
      }
      
      // Check for required API keys
      if (!env.OPENROUTER_API_KEY && request.url.includes('/agents/coderagent')) {
        console.warn("⚠️ OPENROUTER_API_KEY is not set in environment variables!");
        // Still proceed with the request - we'll handle the missing API key in the agent
      }
      
      // Extract the URL and path for logging
      const url = new URL(request.url);
      console.log(`📥 Handling request: ${request.method} ${url.pathname}`);
      
      try {
        // Customize the agent routing based on the Cloudflare agents SDK docs
        console.log(`🚀 Routing request using routeAgentRequest...`);
        
        // Use the routeAgentRequest function to properly handle all agent requests
        const response = await routeAgentRequest(request, env, { cors: true });
        
        // If the request was handled by an agent, return the response
        if (response) {
          console.log(`✅ Request handled by routeAgentRequest - Status: ${response.status}`);
          return response;
        }
      } catch (routeError) {
        // Log detailed information about the routing error
        console.error(`⚠️ routeAgentRequest error:`, routeError);
        
        if (routeError instanceof Error) {
          console.error(`⚠️ Error message: ${routeError.message}`);
          console.error(`⚠️ Error stack: ${routeError.stack}`);
        }
      }
      
      // If no agent route matched, handle the request manually
      const pathParts = url.pathname.split('/').filter(Boolean);
      console.log(`🔍 No agent handled the request. Path parts:`, pathParts);
      
      // Check if this looks like an agent request that wasn't handled
      if (pathParts[0] === 'agents' && pathParts.length >= 2) {
        const agentName = pathParts[1];
        const instanceName = pathParts[2] || 'default';
        
        console.log(`🔄 Found unhandled agent request for ${agentName}/${instanceName}`);
        
        // Explicit handling for CoderAgent - try to create a Durable Object stub
        if (agentName === 'coderagent' && upgradeHeader === 'websocket') {
          // Log diagnostic information
          console.log(`🔎 Diagnostics for CoderAgent request:`);
          console.log(`  - Pattern: ${url.pathname}`);
          console.log(`  - WebSocket request: ${upgradeHeader === 'websocket'}`);
          
          // Try loading the class directly to check if it exists
          try {
            const stub = console.log("🧪 Attempting to check CoderAgent class...");
            if (typeof CoderAgent === 'function') {
              console.log("✅ CoderAgent class exists as a function");
              console.log(`   - Class name: ${CoderAgent.name}`);
              console.log(`   - Prototype methods: ${Object.getOwnPropertyNames(CoderAgent.prototype).join(', ')}`);
            } else {
              console.log("❌ CoderAgent is not a valid class");
            }
          } catch (classError) {
            console.error("❌ Error checking CoderAgent class:", classError);
          }
        }
        
        // Extract hostname and construct server endpoint format for error message
        const hostname = url.hostname;
        
        // This is the recommended format for connecting to an agent
        return new Response(
          `Agent service is experiencing configuration issues. The agent ${agentName}/${instanceName} could not be found or accessed.\n\n` +
          `Technical details: The server tried to route your request to ${url.pathname} but neither routeAgentRequest nor direct access methods succeeded.\n\n` +
          `This is likely due to a server-side configuration issue with Durable Object bindings.`,
          {
            status: 500,
            headers: { 'Content-Type': 'text/plain' }
          }
        );
      }
      
      // Default response for unhandled requests
      return new Response(`No agent found to handle this request.\n` +
                         `The server is experiencing configuration issues with the Agents SDK.`, {
        status: 500,
        headers: { 'Content-Type': 'text/plain' }
      });
      
    } catch (error) {
      console.error("🚨 Unhandled error in server handler:", error);
      
      // Provide more detailed error information
      const errorDetails = error instanceof Error 
        ? { message: error.message, stack: error.stack, name: error.name }
        : { message: String(error) };
      
      console.error("📊 Error details:", JSON.stringify(errorDetails, null, 2));
      
      return new Response(`Server error: ${error instanceof Error ? error.message : String(error)}\n\n` +
                          `This is likely due to a configuration issue with the Cloudflare Agents SDK or Durable Objects.`, {
        status: 500,
        headers: { 'Content-Type': 'text/plain' }
      });
    }
  },
};