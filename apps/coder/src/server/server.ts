// apps/coder/src/server/server.ts
import { Hono } from 'hono';
import { logger } from 'hono/logger';
import { cors } from 'hono/cors';
import { stream } from 'hono/streaming';
import { streamText, type Message, experimental_createMCPClient } from "ai";
import { Experimental_StdioMCPTransport as StdioMCPTransport } from 'ai/mcp-stdio';
import { createOpenRouter } from '@openrouter/ai-sdk-provider';

// Define environment interface
interface Env {
  OPENROUTER_API_KEY?: string;
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

// Health check endpoint
app.get('/api/ping', (c) => {
  console.log('[Server] Received /api/ping request');
  return c.json({ message: 'pong' });
});

// Remote GitHub MCP connection test endpoint
app.get('/api/mcp/github/test', async (c) => {
  console.log('[Server] Testing remote GitHub MCP connection');

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

    console.log('[Server] Successfully connected to remote GitHub MCP');
    return c.json({
      success: true,
      message: 'Successfully connected to remote GitHub MCP',
      clientInfo: {
        connected: true,
        url: GITHUB_MCP_URL
      }
    });
  } catch (error) {
    console.error('[Server] Failed to connect to remote GitHub MCP:', error);
    return c.json({
      success: false,
      message: 'Failed to connect to remote GitHub MCP',
      error: error instanceof Error ? error.message : String(error)
    }, 500);
  } finally {
    // No explicit cleanup needed - the client will be garbage collected
    // The MCP client in AI SDK handles its own cleanup
    console.log('[Server] Remote MCP connection test complete');
  }
});

// Local GitHub MCP connection test endpoint using stdio transport
app.get('/api/mcp/github/local/test', async (c) => {
  console.log('[Server] Testing local GitHub MCP connection');

  try {
    // Create MCP client with stdio transport for local GitHub MCP
    const transport = new StdioMCPTransport({
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-github'],
      env: {
        // Note: For testing we're not providing a real token
        // In production, this should be a valid GitHub token
        GITHUB_PERSONAL_ACCESS_TOKEN: process.env.GITHUB_TOKEN || '<TOKEN_REQUIRED>'
      }
    });

    console.log('[Server] Created stdio transport for local GitHub MCP');

    // Try to connect to the local MCP server
    const mcpClient = await experimental_createMCPClient({
      transport
    });

    console.log('[Server] Successfully connected to local GitHub MCP');
    return c.json({
      success: true,
      message: 'Successfully connected to local GitHub MCP',
      clientInfo: {
        connected: true,
        transport: 'stdio',
        command: 'npx',
        args: ['-y', '@modelcontextprotocol/server-github']
      }
    });
  } catch (error) {
    console.error('[Server] Failed to connect to local GitHub MCP:', error);
    return c.json({
      success: false,
      message: 'Failed to connect to local GitHub MCP',
      error: error instanceof Error ? error.message : String(error)
    }, 500);
  } finally {
    console.log('[Server] Local MCP connection test complete');
  }
});

// Shell MCP connection test endpoint using stdio transport
app.get('/api/mcp/shell/test', async (c) => {
  console.log('[Server] Testing shell MCP connection');

  try {
    // Define allowed commands, falling back to a minimal set if not configured
    const allowCommands = process.env.ALLOW_COMMANDS || 'ls,cat,pwd,echo,grep';
    
    // Create MCP client with stdio transport for shell MCP
    const transport = new StdioMCPTransport({
      command: 'npx',
      args: ['-y', 'mcp-shell-server'],
      env: {
        // Configure allowed commands for security
        ALLOW_COMMANDS: allowCommands
      }
    });

    console.log('[Server] Created stdio transport for shell MCP');

    // Try to connect to the local shell MCP server
    const mcpClient = await experimental_createMCPClient({
      transport
    });

    console.log('[Server] Successfully connected to shell MCP');
    return c.json({
      success: true,
      message: 'Successfully connected to shell MCP',
      clientInfo: {
        connected: true,
        transport: 'stdio',
        command: 'npx',
        args: ['-y', 'mcp-shell-server'],
        allowedCommands: allowCommands.split(',')
      }
    });
  } catch (error) {
    console.error('[Server] Failed to connect to shell MCP:', error);
    return c.json({
      success: false,
      message: 'Failed to connect to shell MCP',
      error: error instanceof Error ? error.message : String(error)
    }, 500);
  } finally {
    console.log('[Server] Shell MCP connection test complete');
  }
});


