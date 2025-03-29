import { routeAgentRequest, type Schedule } from "agents";
import { unstable_getSchedulePrompt } from "agents/schedule";
import { AIChatAgent } from "agents/ai-chat-agent";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  type StreamTextOnFinishCallback,
} from "ai";
import { createOpenRouter } from "@openrouter/ai-sdk-provider";
import { processToolCalls } from "./utils";
import { tools, executions } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";

// Import our CoderAgent
import { CoderAgent } from "./coder-agent";

const openrouter = createOpenRouter({
  apiKey: process.env.OPENROUTER_API_KEY,
});
const model = openrouter("openai/gpt-4o-mini");

// We use ALS to expose the agent context to the tools
export const agentContext = new AsyncLocalStorage<Chat>();

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Chat extends AIChatAgent<Env> {
  /**
   * Handles incoming chat messages and manages the response stream
   * @param onFinish - Callback function executed when streaming completes
   */
  // biome-ignore lint/complexity/noBannedTypes: <explanation>
  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    // Create a streaming response that handles both text and tool outputs
    return agentContext.run(this, async () => {
      const dataStreamResponse = createDataStreamResponse({
        execute: async (dataStream) => {
          // Process any pending tool calls from previous messages
          // This handles human-in-the-loop confirmations for tools
          const processedMessages = await processToolCalls({
            messages: this.messages,
            dataStream,
            tools,
            executions,
          });

          // Stream the AI response using GPT-4
          const result = streamText({
            model,
            system: `You are a helpful assistant that can do various tasks...

${unstable_getSchedulePrompt({ date: new Date() })}

If the user asks to schedule a task, use the schedule tool to schedule the task.
`,
            messages: processedMessages,
            tools,
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
 * This supports both the general Chat agent and the specialized CoderAgent
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Check for required API keys
    if (!process.env.OPENROUTER_API_KEY) {
      console.error(
        "OPENROUTER_API_KEY is not set, don't forget to set it locally in .dev.vars, and use `wrangler secret bulk .dev.vars` to upload it to production"
      );
      return new Response("OPENROUTER_API_KEY is not set", { status: 500 });
    }

    // Extract the URL to check if this is a request for the coder agent
    const url = new URL(request.url);

    // Route to the coder agent if the path indicates it
    if (url.pathname.startsWith('/coder')) {
      console.log("Routing request to CoderAgent");

      // Rewrite the URL to use the standard agent routing pattern
      // This is necessary because routeAgentRequest expects a specific format
      const modifiedRequest = new Request(
        new URL(url.pathname.replace('/coder', ''), url.origin).toString(),
        request
      );

      // For simplicity in development, just use the default agent factory
      // The actual implementation will handle proper instantiation
      console.log("Routing to CoderAgent - using default handler");

      // When the CoderAgent is more stable, we'll customize the factory
      return (
        (await routeAgentRequest(modifiedRequest, env)) ||
        new Response("Coder agent not found", { status: 404 })
      );
    }

    // Default route for the standard Chat agent
    return (
      (await routeAgentRequest(request, env)) ||
      new Response("Agent not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
