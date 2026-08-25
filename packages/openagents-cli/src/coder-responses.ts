/**
 * A reply source speaking the OpenResponses surface at `POST /api/v1/responses`.
 *
 * The dev lane's source: `openagents coder --dev` sends each turn as an
 * OpenResponses request with `stream: true` and renders the semantic events
 * that come back. The conversation is client-held — every call carries the
 * item history — and the tool runtime is the client's: a `function_call`
 * item the model asks for is run here, its output replayed as a
 * `function_call_output` item, and the loop continues until the model
 * answers in text. That is the same division of labor as the thread lane,
 * spoken in the OpenResponses grammar instead of chat-completions.
 *
 * Rendered events are the ones this client understands —
 * `response.output_text.delta`, `response.reasoning_summary_text.delta`,
 * `response.output_item.done` for function calls, `response.completed`,
 * `response.failed` — and the rest of the sequence passes over it, which is
 * exactly what the grammar is for.
 */

import type { ReplyChunk, ReplySource } from "./coder-session.js";
import { tierLabel } from "./coder-tiers.js";
import type { CoderTool } from "./coder-tools.js";
import type { TranscriptSink } from "./coder-transcript.js";

/** Default backoff ladder for transient responses API failures (5xx or network drops). */
const DEFAULT_RETRY_DELAYS_MS: ReadonlyArray<number> = [250, 500, 1000, 2000];

/**
 * How much of one tool's output reaches the durable `tool.ran` event.
 *
 * The same figure the thread and local lanes use: it bounds a record written
 * once, so it is set where every result a real session has produced fits
 * whole. What the model is re-sent each round is bounded separately.
 */
const EVENT_RESULT_KEPT = 64_000;

/** A long tool result, kept at both ends. */
const bounded = (output: string, keep: number): string => {
  if (output.length <= keep) return output;
  const half = Math.floor(keep / 2);
  const cut = output.length - keep;
  return `${output.slice(0, half)}\n\n[${String(cut)} of ${String(output.length)} characters omitted from the middle; run it again more narrowly if you need them]\n\n${output.slice(-half)}`;
};

export interface ResponsesOptions {
  /** The API origin, such as `http://localhost:4000`. */
  readonly origin: string;
  /** The account bearer, sent when held; the surface also answers without one. */
  readonly token?: string | undefined;
  /** Retry backoff delays in milliseconds for testing or custom ladders. */
  readonly retryDelaysMs?: ReadonlyArray<number> | undefined;
}

/** One conversation item, in the OpenResponses input shape. */
type Item =
  | { readonly role: "user" | "assistant" | "system"; readonly content: string }
  | {
      readonly type: "function_call";
      readonly call_id: string;
      readonly name: string;
      readonly arguments: string;
    }
  | { readonly type: "function_call_output"; readonly call_id: string; readonly output: string };

/** A call the model asked for, as the stream's item events carry it. */
interface Call {
  readonly callId: string;
  readonly name: string;
  readonly args: string;
}

export class ResponsesReplySource implements ReplySource {
  private readonly items: Item[] = [];
  private tools: ReadonlyArray<CoderTool> = [];
  private standing: string | undefined;
  private readonly retryDelaysMs: ReadonlyArray<number>;
  /**
   * The transcript writer, when the session opened a transcript-only thread
   * for this lane (OpenAgentsInc/openagents#59). Absent, nothing is recorded —
   * a `--dev` session without a credential runs exactly as before, it just
   * leaves no record on the server.
   */
  private sink: TranscriptSink | undefined;

  constructor(private readonly options: ResponsesOptions) {
    this.retryDelaysMs = options.retryDelaysMs ?? DEFAULT_RETRY_DELAYS_MS;
  }

  /** The dev lane defers to the server, which is what Coder Auto names. */
  get model(): string {
    return tierLabel("auto");
  }

  /** The product id: the server picks what answers, so none is promised. */
  get modelId(): string {
    return "openagents-coder";
  }

  useTools(tools: ReadonlyArray<CoderTool>): void {
    this.tools = tools;
  }

  useContext(standing: string): void {
    this.standing = standing;
  }

