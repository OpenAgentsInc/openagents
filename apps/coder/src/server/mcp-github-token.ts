/**
 * MCP GitHub Token Synchronization
 * 
 * This module handles synchronizing GitHub tokens from the API Keys settings
 * to the MCP GitHub client. It ensures that any token changes in the settings
 * are reflected in MCP tool calls.
 * 
 * CRITICAL: This module must be initialized BEFORE MCP clients are created
 * to ensure tokens are loaded and available during client initialization.
 */

import { getMCPClients } from './mcp-clients';
import type { SettingsRepository } from '@openagents/core/src/db/repositories/settings-repository';

// Global handler for token change events
let tokenChangeHandler: ((event: CustomEvent) => void) | null = null;

// Internal cache of the GitHub token to make it available during MCP initialization
let cachedGithubToken: string | null = null;

/**
 * Get the current GitHub token
 * This allows the MCP client initialization to access the token directly
 */
export function getGithubToken(): string | null {
  return cachedGithubToken;
}

/**
 * Updates the GitHub token in the MCP GitHub client
 * @param token The GitHub token to set
 */
export async function updateMCPGithubToken(token?: string): Promise<void> {
  try {
    console.log(`Attempting to update GitHub token in MCP client (token provided: ${token ? 'yes' : 'no'})`);
    
    // Update the cached token first so it's available for client initialization
    cachedGithubToken = token || null;
    console.log(`Updated cached GitHub token: ${cachedGithubToken ? 'Token set' : 'No token'}`);
    
    // Get the MCP clients
    const mcpClients = getMCPClients();
    
    // Log MCP client state
    console.log('MCP clients state:', {
      initialized: mcpClients.initialized,
      clientsCount: Object.keys(mcpClients.clients).length,
      availableClients: Object.keys(mcpClients.clients),
      hasGithubClient: !!mcpClients.clients['local-github']
    });
    
    // Get the GitHub client
    const githubClient = mcpClients.clients['local-github'];
    
    if (!githubClient) {
      console.warn('MCP GitHub client not initialized, cannot update token in client yet. Token is cached for later use.');
      return;
    }

    // Set the token in the MCP GitHub client environment
    console.log('GitHub client found, attempting to access transport environment...');
    const transport = (githubClient as any).transport;
    
    if (!transport) {
      console.warn('MCP GitHub client has no transport property');
      return;
    }
    
    console.log('Transport type:', transport.constructor ? transport.constructor.name : typeof transport);
    
    const process = transport.process;
    if (!process) {
      console.warn('MCP GitHub client transport has no process property');
      return;
    }
    
    const env = process.env;
    if (env) {
      const oldTokenPresent = !!env.GITHUB_PERSONAL_ACCESS_TOKEN;
      env.GITHUB_PERSONAL_ACCESS_TOKEN = token || '';
      console.log(`Updated GitHub token in MCP client (previous token: ${oldTokenPresent ? 'present' : 'not present'}, new token: ${token ? 'present' : 'not present'})`);
    } else {
      console.warn('MCP GitHub client environment not available');
    }
  } catch (error) {
    console.error('Failed to update GitHub token in MCP client:', error);
    console.error('Error details:', error instanceof Error ? error.stack : String(error));
  }
}

/**
 * Loads the GitHub token from settings and updates the MCP client
 */
