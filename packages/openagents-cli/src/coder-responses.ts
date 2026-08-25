/**
 * A reply source speaking the OpenResponses surface at `POST /api/v1/responses`.
 *
 * The dev lane's source: `openagents coder --dev` sends each prompt as an
 * OpenResponses request with `stream: true` and renders the semantic events
 * that come back — today, from the server's acknowledgement stub, which
 * answers every prompt "Acknowledged." in two deltas. No model stands behind
 * it yet; the point is that the client-side turn loop is built against the
 * OpenResponses event grammar before a provider is, so swapping the stub for
 * a real loop changes the server and nothing here.
 *
 * Only `response.output_text.delta` is rendered. The rest of the sequence —
 * created, item and part boundaries, completed — is parsed and passed over,
 * which is exactly what the grammar is for: a client reads the events it
 * understands and survives the ones it does not.
 */

import type { ReplyChunk, ReplySource } from "./coder-session.js";
import { tierLabel } from "./coder-tiers.js";

export interface ResponsesOptions {
  /** The API origin, such as `http://localhost:4000`. */
  readonly origin: string;
  /** The account bearer, sent when held; the stub also answers without one. */
  readonly token?: string | undefined;
}

export class ResponsesReplySource implements ReplySource {
  constructor(private readonly options: ResponsesOptions) {}

  /** The dev lane defers to the server, which is what Coder Auto names. */
  get model(): string {
    return tierLabel("auto");
  }

  /** The product id: no vendor stands behind the stub, so none is named. */
  get modelId(): string {
    return "openagents-coder";
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
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
      body: JSON.stringify({ input: prompt, stream: true }),
      signal,
    });

    if (!response.ok || response.body === null) {
      throw new Error(
        `The responses API at ${this.options.origin} answered HTTP ${String(response.status)}.`,
      );
    }

    let calls = 0;
    for await (const data of frames(response.body, signal)) {
      const event = parse(data);
      if (event === undefined) continue;
      if (event["type"] === "response.output_text.delta") {
        const delta = event["delta"];
        if (typeof delta === "string" && delta.length > 0) {
          yield { type: "text", value: delta };
        }
      }
      if (event["type"] === "response.completed") calls += 1;
    }

    yield { type: "usage", promptTokens: 0, completionTokens: 0, calls: Math.max(calls, 1) };
  }
}

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
