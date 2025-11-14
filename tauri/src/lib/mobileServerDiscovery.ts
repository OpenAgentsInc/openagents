/**
 * Mobile server discovery utilities
 *
 * Handles discovering OpenAgents desktop servers on the local network
 * via mDNS/Bonjour and managing server connections on mobile devices.
 */

import { invoke } from '@tauri-apps/api/core';
import { getDefaultTinyvexWsUrl } from '@/config/acp';

export interface ServerInfo {
  name: string;
  host: string;
  port: number;
  discoveredAt: number;
}

const LAST_SERVER_KEY = 'openagents_last_server';

/**
 * Get the current platform
 */
export async function getPlatform(): Promise<string> {
  return invoke<string>('get_platform');
}

/**
 * Check if running on a mobile platform
 */
export async function isMobile(): Promise<boolean> {
  const platform = await getPlatform();
  return platform === 'ios' || platform === 'android';
}

/**
 * Discover OpenAgents servers on the local network
 *
 * Scans for mDNS services and returns all discovered servers.
 * Only available on mobile platforms.
 */
export async function discoverServers(): Promise<ServerInfo[]> {
  const platform = await getPlatform();

  if (platform !== 'ios' && platform !== 'android') {
    console.warn('Server discovery is only available on mobile platforms');
    return [];
  }

  try {
    const servers = await invoke<ServerInfo[]>('discover_servers');
    console.log(`Discovered ${servers.length} server(s):`, servers);
    return servers;
  } catch (error) {
    console.error('Failed to discover servers:', error);
    throw error;
  }
}

/**
 * Test connection to a server
 */
export async function testServerConnection(host: string, port: number): Promise<boolean> {
  try {
    const result = await invoke<boolean>('test_server_connection', { host, port });
    console.log(`Connection test to ${host}:${port}: ${result ? 'success' : 'failed'}`);
    return result;
  } catch (error) {
    console.error(`Failed to test connection to ${host}:${port}:`, error);
    return false;
  }
}

/**
 * Get the last connected server from storage
 */
export function getLastServer(): ServerInfo | null {
  try {
    const stored = localStorage.getItem(LAST_SERVER_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (error) {
    console.error('Failed to get last server:', error);
  }
  return null;
}

/**
 * Save the last connected server to storage
 */
export function saveLastServer(server: ServerInfo): void {
  try {
    localStorage.setItem(LAST_SERVER_KEY, JSON.stringify(server));
    console.log('Saved last server:', server);
  } catch (error) {
    console.error('Failed to save last server:', error);
  }
}

/**
 * Clear the last connected server from storage
 */
export function clearLastServer(): void {
  try {
    localStorage.removeItem(LAST_SERVER_KEY);
    console.log('Cleared last server');
  } catch (error) {
    console.error('Failed to clear last server:', error);
  }
}

/**
 * Get the WebSocket URL for a server
 */
export function getServerWebSocketUrl(server: ServerInfo): string {
  return `ws://${server.host}:${server.port}/ws`;
}

/**
 * Automatic server discovery and connection flow
 *
 * 1. Check for saved server and try to connect
 * 2. If no saved server or connection fails, scan for servers
 * 3. Return the best server to connect to
 */
export async function autoDiscoverServer(): Promise<ServerInfo | null> {
  // Try saved server first
  const lastServer = getLastServer();
  if (lastServer) {
    console.log('Trying last connected server:', lastServer);
    const isReachable = await testServerConnection(lastServer.host, lastServer.port);
    if (isReachable) {
      console.log('Last server is reachable, using it');
      return lastServer;
    }
    console.log('Last server is not reachable, scanning for servers');
  }

  // Discover servers on the network
  const servers = await discoverServers();

  if (servers.length === 0) {
    console.log('No servers discovered');
    // Simulator-friendly fallback: try connecting to the host loopback.
    // In the iOS Simulator, localhost/127.0.0.1 maps to the macOS host.
    try {
      const wsUrl = await getDefaultTinyvexWsUrl();
      const match = wsUrl.match(/:(\d+)\//);
      const basePort = match ? parseInt(match[1], 10) : 9100;
      const fallbackHost = '127.0.0.1';
      const candidates = Array.from({ length: 16 }, (_, i) => basePort + i);
      for (const port of candidates) {
        const reachable = await testServerConnection(fallbackHost, port);
        if (reachable) {
          const server: ServerInfo = {
            name: 'localhost',
            host: fallbackHost,
            port,
            discoveredAt: Date.now(),
          };
          console.log('Simulator fallback succeeded. Using', server);
          saveLastServer(server);
          return server;
        }
      }
    } catch (e) {
      console.warn('Simulator fallback check failed:', e);
    }
    return null;
  }

  // If any discovered server is reachable, prefer it
  for (const s of servers) {
    const ok = await testServerConnection(s.host, s.port);
    if (ok) {
      console.log('Using reachable discovered server:', s);
      saveLastServer(s);
      return s;
    }
  }

  // Multiple servers found but none reachable — fall back to simulator loopback scan
  try {
    const wsUrl = await getDefaultTinyvexWsUrl();
    const match = wsUrl.match(/:(\d+)\//);
    const basePort = match ? parseInt(match[1], 10) : 9100;
    const fallbackHost = '127.0.0.1';
    const candidates = Array.from({ length: 16 }, (_, i) => basePort + i);
    for (const port of candidates) {
      if (await testServerConnection(fallbackHost, port)) {
        const server: ServerInfo = { name: 'localhost', host: fallbackHost, port, discoveredAt: Date.now() };
        console.log('Discovered servers unreachable; simulator fallback using', server);
        saveLastServer(server);
        return server;
      }
    }
  } catch (e) {
    console.warn('Fallback scan failed:', e);
  }

  // Finally, prefer the most recent (for manual entry flows), otherwise null
  if (servers.length > 0) {
    const mostRecent = servers.reduce((latest, current) =>
      current.discoveredAt > latest.discoveredAt ? current : latest
    );
    console.log('No reachable server; returning most recent for UI:', mostRecent);
    return null; // keep behavior: return null to trigger error/selection UI
  }
  return null;
}
