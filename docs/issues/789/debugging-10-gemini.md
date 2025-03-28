Okay, the logs stop abruptly during the tool registration phase within the `McpClientManager`. This strongly suggests an unhandled error is occurring while processing one of the specific tools being discovered (likely `search_users`, `list_commits`, or `create_branch`, since the logs stop after `search_issues`).

When this error occurs within the `await mcpClientManager.connectToServer(...)` call in `index.ts`, the promise rejects, and the `catch (mcpError)` block in `index.ts` is executed:

```typescript
    } catch (mcpError) {
      console.error("❌ Failed to ensure MCP connection or extract tools:", mcpError);
      // THIS LINE SENDS A JSON RESPONSE, NOT AN SSE STREAM
      return c.json({ error: "Failed to connect to tool server or extract definitions" }, 500);
    }
```

Sending a standard JSON error response (`application/json`) when the client expects an SSE stream (`text/event-stream`) causes the client-side Vercel AI SDK parser to fail, resulting in the generic "Chat error: Error: An error occurred."

**Plan:**

1.  **Fix Error Handling in `client.ts`:** Add robust `try...catch` inside the `discoverTools` loop to pinpoint which tool causes the error and log it, preventing the entire process from crashing silently.
2.  **Fix Error Response in `index.ts`:** Modify the `catch (mcpError)` block to return a *proper SSE error stream*, consistent with how other errors are handled later, so the client receives a more specific message if this early failure occurs.

**1. Revised `client.ts` (`McpClientManager`)**

Add `try...catch` around the tool registration logic within `discoverTools`.

```typescript
// apps/chatserver/src/mcp/client.ts
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { SSEClientTransport } from "@modelcontextprotocol/sdk/client/sse.js";

interface GenericTool {
  name: string;
  description?: string;
  [key: string]: any;
}

export class McpClientManager {
  private clients: Map<string, Client> = new Map();
  private toolRegistry: Map<string, { server: string; description: string, tool: GenericTool }> = new Map(); // Store full tool
  private connecting: Map<string, Promise<Client>> = new Map();

  async connectToServer(serverUrl: string, serverName: string): Promise<Client> {
    if (this.clients.has(serverName)) {
      // Optionally re-discover tools even if connected? For now, return existing.
      // console.log(`Client for ${serverName} already exists. Re-running discovery...`);
      // await this.discoverTools(this.clients.get(serverName)!, serverName);
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
        // Log the error that caused the connection initiation to fail
        console.error(`🚨 Final error during connection setup for ${serverName}:`, error);
        // Remove from connecting map on failure
        this.connecting.delete(serverName);
        // Re-throw the error to be caught by the caller in index.ts
        throw error;
    } finally {
      // Remove from connecting map (whether success or specific failure handled above)
      this.connecting.delete(serverName);
    }
  }

  private async initiateConnection(serverUrl: string, serverName: string): Promise<Client> {
    console.log(`🔌 Connecting to MCP server: ${serverName} at ${serverUrl}`);
    try {
      const transport = new SSEClientTransport(new URL(serverUrl));
      transport.onerror = (error) => { /* ... */ };
      transport.onclose = () => { /* ... */ };

      console.log(`🏗️ Creating MCP client for ${serverName}`);
      const client = new Client(
        { name: "chatserver", version: "0.0.1" },
        { capabilities: { /* ... */ } }
      );

      console.log(`🔄 Awaiting MCP connection for ${serverName}...`);
      const connectPromise = client.connect(transport);
      const timeoutPromise = new Promise<never>((_, reject) => setTimeout(() => reject(new Error(`Connection to ${serverName} timed out`)), 10000));
      await Promise.race([connectPromise, timeoutPromise]);
      console.log(`✅ Connected to MCP server: ${serverName}`);

      // Discover tools *after* successful connection
      await this.discoverTools(client, serverName); // discoverTools now handles its own errors better

      return client;
    } catch (error) {
      console.error(`🚨 MCP connection/discovery failed for ${serverName}:`, error);
      // Ensure the error is propagated up
      throw error;
    }
  }

  async discoverTools(client: Client, serverName: string): Promise<void> {
    let tools: GenericTool[] | null = null; // Initialize as null
    try {
      console.log(`🔍 Discovering tools from ${serverName}...`);
      const toolsPromise = client.listTools();
      const timeoutPromise = new Promise<never>((_, reject) => setTimeout(() => reject(new Error(`Tool discovery from ${serverName} timed out`)), 5000));
      const toolsResponse = await Promise.race([toolsPromise, timeoutPromise]);
      console.log(`📋 Raw tools response from ${serverName}:`, JSON.stringify(toolsResponse).substring(0, 300));

      // Use optional chaining and nullish coalescing for safety
      const extractedTools = (toolsResponse as any)?.tools ?? toolsResponse;

      if (Array.isArray(extractedTools)) {
        tools = extractedTools; // Assign if valid array
        console.log(`🧰 Found ${tools.length} tools in array format from ${serverName}`);
      } else {
        console.error(`❌ Tools response from ${serverName} is not an array and doesn't contain a 'tools' array. Type: ${typeof extractedTools}`);
        tools = []; // Treat as empty if format is wrong
      }
    } catch (error) {
      console.error(`🚨 Failed during tool discovery request for ${serverName}:`, error);
      // Don't throw here, allow process to continue with potentially zero tools found if needed
      tools = []; // Treat as empty on discovery error
    }

    // --- Safely Process Discovered Tools ---
    console.log(`🔄 Processing ${tools?.length ?? 0} discovered tools for ${serverName}...`);
    if (tools && tools.length > 0) {
        let registeredCount = 0;
        tools.forEach((tool: GenericTool, index: number) => {
            try {
                // Validate basic tool structure
                if (!tool || typeof tool !== 'object' || typeof tool.name !== 'string') {
                    console.warn(`⚠️ Skipping tool at index ${index} due to invalid structure or missing name:`, tool);
                    return; // Skip this iteration
                }

                const toolName = tool.name;
                const toolDescription = tool.description || ""; // Default description if missing

                console.log(`🔧 Registering tool: ${toolName}`);
                this.toolRegistry.set(toolName, {
                    server: serverName,
                    description: toolDescription,
                    tool: tool, // Store the full tool object
                });
                registeredCount++;

            } catch (registrationError) {
                // Log the specific error and the tool that caused it
                console.error(`🚨🚨 FAILED TO REGISTER TOOL at index ${index}:`, registrationError);
                console.error(`🚨🚨 Offending Tool Data:`, JSON.stringify(tool).substring(0, 500));
                // Continue processing other tools
            }
        });
        console.log(`✅ Finished processing tools for ${serverName}. Successfully registered: ${registeredCount}/${tools.length}`);
    } else {
        console.log(`🤷 No valid tools found or processed for ${serverName}.`);
    }
    // --- End Safe Processing ---
  }


  getToolServer(toolName: string): string | undefined {
    return this.toolRegistry.get(toolName)?.server;
  }

  // Modify getAllTools to retrieve the full tool object if needed by extractToolDefinitions
  getAllTools(): Array<{ name: string; description: string; server: string; tool: GenericTool }> {
    return Array.from(this.toolRegistry.entries()).map(([name, info]) => ({
      name,
      description: info.description,
      server: info.server,
      tool: info.tool, // Pass the full tool object
    }));
  }

  async callTool(toolName: string, args: Record<string, any>, token?: string): Promise<any> {
      /* ... no changes needed ... */
  }

  async disconnectAll(): Promise<void> {
      /* ... no changes needed ... */
  }
}

