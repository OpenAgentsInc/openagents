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
