import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { Agent, routeAgentRequest } from "agents"
import { streamText, createDataStreamResponse, type UIMessage } from "ai";
import { env } from "cloudflare:workers";
import { tools } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";

export const agentContext = new AsyncLocalStorage<Coder>();

const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Coder extends Agent<Env> {

  messages: UIMessage[] = [];

  // Public infer method the user can call
  async infer(messages: UIMessage[]) {
    this.messages = messages;

    // Create a data stream for the AI response
    const dataStream = createDataStreamResponse({
      execute: async (stream) => {
        // Stream the AI response
        const result = streamText({
          model,
          messages: this.messages,
          tools,
          onFinish: () => { },
          onError: (error) => {
            console.error("Error while streaming:", error);
          }
        });

        // Merge the AI response stream
        result.mergeIntoDataStream(stream);
      }
    });

    return dataStream;
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
