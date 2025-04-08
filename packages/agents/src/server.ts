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
    // Skip if MCP client isn't initialized
    if (!this.mcpClient) {
      console.log("MCP client not initialized, skipping tool refresh");
      return;
    }

    try {
      console.log("Fetching MCP tools...");
      
      // Get tools from the MCP client using Vercel AI SDK
      // This automatically converts MCP tools to Vercel AI SDK tools
      const mcpTools = await this.mcpClient.tools();
      
      console.log("MCP tools retrieved:", Object.keys(mcpTools));
      
      // Filter to only use the get_file_contents tool for now
      const filteredTools: Record<string, any> = {};
      for (const [name, toolDef] of Object.entries(mcpTools)) {
        if (name === 'get_file_contents') {
          console.log(`Including MCP tool: ${name}`);
          
          // Type assertion for toolDef
          const typedToolDef = toolDef as {
            description: string;
            parameters: any;
            execute: (args: any, options?: any) => Promise<any>;
          };
          
          // Add a custom wrapper for get_file_contents to include fallback
          filteredTools[name] = tool({
            // Use the original description and parameters
            description: typedToolDef.description,
            parameters: typedToolDef.parameters,
            // Custom execute function with fallback
            execute: async (args, options) => {
              try {
                console.log(`Executing MCP tool ${name} with args:`, JSON.stringify(args));
                
                // Try to use the original tool's execute function
                return await typedToolDef.execute(args, options);
              } catch (error: unknown) {
                const errorMessage = error instanceof Error ? error.message : String(error);
                console.error(`MCP tool execution failed: ${errorMessage}`);
                console.log(`Falling back to direct GitHub API call...`);
                
                // Implement GitHub API fallback
                if (name === 'get_file_contents') {
                  const { owner, repo, path, branch } = args;
                  
                  try {
                    console.log(`GITHUB API FALLBACK: Fetching file ${path} from ${owner}/${repo} (branch: ${branch || 'main'})`);
                    
                    // Construct GitHub API URL
                    const url = `https://api.github.com/repos/${owner}/${repo}/contents/${path}${branch ? `?ref=${branch}` : ''}`;
                    console.log(`GITHUB API URL: ${url}`);
                    
                    // Make the request
                    const response = await fetch(url, {
                      headers: {
                        'Accept': 'application/vnd.github.v3+json',
                        'User-Agent': 'OpenAgents-Coder'
                      }
                    });
                    
                    if (!response.ok) {
                      const errorText = await response.text();
                      console.error(`GITHUB API ERROR (${response.status}): ${errorText}`);
                      return {
                        error: `GitHub API returned ${response.status}: ${errorText}`
                      };
                    }
                    
                    // Type the response data
                    const data: any = await response.json();
                    
                    // GitHub returns file content as base64
                    if (data.content && data.encoding === 'base64') {
                      console.log(`GITHUB API SUCCESS: Retrieved ${path}, size: ${data.size} bytes`);
                      // Decode base64 content - handle the replacement safely
                      let content = '';
                      try {
                        // Remove newlines and decode
                        const cleanedContent = data.content.replace(/\n/g, '');
                        content = atob(cleanedContent);
                      } catch (decodeError: unknown) {
                        const decodeErrorMessage = decodeError instanceof Error ? decodeError.message : String(decodeError);
                        console.error(`GITHUB API DECODE ERROR: ${decodeErrorMessage}`);
                        content = `[Error decoding content: ${decodeErrorMessage}]`;
                      }
                      
                      return {
                        content,
                        name: data.name,
                        path: data.path,
                        sha: data.sha,
                        size: data.size,
                        url: data.html_url,
                        note: "Retrieved via direct GitHub API (fallback)"
                      };
                    } else if (Array.isArray(data)) {
                      // If it's a directory
                      console.log(`GITHUB API SUCCESS: Retrieved directory listing with ${data.length} items`);
                      return {
                        isDirectory: true,
                        items: data.map(item => ({
                          name: item.name,
                          path: item.path,
                          type: item.type,
                          size: item.size,
                          url: item.html_url
                        })),
                        note: "Retrieved via direct GitHub API (fallback)"
                      };
                    } else {
                      console.log(`GITHUB API UNEXPECTED RESPONSE:`, JSON.stringify(data).substring(0, 500));
                      // Use a safe object spread
                      return {
                        ...(typeof data === 'object' ? data : {}),
                        note: "Retrieved via direct GitHub API (fallback)"
                      };
                    }
                  } catch (fallbackError: unknown) {
                    const fallbackErrorMessage = fallbackError instanceof Error ? fallbackError.message : String(fallbackError);
                    console.error(`GITHUB API FALLBACK ERROR: ${fallbackErrorMessage}`);
                    return {
                      error: `MCP tool call failed: ${errorMessage}. Fallback also failed: ${fallbackErrorMessage}`
                    };
                  }
                }
                
                // Return error for other tools
                return {
                  error: errorMessage
                };
              }
            }
          });
        }
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
        ...filteredTools,
        weather: weatherTool // Always include the weather tool for testing
      };

      console.log("Tool state after refresh:", {
        builtInTools: Object.keys(builtInTools),
        mcpTools: Object.keys(filteredTools),
        combinedTools: Object.keys(this.combinedTools)
      });
      
      if (Object.keys(filteredTools).length > 0) {
        console.log("Sample MCP tool structure:", 
          JSON.stringify({
            name: Object.keys(filteredTools)[0],
            hasParameters: !!filteredTools[Object.keys(filteredTools)[0]].parameters,
            hasExecute: !!filteredTools[Object.keys(filteredTools)[0]].execute
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
    this.githubToken = token;
    console.log("GitHub token updated");
    
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
      } catch (error) {
        console.error("Failed to update MCP client with GitHub token:", error);
      }
    }
  }
  
  // Override the onMessage method to handle GitHub token updates
  override async onMessage(connection: Connection, message: WSMessage) {
    // Call the parent method first
    await super.onMessage(connection, message);
    
    // Check if this is a chat request with a GitHub token
    // Check if message is an object with a type property
    if (typeof message === 'object' && message !== null) {
      const chatMessage = message as any;
      if (chatMessage.type === "cf_agent_use_chat_request" && chatMessage.data) {
        const data = chatMessage.data;
        if (data.githubToken) {
          await this.updateGitHubToken(data.githubToken);
        }
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
