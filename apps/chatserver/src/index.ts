import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { streamText, type Message } from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
// import { extractToolDefinitions, processToolCall } from './mcp/tools';

interface Env {
  AI: any;
}

const app = new Hono<{ Bindings: Env }>();

// Initialize MCP connections
async function initMcp() {
  try {
    await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
    console.log('Connected to GitHub MCP server');
    // Log discovered tools after connection attempt
    const initialTools = mcpClientManager.getAllTools();
    console.log(`[initMcp] Initially discovered ${initialTools.length} tools.`);
  } catch (error) {
    console.error('[initMcp] Failed initial connection attempt to GitHub MCP server:', error);
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
  if (!c.env.AI) {
    console.error("❌ AI binding is missing");
    return c.json({ error: "AI binding not configured" }, 500);
  }
  console.log("✅ AI binding seems present.");

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

    // --- Force simplest possible messages array for testing ---
    messages = [{
      id: crypto.randomUUID(),
      role: 'user',
      content: messages[messages.length - 1]?.content || 'hi'
    }];
    console.log(`📨 Using simplified message array for test:`, messages);

    // Extract auth token with fallbacks
    const bearerToken = c.req.header('Authorization')?.replace('Bearer ', '');
    const githubTokenHeader = c.req.header('X-GitHub-Token');
    const authToken = bearerToken || githubTokenHeader;
    console.log(`🔑 Auth token present: ${!!authToken}`);

    // Get AI model from Cloudflare Workers AI
    let workersai;
    try {
      workersai = createWorkersAI({ binding: c.env.AI });
      console.log("✅ createWorkersAI provider initialized.");
    } catch (providerError) {
      console.error("❌ Failed to initialize AI provider:", providerError);
      return c.json({ error: "Failed to initialize AI provider" }, 500);
    }

    console.log("🎬 Attempting streamText call (FORCING NO TOOLS)...");

    let streamResult;
    try {
      streamResult = streamText({
        model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
        messages: messages,
        tools: undefined,
        toolChoice: undefined,
        onError: (event: { error: unknown }) => {
          console.error("💥 streamText onError callback:", event.error);
        },
        onFinish: (event) => {
          console.log(`🏁 streamText onFinish callback. Reason: ${event.finishReason}, Usage:`, event.usage);
        }
      });
      console.log("✅ streamText call initiated successfully (NO TOOLS).");
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
    c.header('X-Vercel-AI-Data-Stream', 'v1');
    c.header('Content-Type', 'text/event-stream; charset=utf-8');
    c.header('Cache-Control', 'no-cache');
    c.header('Connection', 'keep-alive');

    // Pipe the SDK's data stream directly to the response
    console.log("🔄 Starting stream response");

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
          throw pipeError; // Re-throw for outer catch
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
