Okay, thanks for providing `client.ts` and `tools.ts`. This clarifies things significantly.

**Analysis of `client.ts` (`McpClientManager`)**

1.  **Connection Management:** Looks solid. It handles concurrent connection attempts (`connecting` map), uses SSE transport, includes basic timeouts for connection and discovery, and attempts to re-register tools upon connection. Error handling for transport/connection failures is present.
2.  **Tool Discovery:** The `discoverTools` function correctly handles the `{ tools: [...] }` response format issue found during debugging. It registers discovered tools with their origin server (`serverName`).
3.  **Tool Calling (`callTool`):**
    *   Finds the correct server based on the registered tool.
    *   **Authentication:** It injects the `token` directly into the `arguments` passed to the MCP server (`toolArgs = token ? { ...args, token } : args;`). This is okay *if* the `mcp-github-server` is specifically designed to look for the token within the arguments object. It's not the typical header-based auth pattern but can work if consistently implemented.
    *   Handles timeouts.
    *   Includes logic to parse the result if it's a text block containing JSON, otherwise returns the text. This is a reasonable heuristic.
4.  **Singleton:** Using a singleton (`mcpClientManager`) is convenient for access within the worker.

**Analysis of `tools.ts`**

1.  **`extractToolDefinitions` - THE MAJOR ISSUE:**
    *   As suspected, **this function completely ignores the dynamically discovered tools** from `mcpClientManager.getAllTools()`.
    *   It returns a **hardcoded list** of predefined GitHub tool schemas (`create_issue`, `get_file_contents`, etc.).
    *   **Consequences:**
        *   The LLM is *only* told about these 5 hardcoded tools, regardless of what the `mcp-github-server` actually offers via discovery.
        *   This explains the `tools as any` cast in `index.ts` – the hardcoded structure matches the Vercel AI SDK format, but it's disconnected from reality.
        *   If the MCP server adds/removes/changes tools, the chat server won't know unless this hardcoded list is manually updated.
        *   The parameter schemas are also hardcoded and might drift from the actual MCP server implementation.
2.  **`processToolCall`:**
    *   This function *correctly* uses `mcpClientManager.getToolServer` (which relies on *dynamic discovery*) to find out where to send the call.
    *   It then calls `mcpClientManager.callTool`, passing the `authToken`.
    *   It handles timeouts and formats the response/error into `ToolResultPayload`.
    *   **Inconsistency:** This function relies on dynamic discovery working correctly, while `extractToolDefinitions` completely bypasses it. This means the LLM might be asked to call tools defined in the hardcoded list, but the execution path relies on those tools *also* having been dynamically discovered and registered correctly by `McpClientManager`.

**Vercel AI SDK Tool Handling (`streamText`)**

Yes, the Vercel AI SDK (`@ai-sdk/react` and the core `ai` package) is absolutely designed to handle this flow much more cleanly than the current manual stream interception.

Key features:

1.  **`tools` parameter:** You pass the tool definitions (like your hardcoded ones, but ideally dynamically generated) to `streamText`.
2.  **`onToolCall` callback:** `streamText` emits this event when the LLM decides to call a tool. It provides the parsed `toolCall` object ({ toolCallId, toolName, args }).
3.  **`submitToolResult` / `submitToolError`:** After executing the tool (e.g., inside `onToolCall`), you call these methods on the `result` object returned by `streamText`. The SDK then automatically formats the result (`data: a:...`) or error (`data: 3:...`) and injects it back into the stream for the LLM and the client.
4.  **`toDataStream()`:** This method on the `result` object produces the correctly formatted SSE stream, including text, tool calls/results, errors, etc., ready to be piped to the client.

**Proposed Refactoring (Leveraging Vercel AI SDK)**

This eliminates the need for `interceptStream` and manual SSE formatting in `index.ts`.

**Step 1: Fix `extractToolDefinitions` in `tools.ts`**

This is the most critical change. You need to translate the dynamically discovered tools into the format `streamText` expects.

