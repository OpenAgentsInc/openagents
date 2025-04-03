// apps/coder/src/server/mcp-clients.ts
import { experimental_createMCPClient } from "ai";
import { Experimental_StdioMCPTransport as StdioMCPTransport } from 'ai/mcp-stdio';
import { settingsRepository } from '@openagents/core/src/db/repositories';
import { MCPClientConfig } from '@openagents/core/src/db/types';
import { v4 as uuidv4 } from 'uuid';

// Define the type for the MCP clients
interface MCPClients {
  clients: Record<string, Awaited<ReturnType<typeof experimental_createMCPClient>> | null>;
  allTools: Record<string, any>; // Combined tools from all clients
  configs: Record<string, MCPClientConfig>; // Configurations for each client
  initialized: boolean;
}

// Global state for MCP clients
const mcpClients: MCPClients = {
  clients: {},
  allTools: {},
  configs: {},
  initialized: false,
};

// Default MCP client configurations
const DEFAULT_MCP_CLIENTS: MCPClientConfig[] = [
  {
    id: 'remote-github',
    name: 'Remote GitHub MCP',
    enabled: true,
    type: 'sse',
    url: 'https://mcp-github.openagents.com/sse',
    status: 'disconnected'
  },
  {
    id: 'local-github',
    name: 'Local GitHub MCP',
    enabled: false,
    type: 'stdio',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-github'],
    env: {
      GITHUB_PERSONAL_ACCESS_TOKEN: '<TOKEN_REQUIRED>'
    },
    status: 'disconnected'
  },
  {
    id: 'local-shell',
    name: 'Local Shell MCP',
    enabled: false,
    type: 'stdio',
    command: 'uvx',
    args: ['mcp-shell-server'],
    env: {
      ALLOW_COMMANDS: 'ls,cat,pwd,echo,grep,find,ps,wc'
    },
    status: 'disconnected'
  }
];

/**
 * Ensure MCP client configurations exist in settings
 */
async function ensureMCPClientConfigs(): Promise<MCPClientConfig[]> {
  try {
    // Get current settings
    const settings = await settingsRepository.getSettings();
    
    // Check if MCP clients already exist in settings
    if (settings.mcpClients && Array.isArray(settings.mcpClients) && settings.mcpClients.length > 0) {
      console.log('[MCP Clients] Found existing MCP client configurations in settings');
      return settings.mcpClients;
    }
    
    // If no MCP clients exist, add the default ones
    console.log('[MCP Clients] Creating default MCP client configurations');
    await settingsRepository.updateSettings({
      mcpClients: DEFAULT_MCP_CLIENTS
    });
    
    return DEFAULT_MCP_CLIENTS;
  } catch (error) {
    console.error('[MCP Clients] Error ensuring MCP client configurations:', error);
    // Return default configurations as fallback
    return DEFAULT_MCP_CLIENTS;
  }
}

/**
 * Update MCP client status
 */
async function updateClientStatus(id: string, status: 'connected' | 'disconnected' | 'error', statusMessage?: string): Promise<void> {
  try {
    // Get current settings
    const settings = await settingsRepository.getSettings();
    
    // Find and update the specific client
    if (settings.mcpClients && Array.isArray(settings.mcpClients)) {
      const updatedClients = settings.mcpClients.map(client => {
        if (client.id === id) {
          return {
            ...client,
            status,
            statusMessage,
            lastConnected: status === 'connected' ? Date.now() : client.lastConnected
          };
        }
        return client;
      });
      
      // Update settings with the new client status
      await settingsRepository.updateSettings({
        mcpClients: updatedClients
      });
      
      // Update local configs cache
      if (mcpClients.configs[id]) {
        mcpClients.configs[id] = {
          ...mcpClients.configs[id],
          status,
          statusMessage,
          lastConnected: status === 'connected' ? Date.now() : mcpClients.configs[id].lastConnected
        };
      }
    }
  } catch (error) {
    console.error(`[MCP Clients] Error updating status for client ${id}:`, error);
  }
}

