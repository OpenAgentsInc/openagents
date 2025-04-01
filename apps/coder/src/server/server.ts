// apps/coder/src/server/server.ts
import { Hono } from 'hono';
import { logger } from 'hono/logger';
import { cors } from 'hono/cors';
import { stream } from 'hono/streaming';
import { streamText, type Message, experimental_createMCPClient, type StreamTextOnFinishCallback } from "ai";
import { Experimental_StdioMCPTransport as StdioMCPTransport } from 'ai/mcp-stdio';
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

// Main chat endpoint
app.post('/api/chat', async (c) => {
  console.log('[Server] Received chat request');

  // Get OpenRouter API key from environment variables
  const OPENROUTER_API_KEY = process.env.OPENROUTER_API_KEY || "";

  // Initialize MCP client for tools
  let localMcpClient;

  // Initialize local MCP client with stdio transport
  try {
    console.log('[Server] Initializing local GitHub MCP client with stdio');

    const transport = new StdioMCPTransport({
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-github'],
      env: {
        // Use GitHub token from environment if available
        GITHUB_PERSONAL_ACCESS_TOKEN: process.env.GITHUB_TOKEN || '<TOKEN_REQUIRED>'
      }
    });

    localMcpClient = await experimental_createMCPClient({
      transport
    });

    console.log('[Server] Local stdio MCP client initialized successfully');
  } catch (localMcpError) {
    console.error('[Server] Failed to initialize local MCP client:', localMcpError);
    // Continue execution - we'll still use LLM without tools
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

    // Get tools from the MCP client
    let tools = {};
    if (localMcpClient) {
      try {
        console.log('[Server] Fetching tools from local MCP client');
        const localTools = await localMcpClient.tools();
        tools = localTools;
        console.log('[Server] Successfully fetched tools from local MCP');
      } catch (toolError) {
        console.error('[Server] Error fetching tools from local MCP:', toolError);
      }
    }

    try {
      // Configure stream options with MCP tools if available
      const streamOptions = {
        model: openrouter(MODEL),
        messages: messages,

        toolCallStreaming: true,

        temperature: 0.7,
        tools, // Add MCP tools to the request

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
        onFinish: (event: Parameters<StreamTextOnFinishCallback<{}>>[0]) => {
          console.log(`🏁 streamText onFinish completed`);
        }
      };

      // Enable MCP tools integration if we have tools
      if (Object.keys(tools).length > 0) {
        console.log(`[Server] Enabling MCP tools integration with ${Object.keys(tools).length} tools`);
      } else {
        console.log('[Server] No MCP tools available, continuing without tools');
        // Remove tools if empty to avoid schema errors
        streamOptions.tools = {};
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
    if (localMcpClient) {
      console.log('[Server] Chat request completed, MCP resources will be cleaned up');
      // Note: The MCP client will be automatically garbage collected
      // For stdio transports, the process will be terminated when the client is garbage collected
    }
  }
});

// --- End API Routes ---

export default app;
