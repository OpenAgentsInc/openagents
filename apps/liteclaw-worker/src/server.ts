import { routeAgentRequest } from "agents";

import { AIChatAgent } from "@cloudflare/ai-chat";
import {
  convertToModelMessages,
  createUIMessageStream,
  createUIMessageStreamResponse,
  stepCountIs,
  streamText,
  type StreamTextOnFinishCallback,
  type StreamTextOnChunkCallback,
  type StreamTextOnErrorCallback,
  type ToolSet
} from "ai";
import { createWorkersAI } from "workers-ai-provider";

const MODEL_ID = "@cf/meta/llama-3.1-8b-instruct";
const MAX_CONTEXT_MESSAGES = 50;
const MAX_OUTPUT_TOKENS = 512;

const SYSTEM_PROMPT =
  "You are LiteClaw, a persistent personal AI agent. " +
  "Be concise, helpful, and remember the ongoing conversation.";

type ChatMetricLog = {
  event: "liteclaw_chat_metrics";
  agent_name: string;
  model_id: string;
  message_count: number;
  ttft_ms: number | null;
  duration_ms: number;
  ok: boolean;
  error?: string;
  finish_reason?: string | null;
};

export class Chat extends AIChatAgent<Env> {
  private logChatMetrics(payload: ChatMetricLog) {
    console.log(JSON.stringify(payload));
  }

  async onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>,
    options?: { abortSignal?: AbortSignal }
  ) {
    const workersai = createWorkersAI({ binding: this.env.AI });
    const model = workersai(MODEL_ID);
    const recentMessages = this.messages.slice(-MAX_CONTEXT_MESSAGES);
    const startTime = Date.now();
    let firstTokenAt: number | null = null;
    let finalized = false;

    const finalize = (params: {
      ok: boolean;
      error?: string;
      finishReason?: string | null;
    }) => {
      if (finalized) return;
      finalized = true;
      const durationMs = Date.now() - startTime;
      const ttftMs = firstTokenAt ? firstTokenAt - startTime : null;
      this.logChatMetrics({
        event: "liteclaw_chat_metrics",
        agent_name: this.name,
        model_id: MODEL_ID,
        message_count: this.messages.length,
        ttft_ms: ttftMs,
        duration_ms: durationMs,
        ok: params.ok,
        error: params.error,
        finish_reason: params.finishReason ?? null
      });
    };

    const handleChunk: StreamTextOnChunkCallback<ToolSet> = ({ chunk }) => {
      if (firstTokenAt) return;
      if (
        chunk.type === "text-delta" ||
        chunk.type === "reasoning-delta" ||
        chunk.type === "raw"
      ) {
        firstTokenAt = Date.now();
      }
    };

    const handleError: StreamTextOnErrorCallback = ({ error }) => {
      const message =
        error instanceof Error ? error.message : "StreamText error";
      finalize({ ok: false, error: message });
    };

    const handleFinish: StreamTextOnFinishCallback<ToolSet> = (event) => {
      finalize({ ok: true, finishReason: event.finishReason });
      return onFinish(event);
    };

    const stream = createUIMessageStream({
      execute: async ({ writer }) => {
        let result;
        try {
          result = streamText({
            system: SYSTEM_PROMPT,
            messages: await convertToModelMessages(recentMessages),
            model,
            maxTokens: MAX_OUTPUT_TOKENS,
            stopWhen: stepCountIs(10),
            onChunk: handleChunk,
            onError: handleError,
            onFinish: handleFinish,
            abortSignal: options?.abortSignal
          });
        } catch (error) {
          const message =
            error instanceof Error ? error.message : "StreamText error";
          finalize({ ok: false, error: message });
          throw error;
        }

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
