import { routeAgentRequest, type Connection, type Schedule, type WSMessage } from "agents";
import { unstable_getSchedulePrompt } from "agents/schedule";
import { AIChatAgent } from "agents/ai-chat-agent";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  type StreamTextOnFinishCallback,
  type ToolExecutionOptions,
} from "ai";
import { processToolCalls } from "./utils";
import { AsyncLocalStorage } from "node:async_hooks";
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { env } from "cloudflare:workers";
import { tools as builtInTools, executions } from "./tools";

import { MCPClientManager } from "agents/mcp/client";
import { CallToolResultSchema } from "@modelcontextprotocol/sdk/types.js";
import type {
  Tool,
  Prompt,
  Resource,
} from "@modelcontextprotocol/sdk/types.js";

export type Server = {
  url: string;
  state: "authenticating" | "connecting" | "ready" | "discovering" | "failed";
  authUrl?: string;
};

export type State = {
  servers: Record<string, Server>;
  tools: (Tool & { serverId: string })[];
  prompts: (Prompt & { serverId: string })[];
  resources: (Resource & { serverId: string })[];
};


const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

// we use ALS to expose the agent context to the tools
export const agentContext = new AsyncLocalStorage<Coder>();

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Coder extends AIChatAgent<Env, State> {

  githubToken?: string;
  combinedTools?: Record<string, any>;
  mcpExecutions?: Record<string, (args: any, context: ToolExecutionOptions) => Promise<unknown>>;

  /**
   * MCP STUFF
   */

  initialState = {
    servers: {},
    tools: [],
    prompts: [],
    resources: [],
  };

  private mcp_: MCPClientManager | undefined;

  get mcp(): MCPClientManager {
    if (!this.mcp_) {
      throw new Error("MCPClientManager not initialized");
    }

    return this.mcp_;
  }

  onStart() {
    this.mcp_ = new MCPClientManager("coder", "0.0.1", {
      baseCallbackUri: `https://agents.openagents.com/agents/coder/${this.name}/callback`,
      storage: this.ctx.storage,
    });
    console.log("MCPClientManager initialized");

    // Initialize combinedTools with built-in tools
    this.combinedTools = { ...builtInTools };

    // Connect to MCP GitHub SSE server
    this.addMcpServer("https://mcp-github.openagents.com/sse")
      .then(async (authUrl) => {
        console.log("Connected to MCP GitHub SSE server, auth URL:", authUrl);
        console.log("Refreshing server data after connection...");
        await this.refreshServerData();
        console.log("Server data refresh complete, tool state:", {
          combinedToolCount: this.combinedTools ? Object.keys(this.combinedTools).length : 0,
          toolNames: this.combinedTools ? Object.keys(this.combinedTools) : []
        });
      })
      .catch((error) => {
        console.error("Failed to connect to MCP GitHub SSE server:", error);
      });
  }

  setServerState(id: string, state: Server) {
    this.setState({
      ...this.state,
      servers: {
        ...this.state.servers,
        [id]: state,
      },
    });
  }

  async refreshServerData() {
    // Get latest MCP data
    const mcpTools = this.mcp.listTools();
    const mcpPrompts = this.mcp.listPrompts();
    const mcpResources = this.mcp.listResources();

    console.log("MCP data retrieved:", {
      toolCount: mcpTools.length,
      promptCount: mcpPrompts.length,
      resourceCount: mcpResources.length,
      toolNames: mcpTools.map(t => t.name)
    });

    // For testing, only use the get_file_contents tool
    const testTool = mcpTools.find(t => t.name === 'get_file_contents');
    const filteredTools = testTool ? [testTool] : [];

    // Update state with filtered MCP data
    this.setState({
      ...this.state,
      prompts: mcpPrompts,
      tools: filteredTools, // Only use the test tool
      resources: mcpResources,
    });

    // Update combinedTools while preserving existing tools
    const mcpToolsMap = filteredTools.reduce((acc, tool) => {
      // Convert MCP tool to match the AI tools system structure
      acc[tool.name] = {
        name: tool.name,
        description: tool.description,
        parameters: tool.parameters,
        // No execute function - this makes it require confirmation
      };
      return acc;
    }, {} as Record<string, any>);

    // Only include built-in tools and our test tool
    this.combinedTools = {
      ...builtInTools,
      ...mcpToolsMap
    };

    console.log("Tool state after refresh:", {
      builtInTools: Object.keys(builtInTools),
      mcpTools: Object.keys(mcpToolsMap),
      combinedTools: Object.keys(this.combinedTools),
      sampleTool: filteredTools[0] ? JSON.stringify(filteredTools[0], null, 2) : 'no tools',
      toolParameters: filteredTools[0] ? JSON.stringify(filteredTools[0].parameters, null, 2) : 'no parameters'
    });
  }

  async addMcpServer(url: string): Promise<string> {
    console.log(`Registering server: ${url}`);
    const { id, authUrl } = await this.mcp.connect(url);
    this.setServerState(id, {
      url,
      authUrl,
      state: this.mcp.mcpConnections[id].connectionState,
    });
    console.log(`Server registered with ID: ${id}`);
    return authUrl ?? "";
  }

  async onRequest(request: Request): Promise<Response> {
    if (this.mcp.isCallbackRequest(request)) {
      try {
        const { serverId } = await this.mcp.handleCallbackRequest(request);
        this.setServerState(serverId, {
          url: this.state.servers[serverId].url,
          state: this.mcp.mcpConnections[serverId].connectionState,
        });
        await this.refreshServerData();
        // Hack: autoclosing window because a redirect fails for some reason
        // return Response.redirect('http://localhost:5173/', 301)
        return new Response("<script>window.close();</script>", {
          status: 200,
          headers: { "content-type": "text/html" },
        });
        // biome-ignore lint/suspicious/noExplicitAny: just bubbling an error up
      } catch (e: any) {
        return new Response(e, { status: 401 });
      }
    }

    const reqUrl = new URL(request.url);
    if (reqUrl.pathname.endsWith("add-mcp") && request.method === "POST") {
      const mcpServer = (await request.json()) as { url: string };
      const authUrl = await this.addMcpServer(mcpServer.url);
      return new Response(authUrl, { status: 200 });
    }

    // Delegate non-MCP requests to parent class
    return super.onRequest(request);
  }


  /**
   * CHAT STUFF
   */

  override async onMessage(connection: Connection, message: WSMessage) {
    console.log('[onMessage] Received message:', typeof message);
    console.log('[onMessage] Message content:', message);

    if (typeof message === "string") {
      let data: any;
      try {
        data = JSON.parse(message);
        // Handle different message types
        if (data.type === "cf_agent_use_chat_request" && data.init?.method === "POST") {
          const { body } = data.init;
          const requestData = JSON.parse(body as string);
          const { messages, githubToken } = requestData;
          console.log('[onMessage] Parsed request data:', {
            messageCount: messages?.length,
            hasGithubToken: !!githubToken,
            githubToken: githubToken?.slice(0, 15),
          });

          // Update GitHub token
          this.githubToken = githubToken;

          // Run the rest of the message handling with the tool context
          return agentContext.run(this, async () => {
            console.log('[onMessage] Delegating to parent handler with tools context');
            return super.onMessage(connection, message);
          });
        } else {
          console.log('[onMessage] Non-chat request message received:', data.type);
          // For other message types, pass to parent handler
          return super.onMessage(connection, message);
        }
      } catch (error) {
        console.error('[onMessage] Failed to parse message:', error);
        console.error('[onMessage] Raw message:', message);
        return;
      }
    }
    console.log('[onMessage] Falling back to parent handler');
    return super.onMessage(connection, message);
  }

  // biome-ignore lint/complexity/noBannedTypes: <explanation>
  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    // Create a streaming response that handles both text and tool outputs
    return agentContext.run(this, async () => {
      const dataStreamResponse = createDataStreamResponse({
        execute: async (dataStream) => {
          // Get the current context with GitHub token
          const context = agentContext.getStore();

          console.log("Current tool state:", {
            hasBuiltInTools: !!builtInTools && Object.keys(builtInTools).length,
            hasCombinedTools: !!this.combinedTools && Object.keys(this.combinedTools).length,
            stateToolCount: this.state.tools.length,
            builtInToolNames: Object.keys(builtInTools),
            combinedToolNames: this.combinedTools ? Object.keys(this.combinedTools) : [],
            stateToolNames: this.state.tools.map(t => t.name)
          });

          // Use the already combined tools instead of recreating the map
          const allTools = this.combinedTools || {};

          console.log('[onChatMessage] Context retrieved:', {
            hasContext: !!context,
            hasGithubToken: !!context?.githubToken,
            hasTools: !!allTools,
            availableTools: Object.keys(allTools),
            tokenPrefix: context?.githubToken ? context.githubToken.slice(0, 15) : 'none'
          });

          // Process any pending tool calls from previous messages
          const processedMessages = await processToolCalls({
            messages: this.messages,
            dataStream,
            tools: allTools,
            executions: {
              ...executions,
              // Handle MCP tool executions
              ...Object.fromEntries(
                this.state.tools.map(tool => [
                  tool.name,
                  async (args: any) => {
                    console.log("Executing MCP tool:", {
                      toolName: tool.name,
                      serverId: tool.serverId,
                      args: JSON.stringify(args, null, 2),
                      hasConnection: !!this.mcp.mcpConnections[tool.serverId]
                    });

                    try {
                      const result = await this.mcp.callTool({
                        serverId: tool.serverId,
                        name: tool.name,
                        args
                      }, CallToolResultSchema, {});

                      console.log("MCP tool execution result:", {
                        toolName: tool.name,
                        success: true,
                        resultType: typeof result,
                        resultPreview: JSON.stringify(result).slice(0, 100)
                      });

                      return result;
                    } catch (error) {
                      console.error("MCP tool execution error:", {
                        toolName: tool.name,
                        error: error instanceof Error ? {
                          message: error.message,
                          name: error.name,
                          stack: error.stack
                        } : error
                      });
                      throw error;
                    }
                  }
                ])
              ),
              // Pass token through the agent context instead
              getGithubToken: async () => context?.githubToken
            } as typeof executions & { getGithubToken: () => Promise<string | undefined> },
          });

          // Stream the AI response using GPT-4
          console.log("Starting stream text with:", {
            messageCount: processedMessages.length,
            toolCount: Object.keys(allTools).length,
            systemPromptLength: this.state.tools.length
          });

          const result = streamText<Record<string, any>>({
            model,
            system: `You are a coding assistant named Coder. Help the user with various software engineering tasks.

${unstable_getSchedulePrompt({ date: new Date() })}

You have access to a few built-in tools described below and a GitHub tool through a separate Model Context Protocol service.
The GitHub token will be automatically provided to the tools that need it.

<built-in-tools>
- getWeatherInformation: Get weather information for a location
- getLocalTime: Get the current time for a location
- scheduleTask: Schedule a task to be executed at a later time
- listScheduledTasks: List all currently scheduled tasks with their details
- deleteScheduledTask: Delete a scheduled task (note: only one task can be scheduled at a time)
</built-in-tools>

<github-tools>
- get_file_contents: Get the contents of a file from a GitHub repository
</github-tools>

When using get_file_contents, you'll need:
- owner: The repository owner
- repo: The repository name
- path: The path to the file
- ref: (optional) The branch/commit/tag to get the file from
`,
            messages: processedMessages,
            tools: allTools,
            onFinish: (result) => {
              console.log("Stream finished successfully:", {
                hasResult: !!result,
                resultKeys: result ? Object.keys(result) : [],
                resultPreview: result ? JSON.stringify(result).slice(0, 100) : 'no result'
              });

              try {
                const finalResult = onFinish(result as any);
                console.log("onFinish callback completed successfully");
                return finalResult;
              } catch (error) {
                console.error("Error in onFinish callback:", {
                  error: error instanceof Error ? {
                    message: error.message,
                    name: error.name,
                    stack: error.stack
                  } : error
                });
                throw error;
              }
            },
            onError: (error: unknown) => {
              // Improved error handling and logging
              const errorDetails = {
                message: error instanceof Error ? error.message :
                  typeof error === 'object' && error !== null ? JSON.stringify(error) :
                    String(error),
                name: error instanceof Error ? error.name : typeof error,
                stack: error instanceof Error ? error.stack : undefined,
                raw: error // For debugging
              };

              console.error("Streaming error:", errorDetails);
            },
            maxSteps: 10,
          });

          try {
            // Merge the AI response stream with tool execution outputs
            result.mergeIntoDataStream(dataStream);
          } catch (error: unknown) {
            // Improved error handling and logging
            const errorDetails = {
              message: error instanceof Error ? error.message :
                typeof error === 'object' && error !== null ? JSON.stringify(error) :
                  String(error),
              name: error instanceof Error ? error.name : typeof error,
              stack: error instanceof Error ? error.stack : undefined,
              raw: error // For debugging
            };

            console.error("Stream merging error:", errorDetails);
          }
        },
      });

      return dataStreamResponse;
    });
  }
  async executeTask(description: string, task: Schedule<string>) {
    await this.saveMessages([
      ...this.messages,
      {
        id: generateId(),
        role: "user",
        content: `Running scheduled task: ${description}`,
        createdAt: new Date(),
      },
    ]);
  }
}

/**
 * Worker entry point that routes incoming requests to the appropriate handler
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Route the request to our agent or return 404 if not found
    return (
      (await routeAgentRequest(request, env, { cors: true })) ||
      new Response("Not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
