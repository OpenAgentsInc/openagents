/**
 * Main server file
 */

import { Hono } from 'hono';
import { serve } from '@hono/node-server';
import { cors } from 'hono/cors';
import { chatRoutes, mcpRoutes } from './routes';
import { createAgentRouterProvider, inferRouted } from '@openagents/core';

// Create Hono app
const app = new Hono();

// Enable CORS
app.use('*', cors());

// Mount chat routes
app.route('/api/chat', chatRoutes);

// Mount MCP routes
app.route('/api/mcp', mcpRoutes);

// For WebSocket connections, the Vite proxy will handle these directly
// This route is kept for HTTP requests that might still go through the server
app.all('/agents/*', async (c) => {
  // Check if this is a WebSocket request (the upgrade header will be present)
  const upgradeHeader = c.req.header('upgrade')?.toLowerCase();
  
  if (upgradeHeader === 'websocket') {
    // For WebSocket requests, return a response that will cause the client
    // to attempt a direct connection, which will be handled by Vite proxy
    return new Response('WebSocket connections should use the Vite proxy', { 
      status: 426, // Upgrade Required status
      headers: {
        'Upgrade': 'WebSocket',
      }
    });
  }
  
  // For regular HTTP requests, proxy to the agent service
  const url = new URL(c.req.url);
  const agentServiceUrl = "https://agents.openagents.com";
  const targetUrl = `${agentServiceUrl}${url.pathname}${url.search}`;
  
  try {
    // Create headers object from Hono request
    const requestHeaders = new Headers();
    
    // Extract headers from the raw request
    const rawHeaders = c.req.raw.headers;
    
    // Try different ways to access headers
    if (typeof rawHeaders.get === 'function') {
      // If it's a Headers-like object
      for (const [key, value] of rawHeaders.entries()) {
        requestHeaders.set(key, value);
      }
    } else if (rawHeaders instanceof Object) {
      // If it's a plain object
      Object.entries(rawHeaders).forEach(([key, value]) => {
        if (value) requestHeaders.set(key, Array.isArray(value) ? value.join(', ') : String(value));
      });
    }
    
    const response = await fetch(targetUrl, {
      method: c.req.method,
      headers: requestHeaders,
      body: c.req.method !== 'GET' && c.req.method !== 'HEAD' ? await c.req.arrayBuffer() : undefined,
    });

    const responseHeaders = new Headers();
    response.headers.forEach((value: string, key: string) => {
      responseHeaders.set(key, value);
    });

    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: responseHeaders,
    });
  } catch (error) {
    console.error('Error proxying to agent service:', error);
    return c.json({ error: 'Failed to connect to agent service' }, 502);
  }
});

// Test endpoint for agent router
app.post('/api/test-router', async (c) => {
  try {
    const body = await c.req.json();
    const apiKey = c.req.header('x-openrouter-key');

    if (!apiKey) {
      return c.json({ error: 'OpenRouter API key is required' }, 400);
    }

    if (!body.prompt) {
      return c.json({ error: 'Prompt is required' }, 400);
    }

    // Create agent router provider
    const provider = createAgentRouterProvider('anthropic/claude-3-opus-20240229', apiKey, {
      baseURL: 'https://openrouter.ai/api/v1'
    });

    // Route the prompt
    const result = await inferRouted(provider, body.prompt);

    return c.json(result);
  } catch (error) {
    console.error('Agent router test error:', error);
    return c.json({ error: error instanceof Error ? error.message : 'Unknown error' }, 500);
  }
});

// Simple health check
app.get('/health', (c) => c.text('OK'));

// Start server
const port = process.env.PORT || 3001;
console.log(`Server running at http://localhost:${port}`);

serve(app, (info) => {
  console.log(`Listening on ${info.address}:${info.port}`);
});

export default app;
