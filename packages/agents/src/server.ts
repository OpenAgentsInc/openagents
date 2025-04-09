import { Agent, routeAgentRequest, unstable_callable } from "agents"
import { type UIMessage, generateId, generateText, experimental_createMCPClient as createMCPClient, type ToolSet } from "ai";
import { env } from "cloudflare:workers";
import { tools } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";
import type { UIPart } from "@openagents/core/src/chat/types";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';

const openrouter = createOpenRouter({
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

  @unstable_callable({
    description: "Load MCP tools for the agent",
    streaming: false
  })
  async loadMCPTools(url: string = "https://mcp-github.openagents.com/sse") {

    console.log("Loading MCP tools from " + url)

    // Create MCP client with a simplified transport configuration
    this.mcpClient = await createMCPClient({
      transport: {
        type: "sse" as const,
        url
      },
      name: "coder-mcp"
    });

    console.log("MCP client created")

    this.tools = await this.mcpClient.tools()

    console.log("Loaded MCP tools: " + Object.keys(this.tools).length)

    return {
      success: true
    };
  }

  @unstable_callable({
    description: "Generate an AI response based on the current messages",
    streaming: true
  })
  async infer() {

    // If there are no tools, load them
    if (Object.keys(this.tools).length === 0) {
      await this.loadMCPTools()
    }

    // Get current state messages
    const messages = this.state.messages || [];

    const result = await generateText({
      system: `You are a helpful assistant with access to tools. When a user asks about weather, ALWAYS use the getWeatherInformation tool - do not make up responses or refuse to help.

      Example interaction:
      User: "what's the weather in austin"
      Assistant: Let me check the weather for you.
      [Uses getWeatherInformation tool with city="austin"]

      Never say you can't help or that you don't have access to weather data - you have the getWeatherInformation tool.`,
      model,
      messages,
      maxTokens: 2500,
      temperature: 0.7,
      tools: this.tools,
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
        const toolResult = result.toolResults?.find(r => r.toolCallId === toolCall.toolCallId);

        // Add the tool call
        messageParts.push({
          type: 'tool-invocation' as const,
          toolInvocation: {
            state: 'call' as const,
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
              toolCallId: toolCall.toolCallId,
              toolName: toolCall.toolName as "getWeatherInformation",
              args: toolCall.args,
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