export const mcpClientManager = new McpClientManager();
```

**2. Revised `index.ts` (Return SSE Error on Early Failure)**

Modify the `catch (mcpError)` block to return a `stream` that sends an SSE error message.

```typescript
// apps/chatserver/src/index.ts
import { Hono } from 'hono';
import { stream } from 'hono/streaming'; // Import stream helper
import { streamText, type Message, type ToolCall, type ToolResult } from "ai";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
import { extractToolDefinitions, processToolCall, type ToolDefinition, type ToolResultPayload } from './mcp/tools';

interface Env {
  AI: any;
  OPENROUTER_API_KEY: string;
}

const app = new Hono<{ Bindings: Env }>();

// Keep MCP Init, CORS, Health check
async function initMcp() { /* ... no changes ... */ }
void initMcp();
app.use('*', cors({ /* ... no changes ... */ }));
app.get('/', c => c.text('200 OK - Chat Server Running'));

app.post('/', async c => {
  console.log("🚀 Chat request received");

  if (!c.env.OPENROUTER_API_KEY) { /* ... */ }
  console.log("✅ OPENROUTER_API_KEY seems present.");

  try {
    const body = await c.req.json();
    console.log("📝 Request body (preview):", JSON.stringify(body)?.substring(0, 300));

    let messages: Message[] = body.messages || [];
    if (!Array.isArray(messages) || messages.length === 0) { /* ... */ }
    if (!messages.every(m => m && typeof m.role === 'string' && typeof m.content === 'string')) { /* ... */ }
    console.log(`📨 Using message array:`, messages);

    const bearerToken = c.req.header('Authorization')?.replace('Bearer ', '');
    const githubTokenHeader = c.req.header('X-GitHub-Token');
    const authToken = bearerToken || githubTokenHeader;
    console.log(`🔑 Auth token present: ${!!authToken}`);

    const openrouter = createOpenRouter({ apiKey: c.env.OPENROUTER_API_KEY });
    console.log("✅ OpenRouter provider initialized.");

    // --- Ensure MCP Connection and Tool Discovery WITHIN Request ---
    console.log("🔄 Ensuring MCP connection and discovering tools for request...");
    let tools: Record<string, ToolDefinition> = {};
    try {
      // Explicitly connect and wait. Errors during discovery inside connectToServer
      // should now be logged but might not prevent completion if handled in discoverTools.
      await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
      console.log("✅ MCP connection attempt finished for request."); // Renamed log

      // Extract tools AFTER connection attempt. Relies on toolRegistry state.
      tools = extractToolDefinitions(); // Assumes tools.ts tries to map all tools again

      const toolNames = Object.keys(tools);
      console.log(`✅ Extracted ${toolNames.length} tools for LLM (within request):`, toolNames.join(', '));
      console.log('🔧 Tools object being passed to streamText:', JSON.stringify(tools, null, 2));

      if (toolNames.length === 0) {
        // Log as warning, but proceed. streamText will handle 'tools: undefined'.
        console.warn("⚠️ No tools extracted after ensuring connection! Proceeding without tools.");
        // NOTE: If tools are ESSENTIAL, you might want to throw an error here instead.
        // throw new Error("Failed to load required tools from MCP server");
      }
    } catch (mcpError) {
      // This catches errors from connectToServer if it re-throws them,
      // OR errors thrown explicitly if tool loading is considered fatal.
      const errorMsg = "Failed to connect to tool server or extract definitions";
      console.error(`❌ ${errorMsg}:`, mcpError instanceof Error ? mcpError.stack : mcpError);

      // --- *** RETURN SSE ERROR STREAM *** ---
      c.header('Content-Type', 'text/event-stream; charset=utf-8');
      c.header('Cache-Control', 'no-cache');
      c.header('Connection', 'keep-alive');
      c.header('X-Vercel-AI-Data-Stream', 'v1'); // Send Vercel header even for error
      return stream(c, async (responseStream) => {
          console.log(`🧪 Sending SSE error due to MCP connection/extraction failure.`);
          try {
              await responseStream.write(`data: 3:${JSON.stringify(`${errorMsg}: ${mcpError instanceof Error ? mcpError.message : String(mcpError)}`)}\n\n`);
          } catch (writeError) {
               console.error("‼️ Failed to write early error message to stream:", writeError);
          }
      });
      // --- *** END SSE ERROR STREAM *** ---
    }
    // --- End Ensure MCP ---

    console.log("🎬 Attempting streamText call (WITH TOOLS)..."); // Log reflects trying with tools

    let streamResult: ReturnType<typeof streamText> | undefined = undefined;

    try {
      const hasTools = Object.keys(tools).length > 0;

      streamResult = streamText({
        model: openrouter("anthropic/claude-3.5-sonnet"),
        messages: messages,
        tools: hasTools ? tools : undefined, // Pass potentially empty but valid 'tools' object or undefined
        toolChoice: hasTools ? 'auto' : undefined,

        onToolCall: async ({ toolCall }: { toolCall: ToolCall<string, Record<string, any>> }) => {
          /* ... existing onToolCall logic ... */
        },
        onError: (error: Error) => { /* ... */ },
        onFinish: (event) => { /* ... */ }
      });

      console.log(`✅ streamText call initiated successfully (${hasTools ? 'WITH' : 'WITHOUT'} TOOLS).`);

    } catch (streamTextSetupError) { /* ... error handling ... */ }

    if (!streamResult || typeof streamResult.toDataStream !== 'function') { /* ... error handling ... */ }

    // --- Setup SSE Response (keep as is) ---
    c.header('Content-Type', 'text/event-stream; charset=utf-8'); /* ... other headers ... */
    console.log("🔄 Preparing to stream response...");

    // --- Stream Handling (keep as is) ---
    return stream(c, async (responseStream) => { /* ... no changes ... */ });

  } catch (error) { // Catch top-level errors
     /* ... error handling ... */
  }
});

