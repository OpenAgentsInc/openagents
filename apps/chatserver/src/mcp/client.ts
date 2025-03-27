import { Client } from "@modelcontextprotocol/sdk/client/index.js";
// Import from package directly
import { SSEClientTransport } from "@modelcontextprotocol/sdk/client/sse.js";

/**
 * Generic tool interface to avoid dependency issues
 */
interface GenericTool {
  name: string;
  description?: string;
  [key: string]: any;
}

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
    console.log(`🔌 Connecting to MCP server: ${serverName} at ${serverUrl}`);
    
    try {
      const transport = new SSEClientTransport(new URL(serverUrl));
      
      // Add event handlers for debugging
      transport.onerror = (error) => {
        console.error(`🚨 MCP Transport error for ${serverName}:`, error);
      };
      
      transport.onclose = () => {
        console.log(`⚠️ MCP Transport closed for ${serverName}`);
        // Remove from clients map to allow reconnect
        this.clients.delete(serverName);
      };
      
      console.log(`🏗️ Creating MCP client for ${serverName}`);
      const client = new Client(
        { name: "chatserver", version: "0.0.1" },
        {
          capabilities: {
            sampling: {},
            roots: { listChanged: true },
          },
        }
      );

      // Connect to server with timeout
      console.log(`🔄 Awaiting MCP connection for ${serverName}...`);
      const connectPromise = client.connect(transport);
      
      // Set a timeout to avoid hanging indefinitely
      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => reject(new Error(`Connection to ${serverName} timed out after 10 seconds`)), 10000);
      });
      
      await Promise.race([connectPromise, timeoutPromise]);
      console.log(`✅ Connected to MCP server: ${serverName}`);
      
      // Discover available tools
      await this.discoverTools(client, serverName);
      
      return client;
    } catch (error) {
      console.error(`🚨 MCP connection failed for ${serverName}:`, error);
      throw error;
    }
  }

  /**
   * Discover tools provided by an MCP server and register them.
   */
  async discoverTools(client: Client, serverName: string): Promise<void> {
    try {
      console.log(`🔍 Discovering tools from ${serverName}...`);
      
      // Set a timeout to avoid hanging indefinitely
      const toolsPromise = client.listTools();
      
      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => reject(new Error(`Tool discovery from ${serverName} timed out after 5 seconds`)), 5000);
      });
      
      const tools = await Promise.race([toolsPromise, timeoutPromise]);
      
      console.log(`📋 Raw tools response:`, JSON.stringify(tools).substring(0, 200));
      
      if (Array.isArray(tools)) {
        console.log(`🧰 Found ${tools.length} tools in array format`);
        
        tools.forEach((tool: GenericTool) => {
          console.log(`🔧 Registering tool: ${tool.name}`);
          this.toolRegistry.set(tool.name, {
            server: serverName,
            description: tool.description || "",
          });
        });
        
        console.log(`✅ Discovered ${tools.length} tools from ${serverName}:`, 
          tools.map((t: GenericTool) => t.name).join(", "));
      } else {
        console.error(`❌ Tools from ${serverName} is not an array:`, typeof tools);
      }
    } catch (error) {
      console.error(`🚨 Failed to discover tools from ${serverName}:`, error);
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
    if (result.content && Array.isArray(result.content) && result.content.length > 0 && 
        typeof result.content[0] === 'object' && result.content[0] !== null && 
        'type' in result.content[0] && result.content[0].type === "text" &&
        'text' in result.content[0]) {
      try {
        const textContent = result.content[0].text as string;
        return JSON.parse(textContent);
      } catch (e) {
        // If not valid JSON, return the text as is
        return (result.content[0] as { text: string }).text;
      }
    }
    
    return result;
  }
  
  /**
   * Disconnect from all MCP servers.
   */
  async disconnectAll(): Promise<void> {
    const disconnectPromises = Array.from(this.clients.values()).map(client => {
      // Use close() instead of disconnect() which doesn't exist
      return client.close().catch((error: Error) => {
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
