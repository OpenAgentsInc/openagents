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
const MODEL_CONFIG_ID = "workers-ai:llama-3.1-8b-instruct";
const MAX_CONTEXT_MESSAGES = 25;
const SUMMARY_TRIGGER_MESSAGES = 35;
const SUMMARY_MAX_TOKENS = 256;
const MAX_OUTPUT_TOKENS = 512;
const RATE_LIMIT_WINDOW_MS = 60_000;
const RATE_LIMIT_MAX_MESSAGES = 20;
const LEGACY_STATE_ROW_ID = "liteclaw_state";
const SKY_MEMORY_SCHEMA_VERSION = 1;
const SKY_EVENT_SCHEMA_VERSION = 1;
const SKY_RUN_SCHEMA_VERSION = 1;
const SKY_RECEIPT_SCHEMA_VERSION = 1;
const LITECLAW_SESSION_VERSION = 1;
const SKY_VERSION = "0.1.0";
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

const MODEL_REGISTRY = [
  {
    id: MODEL_CONFIG_ID,
    provider: "workers-ai",
    model: MODEL_ID,
    options: {
      max_output_tokens: MAX_OUTPUT_TOKENS
    }
  }
];

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

type LegacyLiteClawStateRow = {
  id: string;
  schema_version: number;
  summary: string | null;
  updated_at: number | null;
};

type SkyMemoryRow = {
  thread_id: string;
  summary: string | null;
  updated_at: number | null;
  schema_version: number;
};

type SkyRunRow = {
  run_id: string;
  thread_id: string;
  started_at: number;
  completed_at: number | null;
  status: string;
  model_config_id: string;
  error_code: string | null;
  schema_version: number;
};

type SkyEventRow = {
  run_id: string;
  event_id: number;
  type: string;
  payload_json: string;
  created_at: number;
  schema_version: number;
};

type SkyReceiptRow = {
  run_id: string;
  receipt_json: string;
  created_at: number;
  schema_version: number;
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

const textEncoder = new TextEncoder();

const toHex = (buffer: ArrayBuffer) =>
  [...new Uint8Array(buffer)]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");

const hashText = async (text: string) => {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    textEncoder.encode(text)
  );
  return toHex(digest);
};

const hashJson = async (value: unknown) => hashText(JSON.stringify(value));

