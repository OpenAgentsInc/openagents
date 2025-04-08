import { Agent, routeAgentRequest, type Connection, type Schedule, type WSMessage } from "agents";
import { unstable_getSchedulePrompt } from "agents/schedule";
import { AIChatAgent } from "agents/ai-chat-agent";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  tool,
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
import { z } from "zod";

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
    
    // DEBUGGING: Only use get_file_contents tool
    const filteredTools = mcpTools.filter(tool => tool.name === 'get_file_contents');
    console.log("FILTERED: Using only get_file_contents tool. Found:", filteredTools.length);
    
    if (filteredTools.length > 0) {
      console.log("FILTERED TOOL DETAILS:", JSON.stringify(filteredTools[0], null, 2).substring(0, 500));
    }

    // Update state with filtered MCP data
    this.setState({
      ...this.state,
      prompts: mcpPrompts,
      tools: filteredTools, // Only use filtered tools
      resources: mcpResources,
    });

    // We'll create a simpler implementation that doesn't rely on MCP client directly
    // Instead, use a basic version that matches the weather tool pattern
    // DEBUGGING: Only use filtered tools for conversion
    const mcpToolsMap = filteredTools.reduce((acc, mcpTool) => {
      console.log(`CONVERTING TOOL: ${mcpTool.name}`);
      // Convert the MCP tool to the format expected by Vercel AI SDK
      console.log(`DEBUG: Creating tool definition for ${mcpTool.name}`);
      
      // Log schema details
      const schemaDetails = mcpTool.parameters && typeof mcpTool.parameters === 'object' 
        ? mcpTool.parameters 
        : mcpTool.inputSchema;
      console.log(`DEBUG: Schema details for ${mcpTool.name}:`, JSON.stringify(schemaDetails, null, 2).substring(0, 500));
      
      // Create properties for the tool
      const properties = {};
      
      // If it's get_file_contents, create known parameters
      if (mcpTool.name === 'get_file_contents') {
        properties.owner = z.string().describe('Repository owner (username or organization)');
        properties.repo = z.string().describe('Repository name');
        properties.path = z.string().describe('Path to the file or directory');
        properties.branch = z.string().optional().describe('Branch to get contents from');
      } else {
        // Default fallback
        properties._fallback = z.any();
      }
      
      acc[mcpTool.name] = tool({
        description: mcpTool.description || `Execute the ${mcpTool.name} MCP tool`,
        parameters: z.object(properties),
        execute: async (args) => {
          try {
            console.log(`DEBUG: Executing MCP tool ${mcpTool.name} with args:`, JSON.stringify(args));
            // For now, return a mock response for demonstration
            return {
              success: true,
              tool: mcpTool.name,
              message: "Tool execution simulated - real implementation coming soon",
              args: args
            };
          } catch (error) {
            console.error(`DEBUG: Error executing MCP tool ${mcpTool.name}:`, error);
            return { error: error instanceof Error ? error.message : String(error) };
          }
        },
      });
      return acc;
    }, {} as Record<string, any>);

    // Combine built-in tools with MCP tools
    this.combinedTools = {
      ...builtInTools,
      ...mcpToolsMap
    };

    console.log("Tool state after refresh:", {
      builtInTools: Object.keys(builtInTools),
      mcpTools: Object.keys(mcpToolsMap),
      combinedTools: Object.keys(this.combinedTools)
    });
  }
  
  // Helper method to get the server for a tool
  getToolServer(tool: Tool & { serverId: string }): string | undefined {
    return tool.serverId;
  }
  
  // Helper method to convert JSON Schema properties to Zod object properties
  convertSchemaToZod(schema: Record<string, any>) {
    const zodProperties: Record<string, any> = {};
    
    // Handle empty or invalid schema
    if (!schema || typeof schema !== 'object') {
      return { _fallback: z.any() };
    }
    
    // Handle case where schema has a properties object
    const properties = schema.properties || schema;
    
    Object.entries(properties).forEach(([key, prop]: [string, any]) => {
      if (!prop || typeof prop !== 'object') {
        zodProperties[key] = z.any();
        return;
      }
      
      let zodProp;
      
      switch (prop.type) {
        case 'string':
          zodProp = z.string();
          break;
        case 'number':
        case 'integer':
          zodProp = z.number();
          break;
        case 'boolean':
          zodProp = z.boolean();
          break;
        case 'array':
          // Simple array handling - can be expanded for complex cases
          zodProp = z.array(z.any());
          break;
        case 'object':
          // For nested objects, recursively convert
          zodProp = z.object(this.convertSchemaToZod(prop.properties || {}));
          break;
        default:
          zodProp = z.any();
      }
      
      // Add description if available
      if (prop.description) {
        zodProp = zodProp.describe(prop.description);
      }
      
      zodProperties[key] = zodProp;
    });
    
    // If no properties were found, add a fallback
    if (Object.keys(zodProperties).length === 0) {
      zodProperties._fallback = z.any();
    }
    
    return zodProperties;
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

    console.log("[onChatMessage] Chat history length:", chatHistory.length);

    // Generate a system prompt based on knowledge base
    const systemPrompt = await this.generateSystemPrompt();

    // Get tools
    const tools = await this.generateTools();
    const toolKeys = Object.keys(tools);

    console.log("[onChatMessage] System prompt length:", systemPrompt.length);
    console.log("[onChatMessage] Tools available:", toolKeys);
    
    // Log the structure of the tools for debugging
    if (toolKeys.length > 0) {
      const sampleTool = tools[toolKeys[0]];
      console.log("[onChatMessage] Sample tool structure:", 
        JSON.stringify(
          { 
            name: toolKeys[0],
            description: sampleTool.description,
            hasParameters: !!sampleTool.parameters,
            hasExecute: typeof sampleTool.execute === 'function'
          }, 
          null, 
          2
        )
      );
    }

    const stream = streamText({
      tools,
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
    return `You are a coding assistant named Coder. Help the user with various software engineering tasks.

${unstable_getSchedulePrompt({ date: new Date() })}
`
  }

  async generateTools() {
    if (!this.combinedTools || Object.keys(this.combinedTools).length === 0) {
      console.log("TOOLS: No tools available, using weather example");
      return {
        weather: tool({
          description: 'Get the weather in a location',
          parameters: z.object({
            location: z.string().describe('The location to get the weather for'),
          }),
          execute: async ({ location }) => ({
            location,
            temperature: 72 + Math.floor(Math.random() * 21) - 10,
          }),
        }),
      };
    }
    
    // Get just the tool names for cleaner logging
    const toolNames = Object.keys(this.combinedTools);
    console.log("TOOLS: Using combined tools:", toolNames);
    
    // More detailed logging for debugging
    console.log("TOOLS: Tool structure example:", 
      JSON.stringify(
        this.combinedTools[toolNames[0]], 
        (key, value) => key === 'execute' ? '[Function]' : value, 
        2
      ).substring(0, 500)
    );
    
    // Create a simpler version of tools for debugging
    const simplifiedTools = {};
    for (const [name, toolDef] of Object.entries(this.combinedTools)) {
      // Only include get_file_contents and weather tools
      if (name === 'get_file_contents' || name === 'weather') {
        simplifiedTools[name] = toolDef;
      }
    }
    
    console.log("TOOLS: Final tool count:", Object.keys(simplifiedTools).length);
    return simplifiedTools;
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