/**
 * Initialize an individual MCP client based on configuration
 */
async function initMCPClient(config: MCPClientConfig): Promise<Awaited<ReturnType<typeof experimental_createMCPClient>> | null> {
  // Skip if disabled
  if (!config.enabled) {
    console.log(`[MCP Clients] Skipping disabled client: ${config.name}`);
    return null;
  }
  
  try {
    console.log(`[MCP Clients] Initializing client: ${config.name} (${config.id})`);
    
    // Create the client based on configuration type
    if (config.type === 'sse') {
      if (!config.url) {
        throw new Error('URL is required for SSE type clients');
      }
      
      const client = await experimental_createMCPClient({
        transport: {
          type: 'sse',
          url: config.url,
          ...(config.env ? { headers: config.env } : {})
        }
      });
      
      // Update status
      await updateClientStatus(config.id, 'connected');
      console.log(`[MCP Clients] SSE client initialized successfully: ${config.name}`);
      return client;
    } 
    else if (config.type === 'stdio') {
      if (!config.command) {
        throw new Error('Command is required for stdio type clients');
      }
      
      const transport = new StdioMCPTransport({
        command: config.command,
        args: config.args || [],
        env: config.env || {}
      });
      
      const client = await experimental_createMCPClient({
        transport
      });
      
      // Update status
      await updateClientStatus(config.id, 'connected');
      console.log(`[MCP Clients] stdio client initialized successfully: ${config.name}`);
      return client;
    }
    
    throw new Error(`Unsupported client type: ${config.type}`);
  } catch (error) {
    console.error(`[MCP Clients] Failed to initialize client ${config.name}:`, error);
    // Update status
    await updateClientStatus(config.id, 'error', (error as Error).message);
    return null;
  }
}

/**
 * Initialize MCP clients based on configured settings
 */
export async function initMCPClients(): Promise<void> {
  if (mcpClients.initialized) {
    console.log('[MCP Clients] Already initialized, skipping');
    return;
  }

  console.log('[MCP Clients] Initializing MCP clients from settings...');

  // Ensure MCP client configurations exist
  const configs = await ensureMCPClientConfigs();
  
  // Create a map of configs by ID for easy lookup
  const configsMap: Record<string, MCPClientConfig> = {};
  configs.forEach(config => {
    configsMap[config.id] = config;
  });
  
  // Store configs in the global state
  mcpClients.configs = configsMap;
  
  // Initialize each enabled client
  for (const config of configs) {
    if (config.enabled) {
      const client = await initMCPClient(config);
      if (client) {
        mcpClients.clients[config.id] = client;
      }
    } else {
      console.log(`[MCP Clients] Skipping disabled client: ${config.name}`);
      // Mark as disconnected in the status
      await updateClientStatus(config.id, 'disconnected', 'Client is disabled');
    }
  }

  // Fetch and store tools from all initialized clients
  await refreshTools();

  mcpClients.initialized = true;
  console.log('[MCP Clients] Initialization complete');
}

/**
 * Reinitialize an MCP client by ID
 */
export async function reinitializeClient(id: string): Promise<boolean> {
  try {
    // Get the config for this client
    const config = mcpClients.configs[id];
    if (!config) {
      console.error(`[MCP Clients] Cannot reinitialize client ${id}: configuration not found`);
      return false;
    }
    
    // Clean up existing client if present
    if (mcpClients.clients[id]) {
      console.log(`[MCP Clients] Cleaning up existing client: ${config.name}`);
      mcpClients.clients[id] = null;
    }
    
    // Skip initialization if disabled
    if (!config.enabled) {
      console.log(`[MCP Clients] Client ${config.name} is disabled, skipping initialization`);
      await updateClientStatus(id, 'disconnected', 'Client is disabled');
      return false;
    }
    
    // Initialize the client
    const newClient = await initMCPClient(config);
    if (newClient) {
      mcpClients.clients[id] = newClient;
      
      // Refresh tools
      await refreshTools();
      return true;
    }
    
    return false;
  } catch (error) {
    console.error(`[MCP Clients] Error reinitializing client ${id}:`, error);
    return false;
  }
}

