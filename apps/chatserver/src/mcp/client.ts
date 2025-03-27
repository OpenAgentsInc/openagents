import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { SSEClientTransport } from "@openagents/core";

/**
 * Manages connections to multiple MCP servers and provides a unified interface for tool calls.
 */
export class McpClientManager {
  private clients: Map<string, Client> = new Map();
  private toolRegistry: Map<string, { server: string; description: string }> = new Map();
  private connecting: Map<string, Promise<Client>> = new Map();

  /**
   * Connect to an MCP server with the given URL and name.
   * If already connected, returns the existing client.
   */
  async connectToServer(serverUrl: string, serverName: string): Promise<Client> {
    // If already connected, return existing client
    if (this.clients.has(serverName)) {
      return this.clients.get(serverName)!;
    }

    // If connection in progress, wait for it
    if (this.connecting.has(serverName)) {
      return this.connecting.get(serverName)!;
    }

    // Start new connection
    const connectionPromise = this.initiateConnection(serverUrl, serverName);
    this.connecting.set(serverName, connectionPromise);

    try {
      const client = await connectionPromise;
      this.clients.set(serverName, client);
      return client;
    } finally {
      this.connecting.delete(serverName);
    }
  }

  private async initiateConnection(serverUrl: string, serverName: string): Promise<Client> {
    console.log(`Connecting to MCP server: ${serverName} at ${serverUrl}`);
    
    const transport = new SSEClientTransport(new URL(serverUrl));
    const client = new Client(
      { name: "chatserver", version: "0.0.1" },
      {
        capabilities: {
          sampling: {},
          roots: { listChanged: true },
        },
      }
    );

    // Connect to server
    await client.connect(transport);
    console.log(`Connected to MCP server: ${serverName}`);
    
    // Discover available tools
    await this.discoverTools(client, serverName);
    
    return client;
  }

  /**
   * Discover tools provided by an MCP server and register them.
   */
  async discoverTools(client: Client, serverName: string): Promise<void> {
    try {
      const tools = await client.listTools();
      
      tools.forEach((tool) => {
        this.toolRegistry.set(tool.name, {
          server: serverName,
          description: tool.description || "",
        });
      });
      
      console.log(`Discovered ${tools.length} tools from ${serverName}:`, 
        tools.map(t => t.name).join(", "));
    } catch (error) {
      console.error(`Failed to discover tools from ${serverName}:`, error);
    }
  }

  /**
   * Get the server that provides a given tool.
   */
  getToolServer(toolName: string): string | undefined {
    return this.toolRegistry.get(toolName)?.server;
  }

  /**
   * Get all registered tools with their descriptions.
   */
  getAllTools(): Array<{ name: string; description: string; server: string }> {
    return Array.from(this.toolRegistry.entries()).map(([name, info]) => ({
      name,
      description: info.description,
      server: info.server,
    }));
  }

  /**
   * Call a tool with the given arguments and optional authentication token.
   */
  async callTool(
    toolName: string, 
    args: Record<string, any>, 
    token?: string
  ): Promise<any> {
    const serverName = this.getToolServer(toolName);
    if (!serverName) {
      throw new Error(`Tool ${toolName} not found in any connected MCP server`);
    }

    const client = this.clients.get(serverName);
    if (!client) {
      throw new Error(`MCP server ${serverName} not connected`);
    }

    // Add token to args if provided
    const toolArgs = token ? { ...args, token } : args;
    
    console.log(`Calling tool ${toolName} on server ${serverName}`);
    
    // Call tool with streaming support
    const result = await client.callTool({
      name: toolName,
      arguments: toolArgs,
    });

    // Parse JSON from text response if needed
    if (result.content && result.content.length > 0 && result.content[0].type === "text") {
      try {
        return JSON.parse(result.content[0].text);
      } catch (e) {
        // If not valid JSON, return the text as is
        return result.content[0].text;
      }
    }
    
    return result;
  }
  
  /**
   * Disconnect from all MCP servers.
   */
  async disconnectAll(): Promise<void> {
    const disconnectPromises = Array.from(this.clients.values()).map(client => {
      return client.disconnect().catch(error => {
        console.error("Error disconnecting from MCP server:", error);
      });
    });
    
    await Promise.all(disconnectPromises);
    this.clients.clear();
    this.toolRegistry.clear();
  }
}

// Singleton instance for the application
export const mcpClientManager = new McpClientManager();
