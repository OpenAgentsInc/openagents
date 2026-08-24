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
 * How many rounds of tool calls one turn may take before it has to answer.
 *
 * High, because the ceiling is a backstop against a model that loops forever,
 * not a budget. It was six, and six is a number real work passes: a session
 * reading a package hit it after twenty steps and eighty thousand tokens and
 * ended with `Stopped after 6 rounds of tool calls without an answer` — every
 * one of those reads thrown away. A reader can stop a turn with escape at any
 * time, and that is the control that should decide when enough is enough.
 */
const MAX_TOOL_STEPS = 100;

export interface OllamaOptions {
  /** The Ollama model name, without the `ollama:` prefix. */
  readonly model: string;
  /** The Ollama server endpoint. Defaults to `http://127.0.0.1:11434`. */
  readonly host?: string | undefined;
}

/**
 * The local model to answer from, or undefined when there is no server.
 *
 * Probed with a short deadline because it runs before the first prompt on every
 * session: a machine with no Ollama on it must not pay for the question. A
 * refusal, a timeout, and an empty library are the same answer -- nothing to
 * answer from -- so the caller gets `undefined` for all three rather than a
 * failure to handle.
 *
 * The most recently modified model wins. With one model installed there is no
 * choice to make, and with several the one most recently pulled is the one the
 * reader was last working with.
 */
export const discoverOllamaModel = async (
  host: string = DEFAULT_HOST,
  timeoutMs = 300,
): Promise<string | undefined> => {
  const deadline = AbortSignal.timeout(timeoutMs);
  try {
    const response = await fetch(new URL("/api/tags", host), { signal: deadline });
    if (!response.ok) return undefined;
    const body = (await response.json()) as {
      models?: ReadonlyArray<{ name?: unknown; modified_at?: unknown }>;
    };
    const models = (body.models ?? []).filter(
      (model): model is { name: string; modified_at?: string } => typeof model.name === "string",
    );
    if (models.length === 0) return undefined;
    // Sorting a fresh array, so nothing shared is mutated.
    // eslint-disable-next-line unicorn/no-array-sort -- the spread is the copy
    return [...models].sort((left, right) =>
      String(right.modified_at ?? "").localeCompare(String(left.modified_at ?? "")),
    )[0]?.name;
  } catch {
    return undefined;
  }
};

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

/**
 * What the session tells a local model about itself.
 *
 * Derived from the tools actually declared rather than written out, so it
 * cannot claim a tool the session does not pass or miss one it does.
 *
 * The thread lane sends the server an objective at thread creation. The local
 * lane sent nothing, and a model with no system prompt has nothing anchoring
 * what it is: asked what tools it has, it answered from what a coding agent
 * usually has -- files, shell, search, web -- and none of that is declared
 * here. The invented answer then sat in the transcript, and the next turn read
 * it back as instruction. So the anchor is the tool list itself.
 */
const systemPrompt = (tools: ReadonlyArray<CoderTool>): string => {
  const lines = [
    "You are `openagents coder`, a coding assistant in a terminal. You answer from a model " +
      "running locally on this machine.",
    "",
  ];

  if (tools.length === 0) {
    lines.push(
      "You have no tools in this session: you cannot read or write files, run commands, or " +
        "reach anything outside this conversation. Answer from what the reader tells you, and " +
        "say plainly when something would need a tool you do not have.",
    );
  } else {
    lines.push(
      `You have ${String(tools.length)} tool${tools.length === 1 ? "" : "s"}, and no others:`,
      ...tools.map((tool) => `- \`${tool.name}\``),
      "",
      // Stated as a closed list rather than by naming the capabilities that are
      // absent. The absent ones change as tools are added -- this once said
      // there was no shell, and then there was one -- and a system message that
      // has to be edited when the tool list changes is one that will be wrong
      // in between.
      "That list is complete: a capability not on it is one you do not have, whatever a model " +
        "like you usually has. Read a tool's description before assuming what it covers. Where " +
        "a description says what a child agent can do, that is the child's capability and not " +
        "yours. Never say you ran something you did not run.",
    );
  }

  return lines.join("\n");
};

export class OllamaReplySource implements ReplySource {
  private readonly client: Ollama;
  private readonly host: string;
  private readonly modelName: string;
  private readonly transcript: WireMessage[] = [];
  private tools: ReadonlyArray<CoderTool> = [];
  /**
   * Messages that arrived mid-turn, waiting for the next step of it.
   *
   * Read between two model calls rather than at the end of the turn, which is
   * the difference between steering a model and waiting one out.
   */
  private steered: string[] = [];
  private callCount = 0;

  get model(): string {
    return `Ollama ${this.modelName}`;
  }

