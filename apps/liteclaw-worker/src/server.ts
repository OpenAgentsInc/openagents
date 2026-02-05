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

const MODEL_ID = "@cf/openai/gpt-oss-120b";
const MAX_CONTEXT_MESSAGES = 25;
const MAX_OUTPUT_TOKENS = 512;

const SYSTEM_PROMPT =
  "You are Autopilot, a persistent personal AI agent. " +
  "Be concise, helpful, and remember the ongoing conversation.";

/**
 * Minimal persistent chat agent:
 * - Durable Object-backed transcript via AIChatAgent
 * - Workers AI model (no keys)
 * - No tools, no sandbox, no containers
 */
export class Chat extends AIChatAgent<Env> {
  override async onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>,
    options?: { abortSignal?: AbortSignal }
  ) {
    const workersai = createWorkersAI({ binding: this.env.AI });
    const recentMessages = this.messages.slice(-MAX_CONTEXT_MESSAGES);

    const stream = createUIMessageStream({
      execute: async ({ writer }) => {
        const result = streamText({
          system: SYSTEM_PROMPT,
          messages: await convertToModelMessages(recentMessages),
          model: workersai(MODEL_ID as Parameters<typeof workersai>[0]),
          maxOutputTokens: MAX_OUTPUT_TOKENS,
          stopWhen: stepCountIs(10),
          // Base class uses this callback to persist messages + stream metadata.
          onFinish: onFinish as unknown as StreamTextOnFinishCallback<ToolSet>,
          ...(options?.abortSignal ? { abortSignal: options.abortSignal } : {})
        });

        writer.merge(result.toUIMessageStream());
      }
    });

    return createUIMessageStreamResponse({ stream });
  }
}

export default {
  async fetch(request: Request, env: Env) {
    const response = await routeAgentRequest(request, env);
    return response ?? new Response("Not found", { status: 404 });
  }
} satisfies ExportedHandler<Env>;
