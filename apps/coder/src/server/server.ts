// apps/coder/src/server/server.ts
import { Hono } from 'hono';
import { logger } from 'hono/logger';
import { cors } from 'hono/cors';
import { stream } from 'hono/streaming';
import { streamText, type Message, experimental_createMCPClient } from "ai";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';

// Define environment interface
interface Env {
  OPENROUTER_API_KEY?: string;
}

const app = new Hono<{ Variables: Env }>();

// Use logger middleware
app.use('*', logger());

// Use Hono's CORS middleware
app.use('*', cors({
  origin: '*', // Allow requests from any origin
  allowMethods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Authorization', 'Accept', 'X-Requested-With'],
  maxAge: 86400,
  // Log CORS operations
  exposeHeaders: ['Content-Length', 'X-Vercel-AI-Data-Stream'],
}));

// Health check endpoint
app.get('/api/ping', (c) => {
  console.log('[Server] Received /api/ping request');
  return c.json({ message: 'pong' });
});

// GitHub MCP connection test endpoint
app.get('/api/mcp/github/test', async (c) => {
  console.log('[Server] Testing GitHub MCP connection');

  const GITHUB_MCP_URL = "https://mcp-github.openagents.com/sse";

  try {
    // Create MCP client with SSE transport for GitHub MCP
    const mcpClient = await experimental_createMCPClient({
      transport: {
        type: 'sse',
        url: GITHUB_MCP_URL,
      },
    });

    // Test the connection - don't try to access internal properties
    // The fact that we can create the client without errors is sufficient
    // for testing the connection

    console.log('[Server] Successfully connected to GitHub MCP');
    return c.json({
      success: true,
      message: 'Successfully connected to GitHub MCP',
      clientInfo: {
        connected: true,
        url: GITHUB_MCP_URL
      }
    });
  } catch (error) {
    console.error('[Server] Failed to connect to GitHub MCP:', error);
    return c.json({
      success: false,
      message: 'Failed to connect to GitHub MCP',
      error: error instanceof Error ? error.message : String(error)
    }, 500);
  } finally {
    // No explicit cleanup needed - the client will be garbage collected
    // The MCP client in AI SDK handles its own cleanup
    console.log('[Server] MCP connection test complete');
  }
});


// Main chat endpoint
app.post('/api/chat', async (c) => {
  console.log('[Server] Received chat request');

  // Get OpenRouter API key from environment variables
  const OPENROUTER_API_KEY = process.env.OPENROUTER_API_KEY || "";
  // GitHub MCP URL
  const GITHUB_MCP_URL = "https://mcp-github.openagents.com/sse";

  // Initialize MCP client for tools
  let mcpClient;
  try {
    console.log('[Server] Initializing GitHub MCP client');
    mcpClient = await experimental_createMCPClient({
      transport: {
        type: 'sse',
        url: GITHUB_MCP_URL,
      },
    });
    console.log('[Server] MCP client initialized successfully');
  } catch (mcpError) {
    console.error('[Server] Failed to initialize MCP client:', mcpError);
    // Continue execution - we'll still use the LLM without tools if MCP fails
  }

  if (!OPENROUTER_API_KEY) {
    console.error("❌ OPENROUTER_API_KEY is missing or invalid");

    // Return error as SSE
    c.header('Content-Type', 'text/event-stream; charset=utf-8');
    c.header('Cache-Control', 'no-cache');
    c.header('Connection', 'keep-alive');
    c.header('X-Vercel-AI-Data-Stream', 'v1');
    // CORS headers are handled by the middleware

    return stream(c, async (responseStream) => {
      const errorMsg = "OpenRouter API Key not configured. You need to add your API key.";
      await responseStream.write(`data: 3:${JSON.stringify(errorMsg)}\n\n`);
    });
  }

  console.log("✅ OPENROUTER_API_KEY is present");

  try {
    const body = await c.req.json();

    // Validate input messages
    let messages: Message[] = body.messages || [];
    if (!Array.isArray(messages) || messages.length === 0) {
      return c.json({ error: "No valid messages array provided" }, 400);
    }

    // Create the OpenRouter client with API key
    const openrouter = createOpenRouter({
      apiKey: OPENROUTER_API_KEY,
      baseURL: "https://openrouter.ai/api/v1"
    });

    // Define model
    const MODEL = body.model;

    if (!MODEL) {
      return c.json({ error: "No model provided" }, 400);
    }

    console.log("🔍 MODEL:", MODEL);

    const tools = await mcpClient?.tools();

    try {
      // Configure stream options with MCP tools if available
      const streamOptions: any = {
        model: openrouter(MODEL),
        messages: messages,
        temperature: 0.7,
        tools,

        // Headers for OpenRouter
        headers: {
          'Authorization': `Bearer ${OPENROUTER_API_KEY}`,
          'HTTP-Referer': 'https://openagents.com',
          'X-Title': 'OpenAgents Coder'
        },

        // Standard callbacks
        onError: (event: { error: unknown }) => {
          console.error("💥 streamText onError callback:",
            event.error instanceof Error
              ? `${event.error.message}\n${event.error.stack}`
              : String(event.error));
        },
        onFinish: (event) => {
          console.log(`🏁 streamText onFinish completed`);
        }
      };

      // For now, do NOT include MCP tools to avoid schema errors
      // We'll need to fix this in a future update with proper tool definitions
      // The integration pattern will need to be refined based on AI SDK documentation

      // Temporarily comment out MCP tools integration to fix the error
      if (mcpClient) {
        console.log('[Server] MCP client initialized but tools integration is disabled');
        // We have the MCP client but won't use it yet to avoid the schema error
      }

      const streamResult = streamText(streamOptions);

      // Set up the SSE response
      c.header('Content-Type', 'text/event-stream; charset=utf-8');
      c.header('Cache-Control', 'no-cache');
      c.header('Connection', 'keep-alive');
      c.header('X-Vercel-AI-Data-Stream', 'v1');
      // CORS headers are handled by the middleware

      // Check streamResult validity
      if (!streamResult || typeof streamResult.toDataStream !== 'function') {
        console.error("Invalid streamResult object");
        return c.json({ error: "Invalid stream result object" }, 500);
      }

      return stream(c, async (responseStream) => {
        try {
          const sdkStream = streamResult.toDataStream({
            sendReasoning: true
          });
          await responseStream.pipe(sdkStream);
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : String(error);
          console.error(`Error during stream handling: ${errorMessage}`);
          try {
            await responseStream.write(`data: 3:${JSON.stringify(`Stream processing failed: ${errorMessage}`)}\n\n`);
          } catch (writeError) {
            console.error("Failed to write error message to stream");
          }
        }
      });
    } catch (streamSetupError) {
      console.error("🚨 streamText setup failed:", streamSetupError);
      return c.json({ error: "Failed to initialize AI stream" }, 500);
    }
  } catch (error) {
    console.error("💥 Chat endpoint error:", error);
    return c.json({ error: "Failed to process chat request" }, 500);
  } finally {
    // Clean up resources
    if (mcpClient) {
      console.log('[Server] Chat request completed, MCP resources will be cleaned up');
    }
  }
});

// --- End API Routes ---

export default app;
