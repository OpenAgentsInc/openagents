// apps/coder/src/server/mcp-clients.ts
import { experimental_createMCPClient } from "ai";
import { Experimental_StdioMCPTransport as StdioMCPTransport } from 'ai/mcp-stdio';
// Import type only, don't import the actual repository
import type { MCPClientConfig } from '@openagents/core/src/db/types';
import { v4 as uuidv4 } from 'uuid';

// Define mock interfaces for browser environment
interface MockFS {
  existsSync: (path: string) => boolean;
  readFileSync: (path: string, encoding?: string) => string;
  writeFileSync: (path: string, data: string, encoding?: string) => void;
}

interface MockPath {
  join: (...paths: string[]) => string;
}

interface MockApp {
  getPath: (name: string) => string;
}

// In browser environments, use mock objects
let fsModule: MockFS = { 
  existsSync: () => false,
  readFileSync: () => "{}",
  writeFileSync: () => {} 
};
let pathModule: MockPath = { join: (...args: string[]) => args.join('/') };
let appModule: MockApp = { getPath: () => "" };

// Only import node modules in non-browser environments
if (typeof window === 'undefined') {
  try {
    // @ts-ignore - Dynamic import
    fsModule = require('fs');
    // @ts-ignore - Dynamic import
    pathModule = require('path');
    try {
      // @ts-ignore - Dynamic import
      const electron = require('electron');
      if (electron && electron.app) {
        // @ts-ignore - Type incompatibility but we're careful
        appModule = electron.app;
      }
    } catch (e) {
      console.warn("Electron not available");
    }
  } catch (e) {
    console.warn("Node.js file system modules not available");
  }
}

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

// Helper to get the config file path
function getConfigFilePath(): string {
  try {
    // In browser environment, return a dummy path
    if (typeof window !== 'undefined') {
      return '/mcp-clients.json';
    }
    
    // Get the user data directory in Node.js environment
    const userDataPath = appModule.getPath('userData');
    return pathModule.join(userDataPath, 'mcp-clients.json');
  } catch (error) {
    console.warn('[MCP Clients] Error getting config file path:', error);
    return 'mcp-clients.json';
  }
}

// Load configurations from file
function loadConfigsFromFile(): MCPClientConfig[] {
  try {
    // In browser environment, return default configs
    if (typeof window !== 'undefined') {
      console.log('[MCP Clients] In browser environment, using default configs');
      return [...DEFAULT_MCP_CLIENTS];
    }
    
    const configPath = getConfigFilePath();
    if (fsModule.existsSync(configPath)) {
      const configData = fsModule.readFileSync(configPath, 'utf8');
      const parsedConfigs = JSON.parse(configData);
      if (Array.isArray(parsedConfigs)) {
        console.log('[MCP Clients] Loaded configurations from file');
        return parsedConfigs;
      }
    }
  } catch (error) {
    console.error('[MCP Clients] Error loading configurations from file:', error);
  }
  
  // Return defaults if file doesn't exist or has invalid content
  return [...DEFAULT_MCP_CLIENTS];
}

// Save configurations to file
function saveConfigsToFile(configs: MCPClientConfig[]): void {
  try {
    // In browser environment, do nothing
    if (typeof window !== 'undefined') {
      console.log('[MCP Clients] In browser environment, skipping config save');
      return;
    }
    
    const configPath = getConfigFilePath();
    fsModule.writeFileSync(configPath, JSON.stringify(configs, null, 2), 'utf8');
    console.log('[MCP Clients] Saved configurations to file');
  } catch (error) {
    console.error('[MCP Clients] Error saving configurations to file:', error);
  }
}

/**
 * Ensure MCP client configurations exist
 */
