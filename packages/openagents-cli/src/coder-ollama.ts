/**
 * A reply source that calls a local Ollama server.
 *
 * The `ollama` client is used directly, not through the OpenAgents proxy, so
 * the caller's machine must already be running an Ollama server. The default
 * endpoint is `http://127.0.0.1:11434` and the model is read from the
 * `ollama:<name>` shape of the `--model` flag.
 *
 * Ollama chat messages are kept locally in this source; nothing is sent to the
 * OpenAgents chat API. A local model spends no metered budget, so `budget` is
 * left undefined.
 */

import { Ollama } from "ollama";

import type { ReplyChunk, ReplySource } from "./coder-session.js";

const DEFAULT_HOST = "http://127.0.0.1:11434";

export interface OllamaOptions {
  /** The Ollama model name, without the `ollama:` prefix. */
  readonly model: string;
  /** The Ollama server endpoint. Defaults to `http://127.0.0.1:11434`. */
  readonly host?: string | undefined;
}

/** True when `--model` names an Ollama source. */
export const isOllamaModelFlag = (value: string): boolean => value.startsWith("ollama:");

/** Extract the Ollama model name from an `ollama:<name>` flag value. */
export const parseOllamaModelFlag = (value: string): string | undefined => {
  const match = /^ollama:(.+)$/.exec(value);
  return match?.[1]?.trim();
};

interface WireMessage {
  readonly role: "user" | "assistant";
  readonly content: string;
}

export class OllamaReplySource implements ReplySource {
  private readonly client: Ollama;
  private readonly modelName: string;
  private readonly transcript: WireMessage[] = [];

  get model(): string {
    return `Ollama ${this.modelName}`;
  }

  constructor(options: OllamaOptions) {
    this.client = new Ollama({ host: options.host ?? DEFAULT_HOST });
    this.modelName = options.model;
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    this.transcript.push({ role: "user", content: prompt });

    let assistant = "";
    const stream = await this.client.chat({
      model: this.modelName,
      messages: this.transcript,
      stream: true,
    });

    const onAbort = () => stream.abort();
    signal.addEventListener("abort", onAbort, { once: true });

    try {
      for await (const chunk of stream) {
        if (signal.aborted) break;

        const thinking = chunk.message.thinking;
        if (typeof thinking === "string" && thinking.length > 0) {
          yield { type: "reasoning", value: thinking };
        }

        const content = chunk.message.content;
        if (typeof content === "string" && content.length > 0) {
          assistant += content;
          yield { type: "text", value: content };
        }

        if (chunk.done) break;
      }
    } finally {
      signal.removeEventListener("abort", onAbort);
      if (assistant.length > 0) {
        this.transcript.push({ role: "assistant", content: assistant });
      }
    }
  }
}
