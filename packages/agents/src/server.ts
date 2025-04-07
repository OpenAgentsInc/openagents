import { routeAgentRequest, type Connection, type ConnectionContext, type Schedule } from "agents";

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

  // Handle HTTP requests coming to this Agent instance
  // Returns a Response object
  async onRequest(request: Request): Promise<Response> {

    // log any headers
    console.log("onRequest!!!?!?!?!!!")
    console.log("headers - " + JSON.stringify(request.headers))
    console.log("APIKEY - " + request.headers.get('x-api-key'));
    console.log("GITHUB TOKEN - " + request.headers.get('x-github-token'));

    return new Response("Hello from Agent!");
  }

  async onConnect(connection: Connection, ctx: ConnectionContext) {
    console.log("onConnect!!!?!?!?!!!")
    console.log("headers - " + JSON.stringify(ctx.request.headers))
    console.log("APIKEY - " + ctx.request.headers.get('x-api-key'));
    console.log("GITHUB TOKEN - " + ctx.request.headers.get('x-github-token'));
  }

  // Called when a WebSocket connection is established
  // Access the original request via ctx.request for auth etc.
  // async onConnect(connection: Connection, ctx: ConnectionContext) {

  //   console.log("onConnect", connection, ctx);


  //   console.log("ctx.request", ctx.request);


  //   console.log("ctx.request.headers", ctx.request.headers);

  //   console.log("ctx.request.headers.get('x-api-key')", ctx.request.headers.get('x-api-key'));

  //   console.log("ctx.request.headers.get('x-github-token')", ctx.request.headers.get('x-github-token'));
  //   // Connections are automatically accepted by the SDK.
  //   // You can also explicitly close a connection here with connection.close()
  //   // Access the Request on ctx.request to inspect headers, cookies and the URL
  // }


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
  async fetch(request: Request, env: Env) {
    // Route the request to our agent or return 404 if not found
    return (
      (await routeAgentRequest(request, env)) ||
      new Response("Not found", { status: 404 })
    );
  },
};