```typescript
// apps/chatserver/src/mcp/tools.ts

// ... existing interfaces ...

/**
 * Extracts tool definitions from all connected MCP servers
 * in a format compatible with LLM tool definitions (Vercel AI SDK).
 *
 * NOTE: This requires mapping MCP tool schemas to JSON Schema.
 * This is a simplified example assuming MCP provides basic schema info.
 * You may need a more robust mapping function based on MCP server specifics.
 */
export function extractToolDefinitions(): ToolDefinition[] {
  const discoveredTools = mcpClientManager.getAllTools();
  console.log(`[extractToolDefinitions] Discovered ${discoveredTools.length} tools from MCP Manager.`);

  return discoveredTools.map(mcpTool => {
    console.log(`[extractToolDefinitions] Mapping tool: ${mcpTool.name}`);

    // --- !!! CRITICAL MAPPING LOGIC !!! ---
    // This is a placeholder. You MUST implement a proper mapping
    // from the actual MCP tool schema provided by your MCP servers
    // to the JSON Schema format expected by the Vercel AI SDK.
    // Assuming mcpTool might have a 'parameters' property with some schema info.
    const parameters: ToolDefinition['parameters'] = {
      type: "object",
      properties: mcpTool.parameters?.properties || {}, // Example: Map MCP params here
      required: mcpTool.parameters?.required || [],     // Example: Map required params here
    };
    // --- End Critical Mapping Logic ---

    // Add basic validation logging
    if (!parameters.properties || typeof parameters.properties !== 'object') {
        console.warn(`[extractToolDefinitions] Tool ${mcpTool.name} has invalid or missing properties definition. Using empty object.`);
        parameters.properties = {};
    }
     if (!Array.isArray(parameters.required)) {
        console.warn(`[extractToolDefinitions] Tool ${mcpTool.name} has invalid or missing required definition. Using empty array.`);
        parameters.required = [];
    }


    return {
      name: mcpTool.name,
      description: mcpTool.description || `Execute the ${mcpTool.name} tool.`, // Provide default if missing
      parameters: parameters,
    };
  }).filter(tool => {
      // Basic validation: Ensure required fields exist
      const isValid = !!tool.name && !!tool.description && !!tool.parameters;
      if (!isValid) {
          console.warn(`[extractToolDefinitions] Filtering out invalid tool definition:`, tool);
      }
      return isValid;
  }); // Filter out any tools that failed mapping
}

// ... processToolCall remains largely the same ...
// Modify processToolCall slightly to handle potential errors during the call more gracefully
export async function processToolCall(toolCall: ToolCallPayload, authToken?: string): Promise<ToolResultPayload> {
  // Keep existing logic, but ensure the return type is always ToolResultPayload
  // even on error, matching the structure expected for tool results/errors.
  if (!toolCall) {
    console.log("⚠️ Received null tool call");
    // Return an error structure consistent with success case
    return {
        toolCallId: 'unknown', // Or generate one if possible
        toolName: 'unknown',
        args: {},
        result: { error: 'Received null tool call data' }
    };
  }

  console.log(`🔧 Processing tool call: ${toolCall.toolName}`);
  // ... (rest of the existing logging) ...

  const toolServer = mcpClientManager.getToolServer(toolCall.toolName);

  if (!toolServer) {
    // ... (existing error logging) ...
    return {
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      args: toolCall.args,
      // Ensure result field exists, containing the error details
      result: { error: `Tool "${toolCall.toolName}" not found in any connected MCP server` }
    };
  }

  // ... (rest of the routing log) ...

  try {
    // ... (existing timeout logic) ...
    const result = await Promise.race([toolPromise, timeoutPromise]);
    console.log(`✅ Tool call successful: ${JSON.stringify(result).substring(0, 200)}`);
    return {
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      args: toolCall.args,
      result // The actual result from MCP call
    };
  } catch (error) {
    console.error(`❌ Error processing tool call ${toolCall.toolName}:`, error);
    return {
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      args: toolCall.args,
      // Ensure result field exists, containing the error details
      result: { error: error instanceof Error ? error.message : 'Unknown error during tool execution' }
    };
  }
}

```

**Step 2: Refactor `index.ts`**