/**
 * Reinitialize all MCP clients
 */
export async function reinitializeAllClients(): Promise<void> {
  try {
    console.log('[MCP Clients] Reinitializing all MCP clients...');
    
    // Clean up existing clients
    cleanupMCPClients();
    
    // Reinitialize
    await initMCPClients();
    
    console.log('[MCP Clients] Reinitialization complete');
  } catch (error) {
    console.error('[MCP Clients] Error reinitializing clients:', error);
  }
}

/**
 * Get the initialized MCP clients
 */
export function getMCPClients(): MCPClients {
  return mcpClients;
}

/**
 * Add a new MCP client configuration
 */
export async function addMCPClient(config: Omit<MCPClientConfig, 'id'>): Promise<string> {
  try {
    // Generate a new ID for the client
    const id = uuidv4();
    const newConfig: MCPClientConfig = {
      ...config,
      id,
      status: 'disconnected'
    };
    
    // Get current settings
    const settings = await settingsRepository.getSettings();
    const currentClients = settings.mcpClients || [];
    
    // Add the new configuration
    const updatedClients = [...currentClients, newConfig];
    await settingsRepository.updateSettings({
      mcpClients: updatedClients
    });
    
    // Update our local cache
    mcpClients.configs[id] = newConfig;
    
    // Initialize the client if enabled
    if (newConfig.enabled) {
      const client = await initMCPClient(newConfig);
      if (client) {
        mcpClients.clients[id] = client;
        await refreshTools();
      }
    }
    
    return id;
  } catch (error) {
    console.error('[MCP Clients] Error adding new MCP client:', error);
    throw error;
  }
}

/**
 * Update an existing MCP client configuration
 */
export async function updateMCPClient(id: string, updates: Partial<MCPClientConfig>): Promise<boolean> {
  try {
    // Get current settings
    const settings = await settingsRepository.getSettings();
    
    if (!settings.mcpClients || !Array.isArray(settings.mcpClients)) {
      console.error('[MCP Clients] No MCP clients found in settings');
      return false;
    }
    
    // Find the client to update
    const clientIndex = settings.mcpClients.findIndex(c => c.id === id);
    if (clientIndex === -1) {
      console.error(`[MCP Clients] Client with ID ${id} not found`);
      return false;
    }
    
    // Create the updated client configuration
    const currentConfig = settings.mcpClients[clientIndex];
    const updatedConfig: MCPClientConfig = {
      ...currentConfig,
      ...updates
    };
    
    // Check if enabled state changed - this will require reinitialization
    const enabledChanged = currentConfig.enabled !== updatedConfig.enabled;
    const typeChanged = currentConfig.type !== updatedConfig.type;
    const configChanged = 
      (currentConfig.type === 'sse' && updatedConfig.type === 'sse' && currentConfig.url !== updatedConfig.url) ||
      (currentConfig.type === 'stdio' && updatedConfig.type === 'stdio' && 
        (currentConfig.command !== updatedConfig.command || 
         JSON.stringify(currentConfig.args) !== JSON.stringify(updatedConfig.args) ||
         JSON.stringify(currentConfig.env) !== JSON.stringify(updatedConfig.env)));
    
    // Update the settings
    const updatedClients = [...settings.mcpClients];
    updatedClients[clientIndex] = updatedConfig;
    await settingsRepository.updateSettings({
      mcpClients: updatedClients
    });
    
    // Update our local cache
    mcpClients.configs[id] = updatedConfig;
    
    // Clean up and reinitialize if necessary
    if (enabledChanged || typeChanged || configChanged) {
      console.log(`[MCP Clients] Configuration changed for ${updatedConfig.name}, reinitializing...`);
      
      // Clean up existing client
      if (mcpClients.clients[id]) {
        mcpClients.clients[id] = null;
      }
      
      // Initialize if enabled
      if (updatedConfig.enabled) {
        const client = await initMCPClient(updatedConfig);
        if (client) {
          mcpClients.clients[id] = client;
          await refreshTools();
        }
      } else {
        // Update status to disconnected if disabled
        await updateClientStatus(id, 'disconnected', 'Client is disabled');
        await refreshTools();
      }
    }
    
    return true;
  } catch (error) {
    console.error(`[MCP Clients] Error updating client ${id}:`, error);
    return false;
  }
}

