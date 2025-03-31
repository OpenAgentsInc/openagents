// apps/coder/src/server/server.ts
import { Hono } from 'hono';
import { logger } from 'hono/logger';
import { cors } from 'hono/cors';
import { stream } from 'hono/streaming';
import { streamText, type Message } from "ai";
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

// Test CORS endpoint
app.get('/api/test-cors', (c) => {
  console.log('[Server] Received /api/test-cors request');
  // No need to manually set CORS headers - the middleware handles it
  return c.json({ success: true, message: 'CORS is working with Hono middleware!' });
});

// Main chat endpoint
app.post('/api/chat', async (c) => {
  console.log('[Server] Received chat request');

  // Manually set OpenRouter API key (you'll replace this with your key)
  // In a real app, you'd get this from environment variables
  const OPENROUTER_API_KEY = "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";

  if (!OPENROUTER_API_KEY || OPENROUTER_API_KEY === "sk-or-your-key-here") {
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
    console.log("[Server] Request body (preview):", JSON.stringify(body)?.substring(0, 300));

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
    console.log("✅ OpenRouter provider initialized");

    // Define model
    const MODEL = "anthropic/claude-3.5-sonnet";
    console.log(`📝 Using model: ${MODEL}`);

    try {
      const streamResult = streamText({
        model: openrouter(MODEL),
        messages: messages,
        temperature: 0.7,

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
          console.log(`🏁 streamText onFinish. Event ID: ${event.id}`);
        }
      });

      // Set up the SSE response
      c.header('Content-Type', 'text/event-stream; charset=utf-8');
      c.header('Cache-Control', 'no-cache');
      c.header('Connection', 'keep-alive');
      c.header('X-Vercel-AI-Data-Stream', 'v1');
      // CORS headers are handled by the middleware
      console.log("🔄 Preparing to stream response...");

      // Check streamResult validity
      if (!streamResult || typeof streamResult.toDataStream !== 'function') {
        console.error("❌ Invalid streamResult object");
        return c.json({ error: "Invalid stream result object" }, 500);
      }

      return stream(c, async (responseStream) => {
        console.log("📬 Entered stream() callback");
        try {
          const sdkStream = streamResult.toDataStream();
          console.log("🔄 Piping sdkStream from streamResult.toDataStream()...");
          await responseStream.pipe(sdkStream);
          console.log("✅ Piping completed successfully");
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : String(error);
          console.error(`💥 Error during stream handling: ${errorMessage}`);
          try {
            await responseStream.write(`data: 3:${JSON.stringify(`Stream processing failed: ${errorMessage}`)}\n\n`);
          } catch (writeError) {
            console.error("‼️ Failed to write error message to stream");
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
  }
});

// --- End API Routes ---

export default app;
