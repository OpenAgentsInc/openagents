import { routeAgentRequest } from "agents"
import type { Connection, WSMessage } from "agents";
import { AIChatAgent } from "agents/ai-chat-agent";
import type { StreamTextOnFinishCallback } from "ai";

export class Coder extends AIChatAgent<Env> {

  onMessage(connection: Connection, message: WSMessage): Promise<void> {
    console.log("onMessage", message);
    return super.onMessage(connection, message);
  }

  onChatMessage(onFinish: StreamTextOnFinishCallback<{}>): Promise<Response | undefined> {
    console.log("onChatMessage");
    return super.onChatMessage(onFinish);
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
