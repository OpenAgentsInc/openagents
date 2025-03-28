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
   * Forces a new connection for each request to avoid stale connections in serverless environments.
   */
  async connectToServer(serverUrl: string, serverName: string): Promise<Client> {
    // Get request ID to track connections per request
    const requestId = crypto.randomUUID().substring(0, 8);
    console.log(`🔄 [${requestId}] MCP connection request for ${serverName}`);

    // Check for existing connection in progress
    if (this.connecting.has(serverName)) {
      console.log(`⏳ [${requestId}] Another connection to ${serverName} is in progress, waiting for it`);
      return this.connecting.get(serverName)!;
    }

    // For serverless environments, always create a new connection
    // Clear any existing client to force reconnection
    if (this.clients.has(serverName)) {
      try {
        console.log(`🔄 [${requestId}] Closing existing connection to ${serverName} to create fresh one`);
        const existingClient = this.clients.get(serverName)!;
        await existingClient.close().catch(e => console.warn(`⚠️ [${requestId}] Error closing client:`, e));
        this.clients.delete(serverName);
      } catch (error) {
        console.warn(`⚠️ [${requestId}] Error cleaning up old client:`, error);
        // Continue with reconnection even if cleanup fails
      }
    }

    // Start new connection
    console.log(`🔌 [${requestId}] Initiating fresh connection to ${serverName}`);
    const connectionPromise = this.initiateConnection(serverUrl, serverName, requestId);
    this.connecting.set(serverName, connectionPromise);

    try {
      const client = await connectionPromise;
      this.clients.set(serverName, client);
      console.log(`✅ [${requestId}] Successfully connected to ${serverName}`);
      return client;
    } catch (error) {
      console.error(`🚨 [${requestId}] Connection setup failed for ${serverName}:`, error);
      this.connecting.delete(serverName);
      throw error;
    } finally {
      this.connecting.delete(serverName);
    }
  }

  private async initiateConnection(serverUrl: string, serverName: string, requestId: string): Promise<Client> {
    console.log(`🔌 [${requestId}] Connecting to MCP server: ${serverName} at ${serverUrl}`);
    try {
      // Create new transport with unique connection
      const transport = new SSEClientTransport(new URL(serverUrl));

      // Add event handlers with request ID for better logging
      transport.onerror = (error) => {
        console.error(`🚨 [${requestId}] Transport error for ${serverName}:`, error);
      };
      transport.onclose = () => {
        console.log(`📡 [${requestId}] Transport closed for ${serverName}`);
      };

      console.log(`🏗️ [${requestId}] Creating MCP client for ${serverName}`);
      const client = new Client(
        { name: `chatserver-${requestId}`, version: "0.0.1" }, // Add request ID to client name for tracking
        { capabilities: {} }
      );

      console.log(`🔄 [${requestId}] Awaiting MCP connection for ${serverName}...`);
      const connectPromise = client.connect(transport);
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(`[${requestId}] Connection to ${serverName} timed out`)), 10000)
      );
      await Promise.race([connectPromise, timeoutPromise]);
      console.log(`✅ [${requestId}] Connected to MCP server: ${serverName}`);

      // Discover tools with tracking
      await this.discoverTools(client, serverName, requestId);

      return client;
    } catch (error) {
      console.error(`🚨 [${requestId}] MCP connection/discovery failed for ${serverName}:`, error);
      throw error;
    }
  }

  /**
   * Discover tools provided by an MCP server and register them.
   * @param client The MCP client to use for tool discovery
   * @param serverName The name of the server
   * @param requestId Optional request ID for tracking
   */
  async discoverTools(client: Client, serverName: string, requestId?: string): Promise<void> {
    // Add tracking ID to logs
    const logPrefix = requestId ? `[${requestId}]` : '';
    let tools: GenericTool[] | null = null;

    try {
      console.log(`🔍 ${logPrefix} Discovering tools from ${serverName}...`);

      // Protect against timeout and errors during tool discovery
      const toolsPromise = client.listTools();
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(`${logPrefix} Tool discovery from ${serverName} timed out`)), 5000)
      );

      const toolsResponse = await Promise.race([toolsPromise, timeoutPromise]);
      console.log(`📋 ${logPrefix} Raw tools response from ${serverName}:`, JSON.stringify(toolsResponse).substring(0, 300));

      // Extract tools with better error handling
      const extractedTools = (toolsResponse as any)?.tools ?? toolsResponse;

      if (Array.isArray(extractedTools)) {
        tools = extractedTools;
        console.log(`🧰 ${logPrefix} Found ${tools.length} tools in array format from ${serverName}`);
      } else {
        console.error(`❌ ${logPrefix} Tools response from ${serverName} is not an array and doesn't contain a 'tools' array. Type: ${typeof extractedTools}`);
        tools = [];
      }
    } catch (error) {
      console.error(`🚨 ${logPrefix} Failed during tool discovery request for ${serverName}:`, error);
      tools = [];
    }

    console.log(`🔄 ${logPrefix} Processing ${tools?.length ?? 0} discovered tools for ${serverName}...`);

    // Clear existing tool registry for this server to avoid stale tools
    let existingToolCount = 0;
    for (const [toolName, info] of this.toolRegistry.entries()) {
      if (info.server === serverName) {
        existingToolCount++;
        this.toolRegistry.delete(toolName);
      }
    }
    if (existingToolCount > 0) {
      console.log(`♻️ ${logPrefix} Cleared ${existingToolCount} existing tools for ${serverName}`);
    }

    // Register new tools
    if (tools && tools.length > 0) {
      let registeredCount = 0;
      tools.forEach((tool: GenericTool, index: number) => {
        try {
          if (!tool || typeof tool !== 'object' || typeof tool.name !== 'string') {
            console.warn(`⚠️ ${logPrefix} Skipping tool at index ${index} due to invalid structure or missing name:`, tool);
            return;
          }

          const toolName = tool.name;
          const toolDescription = tool.description || "";

          // console.log(`🔧 ${logPrefix} Registering tool: ${toolName}`);
          this.toolRegistry.set(toolName, {
            server: serverName,
            description: toolDescription,
            tool: tool,
          });
          registeredCount++;

        } catch (registrationError) {
          console.error(`🚨 ${logPrefix} FAILED TO REGISTER TOOL at index ${index}:`, registrationError);
          console.error(`🚨 ${logPrefix} Offending Tool Data:`, JSON.stringify(tool).substring(0, 500));
        }
      });
      console.log(`✅ ${logPrefix} Finished processing tools for ${serverName}. Successfully registered: ${registeredCount}/${tools.length}`);
    } else {
      console.log(`🤷 ${logPrefix} No valid tools found or processed for ${serverName}.`);
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
   * Manages connection state for reliable execution in serverless environments.
   */
  async callTool(toolName: string, args: Record<string, any>, token?: string): Promise<any> {
    // Generate a unique ID for this tool call for tracking
    const callId = crypto.randomUUID().substring(0, 8);
    console.log(`🔄 [${callId}] callTool called for ${toolName} with token present: ${!!token}`);

    // Debug token format/value if present to track auth issues
    if (token) {
      console.log(`🔑 [${callId}] Token format check: Length=${token.length}, Starts with "${token.substring(0, 4)}...", Contains "ghp_": ${token.includes('ghp_')}`);
    } else {
      console.log(`⚠️ [${callId}] No token provided for tool call: ${toolName}`);
    }

    // Get tool info and verify it exists
    const toolInfo = this.toolRegistry.get(toolName);
    if (!toolInfo) {
      console.error(`❌ [${callId}] Tool "${toolName}" not found in registry. Available tools: ${Array.from(this.toolRegistry.keys()).join(", ")}`);
      throw new Error(`Tool "${toolName}" not found in registry`);
    }

    // Get the server name for this tool
    const serverName = toolInfo.server;

    // Check if we have a client for this server, if not try to reconnect
    let client = this.clients.get(serverName);
    if (!client) {
      console.log(`🔄 [${callId}] No client for ${serverName}, attempting to reconnect...`);
      try {
        // Force reconnection to the MCP server
        client = await this.connectToServer(`https://mcp-github.openagents.com/sse`, serverName);
        console.log(`✅ [${callId}] Successfully reconnected to ${serverName}`);
      } catch (reconnectError) {
        console.error(`❌ [${callId}] Failed to reconnect to ${serverName}:`, reconnectError);
        throw new Error(`Could not establish connection to server ${serverName} for tool ${toolName}`);
      }
    } else {
      console.log(`✅ [${callId}] Using existing client for ${serverName}`);
    }

    // Prepare the arguments with token
    const callArgs = {
      name: toolName,
      arguments: args,
      // Always include _meta with requestId even if no token
      _meta: {
        ...(token ? { token } : {}),
        requestId: callId  // Add request ID for tracking in MCP server
      }
    };

    console.log(`📤 [${callId}] Sending tool call to MCP server with args structure:`, Object.keys(callArgs));
    if (token) {
      console.log(`🔐 [${callId}] Token is being passed via _meta.token property`);
    }

    // Execute the tool call with proper timeout and error handling
    try {
      // Add timeout to prevent hanging
      const toolPromise = client.callTool(callArgs);
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(`[${callId}] Tool call ${toolName} timed out after 15 seconds`)), 15000)
      );

      const result = await Promise.race([toolPromise, timeoutPromise]);
      console.log(`✅ [${callId}] MCP tool call ${toolName} succeeded`);
      return result;
    } catch (error) {
      console.error(`❌ [${callId}] MCP tool call ${toolName} error:`, error);

      // Check if it's a connection error and attempt to reconnect once
      if (String(error).includes('connection') || String(error).includes('network')) {
        console.log(`🔄 [${callId}] Attempting one reconnection after connection error...`);
        try {
          // Force close and reconnect
          await this.disconnectServer(serverName);
          client = await this.connectToServer(`https://mcp-github.openagents.com/sse`, serverName);

          // Retry the call once
          console.log(`🔄 [${callId}] Retrying tool call after reconnection...`);
          const result = await client.callTool(callArgs);
          console.log(`✅ [${callId}] Retry successful for ${toolName}`);
          return result;
        } catch (retryError) {
          console.error(`❌ [${callId}] Retry failed:`, retryError);
          throw new Error(`Tool ${toolName} failed after reconnection attempt: ${error}`);
        }
      }

      throw error;
    }
  }

  /**
   * Disconnect from a specific MCP server.
   */
  async disconnectServer(serverName: string): Promise<void> {
    const client = this.clients.get(serverName);
    if (client) {
      try {
        await client.close();
        console.log(`✅ Disconnected from ${serverName}`);
      } catch (error) {
        console.error(`❌ Error disconnecting from ${serverName}:`, error);
      }
      this.clients.delete(serverName);

      // Remove tools for this server
      for (const [toolName, info] of this.toolRegistry.entries()) {
        if (info.server === serverName) {
          this.toolRegistry.delete(toolName);
        }
      }
    }
  }

  /**
   * Disconnect from all MCP servers.
   */
  async disconnectAll(): Promise<void> {
    // In Cloudflare Workers environment, we need to be careful with I/O across requests
    // Instead of actually trying to close connections (which can cause I/O errors),
    // we'll just clear our maps without attempting to close the connections
    
    // Log what we're clearing without trying to perform I/O operations
    const serverNames = Array.from(this.clients.keys());
    if (serverNames.length > 0) {
      console.log(`Clearing connections to: ${serverNames.join(', ')}`);
    }
    
    // Clear maps without attempting to close connections
    this.clients.clear();
    this.toolRegistry.clear();
    console.log(`Cleared all client connections and tool registry`);
  }
  
  /**
   * Clear all client connections and tool registrations
   * This is a public method that can be used to clear the state
   * without attempting to close connections
   */
  clearConnections(): void {
    this.clients.clear();
    this.toolRegistry.clear();
  }
}

// Singleton instance for the application
export const mcpClientManager = new McpClientManager();
