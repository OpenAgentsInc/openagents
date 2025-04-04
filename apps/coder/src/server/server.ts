/**
 * Main server file
 */

import { Hono } from 'hono';
import { logger } from 'hono/logger';
import { cors } from 'hono/cors';
import { chatRoutes, mcpRoutes } from './routes';

// Create Hono app
const app = new Hono();

// Use logger middleware
app.use('*', logger());

// Use CORS middleware
app.use('*', cors({
  origin: '*', // Allow requests from any origin
  allowMethods: ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Authorization', 'Accept', 'X-Requested-With'],
  maxAge: 86400,
  exposeHeaders: ['Content-Length', 'X-Vercel-AI-Data-Stream'],
}));

// Mount chat routes
app.route('/api', chatRoutes);

// Mount MCP routes
app.route('/api/mcp', mcpRoutes);

// Simple health check
app.get('/api/health', (c) => {
  return c.json({ status: 'ok', version: '1.0.0' });
});

export default app;