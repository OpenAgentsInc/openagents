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
  getMCPClients,
  cleanupMCPClients,
  refreshTools,
  initMCPClients
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
    
    // Try to refresh tools first to get the latest
    try {
      // Check if clients need to be reinitialized first
      const { clients } = getMCPClients();
      if (Object.keys(clients).length === 0) {
        console.log('[MCP API] No MCP clients found, attempting to reinitialize all clients first');
        try {
          await reinitializeAllClients();
          console.log('[MCP API] Successfully reinitialized MCP clients');
        } catch (reinitError) {
          console.warn('[MCP API] Error reinitializing MCP clients:', reinitError);
        }
      }
    
      // Now refresh the tools
      await refreshTools();
    } catch (refreshError) {
      console.warn('[MCP API] Error refreshing tools before getting them:', refreshError);
      // Continue anyway to return what we have
    }
    
    // Get all MCP tools from the server
    const mcpTools = getMCPTools();
    
    // Get client information to associate tools with providers
    const { clientTools, configs, clients, allTools } = getMCPClients();
    
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
    
    // Log detailed MCP client information
    console.log('[MCP API] MCP Client Status:');
    console.log(`[MCP API] - Active clients: ${Object.keys(clients).length}`);
    
    Object.entries(configs).forEach(([clientId, config]) => {
      console.log(`[MCP API] - Client ${config.name} (${clientId}): Status ${config.status || 'unknown'}`);
      
      if (clientTools[clientId]) {
        console.log(`[MCP API]   Tools (${clientTools[clientId].length}): ${clientTools[clientId].join(', ')}`);
      } else {
        console.log(`[MCP API]   No tools registered for this client`);
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
    
    // Enhanced tool refresh approach
    try {
      // Get current client state
      console.log('[MCP API] Getting current MCP client state...');
      const { clients, configs } = getMCPClients();
      console.log(`[MCP API] Current client state: ${Object.keys(clients).length} clients, ${Object.keys(configs).length} configs`);

      // First reinitialize all clients - with more aggressive approach if needed
      try {
        if (Object.keys(clients).length === 0) {
          console.log('[MCP API] No clients initialized, using cleanup + full init approach');
          // Force cleanup first to ensure clean state
          cleanupMCPClients();
          
          // Then initialize from scratch
          await initMCPClients();
        } else {
          // Standard reinitialization
          console.log('[MCP API] Reinitializing all existing MCP clients...');
          await reinitializeAllClients();
        }
        
        console.log('[MCP API] All MCP clients reinitialized successfully');
      } catch (initError) {
        console.error('[MCP API] Error reinitializing clients:', initError);
        
        // More aggressive recovery attempt if reinitialization failed
        try {
          console.log('[MCP API] Attempting recovery with cleanup + fresh initialization');
          cleanupMCPClients();
          await initMCPClients();
        } catch (recoveryError) {
          console.error('[MCP API] Recovery attempt also failed:', recoveryError);
        }
      }
      
      // Then refresh tools
      console.log('[MCP API] Refreshing tools from all clients...');
      await refreshTools();
      console.log('[MCP API] Tools refreshed successfully');
    } catch (refreshError) {
      console.error('[MCP API] Error refreshing tools:', refreshError);
      // Continue despite error so we can at least return something
    }
    
    // Get the refreshed tools
    const mcpTools = getMCPTools();
    
    console.log('[MCP API] Current MCP tools:', {
      toolCount: Object.keys(mcpTools).length,
      toolSample: Object.keys(mcpTools).slice(0, 5)
    });
    
    // Detailed logging of available tools
    console.log('[MCP API] Available tools:');
    Object.keys(mcpTools).forEach(toolId => {
      console.log(`[MCP API] - Available tool: ${toolId} (${mcpTools[toolId]?.name || 'unnamed'})`);
    });
    
    return c.json({ 
      toolCount: Object.keys(mcpTools).length,
      tools: Object.keys(mcpTools),
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

/**
 * Execute an MCP tool directly
 * This endpoint serves as a bridge between the Cloudflare Worker and the MCP tools
 */
mcpRoutes.post('/execute', async (c) => {
  try {
    const body = await c.req.json();
    
    // Validate request
    if (!body.tool || !body.parameters) {
      return c.json({ error: 'Tool name and parameters are required' }, 400);
    }
    
    const toolName = body.tool;
    const parameters = body.parameters;
    
    console.log(`[MCP API] Executing tool ${toolName} with parameters:`, parameters);
    
    // Get MCP tools
    const mcpTools = getMCPTools();
    
    // Check if tool exists
    if (!mcpTools[toolName]) {
      return c.json({ error: `Tool ${toolName} not found` }, 404);
    }
    
    // Execute the tool
    try {
      const result = await mcpTools[toolName].execute(parameters);
      console.log(`[MCP API] Tool ${toolName} execution result:`, result);
      
      // Return the result
      return c.json(result);
    } catch (toolError) {
      console.error(`[MCP API] Error executing tool ${toolName}:`, toolError);
      
      // Return error
      return c.json({ 
        error: `Tool execution failed: ${toolError instanceof Error ? toolError.message : String(toolError)}` 
      }, 500);
    }
  } catch (error) {
    console.error('[MCP API] Error processing tool execution request:', error);
    
    return c.json({ 
      error: 'Failed to process tool execution request',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});

export default mcpRoutes;