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
  deleteMCPClient,
  getMCPClients
} from '../mcp-clients';
import { getMCPTools } from '../tools/mcp-tools';
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

/**
 * Get all MCP tools
 * This endpoint is critical for the tool selection component to show available MCP tools
 */
mcpRoutes.get('/tools', async (c) => {
  try {
    console.log('[MCP API] Getting all MCP tools...');
    
    // Get all MCP tools from the server
    const mcpTools = getMCPTools();
    
    // Get client information to associate tools with providers
    const { clientTools, configs } = getMCPClients();
    
    // Create client info map for the response
    const clientInfoMap: Record<string, { id: string; name: string; tools: string[] }> = {};
    
    // Build the client info map from the data
    Object.entries(clientTools).forEach(([clientId, toolIds]) => {
      if (configs[clientId]) {
        clientInfoMap[clientId] = {
          id: clientId,
          name: configs[clientId].name,
          tools: toolIds
        };
      }
    });
    
    console.log('[MCP API] Found MCP tools:', {
      toolCount: Object.keys(mcpTools).length,
      clientCount: Object.keys(clientInfoMap).length,
      toolSample: Object.keys(mcpTools).slice(0, 5)
    });
    
    return c.json({ 
      tools: mcpTools,
      clientInfo: clientInfoMap,
      success: true 
    });
  } catch (error) {
    console.error('[MCP API] Error getting MCP tools:', error);
    
    return c.json({ 
      error: 'Failed to get MCP tools',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});

/**
 * Refresh all MCP tools
 */
mcpRoutes.post('/tools/refresh', async (c) => {
  try {
    console.log('[MCP API] Refreshing MCP tools...');
    
    // Reinitialize all clients to refresh tools
    await reinitializeAllClients();
    
    // Get the refreshed tools
    const mcpTools = getMCPTools();
    
    console.log('[MCP API] Refreshed MCP tools:', {
      toolCount: Object.keys(mcpTools).length,
      toolSample: Object.keys(mcpTools).slice(0, 5)
    });
    
    return c.json({ 
      toolCount: Object.keys(mcpTools).length,
      success: true 
    });
  } catch (error) {
    console.error('[MCP API] Error refreshing MCP tools:', error);
    
    return c.json({ 
      error: 'Failed to refresh MCP tools',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});

export default mcpRoutes;