async function ensureMCPClientConfigs(): Promise<MCPClientConfig[]> {
  try {
    // Load from file
    const configs = loadConfigsFromFile();
    
    // If no configs were loaded (empty array), use defaults
    if (!configs || configs.length === 0) {
      console.log('[MCP Clients] No configurations found, using defaults');
      saveConfigsToFile(DEFAULT_MCP_CLIENTS);
      return [...DEFAULT_MCP_CLIENTS];
    }
    
    return configs;
  } catch (error) {
    console.error('[MCP Clients] Error ensuring MCP client configurations:', error);
    // Return default configurations as fallback
    return [...DEFAULT_MCP_CLIENTS];
  }
}

/**
 * Update MCP client status
 */
async function updateClientStatus(id: string, status: 'connected' | 'disconnected' | 'error', statusMessage?: string): Promise<void> {
  // Always update local configs cache first
  if (mcpClients.configs[id]) {
    mcpClients.configs[id] = {
      ...mcpClients.configs[id],
      status,
      statusMessage,
      lastConnected: status === 'connected' ? Date.now() : mcpClients.configs[id].lastConnected
    };
  }
  
  // Save updated configs to file
  try {
    const allConfigs = Object.values(mcpClients.configs);
    saveConfigsToFile(allConfigs);
  } catch (error) {
    // Just log the error but don't block MCP client initialization
    console.warn(`[MCP Clients] Could not save status for client ${id} to file:`, error);
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

  try {
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
      try {
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
      } catch (clientError) {
        console.error(`[MCP Clients] Error initializing client ${config.name}:`, clientError);
        // Continue with other clients even if one fails
      }
    }

    // Fetch and store tools from all initialized clients
    await refreshTools();

    mcpClients.initialized = true;
    console.log('[MCP Clients] Initialization complete');
  } catch (error) {
    console.error('[MCP Clients] Error during initialization:', error);
    
    // Even if there was an error, mark as initialized so we don't try again
    // This prevents repeated errors during startup
    mcpClients.initialized = true;
    
    // Initialize with default remote client as fallback
    try {
      const remoteConfig = DEFAULT_MCP_CLIENTS.find(c => c.id === 'remote-github');
      if (remoteConfig && remoteConfig.enabled) {
        console.log('[MCP Clients] Attempting to initialize default remote client as fallback');
        const client = await initMCPClient(remoteConfig);
        if (client) {
          mcpClients.clients['remote-github'] = client;
          mcpClients.configs['remote-github'] = remoteConfig;
          await refreshTools();
        }
      }
    } catch (fallbackError) {
      console.error('[MCP Clients] Failed to initialize fallback client:', fallbackError);
    }
  }
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
    
    // Get current configurations
    const currentConfigs = Object.values(mcpClients.configs);
    
    // Add the new configuration
    const updatedConfigs = [...currentConfigs, newConfig];
    
    // Save to file
    saveConfigsToFile(updatedConfigs);
    
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
    // Get current configs
    const currentConfigs = Object.values(mcpClients.configs);
    
    // Find the client to update
    const clientIndex = currentConfigs.findIndex(c => c.id === id);
    if (clientIndex === -1) {
      console.error(`[MCP Clients] Client with ID ${id} not found`);
      return false;
    }
    
    // Create the updated client configuration
    const currentConfig = currentConfigs[clientIndex];
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
    
    // Update the configs array
    const updatedConfigs = [...currentConfigs];
    updatedConfigs[clientIndex] = updatedConfig;
    
    // Save to file
    saveConfigsToFile(updatedConfigs);
    
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
    // Get current configs
    const currentConfigs = Object.values(mcpClients.configs);
    
    // Find the client to delete
    const clientIndex = currentConfigs.findIndex(c => c.id === id);
    if (clientIndex === -1) {
      console.error(`[MCP Clients] Client with ID ${id} not found`);
      return false;
    }
    
    // Remove from configs array
    const updatedConfigs = currentConfigs.filter(c => c.id !== id);
    
    // Save to file
    saveConfigsToFile(updatedConfigs);
    
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
    return Object.values(mcpClients.configs);
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
