/**
 * Example integration of the agents service in the chatserver
 * This shows how to use the service binding to access the Coder Agent
 * 
 * This would be integrated into apps/chatserver/src/index.ts
 */

import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { stream } from 'hono/streaming';

// Environment interface including the AGENTS_SERVICE binding
interface Env {
  AI: any;
  OPENROUTER_API_KEY: string;
  AGENTS_SERVICE: Fetcher; // The service binding to our agents worker
}

const app = new Hono<{ Bindings: Env }>();

// Enable CORS
app.use('*', cors({
  origin: '*',
  allowMethods: ['POST', 'GET', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Accept', 'Authorization', 'X-GitHub-Token'],
  exposeHeaders: ['X-Vercel-AI-Data-Stream'],
  credentials: true,
}));

// Health check
app.get('/', c => c.text('200 OK - Chat Server Running'));

// Coder agent endpoint
app.post('/coder', async c => {
  console.log("🚀 Coder agent request received");
  
  try {
    // Extract any headers we want to pass through
    const headers = new Headers();
    const authHeader = c.req.header('Authorization');
    const githubTokenHeader = c.req.header('X-GitHub-Token');
    
    if (authHeader) {
      headers.set('Authorization', authHeader);
    }
    
    if (githubTokenHeader) {
      headers.set('X-GitHub-Token', githubTokenHeader);
    }
    
    // Forward the request to the agents service, specifically to the coder endpoint
    const agentRequest = new Request(
      // Note the /coder path prefix - this will route to our CoderAgent in the agents service
      new URL('/coder', c.req.url).toString(),
      {
        method: c.req.method,
        headers: headers,
        body: c.req.body
      }
    );
    
    console.log(`🔄 Forwarding request to Coder Agent: ${agentRequest.url}`);
    
    // Use the service binding to call the agents service
    const response = await c.env.AGENTS_SERVICE.fetch(agentRequest);
    
    // Set up the same headers in our response
    c.header('Content-Type', response.headers.get('Content-Type') || 'text/event-stream; charset=utf-8');
    c.header('Cache-Control', 'no-cache');
    c.header('Connection', 'keep-alive');
    
    // Special header for Vercel AI SDK compatibility
    if (response.headers.has('X-Vercel-AI-Data-Stream')) {
      c.header('X-Vercel-AI-Data-Stream', response.headers.get('X-Vercel-AI-Data-Stream')!);
    }
    
    // Stream the response back to the client
    return stream(c, async (streamWriter) => {
      if (!response.body) {
        return streamWriter.write('Error: No response body from agent service');
      }
      
      const reader = response.body.getReader();
      
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          await streamWriter.write(value);
        }
      } catch (error) {
        console.error('Error streaming from agent service:', error);
        await streamWriter.write(`Error during streaming: ${error}`);
      }
    });
    
  } catch (error) {
    console.error("💥 Coder agent error:", error);
    return c.json({ error: "Failed to process coder agent request" }, 500);
  }
});

// Export the app
export default app;