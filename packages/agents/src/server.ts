import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { routeAgentRequest, type Connection, type WSMessage } from "agents"
import { AIChatAgent } from "agents/ai-chat-agent";
import { streamText, createDataStreamResponse, type StreamTextOnFinishCallback } from "ai";
import { env } from "cloudflare:workers";
import { tools } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";

export const agentContext = new AsyncLocalStorage<Coder>();

const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

export class Coder extends AIChatAgent<Env> {
  public githubToken?: string;

  async onMessage(connection: Connection, message: WSMessage): Promise<void> {
    this.extractToken(connection, message);
    return super.onMessage(connection, message);
  }

  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    return agentContext.run(this, async () => {
      const dataStreamResponse = createDataStreamResponse({
        execute: async (dataStream) => {
          const stream = streamText({
            toolCallStreaming: true,
            tools,
            model,
            messages: [
              { role: 'system', content: 'You are Coder, a helpful assistant. Use the provided tools to help the user.' },
              ...this.messages
            ],
            onFinish,
            maxSteps: 5
          });
          stream.mergeIntoDataStream(dataStream);
        }
      });

      return dataStreamResponse;
    })
  }

  extractToken(connection: Connection, message: WSMessage) {
    console.log("extracting token from message");
    if (typeof message === "string") {
      let data: any
      try {
        data = JSON.parse(message)
      } catch (error) {
        console.log("Failed to parse message as JSON");
        return;
      }

      if (data.type === "cf_agent_use_chat_request" && data.init?.method === "POST") {
        const body = data.init.body;
        try {
          const requestData = JSON.parse(body as string);
          const githubToken = requestData.githubToken;

          if (githubToken) {
            console.log(`Found githubToken in message, length: ${githubToken.length}`);
            // Directly set the token on the instance
            this.githubToken = githubToken;
          }
        } catch (e) {
          console.error("Error parsing body:", e);
        }
      }
    }
  }

  // Public method to get the GitHub token for tools
  getGitHubToken(): string | undefined {
    return this.githubToken;
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
