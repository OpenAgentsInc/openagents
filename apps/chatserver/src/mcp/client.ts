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
  private toolRegistry: Map<string, { server: string; description: string, tool: GenericTool }> = new Map();
  private connecting: Map<string, Promise<Client>> = new Map();

  /**
   * Connect to an MCP server with the given URL and name.
   * If already connected, returns the existing client.
   */
  async connectToServer(serverUrl: string, serverName: string): Promise<Client> {
    if (this.clients.has(serverName)) {
      return this.clients.get(serverName)!;
    }
    if (this.connecting.has(serverName)) {
      return this.connecting.get(serverName)!;
    }

    const connectionPromise = this.initiateConnection(serverUrl, serverName);
    this.connecting.set(serverName, connectionPromise);

    try {
      const client = await connectionPromise;
      this.clients.set(serverName, client);
      return client;
    } catch (error) {
      console.error(`🚨 Final error during connection setup for ${serverName}:`, error);
      this.connecting.delete(serverName);
      throw error;
    } finally {
      this.connecting.delete(serverName);
    }
  }

  private async initiateConnection(serverUrl: string, serverName: string): Promise<Client> {
    console.log(`🔌 Connecting to MCP server: ${serverName} at ${serverUrl}`);
    try {
      const transport = new SSEClientTransport(new URL(serverUrl));
      transport.onerror = (error) => {
        console.error(`🚨 Transport error for ${serverName}:`, error);
      };
      transport.onclose = () => {
        console.log(`📡 Transport closed for ${serverName}`);
      };

      console.log(`🏗️ Creating MCP client for ${serverName}`);
      const client = new Client(
        { name: "chatserver", version: "0.0.1" },
        { capabilities: {} }
      );

      console.log(`🔄 Awaiting MCP connection for ${serverName}...`);
      const connectPromise = client.connect(transport);
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(`Connection to ${serverName} timed out`)), 10000)
      );
      await Promise.race([connectPromise, timeoutPromise]);
      console.log(`✅ Connected to MCP server: ${serverName}`);

      await this.discoverTools(client, serverName);

      return client;
    } catch (error) {
      console.error(`🚨 MCP connection/discovery failed for ${serverName}:`, error);
      throw error;
    }
  }

  /**
   * Discover tools provided by an MCP server and register them.
   */
  async discoverTools(client: Client, serverName: string): Promise<void> {
    let tools: GenericTool[] | null = null;
    try {
      console.log(`🔍 Discovering tools from ${serverName}...`);
      const toolsPromise = client.listTools();
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(`Tool discovery from ${serverName} timed out`)), 5000)
      );
      const toolsResponse = await Promise.race([toolsPromise, timeoutPromise]);
      console.log(`📋 Raw tools response from ${serverName}:`, JSON.stringify(toolsResponse).substring(0, 300));

      const extractedTools = (toolsResponse as any)?.tools ?? toolsResponse;

      if (Array.isArray(extractedTools)) {
        tools = extractedTools;
        console.log(`🧰 Found ${tools.length} tools in array format from ${serverName}`);
      } else {
        console.error(`❌ Tools response from ${serverName} is not an array and doesn't contain a 'tools' array. Type: ${typeof extractedTools}`);
        tools = [];
      }
    } catch (error) {
      console.error(`🚨 Failed during tool discovery request for ${serverName}:`, error);
      tools = [];
    }

    console.log(`🔄 Processing ${tools?.length ?? 0} discovered tools for ${serverName}...`);
    if (tools && tools.length > 0) {
      let registeredCount = 0;
      tools.forEach((tool: GenericTool, index: number) => {
        try {
          if (!tool || typeof tool !== 'object' || typeof tool.name !== 'string') {
            console.warn(`⚠️ Skipping tool at index ${index} due to invalid structure or missing name:`, tool);
            return;
          }

          const toolName = tool.name;
          const toolDescription = tool.description || "";

          console.log(`🔧 Registering tool: ${toolName}`);
          this.toolRegistry.set(toolName, {
            server: serverName,
            description: toolDescription,
            tool: tool,
          });
          registeredCount++;

        } catch (registrationError) {
          console.error(`🚨🚨 FAILED TO REGISTER TOOL at index ${index}:`, registrationError);
          console.error(`🚨🚨 Offending Tool Data:`, JSON.stringify(tool).substring(0, 500));
        }
      });
      console.log(`✅ Finished processing tools for ${serverName}. Successfully registered: ${registeredCount}/${tools.length}`);
    } else {
      console.log(`🤷 No valid tools found or processed for ${serverName}.`);
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
  getAllTools(): Array<{ name: string; description: string; server: string; tool: GenericTool }> {
    return Array.from(this.toolRegistry.entries()).map(([name, info]) => ({
      name,
      description: info.description,
      server: info.server,
      tool: info.tool,
    }));
  }

  /**
   * Call a tool with the given arguments and optional authentication token.
   */
  async callTool(toolName: string, args: Record<string, any>, token?: string): Promise<any> {
    const toolInfo = this.toolRegistry.get(toolName);
    if (!toolInfo) {
      throw new Error(`Tool "${toolName}" not found in registry`);
    }

    const client = this.clients.get(toolInfo.server);
    if (!client) {
      throw new Error(`No client found for server ${toolInfo.server}`);
    }

    const callArgs = {
      name: toolName,
      arguments: args,
      ...(token ? { _meta: { token } } : {})
    };

    return client.callTool(callArgs);
  }

  /**
   * Disconnect from all MCP servers.
   */
  async disconnectAll(): Promise<void> {
    for (const [serverName, client] of this.clients.entries()) {
      try {
        await client.close();
        console.log(`Disconnected from ${serverName}`);
      } catch (error) {
        console.error(`Error disconnecting from ${serverName}:`, error);
      }
    }
    this.clients.clear();
    this.toolRegistry.clear();
  }
}

// Singleton instance for the application
export const mcpClientManager = new McpClientManager();