const normalizeMessageForExport = (message: UIMessage): UIMessage => {
  const parts = message.parts.map((part) => {
    if (part.type !== "file") {
      return part;
    }

    const data = (part as { data?: { url?: string; mimeType?: string } }).data;
    const url = data?.url;
    if (!url) {
      return part;
    }

    if (!url.startsWith("r2://")) {
      return part;
    }

    return {
      type: "ref",
      ref: url,
      metadata: data?.mimeType ? { mimeType: data.mimeType } : undefined
    } as unknown as typeof part;
  });

  return { ...message, parts };
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
      create table if not exists liteclaw_rate_limit (
        id text primary key,
        window_start integer not null,
        count integer not null
      )
    `;
    this.sql`
      create table if not exists liteclaw_state (
        id text primary key,
        schema_version integer not null,
        summary text,
        updated_at integer
      )
    `;
    this.sql`
      create table if not exists sky_runs (
        run_id text primary key,
        thread_id text not null,
        started_at integer not null,
        completed_at integer,
        status text not null,
        model_config_id text not null,
        error_code text,
        schema_version integer not null
      )
    `;
    this.sql`
      create table if not exists sky_events (
        run_id text not null,
        event_id integer not null,
        type text not null,
        payload_json text not null,
        created_at integer not null,
        schema_version integer not null,
        primary key (run_id, event_id)
      )
    `;
    this.sql`
      create table if not exists sky_receipts (
        run_id text not null,
        receipt_json text not null,
        created_at integer not null,
        schema_version integer not null,
        primary key (run_id, created_at)
      )
    `;
    this.sql`
      create table if not exists sky_memory (
        thread_id text primary key,
        summary text,
        updated_at integer,
        schema_version integer not null
      )
    `;

    const memoryRows = this.sql<SkyMemoryRow>`
      select thread_id, summary, updated_at, schema_version
      from sky_memory
      where thread_id = ${this.name}
    `;

    if (!memoryRows.length) {
      const legacyRows = this.sql<LegacyLiteClawStateRow>`
        select id, schema_version, summary, updated_at
        from liteclaw_state
        where id = ${LEGACY_STATE_ROW_ID}
      `;
      const legacyRow = legacyRows[0];
      this.summary = legacyRow?.summary ?? null;
      this.sql`
        insert into sky_memory (thread_id, summary, updated_at, schema_version)
        values (
          ${this.name},
          ${this.summary},
          ${legacyRow?.updated_at ?? Date.now()},
          ${SKY_MEMORY_SCHEMA_VERSION}
        )
      `;
    } else {
      const row = memoryRows[0];
      this.summary = row.summary ?? null;
      if (row.schema_version !== SKY_MEMORY_SCHEMA_VERSION) {
        this.sql`
          update sky_memory
          set schema_version = ${SKY_MEMORY_SCHEMA_VERSION}
          where thread_id = ${this.name}
        `;
      }
    }

    this.stateLoaded = true;
  }

  private persistSummary(summary: string | null) {
    this.sql`
      insert into sky_memory (thread_id, summary, updated_at, schema_version)
      values (
        ${this.name},
        ${summary},
        ${Date.now()},
        ${SKY_MEMORY_SCHEMA_VERSION}
      )
      on conflict(thread_id) do update set
        schema_version = excluded.schema_version,
        summary = excluded.summary,
        updated_at = excluded.updated_at
    `;
  }

  private isSkyModeEnabled() {
    return this.env.LITECLAW_SKY_MODE === "1";
  }

  private insertSkyRun(runId: string, startedAt: number) {
    this.sql`
      insert into sky_runs (
        run_id,
        thread_id,
        started_at,
        completed_at,
        status,
        model_config_id,
        error_code,
        schema_version
      )
      values (
        ${runId},
        ${this.name},
        ${startedAt},
        null,
        'started',
        ${MODEL_CONFIG_ID},
        null,
        ${SKY_RUN_SCHEMA_VERSION}
      )
    `;
  }

  private updateSkyRun(options: {
    runId: string;
    status: string;
    completedAt: number;
    errorCode?: string | null;
  }) {
    this.sql`
      update sky_runs
      set
        status = ${options.status},
        completed_at = ${options.completedAt},
        error_code = ${options.errorCode ?? null}
      where run_id = ${options.runId}
    `;
  }

  private insertSkyEvent(options: {
    runId: string;
    eventId: number;
    type: string;
    payload: unknown;
    createdAt: number;
  }) {
    this.sql`
      insert into sky_events (
        run_id,
        event_id,
        type,
        payload_json,
        created_at,
        schema_version
      )
      values (
        ${options.runId},
        ${options.eventId},
        ${options.type},
        ${JSON.stringify(options.payload)},
        ${options.createdAt},
        ${SKY_EVENT_SCHEMA_VERSION}
      )
    `;
  }

  private insertSkyReceipt(options: {
    runId: string;
    receipt: unknown;
    createdAt: number;
  }) {
    this.sql`
      insert into sky_receipts (
        run_id,
        receipt_json,
        created_at,
        schema_version
      )
      values (
        ${options.runId},
        ${JSON.stringify(options.receipt)},
        ${options.createdAt},
        ${SKY_RECEIPT_SCHEMA_VERSION}
      )
    `;
  }

  private async finalizeSkyRun(options: {
    runId: string;
    status: string;
    startedAt: number;
    completedAt: number;
    finishReason: string | null;
    errorCode?: string | null;
    inputHashPromise: Promise<string> | null;
    outputText: string | null;
  }) {
    this.updateSkyRun({
      runId: options.runId,
      status: options.status,
      completedAt: options.completedAt,
      errorCode: options.errorCode ?? null
    });

    let inputHash: string | null = null;
    let outputHash: string | null = null;

    if (options.inputHashPromise) {
      inputHash = await options.inputHashPromise;
    }
    if (options.outputText) {
      outputHash = await hashText(options.outputText);
    }

    const receipt = {
      schema_version: SKY_RECEIPT_SCHEMA_VERSION,
      cf_sky_version: SKY_VERSION,
      run_id: options.runId,
      thread_id: this.name,
      model_config_id: MODEL_CONFIG_ID,
      input_hash: inputHash,
      output_hash: outputHash,
      started_at: options.startedAt,
      completed_at: options.completedAt,
      duration_ms: options.completedAt - options.startedAt,
      status: options.status,
      finish_reason: options.finishReason,
      error_code: options.errorCode ?? null
    };

    this.insertSkyReceipt({
      runId: options.runId,
      receipt,
      createdAt: options.completedAt
    });
  }

  private exportSkyJsonl() {
    this.ensureStateLoaded();

    const threadId = this.name;
    const now = Date.now();
    const messageRows = this.sql<{ message: string }>`
      select message
      from cf_ai_chat_agent_messages
      order by created_at asc
    `;
    const messages = messageRows
      .map((row) => JSON.parse(row.message) as UIMessage)
      .map((message) => normalizeMessageForExport(message));

    const runs = this.sql<SkyRunRow>`
      select run_id, thread_id, started_at, completed_at, status, model_config_id, error_code, schema_version
      from sky_runs
      where thread_id = ${threadId}
      order by started_at asc
    `;

    const memoryRows = this.sql<SkyMemoryRow>`
      select thread_id, summary, updated_at, schema_version
      from sky_memory
      where thread_id = ${threadId}
    `;

    const lines: string[] = [];
    lines.push(
      JSON.stringify({
        type: "liteclaw.export",
        liteclaw_session_version: LITECLAW_SESSION_VERSION,
        cf_sky_version: SKY_VERSION,
        schema_versions: {
          sky_run: SKY_RUN_SCHEMA_VERSION,
          sky_event: SKY_EVENT_SCHEMA_VERSION,
          sky_receipt: SKY_RECEIPT_SCHEMA_VERSION,
          sky_memory: SKY_MEMORY_SCHEMA_VERSION
        },
        thread_id: threadId,
        exported_at: now,
        model_registry: MODEL_REGISTRY
      })
    );

    if (memoryRows.length) {
      const memory = memoryRows[0];
      lines.push(
        JSON.stringify({
          type: "memory",
          payload: memory
        })
      );
    }

    for (const message of messages) {
      lines.push(
        JSON.stringify({
          type: "message",
          payload: message
        })
      );
    }

    for (const run of runs) {
      lines.push(
        JSON.stringify({
          type: "run",
          payload: run
        })
      );

      const events = this.sql<SkyEventRow>`
        select run_id, event_id, type, payload_json, created_at, schema_version
        from sky_events
        where run_id = ${run.run_id}
        order by event_id asc
      `;

      for (const event of events) {
        lines.push(
          JSON.stringify({
            type: "event",
            payload: {
              run_id: event.run_id,
              event_id: event.event_id,
              type: event.type,
              payload: JSON.parse(event.payload_json),
              created_at: event.created_at,
              schema_version: event.schema_version
            }
          })
        );
      }

      const receipts = this.sql<SkyReceiptRow>`
        select run_id, receipt_json, created_at, schema_version
        from sky_receipts
        where run_id = ${run.run_id}
        order by created_at asc
      `;

      for (const receipt of receipts) {
        lines.push(
          JSON.stringify({
            type: "receipt",
            payload: {
              run_id: receipt.run_id,
              receipt: JSON.parse(receipt.receipt_json),
              created_at: receipt.created_at,
              schema_version: receipt.schema_version
            }
          })
        );
      }
    }

    return new Response(lines.join("\n"), {
      headers: {
        "content-type": "application/jsonl; charset=utf-8"
      }
    });
  }

  async onRequest(request: Request) {
    const url = new URL(request.url);
    if (url.pathname.endsWith("/export")) {
      return this.exportSkyJsonl();
    }
    return super.onRequest(request);
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
    const skyEnabled = this.isSkyModeEnabled();
    const runId = skyEnabled ? generateId() : "";
    let skyEventId = 0;
    let inputHashPromise: Promise<string> | null = null;
    let finishReason: string | null = null;
    let finalText: string | null = null;
    let firstTokenAt: number | null = null;
    let finalized = false;
    const { controller, signal } = this.createAbortSignal(options);

    const emitSkyEvent = (type: string, payload: unknown) => {
      if (!skyEnabled) return;
      skyEventId += 1;
      this.insertSkyEvent({
        runId,
        eventId: skyEventId,
        type,
        payload,
        createdAt: Date.now()
      });
    };

    if (skyEnabled) {
      this.insertSkyRun(runId, startTime);
      emitSkyEvent("run.started", {
        thread_id: this.name,
        model_config_id: MODEL_CONFIG_ID,
        started_at: startTime,
        schema_version: SKY_RUN_SCHEMA_VERSION
      });
    }

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

      if (skyEnabled) {
        const completedAt = Date.now();
        const status = params.ok
          ? params.finishReason === "cancelled"
            ? "cancelled"
            : "completed"
          : "error";
        emitSkyEvent("run.completed", {
          status,
          finish_reason: params.finishReason ?? null,
          duration_ms: durationMs
        });
        void this.finalizeSkyRun({
          runId,
          status,
          startedAt: startTime,
          completedAt,
          finishReason: params.finishReason ?? null,
          errorCode: params.error ?? null,
          inputHashPromise,
          outputText: finalText
        });
      }
    };

    const handleChunk: StreamTextOnChunkCallback<ToolSet> = ({ chunk }) => {
      if (
        !firstTokenAt &&
        (chunk.type === "text-delta" ||
          chunk.type === "reasoning-delta" ||
          chunk.type === "raw")
      ) {
        firstTokenAt = Date.now();
      }

      if (chunk.type === "text-delta" || chunk.type === "reasoning-delta") {
        emitSkyEvent("model.delta", {
          kind: chunk.type,
          delta: chunk.delta
        });
      }
    };

    const handleError: StreamTextOnErrorCallback = ({ error }) => {
      if (signal?.aborted) {
        finishReason = "cancelled";
        finalize({ ok: true, finishReason });
        return;
      }
      const message =
        error instanceof Error ? error.message : "StreamText error";
      emitSkyEvent("run.error", { error: message });
      finishReason = "error";
      finalize({ ok: false, error: message, finishReason });
    };

    const handleFinish: StreamTextOnFinishCallback<ToolSet> = (event) => {
      finishReason = event.finishReason ?? null;
      finalText = event.text ?? null;
      emitSkyEvent("model.completed", {
        finish_reason: finishReason,
        text_length: finalText?.length ?? 0
      });
      finalize({ ok: true, finishReason });
      return onFinish(event);
    };

    try {
      this.consumeRateLimit();
    } catch (error) {
      const message =
        error instanceof RateLimitError
          ? error.message
          : "Rate limit exceeded. Please try again shortly.";
      finalText = message;
      finishReason = "rate_limited";
      emitSkyEvent("run.error", { error: "rate_limited" });
      inputHashPromise = skyEnabled
        ? hashJson({
            summary: this.summary,
            messages: this.messages.slice(-MAX_CONTEXT_MESSAGES),
            model_config_id: MODEL_CONFIG_ID
          })
        : null;
      const stream = createUIMessageStream({
        execute: async ({ writer }) => {
          firstTokenAt ??= Date.now();
          await this.writeStaticMessage(writer, message);
          finalize({
            ok: false,
            error: "rate_limited",
            finishReason: "rate_limited"
          });
        }
      });
      return createUIMessageStreamResponse({ stream });
    }

    await this.maybeSummarizeAndTrim(workersai);
    const recentMessages = this.messages.slice(-MAX_CONTEXT_MESSAGES);
    const systemPrompt = buildSystemPrompt(this.summary);
    inputHashPromise = skyEnabled
      ? hashJson({
          summary: this.summary,
          messages: recentMessages,
          model_config_id: MODEL_CONFIG_ID
        })
      : null;

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
            finalText = fallback.text;
            finishReason = "fallback";
            emitSkyEvent("model.completed", {
              finish_reason: finishReason,
              fallback: true,
              text_length: finalText.length
            });
            finalize({ ok: true, error: message, finishReason });
          } catch (fallbackError) {
            console.error("[LiteClaw] Fallback generation failed", fallbackError);
            firstTokenAt ??= Date.now();
            finalText = "LiteClaw hit an error. Please try again.";
            finishReason = "error";
            await this.writeStaticMessage(writer, finalText);
            emitSkyEvent("run.error", { error: message });
            finalize({ ok: false, error: message, finishReason });
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
