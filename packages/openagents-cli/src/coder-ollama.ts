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
import type {
  Message as OllamaMessage,
  Tool as OllamaTool,
  ToolCall as OllamaToolCall,
} from "ollama";

import { budgetedResult } from "./coder-tool-budget.js";
import { declaredDescription } from "./coder-tool-families.js";
import { merge } from "./coder-merge.js";
import type { ReplyChunk, ReplySource } from "./coder-session.js";
import { LOCAL_LANE, systemPrompt } from "./coder-system.js";
import type { CoderTool } from "./coder-tools.js";
import { tierLabel } from "./coder-tiers.js";
import type { TranscriptSink } from "./coder-transcript.js";

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

/**
 * How much of one tool's output reaches the durable `tool.ran` event.
 *
 * What the model is fed is bounded elsewhere and differently: this lane's
 * results are budgeted for the `local` family in `coder-tool-budget.ts`,
 * because they are re-sent on every round and a local model's window is a
 * fraction of a hosted one. This figure bounds a record written once, so it is
 * set where every result a real session has produced fits whole.
 */
const EVENT_RESULT_KEPT = 64_000;

/** A long tool result, kept at both ends. */
const bounded = (output: string, keep: number): string => {
  if (output.length <= keep) return output;
  const half = Math.floor(keep / 2);
  const cut = output.length - keep;
  return `${output.slice(0, half)}\n\n[${String(cut)} of ${String(output.length)} characters omitted from the middle; run it again more narrowly if you need them]\n\n${output.slice(-half)}`;
};

/**
 * What a reasoning level means to Ollama.
 *
 * `think` takes a boolean or, on a model that advertises thinking, one of these
 * names. `off` is the boolean, because a model asked to think at no level still
 * thinks; the way to stop it is to say not to.
 */
const THINK: Record<string, boolean | "low" | "medium" | "high"> = {
  off: false,
  low: "low",
  medium: "medium",
  high: "high",
};

/**
 * What `--reasoning` means here.
 *
 * The flag's ladder is the thread lane's — minimal through max — and Ollama has
 * four rungs, not five. `minimal` is off because a minimal amount of thinking
 * from a model that cannot be asked for a little is none, and `max` is `high`
 * because there is nothing above it. Named rather than silently clamped, so a
 * reader who asked for `max` and sees `high` can see why.
 */
const FROM_FLAG: Record<string, string> = {
  minimal: "off",
  off: "off",
  low: "low",
  medium: "medium",
  high: "high",
  max: "high",
};

/** The levels, in the order the interface cycles them. */
export const OLLAMA_REASONING_LEVELS = Object.keys(THINK);

/**
 * Where a model's reasoning starts when nobody says.
 *
 * `qwen3.8` thinks at length by default — one measured turn produced 150,322
 * characters of reasoning against 8,232 of answer — and on a local machine that
 * is the wall clock. It starts low, and `shift+tab` raises it for the turn that
 * needs it, which is the right way round: the reader asks for more thinking when
 * the work wants it rather than waiting for it on every question.
 */
const defaultReasoningFor = (model: string): string =>
  /^qwen3\.8\b/.test(model) ? "low" : "medium";

export interface OllamaOptions {
  /** The Ollama model name, without the `ollama:` prefix. */
  readonly model: string;
  /** The Ollama server endpoint. Defaults to `http://127.0.0.1:11434`. */
  readonly host?: string | undefined;
  /** Where the reasoning level starts. Defaults to the model's own default. */
  readonly reasoning?: string | undefined;
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
  return (await installedModels(host, timeoutMs))[0];
};

/**
 * The installed models, most recently modified first.
 *
 * Shared by discovery and by resolution, so the two cannot disagree about what
 * is on the machine.
 */
