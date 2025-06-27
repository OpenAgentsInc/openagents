import { DeploymentSession } from './durable-object';
import type { Env } from './types';
import { MockDeploymentSimulator } from './mock-deployment';

// Export Durable Object class
export { DeploymentSession };

// Main Worker handler
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);
    
    // CORS headers for WebSocket handshake
    const corsHeaders = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization, Upgrade',
    };

    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, { status: 204, headers: corsHeaders });
    }

    // Route: Health check
    if (url.pathname === '/health') {
      return new Response('OK', { 
        status: 200,
        headers: { ...corsHeaders, 'Content-Type': 'text/plain' }
      });
    }

    // Route: Test endpoint for mock deployments (development only)
    if (url.pathname === '/test/deploy' && env.ENVIRONMENT !== 'production') {
      return this.handleTestDeploy(request, env, corsHeaders);
    }

    // Route: Internal API for deployment updates
    if (url.pathname.startsWith('/internal/')) {
      return this.handleInternalAPI(request, url, env, corsHeaders);
    }

    // Route: WebSocket connections
    if (url.pathname === '/' || url.pathname === '/deployment-ws') {
      return this.handleWebSocket(request, env, corsHeaders);
    }

    return new Response('Not Found', { status: 404, headers: corsHeaders });
  },

  async handleWebSocket(request: Request, env: Env, corsHeaders: Record<string, string>): Promise<Response> {
    const upgradeHeader = request.headers.get('Upgrade');
    
    if (!upgradeHeader || upgradeHeader !== 'websocket') {
      return new Response('Expected WebSocket', { 
        status: 426,
        headers: corsHeaders 
      });
    }

    const url = new URL(request.url);
    const deploymentId = url.searchParams.get('deploymentId');
    
    if (!deploymentId) {
      return new Response('Missing deploymentId parameter', { 
        status: 400,
        headers: corsHeaders 
      });
    }

    try {
      // Get or create Durable Object instance for this deployment
      const id = env.DEPLOYMENT_SESSIONS.idFromName(deploymentId);
      const deploymentSession = env.DEPLOYMENT_SESSIONS.get(id);
      
      // Forward the WebSocket request to the Durable Object
      return deploymentSession.fetch(request);
    } catch (error) {
      console.error('Failed to handle WebSocket:', error);
      return new Response('Internal Server Error', { 
        status: 500,
        headers: corsHeaders 
      });
    }
  },

  async handleInternalAPI(
    request: Request, 
    url: URL, 
    env: Env, 
    corsHeaders: Record<string, string>
  ): Promise<Response> {
    // Extract deployment ID from path: /internal/deployments/{deploymentId}/update
    const pathParts = url.pathname.split('/');
    if (pathParts.length < 5 || pathParts[2] !== 'deployments' || pathParts[4] !== 'update') {
      return new Response('Invalid internal API path', { 
        status: 400,
        headers: corsHeaders 
      });
    }

    const deploymentId = pathParts[3];
    
    try {
      // Get Durable Object for this deployment
      const id = env.DEPLOYMENT_SESSIONS.idFromName(deploymentId);
      const deploymentSession = env.DEPLOYMENT_SESSIONS.get(id);
      
      // Forward the request to the Durable Object
      const doUrl = new URL(request.url);
      doUrl.pathname = '/internal/update';
      
      const doRequest = new Request(doUrl.toString(), {
        method: request.method,
        headers: request.headers,
        body: request.body
      });
      
      return deploymentSession.fetch(doRequest);
    } catch (error) {
      console.error('Failed to handle internal API:', error);
      return new Response('Internal Server Error', { 
        status: 500,
        headers: corsHeaders 
      });
    }
  },

  async handleTestDeploy(
    request: Request, 
    env: Env, 
    corsHeaders: Record<string, string>
  ): Promise<Response> {
    if (request.method !== 'POST') {
      return new Response('Method Not Allowed', { 
        status: 405,
        headers: corsHeaders 
      });
    }

    try {
      const body = await request.json() as { 
        deploymentId: string; 
        projectName: string;
      };
      
      if (!body.deploymentId || !body.projectName) {
        return new Response('Missing deploymentId or projectName', { 
          status: 400,
          headers: corsHeaders 
        });
      }

      // Start mock deployment simulation
      const simulator = new MockDeploymentSimulator();
      
      // Run simulation in background
      const deploymentPromise = simulator.simulateDeployment(
        body.deploymentId,
        body.projectName,
        async (status) => {
          // Update deployment status via internal API
          const updateUrl = `${request.url.split('/test')[0]}/internal/deployments/${body.deploymentId}/update`;
          
          await fetch(updateUrl, {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              'Authorization': `Bearer ${env.INTERNAL_API_KEY || 'test-key'}`
            },
            body: JSON.stringify({ status })
          });
        }
      );

      // Don't wait for completion
      deploymentPromise.catch(error => {
        console.error('Mock deployment failed:', error);
      });

      return new Response(JSON.stringify({ 
        message: 'Mock deployment started',
        deploymentId: body.deploymentId 
      }), { 
        status: 200,
        headers: { 
          ...corsHeaders,
          'Content-Type': 'application/json'
        }
      });
    } catch (error) {
      console.error('Failed to start test deployment:', error);
      return new Response('Internal Server Error', { 
        status: 500,
        headers: corsHeaders 
      });
    }
  }
};