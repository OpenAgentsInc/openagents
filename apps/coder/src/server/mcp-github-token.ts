/**
 * MCP GitHub Token Synchronization
 * 
 * This module handles synchronizing GitHub tokens from the API Keys settings
 * to the MCP GitHub client. It ensures that any token changes in the settings
 * are reflected in MCP tool calls.
 */

import { eventBus } from '../lib/events';
import { getSettings } from '../lib/settings';
import { mcpClients } from './mcp-clients';

/**
 * Updates the GitHub token in the MCP GitHub client
 * @param token The GitHub token to set
 */
export async function updateMCPGithubToken(token?: string): Promise<void> {
  try {
    if (!mcpClients.github) {
      console.warn('MCP GitHub client not initialized, cannot update token');
      return;
    }

    // Set the token in the MCP GitHub client environment
    mcpClients.github.env.GITHUB_PERSONAL_ACCESS_TOKEN = token || '';
    console.log('Updated GitHub token in MCP client');
  } catch (error) {
    console.error('Failed to update GitHub token in MCP client:', error);
  }
}

/**
 * Loads the GitHub token from settings and updates the MCP client
 */
export async function loadGitHubTokenFromSettings(): Promise<void> {
  try {
    const settings = await getSettings();
    const token = settings.apiKeys?.github;
    
    if (token) {
      console.log('Found GitHub token in settings, updating MCP client');
      await updateMCPGithubToken(token);
    } else {
      console.log('No GitHub token found in settings');
    }
  } catch (error) {
    console.error('Failed to load GitHub token from settings:', error);
  }
}

/**
 * Initializes GitHub token synchronization with MCP
 * - Loads initial token from settings
 * - Sets up event listeners for token changes
 */
export function initMCPGithubTokenSync(): void {
  // Load the initial token
  loadGitHubTokenFromSettings().catch(console.error);
  
  // Listen for API key changes
  eventBus.on('apiKeys:updated', async (keys) => {
    if (keys && typeof keys === 'object' && 'github' in keys) {
      console.log('GitHub token updated in settings, synchronizing with MCP');
      await updateMCPGithubToken(keys.github as string);
    }
  });
  
  console.log('Initialized MCP GitHub token synchronization');
}