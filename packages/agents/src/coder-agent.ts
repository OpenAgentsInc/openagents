// @ts-nocheck
import { routeAgentRequest, type Schedule } from "agents";
import { unstable_getSchedulePrompt } from "agents/schedule";
import { AIChatAgent } from "agents/ai-chat-agent";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  type StreamTextOnFinishCallback,
} from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { processToolCalls } from "./utils";
import { tools, executions } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";

// we use ALS to expose the agent context to the tools
export const agentContext = new AsyncLocalStorage<CoderAgent>();

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class CoderAgent extends AIChatAgent<Env> {
  // Track the current repository/project context
  projectContext: {
    repoOwner?: string;
    repoName?: string;
    branch?: string;
    path?: string;
  } = {};

  /**
   * Set the project context for this agent
   */
  async setProjectContext(context: {
    repoOwner?: string;
    repoName?: string;
    branch?: string;
    path?: string;
  }) {
    console.log("Setting project context:", JSON.stringify(context));
    this.projectContext = { ...this.projectContext, ...context };
    console.log("Updated project context:", JSON.stringify(this.projectContext));

    // Return the updated context to confirm success
    return {
      success: true,
      context: this.projectContext
    };
  }

  /**
   * Handles incoming chat messages and manages the response stream
   * @param onFinish - Callback function executed when streaming completes
   */
  // biome-ignore lint/complexity/noBannedTypes: <explanation>
  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    // Create a streaming response that handles both text and tool outputs
    return agentContext.run(this, async () => {
      console.log("🔄 Starting execution in CoderAgent");

      // Create a data stream response
      const dataStreamResponse = createDataStreamResponse({
        execute: async (dataStream) => {
          try {
            console.log("🔄 Processing messages for", this.messages.length, "messages");

            // Process any pending tool calls from previous messages
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

            // Initialize Workers AI provider with the binding
            const workersai = createWorkersAI({ binding: AI });

            // Create model for Claude
            const model = workersai('@cf/meta/llama-4-scout-17b-16e-instruct');

            console.log("✅ Initialized Workers AI provider");

            // Use streamText with the proper provider - much simpler and more reliable
            const result = streamText({
              model: model,
              system: `You are a helpful assistant that can do various tasks.
                      You're specialized in helping with code and programming questions.

                      ${unstable_getSchedulePrompt({ date: new Date() })}

                      If the user asks to schedule a task, use the schedule tool to schedule the task.`,
              messages: processedMessages,
              tools,
              onFinish,
              onError: (error) => {
                // Log the error
                console.error("❌ Error in streamText:", error);

                try {
                  // Get user-friendly error message
                  const errorMessage = error instanceof Error
                    ? error.message
                    : typeof error === 'string'
                      ? error
                      : JSON.stringify(error);

                  // Send error message to user
                  dataStream.write({
                    type: "text",
                    text: `\n\nI apologize, but I encountered an error: ${errorMessage}\n\nPlease try again with a simpler request.`
                  });
                } catch (writeError) {
                  console.error("❌ Failed to write error message:", writeError);
                }
              },
              maxSteps: 10,
            });

            // Merge the AI response with the data stream
            console.log("🔄 Merging result stream");
            result.mergeIntoDataStream(dataStream);
            console.log("✅ Successfully merged streams");

          } catch (error) {
            console.error("❌ Critical error in execute function:", error);

            try {
              // User-friendly fallback for critical errors
              dataStream.write({
                type: "text",
                text: "\n\nI apologize, but I encountered a system error. Please try again in a few moments."
              });
            } catch (writeError) {
              console.error("❌ Failed to write fallback message:", writeError);
            }
          }
        }
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