export async function loadGitHubTokenFromSettings(): Promise<void> {
  try {
    console.log('Loading GitHub token from settings...');
    
    // In Node.js environments, we'll use environment variables or config files
    // instead of trying to use the settings repository which requires a browser
    const isNode = typeof window === 'undefined';
    
    let githubToken: string | null = null;
    
    if (isNode) {
      console.log('Node.js environment detected, checking for token in environment variables');
      
      // Check process.env if available
      if (typeof process !== 'undefined' && process.env) {
        if (process.env.GITHUB_TOKEN) {
          githubToken = process.env.GITHUB_TOKEN;
          console.log('Found GitHub token in process.env.GITHUB_TOKEN');
        } else if (process.env.GITHUB_PERSONAL_ACCESS_TOKEN) {
          githubToken = process.env.GITHUB_PERSONAL_ACCESS_TOKEN;
          console.log('Found GitHub token in process.env.GITHUB_PERSONAL_ACCESS_TOKEN');
        }
      }
      
      // Check if a token might be in cloudflare environment
      try {
        // @ts-ignore - Cloudflare Workers env might be available
        if (env && env.GITHUB_TOKEN) {
          githubToken = env.GITHUB_TOKEN;
          console.log('Found GitHub token in Cloudflare env.GITHUB_TOKEN');
        }
      } catch (e) {
        // Ignore error when env is not available
      }
      
      if (!githubToken) {
        console.log('No GitHub token found in environment variables');
      }
    } else {
      // In browser environments, use the settings repository
      try {
        const { settingsRepository } = await import('@openagents/core/src/db/repositories');
        githubToken = await settingsRepository.getApiKey('github');
        console.log(`GitHub token fetch result: ${githubToken ? 'Token found' : 'No token found'}`);
      } catch (apiKeyError) {
        console.error('Error fetching GitHub token from settings repository:', apiKeyError);
        return;
      }
    }
    
    if (githubToken) {
      console.log(`Found GitHub token (length: ${githubToken.length}), updating MCP client`);
      await updateMCPGithubToken(githubToken);
      console.log('GitHub token updated successfully');
    } else {
      console.warn('No GitHub token found, MCP GitHub operations will be limited');
      // Clear the cached token
      cachedGithubToken = null;
    }
  } catch (error) {
    console.error('Failed to load GitHub token from settings:', error);
    console.error('Error details:', error instanceof Error ? error.stack : String(error));
  }
}

/**
 * Sets up a window event listener for API key changes
 */
function setupApiKeyChangeListener(): void {
  // Remove existing listener if it exists
  if (tokenChangeHandler) {
    window.removeEventListener('api-key-changed', tokenChangeHandler as EventListener);
  }
  
  // Create new handler
  tokenChangeHandler = async (event: CustomEvent) => {
    try {
      const { provider, deleted } = event.detail;
      
      if (provider === 'github') {
        console.log(`GitHub token ${deleted ? 'deleted' : 'changed'}, updating MCP client`);
        
        if (deleted) {
          // Token was deleted
          await updateMCPGithubToken('');
        } else {
          // Token was updated, load it from settings
          await loadGitHubTokenFromSettings();
        }
      }
    } catch (error) {
      console.error('Error handling API key change event:', error);
    }
  };
  
  // Add the event listener
  window.addEventListener('api-key-changed', tokenChangeHandler as EventListener);
  console.log('Set up API key change listener for GitHub token');
}

/**
 * Initializes GitHub token synchronization with MCP
 * - Loads initial token from settings
 * - Sets up event listeners for token changes (only in browser environment)
 */
export function initMCPGithubTokenSync(): void {
  console.log('=== INITIALIZING MCP GITHUB TOKEN SYNC ===');
  
  // Detect environment
  const isNode = typeof window === 'undefined';
  const isBrowser = !isNode;
  
  console.log(`Environment detected: ${isNode ? 'Node.js' : 'Browser'}`);
  
  // Log MCP clients state before initialization
  try {
    const mcpClients = getMCPClients();
    console.log('Initial MCP clients state:', {
      initialized: mcpClients.initialized,
      clientsCount: Object.keys(mcpClients.clients).length,
      availableClients: Object.keys(mcpClients.clients),
      hasGithubClient: !!mcpClients.clients['local-github']
    });
  } catch (error) {
    console.error('Error getting MCP clients state:', error);
  }
  
  // Load the initial token
  console.log('Loading GitHub token from settings...');
  loadGitHubTokenFromSettings()
    .then(() => {
      console.log('GitHub token successfully loaded from settings');
    })
    .catch(error => {
      console.error('Failed to load GitHub token from settings:', error);
    });
  
  // Set up API key change listener if in browser environment
  if (isBrowser) {
    console.log('Setting up API key change listener for browser environment');
    setupApiKeyChangeListener();
  } else {
    console.log('Skipping API key change listener setup (Node.js environment)');
  }
  
  console.log('=== MCP GITHUB TOKEN SYNC INITIALIZATION COMPLETE ===');
}