import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { routeAgentRequest, type Connection, type WSMessage } from "agents"
import { AIChatAgent } from "agents/ai-chat-agent";
import { streamText, type StreamTextOnFinishCallback } from "ai";
import { env } from "cloudflare:workers";

const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

export class Coder extends AIChatAgent<Env> {

  onMessage(connection: Connection, message: WSMessage): Promise<void> {
    console.log("onMessage", message);
    return super.onMessage(connection, message);
  }

  onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    console.log("onChatMessage");
    // return super.onChatMessage(onFinish);

    const stream = streamText({
      model,
      messages: [
        {
          role: "user",
          content: "Hello, world!",
        },
      ],
      onFinish,
    });

    return Promise.resolve(stream.toDataStreamResponse());
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
