import { Agent, routeAgentRequest, unstable_callable } from "agents"
import { type UIMessage, generateId, generateText, experimental_createMCPClient as createMCPClient, type ToolSet } from "ai";
import { env } from "cloudflare:workers";
import { tools } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";
import type { UIPart } from "@openagents/core/src/chat/types";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';

const openrouter = createOpenRouter({
  // @ts-ignore - env type error
  apiKey: env.OPENROUTER_API_KEY,
})

const model = openrouter("google/gemini-2.5-pro-preview-03-25");

export const agentContext = new AsyncLocalStorage<Coder>();

interface CoderState {
  messages: UIMessage[];
}

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Coder extends Agent<Env, CoderState> {
  // Define initial state
  mcpClient?: Awaited<ReturnType<typeof createMCPClient>>;
  initialState: CoderState = {
    messages: []
  };

  tools: ToolSet = {};

  githubToken?: string;

  // add two unstable callablles, one to set a github token, and one to get the github token
  @unstable_callable({
    description: "Set the github token for the agent",
    streaming: false
  })
  async setGithubToken(token: string) {
    this.githubToken = token;

    // If tools are already loaded, re-wrap them with the new token
    if (Object.keys(this.tools).length > 0 && this.mcpClient) {
      console.log("Re-wrapping existing tools with new token");
      const rawTools = await this.mcpClient.tools();
      this.wrapAndSetTools(rawTools);
    }
  }

  @unstable_callable({
    description: "Get the github token for the agent",
    streaming: false
  })
  async getGithubToken() {
    return this.githubToken;
  }

  /**
   * Helper method to wrap a tool with authentication
   */
  private wrapTool(rawToolDefinition: any): any {
    console.log(`Wrapping tool: ${rawToolDefinition.name}`);

    // Ensure the raw definition has the expected structure
    if (!rawToolDefinition.name || !rawToolDefinition.description || !rawToolDefinition.parameters) {
      console.error("Invalid raw tool definition:", rawToolDefinition);
      throw new Error(`Invalid tool definition received for ${rawToolDefinition.name || 'unknown tool'}`);
    }

    return {
      // Use definition from raw tool
      description: rawToolDefinition.description,
      parameters: rawToolDefinition.parameters,
      // The execute function calls the MCP server with auth headers
      execute: async (args: any): Promise<any> => {
        console.log(`Executing wrapped tool: ${rawToolDefinition.name}`);
        if (!this.mcpClient) {
          console.error("MCP Client not initialized when executing tool!");
          throw new Error("MCP Client connection is not available.");
        }

        // Prepare headers, adding Authorization only if token exists
        const headers: Record<string, string> = {};
        if (this.githubToken) {
          console.log(`Adding Authorization header for ${rawToolDefinition.name}`);
          headers['Authorization'] = `Bearer ${this.githubToken}`;
        } else {
          console.log(`No GitHub token available for ${rawToolDefinition.name}`);
        }

        try {
          // Call the actual tool on the MCP server
          // @ts-ignore - mcpClient.callTool is marked as private in type definitions
          const result = await this.mcpClient.callTool({
            name: rawToolDefinition.name,
            arguments: args, // Pass the original arguments
            headers: headers // Pass the constructed headers
          });

          console.log(`Wrapped tool ${rawToolDefinition.name} execution successful.`);

          // The result from callTool likely needs parsing/checking
          if (result?.content?.[0]?.type === 'text') {
            try {
              return JSON.parse(result.content[0].text);
            } catch (parseError) {
              console.error(`Failed to parse JSON result for ${rawToolDefinition.name}:`, result.content[0].text);
              return { error: "Failed to parse response from tool server." };
            }
          }

          return result; // Return raw result if not expected format
        } catch (error) {
          console.error(`Error executing wrapped tool ${rawToolDefinition.name} via mcpClient:`, error);
          throw error; // Re-throw or format error
        }
      }
    };
  }

  /**
   * Helper method to wrap all tools and set them on the client
   */
  private wrapAndSetTools(rawTools: any) {
    // Reset wrapped tools
    const wrappedTools: ToolSet = {};

    // Wrap each tool
    for (const [name, rawToolDef] of Object.entries(rawTools)) {
      try {
        wrappedTools[name] = this.wrapTool(rawToolDef);
      } catch (wrapError) {
        console.error(`Failed to wrap tool ${name}:`, wrapError);
        // Skip this tool
      }
    }

    // Store the WRAPPED tools on the agent instance
    this.tools = wrappedTools;
    console.log(`Loaded and wrapped ${Object.keys(this.tools).length} tools.`);
    return wrappedTools;
  }

  @unstable_callable({
    description: "Load MCP tools for the agent",
    streaming: false
  })
  async loadMCPTools(url: string = "https://mcp-github.openagents.com/sse") {
    console.log("Loading MCP tools from " + url);

    // Create MCP client WITHOUT token in URL
    this.mcpClient = await createMCPClient({
      transport: {
        type: "sse" as const,
        url: url, // No token in URL
        // No Authorization header at initialization
      },
      name: "coder-mcp"
    });

    console.log("MCP client created (no token during setup)");

    // Get RAW tool definitions from the server
    const rawTools = await this.mcpClient.tools();
    console.log(`Received ${Object.keys(rawTools).length} raw tool definitions.`);

    // Wrap and set the tools
    const tools = this.wrapAndSetTools(rawTools);

    return { success: true, tools }; // Return success
  }

  @unstable_callable({
    description: "Generate an AI response based on the current messages",
    streaming: true
  })
  async infer() {
    return agentContext.run(this, async () => {
      // If there are no tools, load them
      const loaded = await this.loadMCPTools()
      const tools = loaded.tools

      const stringListOfAllToolNames = Object.keys(tools).join(", ")
      console.log("Loaded tools: " + stringListOfAllToolNames)

      // Get current state messages
      const messages = this.state.messages || [];

      const result = await generateText({
        system: `You are a helpful assistant with access to tools. Tell the user which tools you have access to.

      Your tools are: ${stringListOfAllToolNames}
    `,
        model,
        messages,
        maxTokens: 2500,
        temperature: 0.7,
        tools,
        maxSteps: 5,
        toolChoice: "auto"
      });

      // Create message parts array to handle both text and tool calls
      const messageParts: UIPart[] = [];

      // Add text part if there is text content
      if (result.text) {
        messageParts.push({
          type: 'text' as const,
          text: result.text
        });
      }

      // Add tool calls and their results together
      if (result.toolCalls && result.toolCalls.length > 0) {
        for (const toolCall of result.toolCalls) {
          // @ts-ignore - toolCall type issue
          const toolResult = result.toolResults?.find(r => r.toolCallId === toolCall.toolCallId);

          // Add the tool call
          messageParts.push({
            type: 'tool-invocation' as const,
            toolInvocation: {
              state: 'call' as const,
              // @ts-ignore - toolCall type issue
              toolCallId: toolCall.toolCallId,
              toolName: toolCall.toolName as "getWeatherInformation",
              args: toolCall.args
            }
          });

          // Immediately add its result if available
          if (toolResult) {
            messageParts.push({
              type: 'tool-invocation' as const,
              toolInvocation: {
                state: 'result' as const,
                // @ts-ignore - toolCall type issue
                toolCallId: toolCall.toolCallId,
                toolName: toolCall.toolName as "getWeatherInformation",
                args: toolCall.args,
                // @ts-ignore - toolResult type issue
                result: toolResult.result
              }
            });
          }
        }
      }

      // Update state with the new message containing all parts
      this.setState({
        messages: [
          ...messages,
          {
            id: generateId(),
            role: 'assistant' as const,
            content: result.text || '',
            createdAt: new Date(),
            parts: messageParts
          }
        ]
      });

      return {};
    })
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
