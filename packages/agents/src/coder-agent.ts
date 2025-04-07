// @ts-nocheck - Keeping this for now, but aim to resolve TS issues later

// Core AI SDK imports
import {
  createDataStreamResponse,
  generateId, // Keep if used in executeTask or elsewhere
  streamText,
  type StreamTextOnFinishCallback
} from 'ai';

// Agent-specific imports
import { AIChatAgent } from "agents/ai-chat-agent";
import { type Schedule } from "agents"; // Keep for executeTask
import { unstable_getSchedulePrompt } from "agents/schedule"; // For system prompt

// Cloudflare provider import
import { createWorkersAI } from 'workers-ai-provider';

// Tool handling imports (uncomment if using tools)
import { processToolCalls } from "./utils";
import { tools, executions } from "./tools";

// Context import
import { AsyncLocalStorage } from "node:async_hooks";

// Maintain the AsyncLocalStorage for context if tools need agent access
export const agentContext = new AsyncLocalStorage<CoderAgent>();

/**
 * CoderAgent using Cloudflare Workers AI provider and streamText
 */
export class CoderAgent extends AIChatAgent<Env> {

  // Keep project context if needed
  projectContext: {
    repoOwner?: string;
    repoName?: string;
    branch?: string;
    path?: string;
  } = {};

  async setProjectContext(context: { /* ... */ }) {
    // ... implementation from your original code
    console.log("Setting project context:", JSON.stringify(context));
    this.projectContext = { ...this.projectContext, ...context };
    console.log("Updated project context:", JSON.stringify(this.projectContext));
    return { success: true, context: this.projectContext };
  }


  /**
   * Handles incoming chat messages and manages the response stream
   * @param onFinish - Callback function executed when streaming completes
   */
  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    return agentContext.run(this, async () => {
      console.log("🔄 CoderAgent.onChatMessage: Creating data stream response.");

      const dataStreamResponse = createDataStreamResponse({
        execute: async (dataStream) => {
          try {
            console.log(`🔄 Executing stream for ${this.messages.length} messages.`);

            // --- 1. Get AI Binding ---
            const AI = this.env.AI;
            if (!AI) {
              console.error("🔴 AI binding not available in environment.");
              dataStream.write({ type: "error", error: "AI environment binding is missing." });
              dataStream.close(); // Close stream on fatal error
              return;
            }

            // --- 2. Initialize WorkersAI Provider ---
            const workersai = createWorkersAI({ binding: AI });
            // Choose your model
            const model = workersai('@cf/meta/llama-4-scout-17b-16e-instruct');
            // Or: const model = workersai('@cf/anthropic/claude-3-haiku-20240307');
            console.log(`✅ Initialized Workers AI provider with model: ${model.modelId}`);

            // --- 3. Process Messages & Tools (Re-enable if needed) ---
            console.log("🔧 Processing tool calls (if any)...");
            const processedMessages = await processToolCalls({
              messages: this.messages,
              dataStream, // For potential UI updates during tool execution
              tools,
              executions,
            });
            // If NOT using tools yet, simplify:
            // const processedMessages = this.messages;
            console.log(`✉️ Using ${processedMessages.length} processed messages.`);


            // --- 4. Define System Prompt ---
            const systemPrompt = `You are a helpful Coder assistant specialized in code and programming questions.
${unstable_getSchedulePrompt({ date: new Date() })}
If the user asks to schedule a task, use the schedule tool if appropriate.`;

            // --- 5. Call streamText ---
            console.log("🚀 Calling streamText with AI model...");
            const result = await streamText({
              model: model, // Use the initialized workersai model
              system: systemPrompt,
              messages: processedMessages,
              tools: tools, // Pass tool definitions
              onFinish: (result) => {
                console.log("✅ streamText finished successfully.");
                onFinish(result); // Call the original callback
              },
              onError: (error) => {
                // Handles errors *during* the streamText execution (e.g., API errors)
                console.error("❌ Error during streamText execution:", error);
                const errorMessage = error instanceof Error ? error.message : String(error);
                // Send structured error via the data stream
                try {
                  dataStream.write({ type: "error", error: `AI Stream Error: ${errorMessage}` });
                } catch (writeErr) {
                  console.error("Failed to write streamText error to dataStream:", writeErr);
                }
              },
              maxSteps: 10, // Control tool execution loops
            });
            console.log("🏁 streamText call completed.");

            // --- 6. Merge Result into Data Stream ---
            console.log("🔗 Merging AI result stream into the main data stream...");
            await result.mergeIntoDataStream(dataStream);
            console.log("✅ Result stream merged.");

          } catch (error) {
            // Catches errors during setup (before streamText) or unexpected issues
            console.error("❌ Critical error in CoderAgent execute block:", error);
            const errorMessage = error instanceof Error ? error.message : String(error);
            try {
              // Send structured error via the data stream
              dataStream.write({ type: "error", error: `Server Setup Error: ${errorMessage}` });
            } catch (writeError) {
              console.error("❌ Failed to write critical error to data stream:", writeError);
            }
          } finally {
            // Ensure the stream is always closed from the server-side
            console.log("🚪 Closing data stream from server.");
            try {
              dataStream.close();
            } catch (closeErr) {
              // May already be closed or errored, ignore
            }
          }
        }, // End execute
      }); // End createDataStreamResponse

      return dataStreamResponse;
    }); // End agentContext.run
  } // End onChatMessage

  // Keep executeTask method for scheduled tasks functionality
  async executeTask(description: string, task: Schedule<string>) {
    console.log(`⚡ Executing scheduled task: ${description}`);
    await this.saveMessages([
      ...this.messages,
      {
        id: generateId(), // Ensure generateId is imported or defined
        role: "user", // Consider if 'system' role is more appropriate
        content: `Running scheduled task: ${description}`,
        createdAt: new Date(),
      },
    ]);
    // Add logic here to actually perform the task or trigger another AI interaction
  }
}