  /**
   * Attach the writer that puts this session's turns on the server.
   *
   * The same vocabulary the thread and local lanes record — `turn.user`,
   * `turn.reasoning`, `tool.ran`, `turn.assistant` — so `/threads/:id`, the
   * export, and a resume read a dev session exactly as they read a hosted
   * one. Set after construction because the writer needs the thread's id,
   * which does not exist until the transcript-only thread is opened.
   */
  useTranscript(sink: TranscriptSink): void {
    this.sink = sink;
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    this.items.push({ role: "user", content: prompt });
    this.sink?.record("turn.user", { text: prompt });
    let calls = 0;
    /** The answer so far, across steps, for the one `turn.assistant` event. */
    let turnText = "";
    /** How many tools this turn ran, reported on `turn.assistant`. */
    let turnToolCalls = 0;

    for (;;) {
      calls += 1;
      const { text, reasoning, requested } = yield* this.once(signal);
      if (text !== "") this.items.push({ role: "assistant", content: text });

      // One event per block, whole, never deltas: the record is what was
      // thought, not the pieces it arrived in.
      if (reasoning.length > 0) this.sink?.record("turn.reasoning", { text: reasoning });

      // Whatever the model said belongs to the thread even when the turn was
      // interrupted, or the next turn answers a question it cannot see it
      // half-answered.
      if (text.length > 0) {
        turnText = turnText.length === 0 ? text : `${turnText}\n\n${text}`;
      }

      if (requested.length === 0) {
        yield { type: "usage", calls };
        this.recordAnswer(turnText, turnToolCalls, calls, signal.aborted);
        return;
      }

      for (const call of requested) {
        this.items.push({
          type: "function_call",
          call_id: call.callId,
          name: call.name,
          arguments: call.args,
        });
        const outcome = await this.run(call, signal);
        yield {
          type: "tool_result",
          callId: call.callId,
          output: outcome.output,
          error: outcome.error,
        };
        this.items.push({
          type: "function_call_output",
          call_id: call.callId,
          output: outcome.output ?? outcome.error ?? "",
        });
        turnToolCalls += 1;
        // Call and result are one fact, so they are one event — the thread
        // lane's shape exactly, bounded far above the model-wire bound so the
        // record keeps what the model was fed a cut of.
        this.sink?.record("tool.ran", {
          call_id: call.callId,
          tool: call.name,
          arguments: bounded(call.args, EVENT_RESULT_KEPT),
          status: outcome.error === undefined ? "succeeded" : "failed",
          ...(outcome.error === undefined
            ? { output: bounded(outcome.output ?? "", EVENT_RESULT_KEPT) }
            : { error: bounded(outcome.error, EVENT_RESULT_KEPT) }),
        });
      }
    }
  }

  /**
   * Record the turn's answer, with what it cost.
   *
   * One event per turn, whatever the turn took to get there, the same shape
   * the thread lane records. This surface reports no token counts, so the
   * usage carries only the call count — absent figures stay absent rather
   * than being written as zeros a reader would take for measurements.
   */
  private recordAnswer(text: string, toolCalls: number, calls: number, interrupted: boolean): void {
    if (this.sink === undefined) return;
    if (text.length === 0 && toolCalls === 0) return;
    this.sink.record("turn.assistant", {
      text,
      usage: { calls },
      tool_calls: toolCalls,
      ...(interrupted ? { interrupted: true } : {}),
    });
  }

  /**
   * Request POST /api/v1/responses with exponential backoff retries for transient failures.
   */
  private async request(signal: AbortSignal): Promise<Response> {
    const attempts = this.retryDelaysMs.length + 1;

    for (let attempt = 0; attempt < attempts; attempt += 1) {
      let response: Response | undefined;
      let error: unknown;

      try {
        response = await fetch(new URL("/api/v1/responses", this.options.origin), {
          method: "POST",
          headers: {
            ...(this.options.token === undefined
              ? {}
              : { authorization: `Bearer ${this.options.token}` }),
            "content-type": "application/json",
            // Both named: the pipeline negotiates on json, the answer is SSE.
            accept: "text/event-stream, application/json",
          },
          body: JSON.stringify({
            input: this.items,
            stream: true,
            ...(this.standing === undefined ? {} : { instructions: this.standing }),
            ...(this.tools.length === 0
              ? {}
              : {
                  tools: this.tools.map((tool) => ({
                    type: "function",
                    name: tool.name,
                    description: tool.description,
                    parameters: tool.parameters,
                  })),
                }),
          }),
          signal,
        });
      } catch (cause) {
        if (signal.aborted) throw cause;
        error = cause;
      }

      if (response !== undefined && response.ok && response.body !== null) {
        return response;
      }

      const status = response?.status;
      const isTransient =
        error !== undefined || (status !== undefined && status >= 500 && status < 600);

      if (!isTransient || attempt === attempts - 1) {
        if (response !== undefined) {
          throw new Error(
            `The responses API at ${this.options.origin} answered HTTP ${String(response.status)}.`,
          );
        }
        throw new Error(
          `The responses API at ${this.options.origin} could not be reached: ${String(error)}`,
        );
      }

      const delayMs = this.retryDelaysMs[attempt] ?? 1000;
      await new Promise((resolve) => setTimeout(resolve, delayMs));
      if (signal.aborted) {
        throw new Error("Aborted");
      }
    }

    throw new Error(`The responses API at ${this.options.origin} request failed.`);
  }