const installedModels = async (host: string, timeoutMs: number): Promise<ReadonlyArray<string>> => {
  try {
    const response = await fetch(new URL("/api/tags", host), {
      signal: AbortSignal.timeout(timeoutMs),
    });
    if (!response.ok) return [];
    const body = (await response.json()) as {
      models?: ReadonlyArray<{ name?: unknown; modified_at?: unknown }>;
    };
    const models = (body.models ?? []).filter(
      (model): model is { name: string; modified_at?: string } => typeof model.name === "string",
    );
    // eslint-disable-next-line unicorn/no-array-sort -- the spread is the copy
    return [...models]
      .sort((left, right) =>
        String(right.modified_at ?? "").localeCompare(String(left.modified_at ?? "")),
      )
      .map((model) => model.name);
  } catch {
    return [];
  }
};

/**
 * The installed model a name means, or undefined when none does.
 *
 * An Ollama name carries its size and quantisation after a colon —
 * `qwen3.8:27b-mtp-q8_0` — and a reader naming the model they pulled says
 * `qwen3.8`. Sending that unresolved gets `model not found` from a server that
 * has the model, which reads as the model being missing.
 *
 * Exact first, so a full name is never reinterpreted. Then the family prefix,
 * which is what a short name means. The most recently modified wins where
 * several match, the same rule the default uses.
 */