export default app;
```

**3. Revised `tools.ts` (Ensure it uses updated `getAllTools`)**

Make sure `extractToolDefinitions` uses the `tool` property now returned by `getAllTools` if your mapping logic relies on the full tool object from discovery.

```typescript
// apps/chatserver/src/mcp/tools.ts
import { mcpClientManager } from './mcp/client';

// Interfaces (keep as is)
export interface ToolParameter { /* ... */ }
export interface ToolDefinition { /* ... */ }
export interface ToolCallPayload { /* ... */ }
export interface ToolResultPayload { /* ... */ }

/**
 * Extracts tool definitions from all connected MCP servers
 * in a format compatible with LLM tool definitions (Vercel AI SDK).
 * Attempts to map discovered MCP tool schemas.
 */
export function extractToolDefinitions(): Record<string, ToolDefinition> {
  // This call now happens *after* connectToServer in the request handler
  // Use the updated getAllTools which includes the full 'tool' object
  const discoveredToolInfos = mcpClientManager.getAllTools();
  console.log(`[extractToolDefinitions] Received ${discoveredToolInfos.length} tool infos from MCP Manager.`);

  const toolDefinitions: Record<string, ToolDefinition> = {};

  if (discoveredToolInfos.length === 0) {
    console.warn("[extractToolDefinitions] No tool infos returned from mcpClientManager.getAllTools().");
    return {};
  }

  discoveredToolInfos.forEach(toolInfo => {
    const mcpTool = toolInfo.tool; // Get the full tool object

    // Basic validation on the tool object itself
    if (!mcpTool || typeof mcpTool !== 'object' || typeof mcpTool.name !== 'string') {
      console.warn(`[extractToolDefinitions] Skipping tool info due to invalid/missing underlying tool object or name:`, toolInfo);
      return;
    }
    const toolName = mcpTool.name;
    console.log(`[extractToolDefinitions] Attempting to map tool: ${toolName}`);

    // --- Schema Mapping Logic (Using mcpTool) ---
    let parameters: ToolDefinition['parameters'] = {
      type: "object",
      properties: {},
      required: [],
    };

    try {
        // Access inputSchema from the mcpTool object
        const inputSchema = (mcpTool as any).inputSchema;

        if (inputSchema && typeof inputSchema === 'object') {
             if (inputSchema.type === 'object' && inputSchema.properties && typeof inputSchema.properties === 'object') {
                 parameters.properties = inputSchema.properties as Record<string, ToolParameter>;
                 for (const key in parameters.properties) {
                     parameters.properties[key].description = parameters.properties[key].description || `Parameter ${key}`;
                 }
             } else {
                 console.warn(`[extractToolDefinitions] Tool '${toolName}' has inputSchema but not the expected object/properties structure. Using empty parameters.`);
             }
             if (Array.isArray(inputSchema.required)) {
                 parameters.required = inputSchema.required.filter((req: any) => typeof req === 'string');
             }
        } else {
             console.warn(`[extractToolDefinitions] Tool '${toolName}' has no valid inputSchema found. Using empty parameters.`);
        }
    } catch (mapError) {
         console.error(`[extractToolDefinitions] Error mapping schema for tool '${toolName}':`, mapError);
         parameters = { type: "object", properties: {}, required: [] };
    }
    // --- End Schema Mapping Logic ---

    const toolDescription = mcpTool.description || `Executes the ${toolName} tool.`;

    toolDefinitions[toolName] = {
      name: toolName,
      description: toolDescription,
      parameters: parameters
    };
  });

  console.log(`[extractToolDefinitions] Finished mapping ${Object.keys(toolDefinitions).length} tools.`);
  return toolDefinitions;
}

