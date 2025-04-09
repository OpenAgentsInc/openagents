import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { Agent, routeAgentRequest, unstable_callable } from "agents"
import { streamText, createDataStreamResponse, type UIMessage, generateId, type Message } from "ai";
import { env } from "cloudflare:workers";
import { tools } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";

export const agentContext = new AsyncLocalStorage<Coder>();

const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

interface CoderState {
  messages: UIMessage[];
}

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Coder extends Agent<Env, CoderState> {
  // Define initial state
  initialState: CoderState = {
    messages: []
  };

  @unstable_callable({
    description: "Generate an AI response based on the current messages",
    streaming: true
  })
  async infer(messages: UIMessage[]) {
    // Don't overwrite state, just use the messages passed in
    const currentMessages = Array.isArray(messages) ? messages : [];

    // Create a data stream for the AI response
    const dataStream = createDataStreamResponse({
      execute: async (stream) => {
        try {
          // Ensure we have messages to process
          if (currentMessages.length === 0) {
            stream.write(`2:${JSON.stringify({ error: "No messages to process" })}\n`);
            return;
          }

          // Stream the AI response
          const result = streamText({
            model,
            messages: currentMessages,
            tools,
            onFinish: () => {
              // Add the assistant's response to the state
              const lastMessage = currentMessages[currentMessages.length - 1];
              if (lastMessage && lastMessage.role === 'user') {
                this.setState({
                  ...this.state,
                  messages: [
                    ...currentMessages,
                    {
                      id: generateId(),
                      role: 'assistant',
                      content: 'Response complete',
                      parts: [{
                        type: 'text',
                        text: 'Response complete'
                      }]
                    }
                  ]
                });
              }
              console.log("Finished streaming response");
            },
            onError: (error) => {
              console.error("Error while streaming:", error);
              stream.write(`2:${JSON.stringify({ error: "Failed to generate response" })}\n`);
            }
          });

          // Merge the AI response stream into the data stream
          result.mergeIntoDataStream(stream);
        } catch (error) {
          console.error("Error in infer:", error);
          stream.write(`2:${JSON.stringify({ error: "Failed to process request" })}\n`);
        }
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
