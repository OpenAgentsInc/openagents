// @ts-nocheck - Focusing on simplification

// --- Core Imports ---
import {
  createDataStreamResponse,
  generateId, // For executeTask
  generateText, // Use this instead of streamText
  type StreamTextOnFinishCallback, // Still needed for the method signature
  type ToolSet // Import ToolSet for the signature
} from 'ai';
import { AIChatAgent } from "agents/ai-chat-agent";
import { type Schedule } from "agents";
import { createWorkersAI } from 'workers-ai-provider';
import { AsyncLocalStorage } from "node:async_hooks";

// --- Define Env Type ---
type Env = {
  AI: any; // Cloudflare AI binding
};

// --- Agent Context ---
export const agentContext = new AsyncLocalStorage<CoderAgent>();

/**
 * EXTREMELY SIMPLIFIED CoderAgent using generateText
 * This will NOT stream responses token-by-token.
 * Tool support is removed in this simplified version.
 */
export class CoderAgent extends AIChatAgent<Env> {

  // Keep project context methods if needed elsewhere
  projectContext: { /* ... */ } = {};
  async setProjectContext(context: { /* ... */ }) { /* ... */
    console.log("Setting project context:", JSON.stringify(context));
    this.projectContext = { ...this.projectContext, ...context };
    console.log("Updated project context:", JSON.stringify(this.projectContext));
    return { success: true, context: this.projectContext };
  }

  /**
   * Handles incoming chat messages - NON-STREAMING AI RESPONSE.
   * Returns the complete response after generation.
   */
  async onChatMessage(
    // onFinish callback is still required by the signature, even if we call it manually
    onFinish: StreamTextOnFinishCallback<ToolSet> // Use ToolSet from 'ai' if needed, otherwise {}
  ): Promise<Response | undefined> { // Ensure return type matches signature
    return agentContext.run(this, async () => {
      console.log("🔄 SIMPLIFIED CoderAgent.onChatMessage: Using generateText.");

      // createDataStreamResponse is still needed for the AIChatAgent contract
      const dataStreamResponse = createDataStreamResponse({
        execute: async (dataStream) => {
          try {
            console.log(`🔄 Processing ${this.messages.length} messages (non-streaming).`);

            // --- 1. Get AI Binding ---
            const AI = this.env.AI;
            if (!AI) {
              console.error("🔴 AI binding not available.");
              dataStream.write({ type: "error", error: "AI environment binding missing." });
              // Close immediately on fatal error
              dataStream.close();
              return;
            }

            // --- 2. Initialize WorkersAI Provider ---
            const workersai = createWorkersAI({ binding: AI });
            // Use a reliable model for basic generation
            const model = workersai('@cf/meta/llama-3-8b-instruct');
            console.log(`✅ Initialized Workers AI provider with model: ${model.modelId}`);

            // --- 3. Prepare Simple Prompt ---
            // We'll just use the content of the last user message
            const lastUserMessage = this.messages.findLast(m => m.role === 'user');
            const promptContent = lastUserMessage?.content || "Hello, tell me a short fact."; // Default prompt
            const systemPrompt = "You are a concise and helpful assistant."; // Very simple system prompt

            console.log(`✍️ Generating text for prompt: "${promptContent}"`);

            // --- 4. Call generateText ---
            // No tools, no complex message history, just prompt
            const result = await generateText({
              model: model,
              system: systemPrompt,
              prompt: promptContent,
            });

            const responseText = result.text;
            console.log(`✅ AI generation complete. Response length: ${responseText.length}`);

            // --- 5. Write the *complete* text to the stream ---
            // Send the entire response as one text chunk
            dataStream.write({ type: 'text', text: responseText });
            console.log("✉️ Sent complete response text to data stream.");

            // --- 6. Manually call onFinish ---
            // Since we aren't streaming, we call onFinish immediately after sending the text.
            // We construct the object expected by onFinish.
            const finishData = {
              role: 'assistant' as const, // Explicitly type role
              content: responseText,
              // Add other fields if the specific ToolSet requires them, otherwise omit
            };
            onFinish(finishData);
            console.log("🏁 Manually called onFinish callback.");


          } catch (error) {
            // Handle errors during setup or generateText
            console.error("❌ Error during simplified execution:", error);
            const errorMessage = error instanceof Error ? error.message : String(error);
            try {
              dataStream.write({ type: "error", error: `Generation Error: ${errorMessage}` });
            } catch (writeError) {
              console.error("❌ Failed to write error to data stream:", writeError);
            }
          } finally {
            // --- 7. Close the stream ---
            // IMPORTANT: Close the stream after sending all data
            console.log("🚪 Closing data stream.");
            try {
              dataStream.close();
            } catch (closeErr) { /* Ignore */ }
          }
        }, // End execute
      }); // End createDataStreamResponse

      // Return the Response object created by createDataStreamResponse
      return dataStreamResponse;

    }); // End agentContext.run
  } // End onChatMessage

  // Keep executeTask method stub if required by your application structure
  async executeTask(description: string, task: Schedule<string>) {
    console.log(`⚡ SIMPLIFIED: Execute task called: ${description}`);
    // Minimal implementation for the stub
    await this.saveMessages([
      ...this.messages,
      {
        id: generateId(),
        role: "system", // System message indicating task ran
        content: `System: Executed scheduled task '${description}'.`,
        createdAt: new Date(),
      },
    ]);
  }
}