```typescript
// apps/chatserver/src/index.ts
import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { streamText, type ToolCall, type ToolResult } from "ai"; // Import Tool types
import { createWorkersAI } from "workers-ai-provider";
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
// Import ToolDefinition from tools.ts, processToolCall stays
import { extractToolDefinitions, processToolCall, type ToolDefinition, type ToolResultPayload } from './mcp/tools';

interface Env {
  AI: typeof AI;
}

const app = new Hono<{ Bindings: Env }>();

// MCP Initialization (keep as is)
async function initMcp() {
  try {
    // Ensure connection attempt happens, discovery happens within connectToServer/initiateConnection
    await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
    console.log('Attempted connection to GitHub MCP server during init.');
     // Log discovered tools after connection attempt
    const initialTools = mcpClientManager.getAllTools();
    console.log(`[initMcp] Initially discovered ${initialTools.length} tools.`);
  } catch (error) {
    console.error('[initMcp] Failed initial connection attempt to GitHub MCP server:', error);
  }
}
void initMcp(); // Non-blocking

// CORS (keep as is)
app.use('*', cors({
  origin: '*',
  allowMethods: ['POST', 'GET', 'OPTIONS'],
  // Ensure Authorization is allowed if using Bearer tokens
  allowHeaders: ['Content-Type', 'Accept', 'Authorization', 'X-GitHub-Token', 'X-Jira-Token'],
  exposeHeaders: ['X-Vercel-AI-Data-Stream'],
  credentials: true,
}));

// Health check (keep as is)
app.get('/', c => c.text('200 OK'));

// Main chat endpoint - REFACTORED
app.post('/', async c => {
  console.log("🚀 Chat request received");

  try {
    const body = await c.req.json();
    console.log("📝 Request body:", JSON.stringify(body).substring(0, 200));

    const messages = body.messages || [];
    if (messages.length === 0) {
        return c.json({ error: "No messages provided" }, 400);
    }
    console.log(`📨 Message count: ${messages.length}`);

    // Extract auth token (prefer Authorization: Bearer first)
    const bearerToken = c.req.header('Authorization')?.replace('Bearer ', '');
    const githubTokenHeader = c.req.header('X-GitHub-Token');
    // Use Bearer token if present, otherwise fallback to specific header for GitHub
    const authToken = bearerToken || githubTokenHeader;
    console.log(`🔑 Auth token present: ${!!authToken}`);

    // Get AI model
    const workersai = createWorkersAI({ binding: c.env.AI });

    // --- Get Dynamically Mapped Tool Definitions ---
    console.log("🛠️ Extracting tool definitions...");
    // Ensure MCP connection is likely ready before extracting - wait briefly or add readiness check
    // Optional: Add a short delay or readiness check if initMcp might not be done
    // await new Promise(resolve => setTimeout(resolve, 100)); // Example delay, better: check client status
    let tools: ToolDefinition[] = [];
    try {
        // Force connection attempt again here to be sure, if not using readiness checks
        await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
        tools = extractToolDefinitions(); // Now uses the dynamic version
        console.log(`✅ Extracted ${tools.length} tools for LLM:`, tools.map(t => t.name).join(', '));
         if (tools.length === 0) {
            console.warn("⚠️ No tools were extracted for the LLM. Tool usage might fail.");
        }
    } catch (toolError) {
         console.error("❌ Failed to extract tools:", toolError);
         // Decide: Continue without tools or return error?
         // Continuing without tools for now.
    }
    // ---------------------------------------------

    console.log("🎬 Starting streamText with tools...");

    // Use streamText with built-in tool handling
    const result = streamText({
      model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
      messages,
      // Pass the dynamically generated tools (no 'as any' needed if types match)
      tools: tools.length > 0 ? tools : undefined, // Pass undefined if no tools extracted
      // Enable tool usage
      toolChoice: tools.length > 0 ? 'auto' : undefined, // Only enable tool choice if tools exist

      // --- Use onToolCall Callback ---
      onToolCall: async ({ toolCall }: { toolCall: ToolCall }) => {
        console.log(` LLaMA decided to call tool: ${toolCall.toolName}`);

        // Use the existing processToolCall function
        const toolResultPayload: ToolResultPayload = await processToolCall(
          {
            toolCallId: toolCall.toolCallId,
            toolName: toolCall.toolName,
            args: toolCall.args,
          },
          authToken // Pass the extracted auth token
        );

        // Check if the result indicates an error from processToolCall/MCP
        if (toolResultPayload.result && typeof toolResultPayload.result === 'object' && 'error' in toolResultPayload.result) {
            console.error(` Tool call ${toolCall.toolName} resulted in error:`, toolResultPayload.result.error);
             // Submit the error back to the stream
            result.submitToolError(
                {
                    toolCallId: toolCall.toolCallId,
                    toolName: toolCall.toolName, // Include toolName if available/useful
                    args: toolCall.args,
                    error: toolResultPayload.result.error // Pass the error message/details
                }
            );
        } else {
            console.log(` Submitting tool result for ${toolCall.toolName}`);
            // Submit the successful result back to the stream
            // The SDK expects just the result content here.
            result.submitToolResult(
                 {
                    toolCallId: toolCall.toolCallId,
                    toolName: toolCall.toolName, // Include toolName if available/useful
                    args: toolCall.args,
                    result: toolResultPayload.result // The actual result data
                }
            );
        }
      },
      // --- End onToolCall Callback ---

      // Optional: Add other callbacks like onFinish, onError
       onError: (error: Error) => {
            console.error("💥 streamText error:", error);
            // This might be where you handle fundamental model/SDK errors
        },
        onFinish: ({ finishReason, usage, logprobs, text }) => {
            console.log(`🏁 streamText finished. Reason: ${finishReason}, Usage:`, usage);
            if(text) console.log(`📝 Final text: ${text.substring(0,100)}...`);
        }
    });

    // --- Simplified Streaming ---
    // Set SSE headers (keep these)
    c.header('X-Vercel-AI-Data-Stream', 'v1');
    c.header('Content-Type', 'text/event-stream; charset=utf-8');
    c.header('Cache-Control', 'no-cache');
    c.header('Connection', 'keep-alive');

    // Pipe the SDK's data stream directly to the response
    console.log(" Piping SDK stream to response...");
    return stream(c, async (responseStream) => {
         // Optional: Write an initial SSE comment to keep connection alive or indicate start
         // await responseStream.write(': stream starting\n\n');
         await responseStream.pipe(result.toDataStream());
         // Optional: Write a final SSE comment after piping finishes
         // await responseStream.write(': stream finished\n\n');
         console.log("✅ Stream piping complete.");
    });
    // --- End Simplified Streaming ---

  } catch (error) {
    console.error("💥 Critical chat endpoint error:", error);
    // Ensure a standard JSON error response for non-streaming errors
    return c.json({ error: error instanceof Error ? error.message : "Failed to process chat request" }, 500);
  }
});

export default app;

```

