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

  onMessage(connection: Connection, message: WSMessage): Promise<void> {
    const token = this.extractToken(connection, message)
    console.log("onMessage with token " + token);
    return super.onMessage(connection, message);
  }

  onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    console.log("onChatMessage");

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
        return 'token not here'
      }

      if (data.type === "cf_agent_use_chat_request" && data.init.method === "POST") {
        const body = data.init.body
        const requestData = JSON.parse(body as string)
        const githubToken = requestData.githubToken

        // Save the token in the tool context
        return toolContext.run(this, async () => {
          this.githubToken = githubToken;
          return githubToken;
        });
      }
    }
    return "dummy-token";
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
