import { routeAgentRequest } from "agents";

import { AIChatAgent } from "@cloudflare/ai-chat";
import {
  convertToModelMessages,
  createUIMessageStream,
  createUIMessageStreamResponse,
  stepCountIs,
  streamText,
  type StreamTextOnFinishCallback,
  type ToolSet
} from "ai";
import { createWorkersAI } from "workers-ai-provider";

const MODEL_ID = "@cf/meta/llama-3.1-8b-instruct";
const MAX_CONTEXT_MESSAGES = 50;
const MAX_OUTPUT_TOKENS = 512;

const SYSTEM_PROMPT =
  "You are LiteClaw, a persistent personal AI agent. " +
  "Be concise, helpful, and remember the ongoing conversation.";

export class Chat extends AIChatAgent<Env> {
  async onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>,
    options?: { abortSignal?: AbortSignal }
  ) {
    const workersai = createWorkersAI({ binding: this.env.AI });
    const model = workersai(MODEL_ID);
    const recentMessages = this.messages.slice(-MAX_CONTEXT_MESSAGES);

    const stream = createUIMessageStream({
      execute: async ({ writer }) => {
        const result = streamText({
          system: SYSTEM_PROMPT,
          messages: await convertToModelMessages(recentMessages),
          model,
          maxTokens: MAX_OUTPUT_TOKENS,
          stopWhen: stepCountIs(10),
          onFinish,
          abortSignal: options?.abortSignal
        });

        writer.merge(result.toUIMessageStream());
      }
    });

    return createUIMessageStreamResponse({ stream });
  }
}

export default {
  async fetch(request: Request, env: Env) {
    return (
      (await routeAgentRequest(request, env)) ||
      new Response("Not found", { status: 404 })
    );
  }
} satisfies ExportedHandler<Env>;