**Explanation of Changes:**

1.  **`tools.ts`:**
    *   `extractToolDefinitions` now *must* be implemented to dynamically map discovered MCP tools to the Vercel AI SDK's JSON Schema format. The hardcoded list is removed, replaced with mapping logic (placeholder provided). Added filtering for safety.
    *   `processToolCall` is slightly adjusted to consistently return the `ToolResultPayload` structure, embedding errors within the `result` field for easier handling in `onToolCall`.
2.  **`index.ts`:**
    *   Removed the entire `interceptStream` function and the manual loop within the `stream()` callback.
    *   Calls the *new* `extractToolDefinitions` to get dynamic tools. Includes basic error handling/logging around tool extraction.
    *   Passes the dynamic `tools` to `streamText`.
    *   Added the `onToolCall` callback to `streamText`. This is where the logic now resides:
        *   It receives the `toolCall` from the SDK.
        *   It calls `processToolCall` (which handles MCP communication via `mcpClientManager`).
        *   It uses `result.submitToolResult()` or `result.submitToolError()` to send the outcome back via the SDK, letting the SDK handle the SSE formatting (`data: a:...` or `data: 3:...`).
    *   The final `return stream(...)` now simply pipes `result.toDataStream()` directly to the response. The SDK prepares this stream with all the correctly formatted SSE chunks.
    *   Improved auth token extraction to prefer `Authorization: Bearer`.
    *   Added basic check for empty messages array.
    *   Conditionally enable `toolChoice` and pass `tools` only if tools were successfully extracted.

**Next Steps:**

1.  **Implement Tool Schema Mapping:** The most crucial part is correctly implementing the mapping logic within `extractToolDefinitions` in `tools.ts`. You need to know the exact schema structure provided by your `mcp-github-server` via `client.listTools()` and translate it into JSON Schema for the `parameters` field in `ToolDefinition`.
2.  **Test Thoroughly:** Verify that tools are correctly discovered, presented to the LLM, called via `onToolCall`, executed via MCP, and results/errors are streamed back correctly to the client UI. Check the browser's network tab to see the raw SSE stream.
3.  **Refine MCP Connection Readiness:** Add a more robust check in `index.ts` before calling `extractToolDefinitions` to ensure the `mcpClientManager` has successfully connected and discovered tools, perhaps using a status flag or a dedicated readiness promise in `McpClientManager`.

This refactoring significantly simplifies `index.ts`, reduces the risk of SSE formatting errors, and correctly integrates dynamic tool discovery with the Vercel AI SDK's intended tool-handling mechanisms.
