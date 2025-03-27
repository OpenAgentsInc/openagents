import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { streamText } from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { cors } from 'hono/cors';
import { mcpClientManager } from './mcp/client';
import { extractToolDefinitions, processToolCall, type ToolCallPayload, type ToolDefinition, type ToolResultPayload } from './mcp/tools';

interface Env {
  AI: typeof AI;
}

interface ChatMessage {
  content: string;
  role: 'user' | 'assistant';
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
app.get('/', c => c.text('200 OK'));

// Main chat endpoint
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
    
    // Extract auth token with fallbacks
    const bearerToken = c.req.header('Authorization')?.replace('Bearer ', '');
    const githubTokenHeader = c.req.header('X-GitHub-Token');
    const authToken = bearerToken || githubTokenHeader;
    console.log(`🔑 Auth token present: ${!!authToken}`);
    
    // Get AI model from Cloudflare Workers AI
    const workersai = createWorkersAI({ binding: c.env.AI });
    
    console.log("🛠️ Extracting tool definitions...");
    // Force MCP connection to ensure it's ready
    try {
      await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
      const allTools = mcpClientManager.getAllTools();
      console.log(`✅ MCP Connected, discovered ${allTools.length} tools`);
    } catch (mcpError) {
      console.error("❌ MCP Connection failed:", mcpError);
    }
    
    // Get dynamically mapped tool definitions
    let tools: Record<string, ToolDefinition> = {}; // Ensure type is Record
    try {
      tools = extractToolDefinitions();
      const toolNames = Object.keys(tools);
      console.log(`✅ Extracted ${toolNames.length} tools for LLM:`, toolNames.join(', '));
      // Log the full tools object structure for debugging
      console.log('🔧 Tools object being passed to streamText:', JSON.stringify(tools, null, 2));
      if (toolNames.length === 0) {
        console.warn("⚠️ No tools were extracted for the LLM. Tool usage might fail.");
      }
    } catch (toolError) {
      console.error("❌ Failed to extract tools:", toolError);
    }
    
    console.log("🎬 Starting streamText with tools...");
    
    // Try with just one tool with a minimal schema
    let result;
    try {
      // Re-enable tools with our simplified schema
      result = streamText({
        model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
        messages,
        tools: Object.keys(tools).length > 0 ? tools as Record<string, ToolDefinition> : undefined,
        toolChoice: Object.keys(tools).length > 0 ? 'auto' : undefined,
        
        // Tool calling is handled by the built-in stream interceptor
        // We don't need onToolCall since we're using the streamText feature
        // to help debug, we use callbacks
        onError: (event: { error: unknown }) => {
          console.error("💥 streamText reported error:", event.error);
        },
        // Add onFinish for more info
        onFinish: (event) => {
          console.log(`🏁 streamText finished. Reason: ${event.finishReason}`, event);
        }
      });
      console.log("✅ streamText call succeeded initially.");
    } catch (streamTextError) {
      console.error("🚨 streamText call threw immediate exception:", streamTextError);
      // Handle the error - return a JSON error response instead of streaming
      return c.json({ error: "Failed to initialize AI stream" }, 500);
    }

  // Set SSE headers
  c.header('X-Vercel-AI-Data-Stream', 'v1');
  c.header('Content-Type', 'text/event-stream; charset=utf-8');
  c.header('Cache-Control', 'no-cache');
  c.header('Connection', 'keep-alive');

  // Pipe the SDK's data stream directly to the response
  console.log("🔄 Starting stream response");
  
  return stream(c, async responseStream => {
    try {
      console.log("📬 Entered stream() callback."); // Add log here
      
      // REMOVE THE INITIAL PING - this might be causing issues
      // await responseStream.write(":\n\n");
      
      console.log("🔄 Piping result.toDataStream()..."); // Add log here
      
      // Pipe the SDK stream directly to the response
      await responseStream.pipe(result.toDataStream());
      
      console.log("✅ Stream processing complete");
    } catch (error) {
      console.error("💥 Critical error in stream handling:", error);
      
      // Improved error handling
      try {
        // Hono's StreamingApi doesn't have writableEnded property
        const errorMessage = error instanceof Error ? error.message : String(error);
        console.log(`🧪 Sending error: ${errorMessage}`);
        await responseStream.write(`data: 3:${JSON.stringify(`Stream processing failed: ${errorMessage}`)}\n\n`);
      } catch (writeError) {
        console.error("‼️ Failed to write error to stream:", writeError);
      }
    } finally {
      console.log("🚪 Exiting stream() callback."); // Add log here
    }
  });
  } catch (error) {
    console.error("💥 Chat endpoint error:", error);
    return c.json({ error: "Failed to process chat request" }, 500);
  }
});

export default app;