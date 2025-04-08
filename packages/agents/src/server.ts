import { Agent, routeAgentRequest, type Connection, type Schedule, type WSMessage } from "agents";
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

  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    // Access the AI models using environment bindings

    // Get the current conversation history
    const chatHistory = this.messages;

    console.log("[onChatMessage] Chat history:", chatHistory);

    // Generate a system prompt based on knowledge base
    const systemPrompt = await this.generateSystemPrompt();

    console.log("[onChatMessage] System prompt:", systemPrompt);

    const stream = streamText({
      model,
      messages: [
        { role: "system", content: systemPrompt },
        ...chatHistory
      ],
      onFinish
    })

    return stream.toDataStreamResponse()
  }

  // Helper method to generate a system prompt
  async generateSystemPrompt() {
    // Query knowledge base or use static prompt
    return `You are a helpful customer support agent.
              Respond to customer inquiries based on the following guidelines:
              - Be friendly and professional
              - If you don't know an answer, say so
              - Current company policies: ...`;
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