// processToolCall function remains the same
export async function processToolCall(toolCall: ToolCallPayload, authToken?: string): Promise<ToolResultPayload> {
  /* ... no changes needed ... */
}
```

**Summary of Changes:**

*   **`client.ts`**: Added detailed `try...catch` within the `discoverTools` loop to prevent crashes and log errors for specific tools. Stores the full tool object. Modified `getAllTools` to return it.
*   **`index.ts`**: Moved `await mcpClientManager.connectToServer` inside the request handler's `try` block. Changed the `catch (mcpError)` block to return an SSE error stream instead of a JSON response. Added logic to proceed without tools if extraction yields zero but connection succeeded.
*   **`tools.ts`**: Updated `extractToolDefinitions` to use the full `tool` object provided by the revised `getAllTools`. Kept the general schema mapping logic (which you still might need to refine based on your *actual* `inputSchema` structure).

**Testing:**

1.  Deploy all 3 updated files (`client.ts`, `index.ts`, `tools.ts`).
2.  Test with `curl` or your app.
3.  **Observe Logs:**
    *   Does `discoverTools` now log any errors for specific tools (`🚨🚨 FAILED TO REGISTER TOOL...`)? If so, which one and what's the error?
    *   Does the request handler now successfully log `✅ Extracted X tools...` with X > 0?
    *   If extraction fails but `connectToServer` succeeds, does the code proceed without tools (check for `⚠️ No tools extracted... Proceeding without tools.`) and does text generation work?
    *   If `connectToServer` *itself* fails (e.g., timeout, transport error), does the client now receive a specific SSE error message (e.g., `data: 3:"Failed to connect to tool server..."`) instead of the generic "An error occurred."?
