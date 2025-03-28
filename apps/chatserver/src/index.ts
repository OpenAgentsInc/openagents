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
  
  // Enhanced error detection for API key issues
  // Check if API key follows expected format (sk-or-...)
  const apiKey = c.env.OPENROUTER_API_KEY;
  if (!apiKey.startsWith('sk-or-')) {
    console.warn(`⚠️ OPENROUTER_API_KEY format appears incorrect: ${apiKey.substring(0, 6)}... (expected 'sk-or-')`);
  }
  
  // For temporary testing, check if a fallback key is available in case the env binding is invalid
  const fallbackKey = "sk-or-replace-with-valid-key-for-testing"; // Replace with actual key for testing
  if (fallbackKey.startsWith('sk-or-') && fallbackKey !== "sk-or-replace-with-valid-key-for-testing") {
    console.log("🔄 Using fallback key for testing instead of environment variable");
    c.env.OPENROUTER_API_KEY = fallbackKey;
  }

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
    // Verify API key format and add additional logging
    const apiKey = c.env.OPENROUTER_API_KEY;
    const apiKeyFormat = apiKey?.startsWith('sk-or-') ? 'correct format (sk-or-...)' : 
                        apiKey?.startsWith('sk-') ? 'possibly incorrect format (sk-...)' :
                        'unknown format';
    console.log(`🔑 API key format: ${apiKeyFormat}`);
    console.log(`🔑 API key length: ${apiKey?.length || 0}`);
    
    // Create the OpenRouter client with API key
    const openrouter = createOpenRouter({ 
      apiKey: apiKey,
      // Add baseURL explicitly to ensure connection is properly established
      baseURL: "https://openrouter.ai/api/v1"
    });
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

    console.log("🎬 Attempting streamText call (WITH GITHUB MCP TOOLS)...");

    try {
      const hasTools = Object.keys(tools).length > 0;
      
      if (hasTools) {
        console.log(`🧰 Making ${Object.keys(tools).length} GitHub tools available to the model`);
      } else {
        console.warn("⚠️ No GitHub tools available for this request");
      }

      // Log the API key existence and first few characters for troubleshooting
      const apiKeyStatus = c.env.OPENROUTER_API_KEY ? 
        `present (starts with: ${c.env.OPENROUTER_API_KEY.substring(0, 4)}...)` : 
        'missing';
      console.log(`🔑 OpenRouter API key status: ${apiKeyStatus}`);
      
      // --- Call streamText with tool configuration ---
      console.log(`🔄 Creating streamText config with API key: ${c.env.OPENROUTER_API_KEY.substring(0, 8)}...`);
      // Try using an older/simpler model first as a test
      // instead of "anthropic/claude-3.5-sonnet" try "openai/gpt-3.5-turbo-0125"
      const MODEL = "openai/gpt-3.5-turbo-0125"; // More reliable for testing
      console.log(`📝 Using model: ${MODEL}`);

      const streamResult = streamText({
        model: openrouter(MODEL),
        messages: messages,
        // Temporarily disable tools for testing the OpenRouter connection
        // tools: hasTools ? tools : undefined,
        // toolChoice: hasTools ? 'auto' : undefined,
        tools: undefined,
        toolChoice: undefined,
        temperature: 0.7,
        
        toolCallStreaming: false, // Disable for basic testing
        
        // Pass auth token via headers for tool execution
        // Force proper Authorization format for OpenRouter
        headers: {
          'Authorization': `Bearer ${c.env.OPENROUTER_API_KEY}`,
          'HTTP-Referer': 'https://openagents.com',
          'X-Title': 'OpenAgents Chat',
          'X-GitHub-Token': githubTokenHeader,
        },

        // Standard callbacks
        onError: (event: { error: unknown }) => {
          console.error("💥 streamText onError callback:", 
            event.error instanceof Error 
              ? `${event.error.message}\n${event.error.stack}` 
              : String(event.error));
        },
        onFinish: (event) => { console.log(`🏁 streamText onFinish. Full event:`, JSON.stringify(event)); }
      });
      // --- End streamText Call ---

      console.log(`✅ streamText call initiated successfully (${hasTools ? 'WITH' : 'WITHOUT'} GITHUB TOOLS).`);

      // --- *** TOOL MONITORING FOR LOGGING ONLY *** ---
      if (hasTools) {
          // Just monitor tool calls for logging purposes
          // The actual execution happens automatically through the execute function in tool definitions
          (async () => {
              try {
                  console.log("👂 Monitoring tool calls (execution happens automatically)");
                  const toolCallsArray = await streamResult.toolCalls;
                  console.log(`📞 ${toolCallsArray.length} tool call(s) were made during this stream`);
                  
                  // Log details of each tool call (execution already happened via the execute function)
                  for (const toolCall of toolCallsArray) {
                      console.log(`ℹ️ Tool ${toolCall.toolName} was called with args:`, 
                          JSON.stringify(toolCall.args).substring(0, 100));
                  }
              } catch (error) {
                  console.error("❌ Error monitoring tool calls:", error);
              }
          })();
          
          console.log("👂 Set up tool call monitoring for logging");
      }
      // --- *** END TOOL MONITORING *** ---

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