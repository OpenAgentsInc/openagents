/**
 * MCP API routes
 */

import { Hono } from 'hono';
import { 
  getMCPClientConfigs, 
  reinitializeClient,
  reinitializeAllClients,
  addMCPClient,
  updateMCPClient,
  deleteMCPClient
} from '../mcp-clients';
import { MCPClientConfig } from '@openagents/core/src/db/types';
import { ChatError, SystemError } from '@openagents/core/src/chat/errors';

// Create MCP API router
const mcpRoutes = new Hono();

/**
 * Get all MCP client configurations
 */
mcpRoutes.get('/clients', async (c) => {
  try {
    const clients = await getMCPClientConfigs();
    return c.json({ clients });
  } catch (error) {
    console.error('[MCP API] Error getting clients:', error);
    
    const systemError = new SystemError({
      message: error instanceof Error ? error.message : String(error),
      userMessage: 'Failed to retrieve MCP clients',
      originalError: error
    });
    
    return c.json({ 
      error: systemError.userMessage,
      details: process.env.NODE_ENV === 'development' ? systemError.message : undefined
    }, 500);
  }
});

/**
 * Add a new MCP client
 */
mcpRoutes.post('/clients', async (c) => {
  try {
    const body = await c.req.json();
    
    // Validate required fields
    if (!body.name || !body.type) {
      return c.json({ error: 'Name and type are required' }, 400);
    }
    
    // Validate type-specific fields
    if (body.type === 'sse' && !body.url) {
      return c.json({ error: 'URL is required for SSE type clients' }, 400);
    }
    
    if (body.type === 'stdio' && !body.command) {
      return c.json({ error: 'Command is required for stdio type clients' }, 400);
    }
    
    // Create the client
    const clientId = await addMCPClient(body);
    return c.json({ id: clientId, success: true });
  } catch (error) {
    console.error('[MCP API] Error adding client:', error);
    
    return c.json({ 
      error: 'Failed to add MCP client',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});

/**
 * Update an existing MCP client
 */
mcpRoutes.patch('/clients/:id', async (c) => {
  try {
    const id = c.req.param('id');
    const body = await c.req.json();
    
    // Update the client
    const success = await updateMCPClient(id, body);
    
    if (!success) {
      return c.json({ error: 'Client not found or update failed' }, 404);
    }
    
    return c.json({ success: true });
  } catch (error) {
    console.error('[MCP API] Error updating client:', error);
    
    return c.json({ 
      error: 'Failed to update MCP client',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});

/**
 * Delete an MCP client
 */
mcpRoutes.delete('/clients/:id', async (c) => {
  try {
    const id = c.req.param('id');
    
    // Delete the client
    const success = await deleteMCPClient(id);
    
    if (!success) {
      return c.json({ error: 'Client not found or delete failed' }, 404);
    }
    
    return c.json({ success: true });
  } catch (error) {
    console.error('[MCP API] Error deleting client:', error);
    
    return c.json({ 
      error: 'Failed to delete MCP client',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});

/**
 * Refresh (reinitialize) a specific client
 */
mcpRoutes.post('/clients/:id/refresh', async (c) => {
  try {
    const id = c.req.param('id');
    
    // Reinitialize the client
    const success = await reinitializeClient(id);
    
    if (!success) {
      return c.json({ error: 'Client not found or refresh failed' }, 404);
    }
    
    return c.json({ success: true });
  } catch (error) {
    console.error('[MCP API] Error refreshing client:', error);
    
    return c.json({ 
      error: 'Failed to refresh MCP client',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});

/**
 * Refresh (reinitialize) all clients
 */
mcpRoutes.post('/refresh', async (c) => {
  try {
    await reinitializeAllClients();
    return c.json({ success: true });
  } catch (error) {
    console.error('[MCP API] Error refreshing all clients:', error);
    
    return c.json({ 
      error: 'Failed to refresh MCP clients',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});

export default mcpRoutes;