export const resolveOllamaModel = async (
  name: string,
  host: string = DEFAULT_HOST,
  timeoutMs = 2_000,
): Promise<{ readonly model?: string; readonly installed: ReadonlyArray<string> }> => {
  const installed = await installedModels(host, timeoutMs);
  if (installed.includes(name)) return { model: name, installed };

  // `qwen3.8` means `qwen3.8:…` and nothing else. A looser prefix would let it
  // mean `qwen3.85:…`, which is a different model: running one because it
  // shares a few characters with the one that was asked for is worse than
  // saying nothing matched and listing what is there.
  const family = installed.find((candidate) => candidate.startsWith(`${name}:`));
  return { ...(family === undefined ? {} : { model: family }), installed };
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

export class OllamaReplySource implements ReplySource {
  private readonly client: Ollama;
  private readonly host: string;
  private readonly modelName: string;
  private readonly transcript: WireMessage[] = [];
  /** Carried-over turns from a tier switch, spliced in after the anchor. */
  private seeded: ReadonlyArray<WireMessage> = [];
  private tools: ReadonlyArray<CoderTool> = [];
  /**
   * Messages that arrived mid-turn, waiting for the next step of it.
   *
   * Read between two model calls rather than at the end of the turn, which is
   * the difference between steering a model and waiting one out.
   */
  private steered: string[] = [];
  /**
   * Where the turn loop writes the durable transcript, when the session has
   * one. The local lane opens a transcript-only thread when it holds an
   * api-url and a token (coder-local-thread.ts); a session without either
   * attaches nothing and every call below is a no-op through optional
   * chaining. Inference never touches it — only the record travels.
   */
  private sink: TranscriptSink | undefined;
  private reasoningLevel: string;
  /** What the turn in flight has spent, so it is reported however it ends. */
  private spentIn = 0;
  private spentOut = 0;
  private calls = 0;
  private callCount = 0;

  get model(): string {
    return tierLabel("local");
  }

  /** The wire transcript so far, without the system anchor. */
  history(): ReadonlyArray<WireMessage> {
    return this.transcript.filter((message) => message.role !== "system");
  }

  /**
   * Seed the transcript with the conversation a tier switch carries over.
   *
   * Held aside until the first turn composes the system anchor, so the anchor
   * still leads and the carried turns follow it in order. Only plain user and
   * assistant text carries: the thread lane's tool-call structures are another
   * wire shape, and a local model rereads the words, not the plumbing.
   */
  preload(messages: ReadonlyArray<{ readonly role: string; readonly content?: unknown }>): void {
    this.seeded = messages
      .filter(
        (message): message is { role: "user" | "assistant"; content: string } =>
          (message.role === "user" || message.role === "assistant") &&
          typeof message.content === "string" &&
          message.content.length > 0,
      )
      .map((message) => ({ role: message.role, content: message.content }));
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
    this.reasoningLevel =
      options.reasoning === undefined
        ? defaultReasoningFor(options.model)
        : (FROM_FLAG[options.reasoning] ?? "medium");
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
  /** The session's workspace facts and active skills, for the system message. */
  private standing: string | undefined;

  useContext(standing: string): void {
    this.standing = standing;
  }

  useTools(tools: ReadonlyArray<CoderTool>): void {
    this.tools = tools;
  }

  /**
   * Attach the writer that puts this session's turns on the server.
   *
   * The same vocabulary the thread lane records — `turn.user`,
   * `turn.reasoning`, `tool.ran`, `turn.assistant` — so `/threads/:id`, the
   * export, and a resume read a local session exactly as they read a hosted
   * one. Set after construction because the writer needs the thread's id,
   * which does not exist until the transcript-only thread is opened.
   */
  useTranscript(sink: TranscriptSink): void {
    this.sink = sink;
  }

  /**
   * Everything standing that goes to the model: the system message and the tool
   * declarations, rendered from the same values the request carries.
   */
  describeContext(): string {
    const parts = [
      `System message sent with every turn:\n\n${systemPrompt(this.tools, LOCAL_LANE, this.standing)}`,
    ];

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
      function: {
        name: tool.name,
        // The local family's emphasis is the lane's economics — free tokens,
        // slow generation — so it applies whatever weights answer.
        description: declaredDescription(tool, "local"),
        parameters: tool.parameters,
      },
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
  get reasoning(): { readonly level: string; readonly levels: ReadonlyArray<string> } {
    return { level: this.reasoningLevel, levels: OLLAMA_REASONING_LEVELS };
  }

  cycleReasoning(): string {
    const at = OLLAMA_REASONING_LEVELS.indexOf(this.reasoningLevel);
    this.reasoningLevel =
      OLLAMA_REASONING_LEVELS[(at + 1) % OLLAMA_REASONING_LEVELS.length] ?? "medium";
    return this.reasoningLevel;
  }

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
    // Reported on the way out however the turn ends. Yielding it only where a
    // turn finished cleanly lost it on exactly the expensive ones: a long turn
    // that was interrupted, or that lost the server, recorded nothing, and the
    // export showed one turn's figures for a whole session.
    try {
      yield* this.rounds(prompt, signal);
    } finally {
      yield {
        type: "usage",
        promptTokens: this.spentIn,
        completionTokens: this.spentOut,
        calls: this.calls,
      };
    }
  }

  private async *rounds(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    // Built on the first turn rather than in the constructor: the tools are
    // declared after construction, and the prompt is derived from them.
    if (this.transcript.length === 0) {
      this.transcript.push({
        role: "system",
        content: systemPrompt(this.tools, LOCAL_LANE, this.standing),
      });
      this.transcript.push(...this.seeded);
      this.seeded = [];
    }

    this.transcript.push({ role: "user", content: prompt });
    this.sink?.record("turn.user", { text: prompt });

    // A turn is a loop, not a single call: the model may answer, or it may ask
    // for tools and then answer once it has seen what they returned. The
    // ceiling is what stops a model that only ever delegates.
    this.spentIn = 0;
    this.spentOut = 0;
    this.calls = 0;

    /** The answer so far, across rounds, for the one `turn.assistant` event. */
    let turnText = "";
    /** How many tools this turn ran, reported on `turn.assistant`. */
    let turnToolCalls = 0;

    for (let step = 0; step < MAX_TOOL_STEPS; step += 1) {
      if (signal.aborted) {
        this.recordAnswer(turnText, turnToolCalls, true);
        return;
      }

      // Anything the reader said since the last step joins here, before the
      // model is asked again. It reads as an ordinary turn in the conversation,
      // because that is what it is.
      const steered = this.steered.splice(0);
      for (const said of steered) {
        this.transcript.push({ role: "user", content: said });
        // Steered mid-turn rather than asked between turns, and the record
        // says so, or a replay would show a question the answer ignores.
        this.sink?.record("turn.user", { text: said, steered: true });
      }
      // The interface dims a steered message until this says it was read.
      if (steered.length > 0) yield { type: "steered", texts: steered };

      const calls: OllamaToolCall[] = [];
      let assistant = "";
      let reasoning = "";

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
        think: THINK[this.reasoningLevel] ?? "medium",
        ...(this.tools.length === 0 || finalRound
          ? {}
          : {
              tools: this.tools.map((tool) => ({
                type: "function",
                function: {
                  name: tool.name,
                  description: declaredDescription(tool, "local"),
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
            reasoning += thinking;
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
            this.spentIn += chunk.prompt_eval_count ?? 0;
            this.spentOut += chunk.eval_count ?? 0;
            this.calls += 1;
            break;
          }
        }
      } finally {
        signal.removeEventListener("abort", onAbort);
      }

      // One event per block, whole, never deltas: the record is what was
      // thought, not the pieces it arrived in.
      if (reasoning.length > 0) this.sink?.record("turn.reasoning", { text: reasoning });

      // Whatever the model said belongs to the thread even when the turn was
      // interrupted, or the next turn answers a question it cannot see it
      // half-answered.
      if (assistant.length > 0) {
        turnText = turnText.length === 0 ? assistant : `${turnText}\n\n${assistant}`;
      }

      if (signal.aborted) {
        this.recordAnswer(turnText, turnToolCalls, true);
        return;
      }

      // Whatever the model said before asking is kept with the calls, and so is
      // what it thought. Reasoning is part of the turn, not decoration on it: a
      // model that cannot see how it reached the last answer re-reasons its way
      // there, and a transcript missing it is not the turn that happened. Only
      // the deltas are discarded, and only because they are how the text
      // arrived rather than what it is.
      this.transcript.push({
        role: "assistant",
        content: assistant,
        ...(reasoning.length === 0 ? {} : { thinking: reasoning }),
        ...(calls.length === 0 ? {} : { tool_calls: calls }),
      });

      if (calls.length === 0) {
        this.recordAnswer(turnText, turnToolCalls, false);
        return;
      }
      turnToolCalls += calls.length;

      // Concurrently. A model asking for two tools in one turn is saying they do
      // not depend on each other, and running them in order anyway makes a fan-out
      // to two models cost the sum of both.
      if (signal.aborted) {
        this.recordAnswer(turnText, turnToolCalls, true);
        return;
      }
      yield* merge(calls.map((call) => this.invoke(call, signal)));
    }
  }

  /**
   * Record the turn's answer, with what it cost.
   *
   * One event per turn, whatever the turn took to get there, the same shape
   * the thread lane records. An interrupted turn is recorded too, marked as
   * such, because whatever streamed before Escape was said and the next
   * reader of this thread will be answered against it.
   */
  private recordAnswer(text: string, toolCalls: number, interrupted: boolean): void {
    if (this.sink === undefined) return;
    if (text.length === 0 && this.calls === 0) return;
    this.sink.record("turn.assistant", {
      text,
      usage: {
        prompt_tokens: this.spentIn,
        completion_tokens: this.spentOut,
        total_tokens: this.spentIn + this.spentOut,
        calls: this.calls,
      },
      tool_calls: toolCalls,
      ...(interrupted ? { interrupted: true } : {}),
    });
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

    // Call and result are one fact, so they are one event — the thread lane's
    // shape exactly, bounded far above the model-wire bound so the record
    // keeps what the model was fed a cut of.
    this.sink?.record("tool.ran", {
      call_id: callId,
      tool: name,
      arguments: bounded(JSON.stringify(args, undefined, 2), EVENT_RESULT_KEPT),
      status: failure === undefined ? "succeeded" : "failed",
      ...(failure === undefined
        ? { output: bounded(output, EVENT_RESULT_KEPT) }
        : { error: bounded(failure, EVENT_RESULT_KEPT) }),
    });

    // Budgeted for the local family: a small window and slow generation make
    // every re-sent character expensive in wall clock (coder-tool-budget.ts).
    this.transcript.push({
      role: "tool",
      content: budgetedResult(output, "local"),
      tool_name: name,
    });
  }
}
