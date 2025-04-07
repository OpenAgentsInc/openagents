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

            // Select a reliable model
            const model = workersai('@cf/meta/llama-4-scout-17b-16e-instruct');
            console.log(`✅ Using model: @cf/meta/llama-4-scout-17b-16e-instruct`);

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
                // Log the error with details
                console.error("❌ streamText encountered an error:", error);

                // Create detailed error information for logging
                const errorDetails = error instanceof Error
                  ? { name: error.name, message: error.message, stack: error.stack }
                  : { raw: String(error) };
                console.error("❌ Error details:", JSON.stringify(errorDetails, null, 2));

                try {
                  // Get user-friendly error message
                  const errorMessage = error instanceof Error
                    ? error.message
                    : typeof error === 'string'
                      ? error
                      : JSON.stringify(error);

                  // Send structured error using the data stream protocol format
                  // This is important for the useAgentChat hook to properly handle errors
                  dataStream.write({
                    type: "error",
                    error: `AI Stream Error: ${errorMessage}`
                  });

                  // Also send a user-friendly text message
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
            await result.mergeIntoDataStream(dataStream);
            console.log("✅ Successfully merged streams");

          } catch (error) {
            // Catch critical errors during the execution setup
            console.error("❌ Critical error in dataStream execute function:", error);
            const errorMessage = error instanceof Error ? error.message : String(error);

            try {
              // Send a structured error message back to the client
              dataStream.write({
                type: "error",
                error: `Server Error: ${errorMessage}`
              });

              // Also send a user-friendly text message
              dataStream.write({
                type: "text",
                text: "\n\nI apologize, but I encountered a system error. Please try again in a few moments."
              });
            } catch (writeError) {
              console.error("❌ Failed to write critical error message to data stream:", writeError);
            }
          } finally {
            // Ensure the data stream is closed properly, even if errors occurred
            try {
              // No need to explicitly close here as createDataStreamResponse handles it
              // But we could if needed: dataStream.close();
            } catch (closeErr) {
              // Ignore errors during close if it's already closed or errored
            }
          }
        }
      });

      return dataStreamResponse;
    });
  }

  async executeTask(description: string, task: Schedule<string>) {
    console.log(`⚡ Executing scheduled task: ${description}`);
    await this.saveMessages([
      ...this.messages,
      {
        id: generateId(),
        role: "user",
        content: `Running scheduled task: ${description}`,
        createdAt: new Date(),
      },
    ]);
    console.log("✅ Scheduled task message added to conversation history");
  }
}
