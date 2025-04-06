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

// Proxy agent requests to the agent service
app.all('/agents/*', async (c) => {
  const url = new URL(c.req.url);
  // Forward requests to the agent service
  const agentServiceUrl = process.env.AGENT_SERVICE_URL || 'http://localhost:8787';
  const targetUrl = `${agentServiceUrl}${url.pathname}${url.search}`;
  
  try {
    const response = await fetch(targetUrl, {
      method: c.req.method,
      headers: c.req.headers,
      body: c.req.method !== 'GET' && c.req.method !== 'HEAD' ? await c.req.arrayBuffer() : undefined,
    });
    
    const headers = new Headers();
    response.headers.forEach((value, key) => {
      headers.set(key, value);
    });
    
    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers,
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
