import { routeAgentRequest, type Connection, type Schedule, type WSMessage } from "agents";
import { unstable_getSchedulePrompt } from "agents/schedule";
import { AIChatAgent } from "agents/ai-chat-agent";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  type StreamTextOnFinishCallback,
} from "ai";
import { processToolCalls } from "./utils";
import { AsyncLocalStorage } from "node:async_hooks";
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { env } from "cloudflare:workers";

import { MCPClientManager } from "agents/mcp/client";
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
    this.setState({
      ...this.state,
      prompts: this.mcp.listPrompts(),
      tools: this.mcp.listTools(),
      resources: this.mcp.listResources(),
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

    return new Response("Not found", { status: 404 });
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
          console.log('[onChatMessage] Context retrieved:', {
            hasContext: !!context,
            hasGithubToken: !!context?.githubToken,
            hasTools: !!this.combinedTools,
            availableTools: this.combinedTools ? Object.keys(this.combinedTools) : [],
            tokenPrefix: context?.githubToken ? context.githubToken.slice(0, 15) : 'none'
          });

          // Process any pending tool calls from previous messages
          const processedMessages = await processToolCalls({
            messages: this.messages,
            dataStream,
            tools: this.combinedTools || {},
            executions: {
              // ...executions,
              // Pass token through the agent context instead
              getGithubToken: async () => context?.githubToken
            },
          });

          // Stream the AI response using GPT-4
          const result = streamText({
            model,
            system: `You are a coding assistant named Coder. Help the user with various software engineering tasks.

${unstable_getSchedulePrompt({ date: new Date() })}

You have access to a few built-in tools described below and a few GitHub tools through a separate Model Context Protocol service.
The GitHub token will be automatically provided to the tools that need it.

<built-in-tools>

TASK SCHEDULING:
- scheduleTask: Schedule a task to be executed at a later time
- listScheduledTasks: List all currently scheduled tasks with their details
- deleteScheduledTask: Delete a scheduled task (note: only one task can be scheduled at a time)

</built-in-tools>
`,
            messages: processedMessages,
            tools: this.combinedTools || {},
            onFinish,
            onError: (error) => {
              console.error("Error while streaming:", error);
            },
            maxSteps: 10,
          });

          // Merge the AI response stream with tool execution outputs
          result.mergeIntoDataStream(dataStream);
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
