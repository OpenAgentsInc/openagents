import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { Agent, routeAgentRequest, unstable_callable } from "agents"
import { streamText, createDataStreamResponse, type UIMessage, generateId, type Message, generateText } from "ai";
import { env } from "cloudflare:workers";
import { tools } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";
import { createWorkersAI } from 'workers-ai-provider';
import { z } from "zod";
import { createGroq } from '@ai-sdk/groq';

const groq = createGroq({
  apiKey: env.GROQ_API_KEY,
  // custom settings
});

const model = groq("meta-llama/llama-4-scout-17b-16e-instruct");

export const agentContext = new AsyncLocalStorage<Coder>();

// const workersai = createWorkersAI({ binding: env.AI });
// @ts-ignore
// const model = workersai("@cf/meta/llama-4-scout-17b-16e-instruct");
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
      system: `You are a helpful assistant. If the user asks you to do something, use one of your tools. Otherwise respond with a helpful answer.

      Your tools:
      - getWeatherInformation: show the weather in a given city to the user`,
      model,
      messages: currentMessages,
      maxTokens: 2500,
      temperature: 0.9,
      tools: {
        getWeatherInformation: {
          description: "show the weather in a given city to the user",
          parameters: z.object({ city: z.string() }),
          execute: async ({ city }) => {
            console.log(`Getting weather information for ${city}`);
            return `The weather in ${city} is sunny`;
          },
        },
      },
      maxSteps: 5,
      // toolChoice: 'none'
    })

    // Create message parts array to handle both text and tool calls
    const messageParts = [];

    // Add text part if there is text content
    if (result.text) {
      messageParts.push({
        type: 'text' as const,
        text: result.text
      });
    }

    // Add tool call parts if there are any
    if (result.toolCalls && result.toolCalls.length > 0) {
      result.toolCalls.forEach(toolCall => {
        messageParts.push({
          type: 'tool-invocation' as const,
          toolInvocation: {
            state: 'call',
            toolCallId: toolCall.toolCallId,
            toolName: toolCall.toolName,
            args: toolCall.args
          }
        });
      });
    }

    // Add tool results if there are any
    if (result.toolResults && result.toolResults.length > 0) {
      result.toolResults.forEach(toolResult => {
        messageParts.push({
          type: 'tool-invocation' as const,
          toolInvocation: {
            state: 'result',
            toolCallId: toolResult.toolCallId,
            toolName: toolResult.toolName,
            result: toolResult.result
          }
        });
      });
    }

    // Update state with the new message containing all parts
    this.setState({
      messages: [
        ...currentMessages,  // Preserve all existing messages
        {
          id: generateId(),
          role: 'assistant',
          content: result.text,
          parts: messageParts
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
