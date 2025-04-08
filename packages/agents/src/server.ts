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
      
      // Log complete tool for debugging
      console.log(`DEBUG: Full tool definition for ${mcpTool.name}:`, JSON.stringify(mcpTool, null, 2).substring(0, 1000));
      
      // Extract schema from inputSchema if available
      const schemaDetails = mcpTool.inputSchema || {};
      console.log(`DEBUG: Schema details for ${mcpTool.name}:`, JSON.stringify(schemaDetails, null, 2).substring(0, 500));
      
      // Create properties for the tool based on inputSchema
      const properties = {};
      
      // Extract required properties list
      const requiredProps = schemaDetails.required || [];
      console.log(`DEBUG: Required props for ${mcpTool.name}:`, requiredProps);
      
      // Parse schema properties if available
      if (schemaDetails.properties && typeof schemaDetails.properties === 'object') {
        Object.entries(schemaDetails.properties).forEach(([propName, propDef]: [string, any]) => {
          console.log(`DEBUG: Processing property ${propName} for ${mcpTool.name}`);
          
          if (!propDef || typeof propDef !== 'object') {
            properties[propName] = z.any();
            return;
          }
          
          const isRequired = requiredProps.includes(propName);
          const description = propDef.description || `Parameter ${propName}`;
          
          // Create appropriate Zod schema based on type
          let zodProp;
          switch (propDef.type) {
            case 'string':
              zodProp = z.string().describe(description);
              break;
            case 'number':
            case 'integer':
              zodProp = z.number().describe(description);
              break;
            case 'boolean':
              zodProp = z.boolean().describe(description);
              break;
            case 'array':
              zodProp = z.array(z.any()).describe(description);
              break;
            case 'object':
              zodProp = z.record(z.unknown()).describe(description);
              break;
            default:
              zodProp = z.any().describe(description);
          }
          
          // Make optional if not required
          if (!isRequired) {
            zodProp = zodProp.optional();
          }
          
          properties[propName] = zodProp;
        });
      } else {
        // Fallback for get_file_contents if schema isn't available
        if (mcpTool.name === 'get_file_contents') {
          properties.owner = z.string().describe('Repository owner (username or organization)');
          properties.repo = z.string().describe('Repository name');
          properties.path = z.string().describe('Path to the file or directory');
          properties.branch = z.string().optional().describe('Branch to get contents from');
        } else {
          // Default fallback for other tools
          properties._fallback = z.any();
        }
      }
      
      acc[mcpTool.name] = tool({
        description: mcpTool.description || `Execute the ${mcpTool.name} MCP tool`,
        parameters: z.object(properties),
        execute: async (args) => {
          try {
            console.log(`DEBUG: Executing MCP tool ${mcpTool.name} with args:`, JSON.stringify(args));
            
            // Use the MCP tool via mcpClientManager
            if (mcpTool.name === 'get_file_contents') {
              const { owner, repo, path, branch } = args;
              console.log(`MCP TOOL CALL: ${mcpTool.name} for file ${path} from ${owner}/${repo}`);
              
              try {
                // Need to get the proper server ID from the tool
                const serverId = mcpTool.serverId;
                console.log(`MCP TOOL: Using server ID ${serverId} for tool ${mcpTool.name}`);
                
                // Create a tool call directly to the MCP infrastructure
                const mcpClient = this.mcp;
                
                // Get server from the registry
                const serverInfo = this.state.servers[serverId];
                if (!serverInfo) {
                  console.error(`MCP TOOL ERROR: No server info found for ID ${serverId}`);
                  return {
                    error: `No MCP server found with ID ${serverId} for tool ${mcpTool.name}`
                  };
                }
                
                console.log(`MCP TOOL: Server status for ${serverInfo.url}: ${serverInfo.state}`);
                
                // Get the appropriate client for this server
                try {
                  // Manually format arguments in the way the MCP client expects
                  console.log(`MCP TOOL: Calling with args:`, JSON.stringify(args));
                  
                  // Call the tool using a direct client method - with defensive error handling
                  try {
                    // Get any GitHub token
                    const token = this.githubToken || undefined;
                    
                    // Create object compatible with the direct method
                    const result = await mcpClient.callTool(mcpTool.name, args, token);
                    
                    console.log(`MCP TOOL SUCCESS: ${mcpTool.name} returned`, 
                      typeof result === 'object' ? JSON.stringify(result).substring(0, 500) : result);
                    
                    return result;
                  } catch (callError) {
                    console.error(`MCP TOOL CALL ERROR for ${mcpTool.name}:`, callError);
                    // Fall back to direct GitHub API call if MCP call fails
                    console.log(`MCP TOOL: Falling back to direct GitHub API call`);
                    
                    // Implement direct GitHub API call as fallback
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
                      
                      const data = await response.json();
                      
                      // GitHub returns file content as base64
                      if (data.content && data.encoding === 'base64') {
                        console.log(`GITHUB API SUCCESS: Retrieved ${path}, size: ${data.size} bytes`);
                        // Decode base64 content - handle the replacement safely
                        let content = '';
                        try {
                          // Remove newlines and decode
                          const cleanedContent = data.content.replace(/\n/g, '');
                          content = atob(cleanedContent);
                        } catch (decodeError) {
                          console.error(`GITHUB API DECODE ERROR: ${decodeError}`);
                          content = `[Error decoding content: ${decodeError.message}]`;
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
                        return {
                          ...data,
                          note: "Retrieved via direct GitHub API (fallback)"
                        };
                      }
                    } catch (fallbackError) {
                      console.error(`GITHUB API FALLBACK ERROR: ${fallbackError}`);
                      return {
                        error: `MCP tool call failed: ${callError.message}. Fallback also failed: ${fallbackError.message}`
                      };
                    }
                  }
                } catch (clientError) {
                  console.error(`MCP CLIENT ERROR for ${mcpTool.name}:`, clientError);
                  return {
                    error: `Failed to get MCP client: ${clientError.message}`
                  };
                }
              } catch (error) {
                console.error(`MCP TOOL ERROR for ${mcpTool.name}:`, error);
                return {
                  error: `MCP tool execution failed: ${error.message}`
                };
              }
            }
            
            // For other tools, return a mock response
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
