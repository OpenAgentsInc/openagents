same error


2025-03-27 20:01:49:273
UTC
POST https://chat.openagents.com/
2025-03-27 20:01:49:273
UTC
🚀 Chat request received
2025-03-27 20:01:49:273
UTC
✅ OPENROUTER_API_KEY seems present.
2025-03-27 20:01:49:273
UTC
📝 Request body (preview): {"id":"5jwMLLlbpG1ZP0Sb","messages":[{"role":"user","content":"test","parts":[{"type":"text","text":"test"}]},{"role":"user","content":"test","parts":[{"type":"text","text":"test"}]}]}
2025-03-27 20:01:49:273
UTC
📨 Using message array:
2025-03-27 20:01:49:273
UTC
🔑 Auth token present: false
2025-03-27 20:01:49:273
UTC
✅ OpenRouter provider initialized.
2025-03-27 20:01:49:273
UTC
🔄 Ensuring MCP connection and discovering tools for request...
2025-03-27 20:01:49:273
UTC
✅ MCP connection attempt finished for request.
2025-03-27 20:01:49:273
UTC
[extractToolDefinitions] Received 26 tool infos from MCP Manager.
2025-03-27 20:01:49:273
UTC
[extractToolDefinitions] Mapping ABSOLUTE MINIMAL tool: create_issue
2025-03-27 20:01:49:273
UTC
[extractToolDefinitions] Added ABSOLUTE MINIMAL schema for create_issue: { "name": "create_issue", "description": "Executes the create_issue tool.", "parameters": { "type": "object", "properties": {} } }
2025-03-27 20:01:49:273
UTC
[extractToolDefinitions] Finished mapping 1 tools with ABSOLUTE MINIMAL schema.
2025-03-27 20:01:49:273
UTC
✅ Extracted 1 tools for LLM (within request): create_issue
2025-03-27 20:01:49:273
UTC
🔧 Tools object being passed to streamText: { "create_issue": { "name": "create_issue", "description": "Executes the create_issue tool.", "parameters": { "type": "object", "properties": {} } } }
2025-03-27 20:01:49:273
UTC
🎬 Attempting streamText call (WITH TOOLS)...
2025-03-27 20:01:49:273
UTC
✅ streamText call initiated successfully (WITH TOOLS).
2025-03-27 20:01:49:273
UTC
🔄 Preparing to stream response...
2025-03-27 20:01:49:273
UTC
📬 Entered stream() callback.
2025-03-27 20:01:49:273
UTC
🔄 Piping sdkStream from streamResult.toDataStream()...
2025-03-27 20:01:49:273
UTC
💥 streamText onError callback: TypeError: Cannot read properties of undefined (reading 'typeName')
2025-03-27 20:01:49:273
UTC
✅ Piping completed successfully.
2025-03-27 20:01:49:273
UTC
✅ Stream processing apparently complete (after pipe).
2025-03-27 20:01:49:273
UTC
🚪 Exiting stream() callback.



with

index.ts
import { Hono } from 'hono';
import { stream } from 'hono/streaming';
// Import necessary types from 'ai' SDK
import { streamText, type Message, type ToolCall, type ToolResult } from "ai";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
// Re-import functions from tools.ts
import { extractToolDefinitions, processToolCall, type ToolDefinition, type ToolResultPayload } from './mcp/tools';

interface Env {
  AI: any; // Keep 'any' for now
  OPENROUTER_API_KEY: string;
}

const app = new Hono<{ Bindings: Env }>();

// Initialize MCP connections
async function initMcp() {
  try {
    await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
    console.log('[initMcp] Initial connection attempt finished.');
    const initialTools = mcpClientManager.getAllTools();
    console.log(`[initMcp] Tools immediately after initial connect attempt: ${initialTools.length}`);
  } catch (error) {
    console.error('[initMcp] Failed initial connection attempt:', error);
  }
}

// Call in a non-blocking way to avoid delaying server startup
void initMcp();

