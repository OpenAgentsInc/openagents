import { routeAgentRequest, type Connection, type Schedule, type WSMessage } from "agents";

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
// import { createWorkersAI } from 'workers-ai-provider';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { env } from "cloudflare:workers";
import { OpenAIAgentPlugin } from "./plugins/github-plugin";
import type { AgentPlugin } from "./plugins/plugin-interface";

interface IncomingMessage {
  type: string;
  init: {
    method: string;
    body: string;
  };
}

// const workersai = createWorkersAI({ binding: env.AI });
// const model = workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast");

const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

// we use ALS to expose the agent context to the tools
export const agentContext = new AsyncLocalStorage<Coder>();

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Coder extends AIChatAgent<Env> {
  /**
   * Handles incoming chat messages and manages the response stream
   * @param onFinish - Callback function executed when streaming completes
   */

  override async onMessage(connection: Connection, message: WSMessage) {
    console.log('[onMessage] Received message:', typeof message);

    if (typeof message === "string") {
      let data: IncomingMessage;
      try {
        data = JSON.parse(message) as IncomingMessage;
        console.log('[onMessage] Parsed message data:', { type: data.type, method: data.init.method });
      } catch (error) {
        console.error('[onMessage] Failed to parse message:', error);
        return;
      }

      if (
        data.type === "cf_agent_use_chat_request" &&
        data.init.method === "POST"
      ) {
        console.log('[onMessage] Processing chat request');
        const { body } = data.init;
        const requestData = JSON.parse(body as string);
        const { messages, githubToken } = requestData;
        console.log('[onMessage] Parsed request data:', {
          messageCount: messages?.length,
          hasGithubToken: !!githubToken
        });

        // Set up tool context with GitHub token
        const context = {
          githubToken,
          tools,
        };

        // Run the rest of the message handling with the tool context
        console.log('[onMessage] Running with agent context');
        return agentContext.run(this, async () => {
          const ctx = { githubToken, tools };
          console.log('[onMessage] Delegating to parent handler');
          return super.onMessage(connection, message);
        });
      }
    }
    console.log('[onMessage] Falling back to parent handler');
    return super.onMessage(connection, message);
  }

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
            tools: {},
            executions,
          });

          // Stream the AI response using GPT-4
          const result = streamText({
            model,
            system: `You are a helpful assistant that can do various tasks...

${unstable_getSchedulePrompt({ date: new Date() })}

You have access to GitHub tools that let you interact with GitHub repositories through the Model Context Protocol (MCP):

`,
            messages: processedMessages,
            tools: {},
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

/**
 * Worker entry point that routes incoming requests to the appropriate handler
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Route the request to our agent or return 404 if not found
    return (
      (await routeAgentRequest(request, env)) ||
      new Response("Not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
