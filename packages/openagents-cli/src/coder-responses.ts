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

export interface ResponsesOptions {
  /** The API origin, such as `http://localhost:4000`. */
  readonly origin: string;
  /** The account bearer, sent when held; the surface also answers without one. */
  readonly token?: string | undefined;
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

/**
 * Rounds of tool calls one turn may take before it must answer. A backstop
 * against a loop, not a budget; the reader can stop a turn at any time.
 */
const MAX_TOOL_ROUNDS = 24;

export class ResponsesReplySource implements ReplySource {
  private readonly items: Item[] = [];
  private tools: ReadonlyArray<CoderTool> = [];
  private standing: string | undefined;

  constructor(private readonly options: ResponsesOptions) {}

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

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    this.items.push({ role: "user", content: prompt });
    let calls = 0;

    for (let round = 0; round < MAX_TOOL_ROUNDS; round += 1) {
      calls += 1;
      const { text, requested } = yield* this.once(signal);
      if (text !== "") this.items.push({ role: "assistant", content: text });

      if (requested.length === 0) {
        yield { type: "usage", calls };
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
      }
    }

    yield {
      type: "text",
      value: `\n\nStopped after ${String(MAX_TOOL_ROUNDS)} rounds of tool calls without an answer.`,
    };
    yield { type: "usage", calls };
  }

  /** One request: stream the events, yield what renders, return the rest. */
  private async *once(
    signal: AbortSignal,
  ): AsyncGenerator<ReplyChunk, { text: string; requested: Call[] }> {
    const response = await fetch(new URL("/api/v1/responses", this.options.origin), {
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

    if (!response.ok || response.body === null) {
      throw new Error(
        `The responses API at ${this.options.origin} answered HTTP ${String(response.status)}.`,
      );
    }

    let text = "";
    const requested: Call[] = [];

    for await (const data of frames(response.body, signal)) {
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

    return { text, requested };
  }

  /** Run one tool call; a missing tool or a throw is a result, not a crash. */
  private async run(
    call: Call,
    signal: AbortSignal,
  ): Promise<{ output?: string; error?: string }> {
    const tool = this.tools.find((candidate) => candidate.name === call.name);
    if (tool === undefined) {
      return { error: `No tool named \`${call.name}\` is declared in this session.` };
    }
    let args: Record<string, unknown>;
    try {
      const parsed: unknown = JSON.parse(call.args === "" ? "{}" : call.args);
      args = parsed !== null && typeof parsed === "object" ? (parsed as Record<string, unknown>) : {};
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
