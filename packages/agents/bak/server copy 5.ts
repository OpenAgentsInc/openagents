import { Agent, routeAgentRequest, type Connection, type Schedule, type WSMessage } from "agents";
import { unstable_getSchedulePrompt } from "agents/schedule";
import { AIChatAgent } from "agents/ai-chat-agent";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  tool,
  experimental_createMCPClient as createMCPClient,
  type StreamTextOnFinishCallback,
  type ToolExecutionOptions,
} from "ai";
import { processToolCalls } from "./utils";
import { AsyncLocalStorage } from "node:async_hooks";
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { env } from "cloudflare:workers";
import { tools as builtInTools, executions } from "./tools";

// Keep old MCP types for state compatibility
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
  mcpClient?: Awaited<ReturnType<typeof createMCPClient>>;

  /**
   * MCP STUFF
   */

  initialState = {
    servers: {},
    tools: [],
    prompts: [],
    resources: [],
  };

  async onStart() {
    console.log("Initializing Coder agent...");

    // Initialize combinedTools with built-in tools
    this.combinedTools = { ...builtInTools };

    try {
      // Create MCP client with a simplified transport configuration
      this.mcpClient = await createMCPClient({
        transport: {
          type: 'sse' as const,
          url: "https://mcp-github.openagents.com/sse"
        },
        name: "coder"
      });
      
      console.log("Vercel AI SDK MCP client initialized");

      // Fetch tools from MCP
      await this.refreshServerData();
      
      console.log("MCP tools loaded, tool state:", {
        combinedToolCount: this.combinedTools ? Object.keys(this.combinedTools).length : 0,
        toolNames: this.combinedTools ? Object.keys(this.combinedTools) : []
      });
    } catch (error) {
      console.error("Failed to initialize MCP client:", error);
      // We'll continue with just the built-in tools
    }
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
    // Attempt to ensure MCP client is initialized
    if (!this.mcpClient) {
      console.log("MCP client not initialized, attempting to initialize it");
      try {
        this.mcpClient = await createMCPClient({
          transport: {
            type: 'sse' as const,
            url: "https://mcp-github.openagents.com/sse"
          },
          name: "coder"
        });
        console.log("MCP client initialized during refresh");
      } catch (error) {
        console.error("Failed to initialize MCP client during refresh:", error);
        return;
      }
    }

    try {
      console.log("Checking MCP client status...");
      
      // Test the connection
      try {
        // A small operation to test if the client is connected
        const testTools = await this.mcpClient.tools();
        console.log(`MCP client connection verified, found ${Object.keys(testTools).length} tools`);
      } catch (error) {
        console.log("MCP client connection failed, reinitializing...");
        // Reinitialize the client if the test failed
        this.mcpClient = await createMCPClient({
          transport: {
            type: 'sse' as const,
            url: "https://mcp-github.openagents.com/sse"
          },
          name: "coder"
        });
        console.log("MCP client reinitialized");
      }
      
      console.log("Fetching MCP tools...");
      
      // Get tools from the MCP client using Vercel AI SDK
      // This automatically converts MCP tools to Vercel AI SDK tools
      const mcpTools = await this.mcpClient.tools();
      
      console.log("MCP tools retrieved:", Object.keys(mcpTools));
      
      // Process all MCP tools
      const processedTools: Record<string, any> = {};
      for (const [name, toolDef] of Object.entries(mcpTools)) {
        console.log(`Including MCP tool: ${name}`);
        
        // Type assertion for toolDef
        const typedToolDef = toolDef as {
          description: string;
          parameters: any;
          execute: (args: any, options?: any) => Promise<any>;
        };
        
        // Get the token from the context for use in the closure
        const coderAgent = this;
        
        // Create a tool wrapper
        const toolWrapper = tool({
          // Use the original description and parameters
          description: typedToolDef.description,
          parameters: typedToolDef.parameters,
          // Simple execute function with clear error handling
          execute: async (args, options) => {
            try {
              // Get fresh token value at execution time
              const githubToken = coderAgent.githubToken;
              
              // Add GitHub token to arguments
              const argsWithToken = githubToken ? { ...args, token: githubToken } : args;
              
              // Simple logging
              console.log(`Executing MCP tool ${name} with token: ${githubToken ? "YES" : "NO"}`);
              
              // Execute the tool with token included
              return await typedToolDef.execute(argsWithToken, options);
            } catch (error: unknown) {
              // Create a user-friendly error message
              const errorMessage = error instanceof Error ? error.message : String(error);
              console.error(`MCP tool execution failed for ${name}: ${errorMessage}`);
              
              // Return error to user
              return {
                error: `The GitHub tool "${name}" failed: ${errorMessage}`,
                toolName: name,
                args: args
              };
            }
          }
        });
        
        // Add the tool to our processed tools
        processedTools[name] = toolWrapper;
      }

      // Get the weather example tool and combine with MCP tools
      const weatherTool = tool({
        description: 'Get the weather in a location',
        parameters: z.object({
          location: z.string().describe('The location to get the weather for'),
        }),
        execute: async ({ location }) => ({
          location,
          temperature: 72 + Math.floor(Math.random() * 21) - 10,
        }),
      });

      // Combine built-in tools with MCP tools
      this.combinedTools = {
        ...builtInTools,
        ...processedTools,
        weather: weatherTool // Always include the weather tool for testing
      };

      console.log("Tool state after refresh:", {
        builtInTools: Object.keys(builtInTools),
        mcpTools: Object.keys(processedTools),
        combinedTools: Object.keys(this.combinedTools)
      });
      
      if (Object.keys(processedTools).length > 0) {
        console.log("Sample MCP tool structure:", 
          JSON.stringify({
            name: Object.keys(processedTools)[0],
            hasParameters: !!processedTools[Object.keys(processedTools)[0]].parameters,
            hasExecute: !!processedTools[Object.keys(processedTools)[0]].execute
          }, null, 2)
        );
      }
    } catch (error) {
      console.error("Error refreshing MCP tools:", error);
      
      // Fall back to built-in tools and weather example
      this.combinedTools = {
        ...builtInTools,
        weather: tool({
          description: 'Get the weather in a location',
          parameters: z.object({
            location: z.string().describe('The location to get the weather for'),
          }),
          execute: async ({ location }) => ({
            location,
            temperature: 72 + Math.floor(Math.random() * 21) - 10,
          }),
        })
      };
      
      console.log("Using fallback tools:", Object.keys(this.combinedTools));
    }
  }
  
  // Update the MCP client with a GitHub token if needed
  async updateGitHubToken(token: string) {
    // Verify token format and log safely
    if (token && typeof token === 'string') {
      const tokenPrefix = token.substring(0, 8);
      const tokenLength = token.length;
      console.log(`TOKEN_UPDATE: Valid token received. Starts with: ${tokenPrefix}..., length: ${tokenLength}`);
      
      // Set the token on the instance
      this.githubToken = token;
      console.log("GitHub token stored in agent instance");
    } else {
      console.log(`TOKEN_UPDATE: Invalid token received: ${token ? "non-string value" : "null or undefined"}`);
      return;
    }
    
    if (this.mcpClient) {
      try {
        // Create a new MCP client with a simplified transport configuration
        // Note: The token handling will be managed by the MCP client internally
        this.mcpClient = await createMCPClient({
          transport: {
            type: 'sse' as const,
            url: "https://mcp-github.openagents.com/sse"
          },
          name: "coder"
        });
        
        console.log("MCP client re-initialized with GitHub token");
        
        // Refresh tools with the new authenticated client
        await this.refreshServerData();
        
        // Verify that token is available in the instance after refresh
        if (this.githubToken) {
          const tokenPrefix = this.githubToken.substring(0, 8);
          console.log(`TOKEN_VERIFY: After refresh, token is still present. Prefix: ${tokenPrefix}...`);
        } else {
          console.log(`TOKEN_VERIFY: WARNING - Token is no longer present after refresh!`);
        }
      } catch (error) {
        console.error("Failed to update MCP client with GitHub token:", error);
      }
    }
  }
  
  // Override the onMessage method to handle GitHub token updates
  override async onMessage(connection: Connection, message: WSMessage) {
    // Call the parent method first
    await super.onMessage(connection, message);
    
    // Cast message to any to allow property access (avoid TypeScript errors)
    const msg = message as any;
    
    // Only check for githubToken in object messages
    if (typeof msg === 'object' && msg !== null) {
      // Try to extract token from data
      try {
        if (msg.data && msg.data.githubToken) {
          console.log("Found githubToken in message.data");
          this.githubToken = msg.data.githubToken;
        } else if (msg.body && msg.body.githubToken) {
          console.log("Found githubToken in message.body");
          this.githubToken = msg.body.githubToken;
        }
      } catch (e) {
        console.error("Error extracting token:", e);
      }
    }
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
      console.log("TOOLS: No tools available, using weather example only");
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
    
    // In this implementation, we'll use all available tools
    // This works because the Vercel AI SDK tools are already properly formatted
    return this.combinedTools;
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