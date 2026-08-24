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
import type { Message as OllamaMessage, Tool as OllamaTool, ToolCall as OllamaToolCall } from "ollama";

import type { ReplyChunk, ReplySource } from "./coder-session.js";
import type { CoderTool } from "./coder-tools.js";

const DEFAULT_HOST = "http://127.0.0.1:11434";

/**
 * How many times one turn may call tools before it has to answer.
 *
 * The same ceiling the thread lane uses, for the same reason: a model that
 * keeps delegating never reports to the reader. A local model spends no metered
 * budget, but it does spend the reader's wall clock and the children's.
 */
const MAX_TOOL_STEPS = 6;

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

/**
 * The transcript this source keeps, in Ollama's own message shape.
 *
 * A tool exchange is kept as the assistant turn that asked and a `tool` turn
 * carrying the output, which is what Ollama's chat API takes back. A model that
 * cannot see what its own call returned calls it again.
 */
type WireMessage = OllamaMessage;

export class OllamaReplySource implements ReplySource {
  private readonly client: Ollama;
  private readonly modelName: string;
  private readonly transcript: WireMessage[] = [];
  private tools: ReadonlyArray<CoderTool> = [];
  private callCount = 0;

  get model(): string {
    return `Ollama ${this.modelName}`;
  }

  constructor(options: OllamaOptions) {
    this.client = new Ollama({ host: options.host ?? DEFAULT_HOST });
    this.modelName = options.model;
  }

  /**
   * Declare the tools the model may call.
   *
   * Set after construction for the same reason the thread lane sets them then:
   * the tools need things built after the source exists, such as the fleet a
   * `delegate` call submits to.
   */
  useTools(tools: ReadonlyArray<CoderTool>): void {
    this.tools = tools;
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    this.transcript.push({ role: "user", content: prompt });

    // A turn is a loop, not a single call: the model may answer, or it may ask
    // for tools and then answer once it has seen what they returned. The
    // ceiling is what stops a model that only ever delegates.
    for (let step = 0; step < MAX_TOOL_STEPS; step += 1) {
      if (signal.aborted) return;

      const calls: OllamaToolCall[] = [];
      let assistant = "";

      const stream = await this.client.chat({
        model: this.modelName,
        // A snapshot, not the live array: the transcript grows while the round
        // streams, and a request that keeps growing after it was sent is a
        // request nobody can reason about.
        messages: [...this.transcript],
        stream: true,
        ...(this.tools.length === 0
          ? {}
          : {
              tools: this.tools.map((tool) => ({
                type: "function",
                function: {
                  name: tool.name,
                  description: tool.description,
                  // The client's type for a schema is narrower than JSON
                  // Schema. The server takes the schema as written.
                  parameters: tool.parameters as NonNullable<OllamaTool["function"]["parameters"]>,
                },
              })),
            }),
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

          // Ollama sends whole calls rather than the fragments the chat lane
          // folds together, so they are collected as they arrive.
          const toolCalls = chunk.message.tool_calls;
          if (Array.isArray(toolCalls)) calls.push(...toolCalls);

          if (chunk.done) break;
        }
      } finally {
        signal.removeEventListener("abort", onAbort);
      }

      if (signal.aborted) return;

      // Whatever the model said before asking is kept with the calls, so the
      // next round sees its own turn as it happened.
      this.transcript.push({
        role: "assistant",
        content: assistant,
        ...(calls.length === 0 ? {} : { tool_calls: calls }),
      });

      if (calls.length === 0) return;

      for (const call of calls) {
        if (signal.aborted) return;
        yield* this.invoke(call, signal);
      }
    }

    // The ceiling was reached. Say so rather than ending on a tool result the
    // reader has to interpret as an answer.
    yield {
      type: "text",
      value: `\n\nStopped after ${String(MAX_TOOL_STEPS)} rounds of tool calls without an answer.`,
    };
  }

  /**
   * Run one call, report it, and put the result on the transcript.
   *
   * A tool that throws is reported as a failed call rather than ending the
   * turn: the model can act on "that needs a prompt" and cannot act on a turn
   * that died.
   */
  private async *invoke(call: OllamaToolCall, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    const name = call.function.name;
    // Ollama hands back parsed arguments where the chat lane sends JSON source.
    // The chunk carries the source because that is what the renderer shows.
    const args = (call.function.arguments ?? {}) as Record<string, unknown>;
    // Ollama numbers no calls, so the id is this session's own. It only has to
    // be stable between the `tool_call` chunk and its `tool_result`.
    this.callCount += 1;
    const callId = `${name}-${String(this.callCount)}`;

    yield { type: "tool_call", callId, name, arguments: JSON.stringify(args, undefined, 2) };

    const tool = this.tools.find((candidate) => candidate.name === name);
    let output: string;
    let failure: string | undefined;

    if (tool === undefined) {
      failure = `This session has no \`${name}\` tool.`;
      output = failure;
    } else {
      try {
        output = await tool.run(args, signal);
      } catch (cause) {
        failure = cause instanceof Error ? cause.message : String(cause);
        output = failure;
      }
    }

    yield { type: "tool_result", callId, output, error: failure };

    this.transcript.push({ role: "tool", content: output, tool_name: name });
  }
}
