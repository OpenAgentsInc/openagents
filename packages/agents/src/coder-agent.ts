import { routeAgentRequest, type Schedule } from "agents";
import { unstable_getSchedulePrompt } from "agents/schedule";
import { AIChatAgent } from "agents/ai-chat-agent";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  type StreamTextOnFinishCallback,
} from "ai";
import { processToolCalls } from "./utils";
import { tools, executions } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";

// we use ALS to expose the agent context to the tools
export const agentContext = new AsyncLocalStorage<CoderAgent>();

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class CoderAgent extends AIChatAgent<Env> {
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

          // Get the AI environment from the Durable Object's environment
          const AI = this.env.AI;

          if (!AI) {
            console.error("AI binding not available");
            throw new Error("AI binding not available");
          }

          // Create a wrapper for streamText to use Cloudflare AI
          const result = streamText({
            model: {
              invoke: async ({ messages, tools: toolsInput, stream }) => {
                // Convert tools format if necessary for CF Workers AI
                const cfTools = toolsInput?.map(tool => ({
                  type: "function",
                  function: {
                    name: tool.name,
                    description: tool.description,
                    parameters: tool.parameters,
                  }
                }));

                try {
                  const response = await AI.run("@cf/meta/llama-4-scout-17b-16e-instruct", {
                    messages,
                    tools: cfTools,
                    stream: true,
                    max_tokens: 2048,
                    temperature: 0.7,
                  });

                  if (stream) {
                    // Process the streaming response from Cloudflare Workers AI
                    return {
                      type: "stream",
                      stream: response
                    };
                  } else {
                    // For non-streaming, get the full response
                    const result = await response.text();
                    return { text: result };
                  }
                } catch (error) {
                  console.error("Error calling Llama model:", error);
                  throw error;
                }
              }
            },
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
