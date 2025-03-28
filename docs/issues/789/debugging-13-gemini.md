Okay, I have scraped the documentation page (`https://sdk.vercel.ai/docs/reference/ai-sdk-core/stream-text`).

**Findings:**

1.  **Tool Handler Property Name:** The documentation confirms that the correct property for handling server-side tool execution is indeed **`experimental_toolCallHandler`**.
2.  **Signature:** The expected signature for this handler is:
    ```typescript
    experimental_toolCallHandler?: ({
        toolCall: ToolCall; // The tool call details from the model
        submitToolResult: (result: ToolResult | Promise<ToolResult>) => void; // Function to submit result
    }) => Promise<void> | void;
    ```
3.  **`ToolResult` Type:** The object passed to `submitToolResult` needs to match the `ToolResult` type, which essentially requires:
    *   `toolCallId`: The ID of the tool call this result corresponds to.
    *   `result`: The actual data returned by the tool execution (or an error object).
    *   It *can* also include `toolName` and `args`, but these are often optional when submitting the result, as the SDK tracks the call by `toolCallId`.

**Conclusion:**

Your previous TypeScript error (`'onToolCall' does not exist`) was correct because `onToolCall` is not a valid property for the core `streamText` function (it's used in the React hook `useChat`). The switch to `experimental_toolCallHandler` is the right direction based on the documentation.

The fact that you *still* saw a `TypeError: ... (reading 'typeName')` *after* apparently switching to `experimental_toolCallHandler` in the code (based on the agent's description) is puzzling. Let's try applying the code with `experimental_toolCallHandler` one more time, ensuring everything aligns perfectly. It's possible the previous attempt still had a subtle mistake or wasn't deployed correctly.

**Complete Code Files (Using `experimental_toolCallHandler`)**

**`index.ts` (Corrected Tool Handler)**

```typescript
// apps/chatserver/src/index.ts
import { Hono } from 'hono';
import { stream } from 'hono/streaming';
// Import necessary types from 'ai' SDK
import { streamText, type Message, type ToolCall, type ToolResult } from "ai";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
// Re-import functions from tools.ts
import { extractToolDefinitions, processToolCall, type ToolDefinition, type ToolResultPayload } from './mcp/tools'; // Using minimal tools.ts for now

interface Env {
  AI: any; // Not used by OpenRouter, but keep if needed elsewhere
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
void initMcp();

// CORS
app.use('*', cors({
  origin: '*',
  allowMethods: ['POST', 'GET', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Accept', 'Authorization', 'X-GitHub-Token'],
  exposeHeaders: ['X-Vercel-AI-Data-Stream'],
  credentials: true,
}));

// Health check
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

    // --- Tool Extraction ---
    console.log("🔄 Ensuring MCP connection and discovering tools for request...");
    let tools: Record<string, ToolDefinition> = {};
    try {
      await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
      console.log("✅ MCP connection attempt finished for request.");
      tools = extractToolDefinitions(); // Using minimal version from tools.ts
      const toolNames = Object.keys(tools);
      console.log(`✅ Extracted ${toolNames.length} tools for LLM (within request):`, toolNames.join(', '));
      console.log('🔧 Tools object being passed to streamText:', JSON.stringify(tools, null, 2));
      if (toolNames.length === 0) {
        console.warn("⚠️ No tools extracted! Ensure tools.ts is mapping correctly.");
      }
    } catch (mcpError) {
      const errorMsg = "Failed to connect to tool server or extract definitions";
      console.error(`❌ ${errorMsg}:`, mcpError instanceof Error ? mcpError.stack : mcpError);
      c.header('Content-Type', 'text/event-stream; charset=utf-8'); /* ... */
      return stream(c, async (rs) => { /* ... send SSE error ... */ });
    }

    console.log("🎬 Attempting streamText call (WITH MINIMAL TOOL)...");

    // No need to declare streamResult beforehand with experimental_toolCallHandler

    try {
      const hasTools = Object.keys(tools).length > 0;

      const streamResult = streamText({ // Assign directly
        model: openrouter("anthropic/claude-3.5-sonnet"),
        messages: messages,
        tools: hasTools ? tools : undefined,
        toolChoice: hasTools ? 'auto' : undefined,

        // --- Use Correct Handler Name and Signature ---
        experimental_toolCallHandler: async ({ toolCall, submitToolResult }) => {
          console.log(`🤖 Model wants to call tool: ${toolCall.toolName}`);

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
            // Submit the error result using the provided function
            submitToolResult(
              {
                toolCallId: toolCall.toolCallId,
                // toolName, args optional here when submitting
                result: { tool_execution_error: toolResultPayload.result.error }
              } // Type should align with ToolResult
            );
          } else {
            console.log(`✅ Submitting successful tool result for ${toolCall.toolName}`);
            // Submit the successful result using the provided function
            submitToolResult(
              {
                toolCallId: toolCall.toolCallId,
                // toolName, args optional here when submitting
                result: toolResultPayload.result // Actual success result
              } // Type should align with ToolResult
            );
          }
        }, // End of experimental_toolCallHandler

        // Keep other callbacks
        onError: (error: Error) => {
          // Log the error with stack trace if available
          console.error("💥 streamText onError callback:", error?.message, error?.stack);
        },
        onFinish: (event) => {
          console.log(`🏁 streamText onFinish callback. Full event:`, JSON.stringify(event));
        }
      }); // End of streamText call

      console.log(`✅ streamText call initiated successfully (${hasTools ? 'WITH' : 'WITHOUT'} MINIMAL TOOL).`);

      // --- Setup SSE Response ---
      c.header('Content-Type', 'text/event-stream; charset=utf-8');
      c.header('Cache-Control', 'no-cache');
      c.header('Connection', 'keep-alive');
      c.header('X-Vercel-AI-Data-Stream', 'v1');
      console.log("🔄 Preparing to stream response...");

      // --- Stream Handling ---
      // Check streamResult validity before returning stream
       if (!streamResult || typeof streamResult.toDataStream !== 'function') {
         console.error("❌ Invalid streamResult object AFTER streamText call");
         return c.json({ error: "Invalid stream result object" }, 500);
       }

      return stream(c, async (responseStream) => {
          console.log("📬 Entered stream() callback.");
          try {
              const sdkStream = streamResult.toDataStream();
              console.log("🔄 Piping sdkStream from streamResult.toDataStream()...");
              try { await responseStream.pipe(sdkStream); console.log("✅ Piping completed successfully."); }
              catch (pipeError) { console.error("‼️ Error during pipe():", pipeError instanceof Error ? pipeError.stack : pipeError); throw pipeError; }
              console.log("✅ Stream processing apparently complete.");
          } catch (error) {
              const errorMessage = error instanceof Error ? error.message : String(error);
              console.error(`💥 Critical error during stream handling: ${errorMessage}`, error instanceof Error ? error.stack : '');
              try {
                  const detailedErrorMessage = `Stream processing failed: ${errorMessage}`;
                  console.log(`🧪 Attempting to send error: ${detailedErrorMessage}`);
                  await responseStream.write(`data: 3:${JSON.stringify(detailedErrorMessage)}\n\n`);
                  console.log("✅ Wrote error message.");
              } catch (writeError) { console.error("‼️ Failed to write error:", writeError); }
          } finally { console.log("🚪 Exiting stream() callback."); }
      });


    } catch (streamTextSetupError) { // Catch synchronous errors from streamText setup
      console.error("🚨 streamText setup failed:", streamTextSetupError instanceof Error ? streamTextSetupError.stack : streamTextSetupError);
      return c.json({ error: "Failed to initialize AI stream during setup" }, 500);
    }

  } catch (error) { // Catch top-level errors
    console.error("💥 Top-level endpoint error:", error instanceof Error ? error.stack : error);
    return c.json({ error: "Failed to process chat request" }, 500);
  }
});

export default app;
```

**`tools.ts` (Keep Minimal Version)**

Keep `tools.ts` exactly as it was in the previous step (returning only `create_issue` with the absolute minimal `{ type: "object", properties: {} }` schema).

```typescript
// apps/chatserver/src/mcp/tools.ts
import { mcpClientManager } from './client';

export interface ToolParameter {
  type: string;
  description?: string;
}
export interface ToolDefinition {
  name: string;
  description: string;
  parameters: {
    type: 'object';
    properties: Record<string, ToolParameter>;
    required?: string[];
  };
}
export interface ToolCallPayload { /* ... */ }
export interface ToolResultPayload { /* ... */ }

/**
 * Extracts tool definitions.
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
 * Process a tool call
 */
export async function processToolCall(toolCall: ToolCallPayload, authToken?: string): Promise<ToolResultPayload> {
  // --- NO CHANGES NEEDED ---
  /* ... function body ... */
}
```

**`client.ts` (Keep As Is)**

No changes needed to `client.ts` from the previous version.

**Next Steps:**

1.  **Apply Changes:** Replace `index.ts` with the corrected version above. Ensure `tools.ts` is the minimal version.
2.  **Type Check:** Run `yarn tsc --noEmit` (or equivalent). The TS2353 error for `onToolCall` should be gone. If you get *new* type errors related to `experimental_toolCallHandler` or `submitToolResult`, check your installed `ai` package version. `experimental_` features might require a recent version. If necessary, update the `ai` package (`yarn upgrade ai` or `npm update ai`).
3.  **Deploy & Test:** Deploy the code.
4.  **Observe Logs Carefully:**
    *   Do you still get the `TypeError: ... (reading 'typeName')` in the `streamText onError` callback?
    *   If the error is gone, does basic text streaming work?
    *   If you send a prompt designed to use the `create_issue` tool (even with its minimal schema), does the `experimental_toolCallHandler` log "🤖 Model wants to call tool..."?

This focuses on using the documented (albeit experimental) handler name. If the `TypeError` *persists* even with this change and the minimal schema, it further strengthens the case for an SDK/provider bug.
