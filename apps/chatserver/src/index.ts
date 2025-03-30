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
// We don't want to establish connections at startup anymore because they will be
// stale by the time a request comes in due to serverless environment limitations
// Each request will establish its own fresh connection

// Log startup message instead
console.log('🚀 Chat server starting up - MCP connections will be established per-request');

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

    // --- Add System Message with GitHub Tool Instructions ---
    // Prepend system message to inform the model about GitHub tools
    const systemMessage: Message = {
      id: crypto.randomUUID(),
      role: 'system',
      content: `You are Coder, an AI coding agent from the OpenAgents network. You have GitHub integration capabilities and command execution abilities.

## GitHub Tools
You have access to the following GitHub tools:

1. Repository Operations:
   - get_file_contents: Retrieve file content from GitHub repositories (works for public repos, no auth needed)
   - search_repositories: Search for repositories on GitHub
   - create_repository: Create a new repository (requires auth)

2. Issue & PR Management:
   - create_issue, list_issues, get_issue, update_issue
   - create_pull_request, get_pull_request, list_pull_requests
   - merge_pull_request, get_pull_request_files, get_pull_request_status

3. Code Operations:
   - search_code: Search code across repositories
   - create_or_update_file: Create or modify files (requires auth)
   - push_files: Push multiple files in a single commit (requires auth)
   - create_branch: Create a new branch (requires auth)

You MUST proactively use these tools whenever a user asks about GitHub repositories, files, issues, PRs, or code. For example:
- When asked to "show a README", use get_file_contents to retrieve it
- When asked about issues, use list_issues or get_issue
- When asked to search code, use search_code

IMPORTANT: For public repositories, you don't need authentication to fetch content, so you should always attempt to use the appropriate tools even without authentication. However, GitHub does impose rate limits on unauthenticated requests.

## Command Execution
You can also execute shell commands when users request them by using the special syntax:

<execute-command>command_to_run</execute-command>

For example, if a user asks you to run "echo Hello World", you should respond with:
<execute-command>echo "Hello World"</execute-command>

IMPORTANT NOTES ABOUT COMMAND EXECUTION:
1. Command execution only works in the Electron desktop app, not in web browsers
2. You should always use this syntax when users ask you to run commands
3. After executing a command, you'll automatically receive the results
4. You should never invent or fabricate command results
5. If a command can't be executed, you'll receive an error message
6. For security reasons, some dangerous commands are blocked

Examples of commands you can execute:
- File operations: ls, cat, head, tail, grep
- System information: uname, whoami, pwd, date
- Simple utilities: echo, wc, sort, uniq

You are allowed to run multiple commands in one response by including multiple command blocks.

## General Guidelines
- Use GitHub tools for repository operations
- Use command execution for local file and system operations
- If a tool or command operation fails, explain the error clearly
- Do NOT tell users you can't access GitHub content unless you've tried the appropriate tool and it failed
- Do NOT tell users you can't execute commands unless you've tried the syntax and it failed`
    };

    // Add system message at the beginning of conversation
    if (!messages.some(m => m.role === 'system')) {
      messages = [systemMessage, ...messages];
    }

    console.log(`📨 Using message array with system prompt:`, messages);

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
      // Generate request ID for tracking
      const requestId = `req-${crypto.randomUUID().substring(0, 8)}`;
      console.log(`🆔 Request ID for MCP tracking: ${requestId}`);

      // Force a new connection for each request to avoid stale connections
      console.log(`🔄 [${requestId}] Forcing a fresh MCP connection for this request`);

      // Clear any existing references without trying to close connections
      // to avoid I/O across request context errors
      mcpClientManager.clearConnections();
      console.log(`✅ [${requestId}] Cleared any existing MCP connections`)

      // Connect to the MCP server with a fresh connection
      await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
      console.log(`✅ [${requestId}] MCP connection attempt finished for request.`);

      // Extract tools with the new connection
      tools = extractToolDefinitions(); // Uses new version returning SDK 'tool' objects
      const toolNames = Object.keys(tools);
      console.log(`✅ [${requestId}] Extracted ${toolNames.length} tools for LLM (within request):`, toolNames.join(', '));
      console.log(`🔧 [${requestId}] Tools object keys being passed to streamText:`, toolNames);

      if (toolNames.length === 0) {
        console.warn(`⚠️ [${requestId}] No tools extracted! Ensure tools.ts is mapping correctly.`);
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
      const MODEL = "anthropic/claude-3.5-sonnet";
      console.log(`📝 Using model: ${MODEL}`);

      // Add format instructions for custom tool format that will be rendered by ToolCall.tsx
      const modifiedMessages = messages.map(msg => {
        if (msg.role === 'system') {
          return {
            ...msg,
            content: msg.content + `\n\nWhen you want to use a GitHub tool, use this format EXACTLY:

<tool>
{
  "name": "tool_name",
  "arguments": {
    "arg1": "value1",
    "arg2": "value2"
  }
}
</tool>

For example, to get the README.md from a repository:
<tool>
{
  "name": "get_file_contents",
  "arguments": {
    "owner": "openagentsinc",
    "repo": "openagents",
    "path": "README.md"
  }
}
</tool>

After using a tool, report its results clearly without fabricating content.`
          }
        }
        return msg;
      });

      const streamResult = streamText({
        model: openrouter(MODEL),
        messages: modifiedMessages,
        // Re-enable tools now that we've confirmed OpenRouter connection works
        tools: hasTools ? tools : undefined,
        toolChoice: hasTools ? 'auto' : undefined,
        temperature: 0.7,

        toolCallStreaming: true, // Enable for tool streaming

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