  /** One request: stream the events, yield what renders, return the rest. */
  private async *once(
    signal: AbortSignal,
  ): AsyncGenerator<ReplyChunk, { text: string; reasoning: string; requested: Call[] }> {
    const response = await this.request(signal);

    let text = "";
    let reasoning = "";
    const requested: Call[] = [];

    for await (const data of frames(response.body!, signal)) {
      const event = parse(data);
      if (event === undefined) continue;

      switch (event["type"]) {
        case "response.output_text.delta": {
          const delta = event["delta"];
          if (typeof delta === "string" && delta.length > 0) {
            text += delta;
            yield { type: "text", value: delta };
          }
          break;
        }
        case "response.reasoning_summary_text.delta": {
          const delta = event["delta"];
          if (typeof delta === "string" && delta.length > 0) {
            reasoning += delta;
            yield { type: "reasoning", value: delta };
          }
          break;
        }
        case "response.output_item.done": {
          const item = event["item"];
          const call = functionCall(item);
          if (call !== undefined) {
            requested.push(call);
            yield { type: "tool_call", callId: call.callId, name: call.name, arguments: call.args };
          }
          break;
        }
        case "response.failed": {
          const failure = failureOf(event);
          throw new Error(`The responses API reported a failure: ${failure}`);
        }
        default:
          break;
      }
    }

    return { text, reasoning, requested };
  }

  /** Run one tool call; a missing tool or a throw is a result, not a crash. */
  private async run(call: Call, signal: AbortSignal): Promise<{ output?: string; error?: string }> {
    const tool = this.tools.find((candidate) => candidate.name === call.name);
    if (tool === undefined) {
      return { error: `No tool named \`${call.name}\` is declared in this session.` };
    }
    let args: Record<string, unknown>;
    try {
      const parsed: unknown = JSON.parse(call.args === "" ? "{}" : call.args);
      args =
        parsed !== null && typeof parsed === "object" ? (parsed as Record<string, unknown>) : {};
    } catch {
      return { error: "The call's arguments were not valid JSON." };
    }
    try {
      return { output: await tool.run(args, signal) };
    } catch (cause) {
      return { error: cause instanceof Error ? cause.message : String(cause) };
    }
  }
}

const functionCall = (item: unknown): Call | undefined => {
  if (item === null || typeof item !== "object") return undefined;
  const record = item as Record<string, unknown>;
  if (record["type"] !== "function_call") return undefined;
  const callId = record["call_id"];
  const name = record["name"];
  const args = record["arguments"];
  if (typeof callId !== "string" || typeof name !== "string") return undefined;
  return { callId, name, args: typeof args === "string" ? args : "{}" };
};

const failureOf = (event: Record<string, unknown>): string => {
  const response = event["response"];
  if (response !== null && typeof response === "object") {
    const error = (response as Record<string, unknown>)["error"];
    if (error !== null && typeof error === "object") {
      const message = (error as Record<string, unknown>)["message"];
      if (typeof message === "string") return message;
    }
  }
  return "no reason was given";
};

/** Each SSE frame's `data:` payload, in order. */
async function* frames(
  body: ReadableStream<Uint8Array>,
  signal: AbortSignal,
): AsyncIterable<string> {
  const decoder = new TextDecoder();
  const reader = body.getReader();
  let buffer = "";
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done || signal.aborted) break;
      buffer += decoder.decode(value, { stream: true });
      for (;;) {
        const boundary = buffer.indexOf("\n\n");
        if (boundary < 0) break;
        const frame = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + 2);
        for (const line of frame.split("\n")) {
          if (line.startsWith("data: ")) yield line.slice(6);
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}

const parse = (data: string): Record<string, unknown> | undefined => {
  try {
    const value: unknown = JSON.parse(data);
    return value !== null && typeof value === "object"
      ? (value as Record<string, unknown>)
      : undefined;
  } catch {
    return undefined;
  }
};