/**
 * Delete an MCP client configuration
 */
export async function deleteMCPClient(id: string): Promise<boolean> {
  try {
    // Get current settings
    const settings = await settingsRepository.getSettings();
    
    if (!settings.mcpClients || !Array.isArray(settings.mcpClients)) {
      console.error('[MCP Clients] No MCP clients found in settings');
      return false;
    }
    
    // Find the client to delete
    const clientIndex = settings.mcpClients.findIndex(c => c.id === id);
    if (clientIndex === -1) {
      console.error(`[MCP Clients] Client with ID ${id} not found`);
      return false;
    }
    
    // Remove from settings
    const updatedClients = settings.mcpClients.filter(c => c.id !== id);
    await settingsRepository.updateSettings({
      mcpClients: updatedClients
    });
    
    // Clean up client
    if (mcpClients.clients[id]) {
      mcpClients.clients[id] = null;
      delete mcpClients.clients[id];
    }
    
    // Remove from config cache
    delete mcpClients.configs[id];
    
    // Refresh tools
    await refreshTools();
    
    return true;
  } catch (error) {
    console.error(`[MCP Clients] Error deleting client ${id}:`, error);
    return false;
  }
}

/**
 * Get all MCP client configurations
 */
export async function getMCPClientConfigs(): Promise<MCPClientConfig[]> {
  try {
    // Get current settings
    const settings = await settingsRepository.getSettings();
    return settings.mcpClients || [];
  } catch (error) {
    console.error('[MCP Clients] Error getting MCP client configurations:', error);
    return [];
  }
}

/**
 * Refresh tools from all initialized MCP clients
 */
export async function refreshTools(): Promise<Record<string, any>> {
  const tools: Record<string, any> = {};

  // Fetch tools from all initialized clients
  for (const [id, client] of Object.entries(mcpClients.clients)) {
    if (client) {
      try {
        const config = mcpClients.configs[id];
        console.log(`[MCP Clients] Fetching tools from client: ${config?.name || id}`);
        const clientTools = await client.tools();
        Object.assign(tools, clientTools);
        console.log(`[MCP Clients] Successfully fetched tools from client: ${config?.name || id}`);
      } catch (toolError) {
        console.error(`[MCP Clients] Error fetching tools from client ${id}:`, toolError);
      }
    }
  }

  // Update the global tools cache
  mcpClients.allTools = tools;

  return tools;
}

/**
 * Clean up MCP clients
 */
export function cleanupMCPClients(): void {
  console.log('[MCP Clients] Cleaning up MCP clients');

  // Reset the global state
  for (const id in mcpClients.clients) {
    mcpClients.clients[id] = null;
  }
  
  mcpClients.clients = {};
  mcpClients.allTools = {};
  mcpClients.initialized = false;
  // Keep the configs cache for reference, but mark all as disconnected
  
  // Update status for all configs to disconnected
  Object.keys(mcpClients.configs).forEach(id => {
    if (mcpClients.configs[id]) {
      mcpClients.configs[id] = {
        ...mcpClients.configs[id],
        status: 'disconnected',
        statusMessage: 'Cleaned up during application shutdown'
      };
    }
  });

  console.log('[MCP Clients] Cleanup complete');
}
