import { Hono } from 'hono';
import { stream } from 'hono/streaming';
// Import necessary types from 'ai' SDK
import { streamText, type Message, type ToolCall, type ToolResult, type CoreTool } from "ai";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
// Re-import functions from tools.ts
import { extractToolDefinitions, processToolCall, type ToolResultPayload } from './mcp/tools';

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

    // --- Tool Extraction ---
    console.log("🔄 Ensuring MCP connection and discovering tools for request...");
    // Type is now Record<string, CoreTool> when using the 'tool' helper
    let tools: Record<string, CoreTool> = {};
    try {
      await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
      console.log("✅ MCP connection attempt finished for request.");
      tools = extractToolDefinitions(); // Uses new version returning SDK 'tool' objects
      const toolNames = Object.keys(tools);
      console.log(`✅ Extracted ${toolNames.length} tools for LLM (within request):`, toolNames.join(', '));
      // Note: Logging the full 'tools' object might be verbose now as it contains functions/zod schemas
      console.log('🔧 Tools object keys being passed to streamText:', toolNames);
      if (toolNames.length === 0) {
        console.warn("⚠️ No tools extracted! Ensure tools.ts is mapping correctly.");
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

    console.log("🎬 Attempting streamText call (WITH ZOD TOOL)...");

    try {
      const hasTools = Object.keys(tools).length > 0;

      // --- Call streamText, NO handler property ---
      const streamResult = streamText({
        model: openrouter("anthropic/claude-3.5-sonnet"),
        messages: messages,
        tools: hasTools ? tools : undefined, // Pass the tools defined with the 'tool' helper
        toolChoice: hasTools ? 'auto' : undefined,

        // NO 'experimental_toolCallHandler' or 'onToolCall' INPUT PROPERTY needed here

        // Standard callbacks
        onError: (event: { error: unknown }) => {
          console.error("💥 streamText onError callback:", 
            event.error instanceof Error ? event.error.message : String(event.error));
        },
        onFinish: (event) => { console.log(`🏁 streamText onFinish. Full event:`, JSON.stringify(event)); }
      });
      // --- End streamText Call ---

      console.log(`✅ streamText call initiated successfully (WITH ZOD TOOL).`);

      // --- *** HANDLE TOOL CALLS VIA TOOLCALLS PROMISE *** ---
      if (hasTools) {
          // Monitor tool calls via the toolCalls property (Promise)
          (async () => {
            try {
              console.log("👂 Waiting for tool calls from the model...");
              const toolCallsArray = await streamResult.toolCalls;
              console.log(`📞 Received ${toolCallsArray.length} tool calls from model`);
              
              for (const toolCallItem of toolCallsArray) {
                // Handle each tool call
                console.log(`🔧 Processing tool call: ${toolCallItem.toolName}`);
                
                // Call your MCP processing function
                const toolResultPayload: ToolResultPayload = await processToolCall(
                  {
                    toolCallId: toolCallItem.toolCallId,
                    toolName: toolCallItem.toolName,
                    args: toolCallItem.args,
                  },
                  authToken
                );
                
                // Check for functional error from MCP/processToolCall
                if (toolResultPayload?.result?.error) {
                  console.error(`❌ MCP tool call ${toolCallItem.toolName} error:`, toolResultPayload.result.error);
                  // Note: In newer versions, we can't directly submit tool results this way
                  // The SDK handles this internally via the streams
                  console.log(`⚠️ Unable to submit tool result for ${toolCallItem.toolName} - API limitation`);
                } else {
                  console.log(`✅ Tool call ${toolCallItem.toolName} processed successfully`);
                  // Results are handled by the stream automatically
                }
              }
            } catch (toolCallError) {
              console.error("❌ Error handling tool calls:", toolCallError);
            }
          })();
      }
      // --- *** END TOOL CALLS HANDLING *** ---

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
  } catch (error) {
    console.error("💥 Chat endpoint error:", error instanceof Error ? error.stack : error);
    return c.json({ error: "Failed to process chat request" }, 500);
  }
});

export default app;