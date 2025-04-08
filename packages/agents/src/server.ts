import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { routeAgentRequest, type Connection, type WSMessage } from "agents"
import { AIChatAgent } from "agents/ai-chat-agent";
import { streamText, type StreamTextOnFinishCallback } from "ai";
import { env } from "cloudflare:workers";
import { AsyncLocalStorage } from "node:async_hooks";

const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

export const toolContext = new AsyncLocalStorage<Coder>();

export class Coder extends AIChatAgent<Env> {
  protected githubToken?: string;

  async onMessage(connection: Connection, message: WSMessage): Promise<void> {
    // Extract token first, without returning it
    this.extractToken(connection, message);

    // Don't log the token value directly
    console.log("onMessage - token extracted");

    // Call parent method
    return super.onMessage(connection, message);
  }

  onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    console.log("onChatMessage");

    // check if we have token here
    if (this.githubToken) {
      console.log("WE HAVE TOKEN HERE");
    } else {
      console.log("NO TOKEN HERE");
    }

    const stream = streamText({
      model,
      messages: this.messages,
      onFinish,
    });

    return Promise.resolve(stream.toDataStreamResponse());
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

            // Store in the context for tools to access
            toolContext.enterWith(this);
          }
        } catch (e) {
          console.error("Error parsing body:", e);
        }
      }
    }
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
