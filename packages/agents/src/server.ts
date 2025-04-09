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
    // Get current state messages and ensure they're an array
    const stateMessages = this.state.messages || [];
    const incomingMessages = Array.isArray(messages) ? messages : [];

    // Combine existing state messages with any new incoming messages
    const currentMessages = [...stateMessages, ...incomingMessages];

    // Add a simple dummy response
    this.setState({
      messages: [
        ...currentMessages,  // Preserve all existing messages
        {
          id: generateId(),
          role: 'assistant',
          content: 'Hello! I am your AI assistant. How can I help you today?',
          parts: [{
            type: 'text',
            text: 'Hello! I am your AI assistant. How can I help you today?'
          }]
        }
      ]
    });

    // Return empty object since we're not using streams anymore
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
