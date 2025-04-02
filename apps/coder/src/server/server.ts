// apps/coder/src/server/server.ts
import { Hono } from 'hono';
import { logger } from 'hono/logger';
import { cors } from 'hono/cors';
import { stream } from 'hono/streaming';
import { streamText, type Message, type StreamTextOnFinishCallback } from "ai";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import { ollama, createOllama } from 'ollama-ai-provider';
import { getMCPClients } from './mcp-clients';
import { MODELS } from "@openagents/core";

// Define environment interface
interface Env {
  OPENROUTER_API_KEY?: string;
  OLLAMA_BASE_URL?: string;
  ALLOW_COMMANDS?: string; // Shell commands whitelist for mcp-shell-server
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

  // Get environment variables
  const OPENROUTER_API_KEY = process.env.OPENROUTER_API_KEY || "";
  const OLLAMA_BASE_URL = process.env.OLLAMA_BASE_URL || "http://localhost:11434/api";

  // Get the globally initialized MCP clients
  const { allTools: tools } = getMCPClients();

  // Create the Ollama client with custom base URL if specified
  const customOllama = createOllama({
    baseURL: OLLAMA_BASE_URL,
  });
  
  // Log configuration
  console.log(`[Server] OLLAMA_BASE_URL: ${OLLAMA_BASE_URL}`);

  // For OpenRouter provider models, we need to check if API key is present
  if (!OPENROUTER_API_KEY) {
    console.warn("⚠️ OPENROUTER_API_KEY is missing - OpenRouter models will not be available");
  } else {
    console.log("✅ OPENROUTER_API_KEY is present - OpenRouter models are available");
  }

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

    // Find model info in MODELS
    const modelInfo = MODELS.find(m => m.id === MODEL);

    if (!modelInfo) {
      return c.json({ error: `Model ${MODEL} not found in the MODELS array` }, 400);
    }

    // Get provider from model info
    const provider = modelInfo.provider;

    console.log(`🔍 MODEL: ${MODEL}, PROVIDER: ${provider}`);
    
    // Check if we need OpenRouter
    if (provider === "openrouter" && !OPENROUTER_API_KEY) {
      // Return error as SSE
      c.header('Content-Type', 'text/event-stream; charset=utf-8');
      c.header('Cache-Control', 'no-cache');
      c.header('Connection', 'keep-alive');
      c.header('X-Vercel-AI-Data-Stream', 'v1');
      
      return stream(c, async (responseStream) => {
        const errorMsg = "OpenRouter API Key not configured. You need to add your API key to use OpenRouter models.";
        await responseStream.write(`data: 3:${JSON.stringify(errorMsg)}\n\n`);
      });
    }

    try {
      // Check for system prompt in request
      const systemPrompt = body.systemPrompt;

      // If system prompt exists, prepend it to messages array
      if (systemPrompt && typeof systemPrompt === 'string' && systemPrompt.trim() !== '') {
        console.log('[Server] Using custom system prompt');

        // Add system message at the beginning if it doesn't already exist
        const hasSystemMessage = messages.some(msg => msg.role === 'system');

        if (!hasSystemMessage) {
          messages = [
            { role: 'system', content: systemPrompt, id: `system-${Date.now()}` },
            ...messages
          ];
        }
      }

      // Check if the model supports tools
      const modelSupportsTools = modelInfo.supportsTools ?? false;

      console.log(`[Server] Model ${MODEL} ${modelSupportsTools ? 'supports' : 'does not support'} tools`);

      // Determine which provider to use based on model info
      let model;
      let headers = {};

      if (provider === "ollama") {
        console.log(`[Server] Using Ollama provider with base URL: ${OLLAMA_BASE_URL}`);
        // For Ollama models, we need to extract the model name without version
        // Ollama API expects just the model name without version specifiers
        const ollamaModelName = MODEL.split(":")[0];
        console.log(`[Server] Using Ollama model: ${ollamaModelName} (from ${MODEL})`);
        
        // Log availability information
        logOllamaModelAvailability(OLLAMA_BASE_URL, ollamaModelName);
        
        // For Ollama models, use the Ollama provider
        model = customOllama(ollamaModelName);
      } else if (provider === "openrouter") {
        console.log(`[Server] Using OpenRouter provider`);
        // For OpenRouter models, use the OpenRouter provider
        model = openrouter(MODEL);
        // Set OpenRouter specific headers
        headers = {
          'Authorization': `Bearer ${OPENROUTER_API_KEY}`,
          'HTTP-Referer': 'https://openagents.com',
          'X-Title': 'OpenAgents Coder'
        };
      } else {
        console.log(`[Server] Using default provider: OpenRouter`);
        // Default to OpenRouter for unspecified providers
        model = openrouter(MODEL);
        headers = {
          'Authorization': `Bearer ${OPENROUTER_API_KEY}`,
          'HTTP-Referer': 'https://openagents.com',
          'X-Title': 'OpenAgents Coder'
        };
      }

      // Configure stream options with MCP tools if available and if the model supports tools
      const streamOptions = {
        model,
        messages,
        toolCallStreaming: modelSupportsTools,
        temperature: 0.7,
        
        // Only include tools if the model supports them and we're not using Ollama
        // Temporarily disable tools for Ollama as it might be causing issues
        ...((modelSupportsTools && Object.keys(tools).length > 0 && provider !== "ollama") ? { tools } : {}),
        
        // Include headers if present
        ...(Object.keys(headers).length > 0 ? { headers } : {}),

        // Standard callbacks
        onError: (event: { error: unknown }) => {
          console.error("💥 streamText onError callback:",
            event.error instanceof Error
              ? `${event.error.message}\n${event.error.stack}`
              : String(event.error));
          
          // Additional debugging for Ollama-specific errors
          if (provider === "ollama") {
            console.error("Ollama error details:", JSON.stringify(event.error, null, 2));
            console.error("Please check if the Ollama server is running and the model is available.");
            console.error("You can pull the model with: ollama pull " + MODEL.split(":")[0]);
          }
        },
        onFinish: (event: Parameters<StreamTextOnFinishCallback<{}>>[0]) => {
          console.log(`🏁 streamText onFinish completed`);
        }
      };

      // Log tools integration status
      if (modelSupportsTools && Object.keys(tools).length > 0) {
        console.log(`[Server] Enabling MCP tools integration with ${Object.keys(tools).length} tools`);
      } else if (!modelSupportsTools) {
        console.log('[Server] Model does not support tools, continuing without tools');
      } else {
        console.log('[Server] No MCP tools available, continuing without tools');
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
    // No need to clean up - we're using persistent MCP clients
    console.log('[Server] Chat request completed');
  }
});

// Utility function to check if Ollama is available and the model exists
// This can be enhanced to actually make an API call to verify
const logOllamaModelAvailability = (baseUrl: string, modelName: string) => {
  console.log(`[Server] Ollama model check - baseUrl: ${baseUrl}, model: ${modelName}`);
  console.log(`[Server] If experiencing issues with Ollama, check that:`);
  console.log(`[Server]   1. Ollama server is running (run 'ollama serve')`);
  console.log(`[Server]   2. The model is available (run 'ollama list')`);
  console.log(`[Server]   3. If needed, pull the model with: 'ollama pull ${modelName}'`);
};

// --- End API Routes ---

export default app;