// Enable CORS for all routes
app.use('*', cors({
  origin: '*',
  allowMethods: ['POST', 'GET', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Accept', 'Authorization', 'X-GitHub-Token'],
  exposeHeaders: ['X-Vercel-AI-Data-Stream'],
  credentials: true,
}));

// Health check endpoint
app.get('/', c => c.text('200 OK - Chat Server Running'));

// Main chat endpoint
app.post('/', async c => {
  console.log("🚀 Chat request received");

  // --- Basic Binding Check ---
  if (!c.env.OPENROUTER_API_KEY) {
    console.error("❌ OPENROUTER_API_KEY binding is missing");
    return c.json({ error: "OpenRouter API Key not configured" }, 500);
  }
  console.log("✅ OPENROUTER_API_KEY seems present.");

  try {
    const body = await c.req.json();
    console.log("📝 Request body (preview):", JSON.stringify(body)?.substring(0, 300));

    // --- Validate Input Messages ---
    let messages: Message[] = body.messages || [];
    if (!Array.isArray(messages) || messages.length === 0) {
      return c.json({ error: "No valid messages array provided" }, 400);
    }
    if (!messages.every(m => m && typeof m.role === 'string' && typeof m.content === 'string')) {
      return c.json({ error: "Invalid message format" }, 400);
    }
    console.log(`📨 Using message array:`, messages);

    // --- Auth Token Extraction ---
    const bearerToken = c.req.header('Authorization')?.replace('Bearer ', '');
    const githubTokenHeader = c.req.header('X-GitHub-Token');
    const authToken = bearerToken || githubTokenHeader;
    console.log(`🔑 Auth token present: ${!!authToken}`);

    // --- Initialize OpenRouter ---
    const openrouter = createOpenRouter({ apiKey: c.env.OPENROUTER_API_KEY });
    console.log("✅ OpenRouter provider initialized.");

    // --- Re-enable Tool Extraction ---
    console.log("🔄 Ensuring MCP connection and discovering tools for request...");
    let tools: Record<string, ToolDefinition> = {};
    try {
      await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
      console.log("✅ MCP connection attempt finished for request.");

      tools = extractToolDefinitions();

      const toolNames = Object.keys(tools);
      console.log(`✅ Extracted ${toolNames.length} tools for LLM (within request):`, toolNames.join(', '));
      console.log('🔧 Tools object being passed to streamText:', JSON.stringify(tools, null, 2));

      if (toolNames.length === 0) {
        console.warn("⚠️ No tools extracted after ensuring connection! Proceeding without tools.");
      }
    } catch (mcpError) {
      const errorMsg = "Failed to connect to tool server or extract definitions";
      console.error(`❌ ${errorMsg}:`, mcpError instanceof Error ? mcpError.stack : mcpError);

      c.header('Content-Type', 'text/event-stream; charset=utf-8');
      c.header('Cache-Control', 'no-cache');
      c.header('Connection', 'keep-alive');
      c.header('X-Vercel-AI-Data-Stream', 'v1');
      return stream(c, async (responseStream) => {
        console.log(`🧪 Sending SSE error due to MCP connection/extraction failure.`);
        try {
          await responseStream.write(`data: 3:${JSON.stringify(`${errorMsg}: ${mcpError instanceof Error ? mcpError.message : String(mcpError)}`)}\n\n`);
        } catch (writeError) {
          console.error("‼️ Failed to write early error message to stream:", writeError);
        }
      });
    }

    console.log("🎬 Attempting streamText call (WITH TOOLS)...");

    // Declare streamResult beforehand so it's accessible in onToolCall
    let streamResult: ReturnType<typeof streamText> | undefined = undefined;

    try {
      const hasTools = Object.keys(tools).length > 0;

      streamResult = streamText({
        model: openrouter("anthropic/claude-3.5-sonnet"),
        messages: messages,
        tools: hasTools ? tools : undefined,
        toolChoice: hasTools ? 'auto' : undefined,

        onToolCall: async ({ toolCall }: { toolCall: ToolCall<string, Record<string, any>> }) => {
          console.log(`🤖 Model wants to call tool: ${toolCall.toolName}`);

          // Ensure streamResult is defined before using it
          if (!streamResult) {
            console.error("🚨 FATAL: streamResult is undefined inside onToolCall!");
            return;
          }

          // Call your MCP processing function
          const toolResultPayload: ToolResultPayload = await processToolCall(
            {
              toolCallId: toolCall.toolCallId,
              toolName: toolCall.toolName,
              args: toolCall.args,
            },
            authToken
          );

          // Check for functional error from MCP/processToolCall
          if (toolResultPayload?.result?.error) {
            console.error(`❌ MCP tool call ${toolCall.toolName} resulted in error:`, toolResultPayload.result.error);
            (streamResult as any).submitToolResult({
              toolCallId: toolCall.toolCallId,
              toolName: toolCall.toolName,
              args: toolCall.args,
              result: { tool_execution_error: toolResultPayload.result.error }
            });
          } else {
            console.log(`✅ Submitting successful tool result for ${toolCall.toolName}`);
            (streamResult as any).submitToolResult({
              toolCallId: toolCall.toolCallId,
              toolName: toolCall.toolName,
              args: toolCall.args,
              result: toolResultPayload.result
            });
          }
        },

        onError: (event: { error: unknown }) => {
          const error = event.error;
          console.error("💥 streamText onError callback:", error);
        },
        onFinish: (event) => {
          console.log(`🏁 streamText onFinish callback. Full event:`, JSON.stringify(event));
        }
      });

      console.log(`✅ streamText call initiated successfully (${hasTools ? 'WITH' : 'WITHOUT'} TOOLS).`);
    } catch (streamTextSetupError) {
      console.error("🚨 streamText setup failed:", streamTextSetupError);
      return c.json({ error: "Failed to initialize AI stream" }, 500);
    }

    // Check streamResult validity
    if (!streamResult || typeof streamResult.toDataStream !== 'function') {
      console.error("❌ Invalid streamResult object");
      return c.json({ error: "Invalid stream result" }, 500);
    }

    // Set SSE headers
    c.header('Content-Type', 'text/event-stream; charset=utf-8');
    c.header('Cache-Control', 'no-cache');
    c.header('Connection', 'keep-alive');
    c.header('X-Vercel-AI-Data-Stream', 'v1');
    console.log("🔄 Preparing to stream response...");

    return stream(c, async responseStream => {
      console.log("📬 Entered stream() callback.");
      try {
        const sdkStream = streamResult.toDataStream();
        console.log("🔄 Piping sdkStream from streamResult.toDataStream()...");
        try {
          await responseStream.pipe(sdkStream);
          console.log("✅ Piping completed successfully.");
        } catch (pipeError) {
          console.error("‼️ Error occurred during responseStream.pipe():", pipeError instanceof Error ? pipeError.stack : pipeError);
          throw pipeError;
        }
        console.log("✅ Stream processing apparently complete (after pipe).");
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        console.error(`💥 Critical error during stream handling: ${errorMessage}`, error instanceof Error ? error.stack : '');
        try {
          const detailedErrorMessage = `Stream processing failed: ${errorMessage}`;
          console.log(`🧪 Attempting to send error to client: ${detailedErrorMessage}`);
          await responseStream.write(`data: 3:${JSON.stringify(detailedErrorMessage)}\n\n`);
          console.log("✅ Wrote error message to stream.");
        } catch (writeError) {
          console.error("‼️ Failed to write error message to stream:", writeError instanceof Error ? writeError.stack : writeError);
        }
      } finally {
        console.log("🚪 Exiting stream() callback.");
      }
    });
  } catch (error) {
    console.error("💥 Chat endpoint error:", error instanceof Error ? error.stack : error);
    return c.json({ error: "Failed to process chat request" }, 500);
  }
});

export default app;


client:

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


tools:

import { mcpClientManager } from './client';

export interface ToolParameter {
  type: string;
  description?: string; // Allow optional description
}

export interface ToolDefinition {
  name: string;
  description: string;
  parameters: {
    type: 'object'; // Must be object for properties
    properties: Record<string, ToolParameter>;
    required?: string[]; // Allow optional required array
  };
}

export interface ToolCallPayload {
  toolCallId: string;
  toolName: string;
  args: Record<string, any>;
}

export interface ToolResultPayload {
  toolCallId: string;
  toolName: string;
  args: Record<string, any>;
  result: any;
}

/**
 * Extracts tool definitions from all connected MCP servers
 * in a format compatible with LLM tool definitions (Vercel AI SDK).
 * FOR DEBUGGING: Returns only 'create_issue' with ABSOLUTE MINIMAL schema.
 */
export function extractToolDefinitions(): Record<string, ToolDefinition> {
  const discoveredToolInfos = mcpClientManager.getAllTools();
  console.log(`[extractToolDefinitions] Received ${discoveredToolInfos.length} tool infos from MCP Manager.`);

  const toolDefinitions: Record<string, ToolDefinition> = {};
  const singleToolName = "create_issue"; // Focus on this tool

  // Find the specific tool info
  const toolInfo = discoveredToolInfos.find(info => info.tool?.name === singleToolName);

  if (toolInfo && toolInfo.tool) {
    const mcpTool = toolInfo.tool;
    const toolName = mcpTool.name;
    console.log(`[extractToolDefinitions] Mapping ABSOLUTE MINIMAL tool: ${toolName}`);

    // --- ABSOLUTE MINIMAL SCHEMA ---
    const minimalParameters: ToolDefinition['parameters'] = {
        type: "object",
        properties: {}, // NO PROPERTIES
        // required: [] // OMITTING required array entirely for maximum simplicity
    };
    // --- END MINIMAL SCHEMA ---

    const toolDescription = mcpTool.description || `Executes the ${toolName} tool.`; // Ensure description

    toolDefinitions[toolName] = {
      name: toolName,
      description: toolDescription,
      parameters: minimalParameters
    };

    console.log(`[extractToolDefinitions] Added ABSOLUTE MINIMAL schema for ${toolName}:`,
      JSON.stringify(toolDefinitions[toolName], null, 2));

  } else {
      console.warn(`[extractToolDefinitions] Tool '${singleToolName}' not found among discovered tools.`);
  }

  console.log(`[extractToolDefinitions] Finished mapping ${Object.keys(toolDefinitions).length} tools with ABSOLUTE MINIMAL schema.`);
  return toolDefinitions;
}

/**
 * Process a tool call by routing it to the appropriate MCP server.
 * @param toolCall The tool call from the LLM
 * @param authToken Optional authentication token to pass to the MCP server
 * @returns The tool result payload with result or error
 */
export async function processToolCall(toolCall: ToolCallPayload, authToken?: string): Promise<ToolResultPayload> {
  if (!toolCall) {
    console.log("⚠️ Received null tool call");
    return {
      toolCallId: 'unknown',
      toolName: 'unknown',
      args: {},
      result: { error: 'Received null tool call data' }
    };
  }

  console.log(`🔧 Processing tool call: ${toolCall.toolName}`);
  console.log(`📦 Tool args: ${JSON.stringify(toolCall.args).substring(0, 200)}`);
  console.log(`🔑 Auth token present: ${!!authToken}`);

  const toolServer = mcpClientManager.getToolServer(toolCall.toolName);

  if (!toolServer) {
    console.error(`❌ Unknown tool: ${toolCall.toolName}`);
    const allTools = mcpClientManager.getAllTools();
    console.log(`🧰 Available tools: ${allTools.map(t => t.name).join(', ') || 'none'}`);

    return {
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      args: toolCall.args,
      result: { error: `Tool "${toolCall.toolName}" not found in any connected MCP server` }
    };
  }

  console.log(`🔄 Routing tool call to server: ${toolServer}`);

  try {
    const toolPromise = mcpClientManager.callTool(
      toolCall.toolName,
      toolCall.args,
      authToken
    );

    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error(`Tool call ${toolCall.toolName} timed out after 15 seconds`)), 15000);
    });

    const result = await Promise.race([toolPromise, timeoutPromise]);

    console.log(`✅ Tool call successful: ${JSON.stringify(result).substring(0, 200)}`);

    return {
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      args: toolCall.args,
      result
    };
  } catch (error) {
    console.error(`❌ Error processing tool call ${toolCall.toolName}:`, error);

    return {
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      args: toolCall.args,
      result: { error: error instanceof Error ? error.message : 'Unknown error' }
    };
  }
}
