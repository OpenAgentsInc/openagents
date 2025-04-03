// apps/coder/src/server/mcp-clients.ts
import { experimental_createMCPClient } from "ai";
import { Experimental_StdioMCPTransport as StdioMCPTransport } from 'ai/mcp-stdio';

// Define the type for the MCP clients
interface MCPClients {
  remoteMcpClient: Awaited<ReturnType<typeof experimental_createMCPClient>> | null;
  localGithubMcpClient: Awaited<ReturnType<typeof experimental_createMCPClient>> | null;
  localShellMcpClient: Awaited<ReturnType<typeof experimental_createMCPClient>> | null;
  allTools: Record<string, any>; // Combined tools from all clients
  initialized: boolean;
}

// Global state for MCP clients
const mcpClients: MCPClients = {
  remoteMcpClient: null,
  localGithubMcpClient: null,
  localShellMcpClient: null,
  allTools: {},
  initialized: false,
};

/**
 * Initialize MCP clients once when the server starts
 */
export async function initMCPClients(): Promise<void> {
  console.log('[MCP Clients] Skipping MCP client initialization');
  return;

  if (mcpClients.initialized) {
    console.log('[MCP Clients] Already initialized, skipping');
    return;
  }

  console.log('[MCP Clients] Initializing MCP clients...');

  // GitHub MCP URL
  const GITHUB_MCP_URL = "https://mcp-github.openagents.com/sse";

  // Try to initialize remote MCP client
  try {
    console.log('[MCP Clients] Initializing remote GitHub MCP client');
    mcpClients.remoteMcpClient = await experimental_createMCPClient({
      transport: {
        type: 'sse',
        url: GITHUB_MCP_URL,
      },
    });
    console.log('[MCP Clients] Remote MCP client initialized successfully');
  } catch (remoteMcpError) {
    console.error('[MCP Clients] Failed to initialize remote MCP client:', remoteMcpError);
    // Continue execution - we'll still use local MCP or LLM without tools
  }

  // Try to initialize local GitHub MCP client with stdio transport
  try {
    console.log('[MCP Clients] Initializing local GitHub MCP client with stdio');

    const transport = new StdioMCPTransport({
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-github'],
      env: {
        // Use GitHub token from environment if available
        GITHUB_PERSONAL_ACCESS_TOKEN: process.env.GITHUB_TOKEN || '<TOKEN_REQUIRED>'
      }
    });

    mcpClients.localGithubMcpClient = await experimental_createMCPClient({
      transport
    });

    console.log('[MCP Clients] Local GitHub stdio MCP client initialized successfully');
  } catch (localMcpError) {
    console.error('[MCP Clients] Failed to initialize local GitHub MCP client:', localMcpError);
    // Continue execution - we'll still use remote MCP or LLM without tools
  }

  // Try to initialize local Shell MCP client with stdio transport
  try {
    console.log('[MCP Clients] Initializing local Shell MCP client with stdio');

    // Define allowed commands, falling back to a minimal set if not configured
    const allowCommands = process.env.ALLOW_COMMANDS || 'ls,cat,pwd,echo,grep,find,ps,wc';

    const transportShell = new StdioMCPTransport({
      command: 'uvx',
      args: ['mcp-shell-server'],
      env: {
        // Configure allowed commands for security
        ALLOW_COMMANDS: allowCommands
      }
    });

    mcpClients.localShellMcpClient = await experimental_createMCPClient({
      transport: transportShell
    });

    console.log('[MCP Clients] Local Shell stdio MCP client initialized successfully');
  } catch (shellMcpError) {
    console.error('[MCP Clients] Failed to initialize local Shell MCP client:', shellMcpError);
    // Continue execution - we'll still use other MCP clients or LLM without tools
  }

  // Fetch and store tools from all available clients
  await refreshTools();

  mcpClients.initialized = true;
  console.log('[MCP Clients] Initialization complete');
}

/**
 * Get the initialized MCP clients
 */
export function getMCPClients(): MCPClients {
  return mcpClients;
}

/**
 * Refresh tools from all initialized MCP clients
 */
export async function refreshTools(): Promise<Record<string, any>> {
  const tools: Record<string, any> = {};

  // Add GitHub tools if available
  if (mcpClients.localGithubMcpClient) {
    try {
      console.log('[MCP Clients] Fetching tools from local GitHub MCP client');
      const githubTools = await mcpClients.localGithubMcpClient.tools();
      Object.assign(tools, githubTools);
      console.log('[MCP Clients] Successfully fetched tools from local GitHub MCP');
    } catch (toolError) {
      console.error('[MCP Clients] Error fetching tools from local GitHub MCP:', toolError);
    }
  } else if (mcpClients.remoteMcpClient) {
    try {
      console.log('[MCP Clients] Fetching tools from remote MCP client');
      const remoteTools = await mcpClients.remoteMcpClient.tools();
      Object.assign(tools, remoteTools);
      console.log('[MCP Clients] Successfully fetched tools from remote MCP');
    } catch (toolError) {
      console.error('[MCP Clients] Error fetching tools from remote MCP:', toolError);
    }
  }

  // Add Shell tools if available
  if (mcpClients.localShellMcpClient) {
    try {
      console.log('[MCP Clients] Fetching tools from local Shell MCP client');
      const shellTools = await mcpClients.localShellMcpClient.tools();
      Object.assign(tools, shellTools);
      console.log('[MCP Clients] Successfully fetched tools from local Shell MCP');
    } catch (toolError) {
      console.error('[MCP Clients] Error fetching tools from local Shell MCP:', toolError);
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

  // Reset the global state - this will allow the garbage collector
  // to clean up the clients and their transports
  mcpClients.remoteMcpClient = null;
  mcpClients.localGithubMcpClient = null;
  mcpClients.localShellMcpClient = null;
  mcpClients.allTools = {};
  mcpClients.initialized = false;

  console.log('[MCP Clients] Cleanup complete');
}
