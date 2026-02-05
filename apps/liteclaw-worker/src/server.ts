import { routeAgentRequest } from "agents";

import { AIChatAgent } from "@cloudflare/ai-chat";
import {
  convertToModelMessages,
  createUIMessageStream,
  createUIMessageStreamResponse,
  generateId,
  generateText,
  stepCountIs,
  streamText,
  type StreamTextOnFinishCallback,
  type StreamTextOnChunkCallback,
  type StreamTextOnErrorCallback,
  type ToolSet,
  type UIMessage,
  type UIMessageStreamWriter
} from "ai";
import { createWorkersAI } from "workers-ai-provider";

const MODEL_ID = "@cf/meta/llama-3.1-8b-instruct";
const MAX_CONTEXT_MESSAGES = 25;
const SUMMARY_TRIGGER_MESSAGES = 35;
const SUMMARY_MAX_TOKENS = 256;
const MAX_OUTPUT_TOKENS = 512;
const RATE_LIMIT_WINDOW_MS = 60_000;
const RATE_LIMIT_MAX_MESSAGES = 20;
const STATE_SCHEMA_VERSION = 1;
const STATE_ROW_ID = "liteclaw_state";
const RATE_LIMIT_ROW_ID = "liteclaw_rate_limit";

const SYSTEM_PROMPT =
  "You are LiteClaw, a persistent personal AI agent. " +
  "Be concise, helpful, and remember the ongoing conversation.";
const SUMMARY_PROMPT = [
  "You update LiteClaw's memory summary.",
  "Summarize durable facts, user preferences, ongoing tasks, and decisions.",
  "Be concise and use short bullet points.",
  "Avoid quotes or chatty phrasing."
].join(" ");

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

type LiteClawStateRow = {
  id: string;
  schema_version: number;
  summary: string | null;
  updated_at: number | null;
};

type RateLimitRow = {
  id: string;
  window_start: number;
  count: number;
};

class RateLimitError extends Error {
  retryAfterMs: number;

  constructor(retryAfterMs: number) {
    const retrySeconds = Math.max(1, Math.ceil(retryAfterMs / 1000));
    super(
      `You're sending messages too quickly. Please wait about ${retrySeconds} seconds and try again.`
    );
    this.retryAfterMs = retryAfterMs;
    this.name = "RateLimitError";
  }
}

const buildSystemPrompt = (summary: string | null) => {
  if (!summary) return SYSTEM_PROMPT;
  return `${SYSTEM_PROMPT}\n\nMemory summary:\n${summary}`;
};

const buildSummaryPrompt = (summary: string | null) => {
  if (!summary) return SUMMARY_PROMPT;
  return `${SUMMARY_PROMPT}\n\nExisting summary:\n${summary}`;
};

const combineAbortSignals = (
  ...signals: Array<AbortSignal | undefined>
): AbortSignal | undefined => {
  const activeSignals = signals.filter(
    (signal): signal is AbortSignal => Boolean(signal)
  );
  if (activeSignals.length === 0) return undefined;
  if (activeSignals.length === 1) return activeSignals[0];

  const controller = new AbortController();
  const onAbort = () => controller.abort();

  for (const signal of activeSignals) {
    if (signal.aborted) {
      controller.abort();
      break;
    }
    signal.addEventListener("abort", onAbort, { once: true });
  }

  return controller.signal;
};

export class Chat extends AIChatAgent<Env> {
  private summary: string | null = null;
  private stateLoaded = false;
  private activeAbortController: AbortController | null = null;

  private logChatMetrics(payload: ChatMetricLog) {
    console.log(JSON.stringify(payload));
  }

  private ensureStateLoaded() {
    if (this.stateLoaded) return;

    this.sql`
      create table if not exists liteclaw_state (
        id text primary key,
        schema_version integer not null,
        summary text,
        updated_at integer
      )
    `;
    this.sql`
      create table if not exists liteclaw_rate_limit (
        id text primary key,
        window_start integer not null,
        count integer not null
      )
    `;

    const rows = this.sql<LiteClawStateRow>`
      select id, schema_version, summary, updated_at
      from liteclaw_state
      where id = ${STATE_ROW_ID}
    `;

    if (!rows.length) {
      this.sql`
        insert into liteclaw_state (id, schema_version, summary, updated_at)
        values (${STATE_ROW_ID}, ${STATE_SCHEMA_VERSION}, null, ${Date.now()})
      `;
      this.summary = null;
    } else {
      const row = rows[0];
      this.summary = row.summary ?? null;
      if (row.schema_version !== STATE_SCHEMA_VERSION) {
        this.sql`
          update liteclaw_state
          set schema_version = ${STATE_SCHEMA_VERSION}
          where id = ${STATE_ROW_ID}
        `;
      }
    }

    this.stateLoaded = true;
  }

  private persistSummary(summary: string | null) {
    this.sql`
      insert into liteclaw_state (id, schema_version, summary, updated_at)
      values (${STATE_ROW_ID}, ${STATE_SCHEMA_VERSION}, ${summary}, ${Date.now()})
      on conflict(id) do update set
        schema_version = excluded.schema_version,
        summary = excluded.summary,
        updated_at = excluded.updated_at
    `;
  }