// Main chat endpoint
app.post('/api/chat', async (c) => {
  console.log('[Server] Received chat request');

  // Get OpenRouter API key from environment variables
  const OPENROUTER_API_KEY = process.env.OPENROUTER_API_KEY || "";
  // GitHub MCP URL
  const GITHUB_MCP_URL = "https://mcp-github.openagents.com/sse";

  // Initialize MCP clients for tools
  let remoteMcpClient;
  let localGithubMcpClient;
  let localShellMcpClient;

  // Try to initialize remote MCP client
  try {
    console.log('[Server] Initializing remote GitHub MCP client');
    remoteMcpClient = await experimental_createMCPClient({
      transport: {
        type: 'sse',
        url: GITHUB_MCP_URL,
      },
    });
    console.log('[Server] Remote MCP client initialized successfully');
  } catch (remoteMcpError) {
    console.error('[Server] Failed to initialize remote MCP client:', remoteMcpError);
    // Continue execution - we'll still use local MCP or LLM without tools
  }

  // Try to initialize local GitHub MCP client with stdio transport
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

    localGithubMcpClient = await experimental_createMCPClient({
      transport
    });

    console.log('[Server] Local GitHub stdio MCP client initialized successfully');
  } catch (localMcpError) {
    console.error('[Server] Failed to initialize local GitHub MCP client:', localMcpError);
    // Continue execution - we'll still use remote MCP or LLM without tools
  }
  
  // Try to initialize local Shell MCP client with stdio transport
  try {
    console.log('[Server] Initializing local Shell MCP client with stdio');
    
    // Define allowed commands, falling back to a minimal set if not configured
    const allowCommands = process.env.ALLOW_COMMANDS || 'ls,cat,pwd,echo,grep,find,ps,wc';
    
    const transportShell = new StdioMCPTransport({
      command: 'npx',
      args: ['-y', 'mcp-shell-server'],
      env: {
        // Configure allowed commands for security
        ALLOW_COMMANDS: allowCommands
      }
    });

    localShellMcpClient = await experimental_createMCPClient({
      transport: transportShell
    });

    console.log('[Server] Local Shell stdio MCP client initialized successfully');
  } catch (shellMcpError) {
    console.error('[Server] Failed to initialize local Shell MCP client:', shellMcpError);
    // Continue execution - we'll still use other MCP clients or LLM without tools
  }

  // Determine which MCP client to use (prefer local)
  const mcpClient = localGithubMcpClient || remoteMcpClient;

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

    // Get tools from the MCP clients
    let tools = {};
    
    // Add GitHub tools if available
    if (localGithubMcpClient) {
      try {
        console.log('[Server] Fetching tools from local GitHub MCP client');
        const githubTools = await localGithubMcpClient.tools();
        tools = {...tools, ...githubTools};
        console.log('[Server] Successfully fetched tools from local GitHub MCP');
      } catch (toolError) {
        console.error('[Server] Error fetching tools from local GitHub MCP:', toolError);
      }
    } else if (remoteMcpClient) {
      try {
        console.log('[Server] Fetching tools from remote MCP client');
        const remoteTools = await remoteMcpClient.tools();
        tools = {...tools, ...remoteTools};
        console.log('[Server] Successfully fetched tools from remote MCP');
      } catch (toolError) {
        console.error('[Server] Error fetching tools from remote MCP:', toolError);
      }
    }
    
    // Add Shell tools if available
    if (localShellMcpClient) {
      try {
        console.log('[Server] Fetching tools from local Shell MCP client');
        const shellTools = await localShellMcpClient.tools();
        tools = {...tools, ...shellTools};
        console.log('[Server] Successfully fetched tools from local Shell MCP');
      } catch (toolError) {
        console.error('[Server] Error fetching tools from local Shell MCP:', toolError);
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
        onFinish: (event: { detail: { fromButton: boolean } }) => {
          console.log(`🏁 streamText onFinish completed`);
        }
      };

      // Enable MCP tools integration if we have tools
      if (Object.keys(tools).length > 0) {
        console.log(`[Server] Enabling MCP tools integration with ${Object.keys(tools).length} tools`);
      } else {
        console.log('[Server] No MCP tools available, continuing without tools');
        // Remove tools if empty to avoid schema errors
        delete streamOptions.tools;
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
    if (localGithubMcpClient || remoteMcpClient || localShellMcpClient) {
      console.log('[Server] Chat request completed, MCP resources will be cleaned up');

      // Note: The MCP clients will be automatically garbage collected
      // For stdio transports, the process will be terminated when the client is garbage collected
    }
  }
});

// --- End API Routes ---

export default app;