  /**
   * The identifier, as against `model`, which is the label a status line shows.
   *
   * A record has to name the model a reader could run again. "Ollama qwen3.8"
   * is for a narrow bar; `qwen3.8:27b-mtp-q8_0` is the thing itself.
   */
  get modelId(): string {
    return this.modelName;
  }

  constructor(options: OllamaOptions) {
    this.host = options.host ?? DEFAULT_HOST;
    this.client = new Ollama({ host: this.host });
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

  /**
   * Everything standing that goes to the model: the system message and the tool
   * declarations, rendered from the same values the request carries.
   */
  describeContext(): string {
    const parts = [`System message sent with every turn:\n\n${systemPrompt(this.tools)}`];

    parts.push(
      this.tools.length === 0
        ? "\nNo tools are declared to the model."
        : `\n${String(this.tools.length)} tool${this.tools.length === 1 ? "" : "s"} declared to the model:\n\n${this.tools
            .map(
              (tool) =>
                `- \`${tool.name}\`\n  ${tool.description}\n  parameters: ${JSON.stringify(tool.parameters)}`,
            )
            .join("\n\n")}`,
    );

    return parts.join("\n");
  }

  /** The tools as declared, in the shape ATIF records them. */
  toolDefinitions(): ReadonlyArray<Record<string, unknown>> {
    return this.tools.map((tool) => ({
      type: "function",
      function: { name: tool.name, description: tool.description, parameters: tool.parameters },
    }));
  }

  /**
   * The turn, with the endpoint named if the server stops answering.
   *
   * A local server that has stopped, or dropped the connection part way through
   * a long turn, reports as `fetch failed` and nothing else. Which server and
   * which model is the part a reader needs, and so is knowing the transcript
   * survives.
   */
  /** Take a message for the next step of the running turn. */
  steer(text: string): boolean {
    this.steered.push(text);
    return true;
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    try {
      yield* this.turn(prompt, signal);
    } catch (cause) {
      // Neutral about the reason, because the reason follows: a refused
      // connection and a model that does not exist are both reported here, and
      // claiming the server is down when it answered would send a reader to
      // check the wrong thing.
      throw new Error(
        `Ollama at ${this.host} could not answer for ${this.modelName}. ` +
          "This conversation is kept, so say `continue` once it can",
        { cause },
      );
    }
  }

  private async *turn(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    // Built on the first turn rather than in the constructor: the tools are
    // declared after construction, and the prompt is derived from them.
    if (this.transcript.length === 0) {
      this.transcript.push({ role: "system", content: systemPrompt(this.tools) });
    }

    this.transcript.push({ role: "user", content: prompt });

    // A turn is a loop, not a single call: the model may answer, or it may ask
    // for tools and then answer once it has seen what they returned. The
    // ceiling is what stops a model that only ever delegates.
    let promptTokens = 0;
    let completionTokens = 0;
    let llmCalls = 0;

    for (let step = 0; step < MAX_TOOL_STEPS; step += 1) {
      if (signal.aborted) return;

      // Anything the reader said since the last step joins here, before the
      // model is asked again. It reads as an ordinary turn in the conversation,
      // because that is what it is.
      for (const said of this.steered.splice(0)) {
        this.transcript.push({ role: "user", content: said });
      }

      const calls: OllamaToolCall[] = [];
      let assistant = "";

      // The last round is answered without tools. Reaching the ceiling with the
      // tools still on the table produced a turn that stopped mid-work and said
      // so, throwing away everything it had read; taking them away instead
      // leaves the model one thing it can do, which is report what it found.
      const finalRound = step === MAX_TOOL_STEPS - 1;
      if (finalRound && this.tools.length > 0) {
        this.transcript.push({
          role: "user",
          content:
            "You have reached this turn's limit on tool calls. Do not call another tool. " +
            "Answer now with what you have found, and say plainly what is still unfinished.",
        });
      }

      const stream = await this.client.chat({
        model: this.modelName,
        // A snapshot, not the live array: the transcript grows while the round
        // streams, and a request that keeps growing after it was sent is a
        // request nobody can reason about.
        messages: [...this.transcript],
        stream: true,
        ...(this.tools.length === 0 || finalRound
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

          if (chunk.done) {
            // The counts ride on the final chunk of each round, so they are
            // summed across the rounds a turn took rather than reported from
            // the last one.
            promptTokens += chunk.prompt_eval_count ?? 0;
            completionTokens += chunk.eval_count ?? 0;
            llmCalls += 1;
            break;
          }
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

      if (calls.length === 0) {
        yield { type: "usage", promptTokens, completionTokens, calls: llmCalls };
        return;
      }

      for (const call of calls) {
        if (signal.aborted) return;
        yield* this.invoke(call, signal);
      }
    }

    yield { type: "usage", promptTokens, completionTokens, calls: llmCalls };
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