  private consumeRateLimit() {
    const now = Date.now();
    const rows = this.sql<RateLimitRow>`
      select id, window_start, count
      from liteclaw_rate_limit
      where id = ${RATE_LIMIT_ROW_ID}
    `;

    if (!rows.length) {
      this.sql`
        insert into liteclaw_rate_limit (id, window_start, count)
        values (${RATE_LIMIT_ROW_ID}, ${now}, 1)
      `;
      return;
    }

    const row = rows[0];
    const windowStart = Number(row.window_start);
    const count = Number(row.count);

    if (Number.isNaN(windowStart) || now - windowStart >= RATE_LIMIT_WINDOW_MS) {
      this.sql`
        insert into liteclaw_rate_limit (id, window_start, count)
        values (${RATE_LIMIT_ROW_ID}, ${now}, 1)
        on conflict(id) do update set
          window_start = excluded.window_start,
          count = excluded.count
      `;
      return;
    }

    if (count >= RATE_LIMIT_MAX_MESSAGES) {
      const retryAfterMs = windowStart + RATE_LIMIT_WINDOW_MS - now;
      throw new RateLimitError(retryAfterMs);
    }

    this.sql`
      update liteclaw_rate_limit
      set count = ${count + 1}
      where id = ${RATE_LIMIT_ROW_ID}
    `;
  }

  private pruneMessages(keepMessages: UIMessage[]) {
    const keepIds = new Set(keepMessages.map((message) => message.id));
    for (const message of this.messages) {
      if (!keepIds.has(message.id)) {
        this.sql`
          delete from cf_ai_chat_agent_messages
          where id = ${message.id}
        `;
      }
    }
    this.messages = keepMessages;
  }

  private async maybeSummarizeAndTrim(
    workersAi: ReturnType<typeof createWorkersAI>
  ) {
    if (this.messages.length <= SUMMARY_TRIGGER_MESSAGES) return;

    const pruneCount = this.messages.length - MAX_CONTEXT_MESSAGES;
    if (pruneCount <= 0) return;

    const messagesToSummarize = this.messages.slice(0, pruneCount);
    const messagesToKeep = this.messages.slice(pruneCount);
    const model = workersAi(MODEL_ID);

    try {
      const result = await generateText({
        model,
        system: buildSummaryPrompt(this.summary),
        messages: await convertToModelMessages(messagesToSummarize),
        maxTokens: SUMMARY_MAX_TOKENS,
        temperature: 0.2
      });

      const nextSummary = result.text.trim();
      if (nextSummary) {
        this.summary = nextSummary;
        this.persistSummary(nextSummary);
      }
    } catch (error) {
      console.warn("[LiteClaw] Summary generation failed", error);
    }

    this.pruneMessages(messagesToKeep);
  }

  private createAbortSignal(options?: { abortSignal?: AbortSignal }) {
    if (this.activeAbortController) {
      this.activeAbortController.abort();
    }

    const controller = new AbortController();
    this.activeAbortController = controller;

    return {
      controller,
      signal: combineAbortSignals(controller.signal, options?.abortSignal)
    };
  }

  private async writeStaticMessage(
    writer: UIMessageStreamWriter,
    text: string
  ) {
    const id = generateId();
    writer.write({ type: "text-start", id });
    writer.write({ type: "text-delta", id, delta: text });
    writer.write({ type: "text-end", id });
  }

  async onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>,
    options?: { abortSignal?: AbortSignal }
  ) {
    this.ensureStateLoaded();
    const workersai = createWorkersAI({ binding: this.env.AI });
    const startTime = Date.now();
    let firstTokenAt: number | null = null;
    let finalized = false;
    const { controller, signal } = this.createAbortSignal(options);

    const finalize = (params: {
      ok: boolean;
      error?: string;
      finishReason?: string | null;
    }) => {
      if (finalized) return;
      finalized = true;
      if (this.activeAbortController === controller) {
        this.activeAbortController = null;
      }
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
      if (signal?.aborted) {
        finalize({ ok: true, finishReason: "cancelled" });
        return;
      }
      const message =
        error instanceof Error ? error.message : "StreamText error";
      finalize({ ok: false, error: message });
    };

    const handleFinish: StreamTextOnFinishCallback<ToolSet> = (event) => {
      finalize({ ok: true, finishReason: event.finishReason });
      return onFinish(event);
    };

    try {
      this.consumeRateLimit();
    } catch (error) {
      const message =
        error instanceof RateLimitError
          ? error.message
          : "Rate limit exceeded. Please try again shortly.";
      const stream = createUIMessageStream({
        execute: async ({ writer }) => {
          firstTokenAt ??= Date.now();
          await this.writeStaticMessage(writer, message);
          finalize({ ok: false, error: "rate_limited", finishReason: "error" });
        }
      });
      return createUIMessageStreamResponse({ stream });
    }

    await this.maybeSummarizeAndTrim(workersai);
    const recentMessages = this.messages.slice(-MAX_CONTEXT_MESSAGES);
    const systemPrompt = buildSystemPrompt(this.summary);

    const stream = createUIMessageStream({
      execute: async ({ writer }) => {
        let result;
        try {
          result = streamText({
            system: systemPrompt,
            messages: await convertToModelMessages(recentMessages),
            model: workersai(MODEL_ID),
            maxTokens: MAX_OUTPUT_TOKENS,
            stopWhen: stepCountIs(10),
            onChunk: handleChunk,
            onError: handleError,
            onFinish: handleFinish,
            abortSignal: signal
          });
        } catch (error) {
          const message =
            error instanceof Error ? error.message : "StreamText error";
          try {
            const fallback = await generateText({
              model: workersai(MODEL_ID),
              system: systemPrompt,
              messages: await convertToModelMessages(recentMessages),
              maxTokens: MAX_OUTPUT_TOKENS
            });
            firstTokenAt ??= Date.now();
            await this.writeStaticMessage(writer, fallback.text);
            finalize({ ok: true, error: message, finishReason: "fallback" });
          } catch (fallbackError) {
            console.error("[LiteClaw] Fallback generation failed", fallbackError);
            firstTokenAt ??= Date.now();
            await this.writeStaticMessage(
              writer,
              "LiteClaw hit an error. Please try again."
            );
            finalize({ ok: false, error: message, finishReason: "error" });
          }
          return;
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
