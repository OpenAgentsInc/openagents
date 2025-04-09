import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { Agent, routeAgentRequest, unstable_callable } from "agents"
import { streamText, createDataStreamResponse, type UIMessage, generateId, type Message, generateText } from "ai";
import { env } from "cloudflare:workers";
import { tools } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";
import { createWorkersAI } from 'workers-ai-provider';

export const agentContext = new AsyncLocalStorage<Coder>();

const workersai = createWorkersAI({ binding: env.AI });
// @ts-ignore
const model = workersai("@cf/meta/llama-4-scout-17b-16e-instruct");
// const model = workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast");

// const google = createGoogleGenerativeAI({
//   apiKey: env.GOOGLE_API_KEY,
// });

// const model = google("gemini-2.5-pro-exp-03-25");

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

    const result = await generateText({
      system: "You are a helpful assistant. Respond directly to the user's questions without using any tools unless specifically asked about the weather.",
      model,
      messages: currentMessages,
      maxTokens: 2500,
      temperature: 0.9,
      toolChoice: 'none',  // Disable automatic tool usage
      tools: tools  // Keep tools available but don't use them by default
    })

    // Add the response to messages
    this.setState({
      messages: [
        ...currentMessages,  // Preserve all existing messages
        {
          id: generateId(),
          role: 'assistant',
          content: result.text,
          parts: [{
            type: 'text',
            text: result.text
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
