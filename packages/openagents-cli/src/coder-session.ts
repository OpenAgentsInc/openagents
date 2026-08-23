/**
 * Session state for `openagents coder`.
 *
 * This module holds the transcript and the reply source, and knows nothing
 * about how either is drawn. The interface in `coder-ui.ts` and the
 * line-oriented fallback in `coder-plain.ts` both render the same snapshot, so
 * the two cannot disagree about what a session contains.
 *
 * A reply is not one string. A turn interleaves reasoning, tool calls, and
 * assistant text, and the transcript keeps them as separate entries in the
 * order the source produced them. An earlier version yielded only text, so a
 * tool call left no entry at all and the sentences on either side of it were
 * appended to the same entry and read as one run-on sentence.
 */

/** What a reply source produces. One entry kind per member. */
export type ReplyChunk =
  | { readonly type: "text"; readonly value: string }
  | { readonly type: "reasoning"; readonly value: string }
  | {
      readonly type: "tool_call";
      readonly callId: string;
      readonly name: string;
      /** Arguments as JSON source, pretty-printed when the server printed it. */
      readonly arguments: string;
    }
  | {
      readonly type: "tool_result";
      readonly callId: string;
      readonly output: string | undefined;
      readonly error: string | undefined;
    };

/** The tool half of a `tool` entry. Grows when the outcome arrives. */
export interface CoderToolCall {
  readonly callId: string;
  readonly name: string;
  readonly arguments: string;
  output: string | undefined;
  error: string | undefined;
  status: "running" | "succeeded" | "failed";
}

/** One entry in the transcript. */
export interface CoderEntry {
  readonly role: "you" | "assistant" | "notice" | "tool" | "reasoning";
  /** Rendered text. Streaming entries grow while chunks arrive. */
  text: string;
  /** False while chunks are still arriving, so a renderer can show a caret. */
  settled: boolean;
  /** Present on a `tool` entry only. */
  readonly tool?: CoderToolCall;
}

/** Everything a renderer needs. No renderer reads anything else. */
export interface CoderSnapshot {
  readonly entries: ReadonlyArray<CoderEntry>;
  /** True while a reply is streaming, which disables submission. */
  readonly running: boolean;
  readonly repository: string;
  readonly branch: string;
  readonly model: string;
  /**
   * Turns this process has submitted, counted from the moment one starts.
   *
   * A turn in flight is counted, because a status line that reads `0` under a
   * visibly streaming reply contradicts what the reader can see. The number is
   * deliberately about this process and nothing else, and the renderer says so:
   * the source may be writing into a conversation that already holds turns
   * this process never saw.
   */
  readonly turns: number;
  /**
   * What a reader needs to know about where this source records its turns, or
   * undefined when there is nothing to say. Shown once, at the start.
   */
  readonly scope: string | undefined;
}

/** Where reply chunks come from. One implementation today; ACP is the next. */
export interface ReplySource {
  /** The label the status line shows for the reply source. */
  readonly model: string;
  /**
   * One sentence about where this source's turns are recorded, shown once at
   * the start of a session. A source whose turns are private to this process
   * leaves it unset; a source that writes into a conversation shared with
   * another surface has to say so, because nothing else on screen would.
   */
  readonly scopeNotice?: string;
  /**
   * Yield the reply to `prompt` in chunks. Rendering appends each chunk as it
   * arrives, so a slow source shows partial text rather than nothing.
   */
  reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk>;
}

const DUMMY_PREAMBLE =
  "This is a dummy reply. `openagents coder` has its interface and its session " +
  "loop, and no agent behind them yet.";

/**
 * A reply source that answers without a model, so the interface can be run and
 * driven before the runtime exists.
 *
 * It streams word by word with a small delay because a reply that appears all
 * at once would not exercise the incremental rendering the real source needs,
 * and a rendering bug that only shows up mid-stream would stay hidden until
 * the runtime landed. It emits reasoning, a tool call, and Markdown for the
 * same reason: `--offline` has to exercise every entry kind the interface
 * draws, or a rendering defect only appears against the live model.
 */
export class DummyReplySource implements ReplySource {
  readonly model = "dummy (no agent attached)";

  constructor(private readonly delayMs = 18) {}

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    const thought =
      "The prompt asks about this repository. I should check what is connected " +
      "before answering, then describe what the session can do.";

    for (const token of tokenize(thought)) {
      if (signal.aborted) return;
      await sleep(this.delayMs, signal);
      if (signal.aborted) return;
      yield { type: "reasoning", value: token };
    }

    if (signal.aborted) return;
    const callId = "dummy-call-1";
    yield {
      type: "tool_call",
      callId,
      name: "repo_grep",
      arguments: `{\n  "max_results": 30,\n  "pattern": ${JSON.stringify(prompt.trim())}\n}`,
    };
    await sleep(this.delayMs * 6, signal);
    if (signal.aborted) return;
    yield {
      type: "tool_result",
      callId,
      output: `{\n  "matches": [],\n  "status": "empty"\n}`,
      error: undefined,
    };

    const body = [
      `## ${DUMMY_PREAMBLE}`,
      "",
      `You said: **${prompt.trim()}**`,
      "",
      "What works right now:",
      "",
      "- the transcript, the composer, and *streaming*",
      "- interruption, the status line, and `--plain`",
      "- Markdown, including a wrapped list item whose continuation lines line " +
        "up under the item text rather than under its bullet",
      "",
      "What does not: reading files, running commands, and answering the " +
        "question you actually asked.",
      "",
      "```elixir",
      'def hello, do: "world"',
      "```",
    ].join("\n");

    for (const token of tokenize(body)) {
      if (signal.aborted) return;
      await sleep(this.delayMs, signal);
      if (signal.aborted) return;
      yield { type: "text", value: token };
    }
  }
}

/** Split into chunks that keep newlines attached, so blank lines survive. */
function tokenize(text: string): ReadonlyArray<string> {
  return text.split(/(\s+)/).filter((piece) => piece.length > 0);
}

function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal.aborted) return resolve();
    const timer = setTimeout(finish, ms);
    signal.addEventListener("abort", finish, { once: true });
    function finish() {
      clearTimeout(timer);
      signal.removeEventListener("abort", finish);
      resolve();
    }
  });
}

/**
 * The session: a transcript, one reply at a time, and cancellation.
 *
 * A renderer subscribes with `onChange` and redraws from `snapshot()`. It never
 * mutates state directly, so the plain and interface paths cannot drift.
 */
export class CoderSession {
  private readonly entries: CoderEntry[] = [];
  private readonly listeners = new Set<() => void>();
  private controller: AbortController | undefined;
  private turnCount = 0;

  constructor(
    private readonly source: ReplySource,
    private readonly repository: string,
    private readonly branch: string,
  ) {}

  snapshot(): CoderSnapshot {
    return {
      entries: this.entries.map(copyEntry),
      running: this.controller !== undefined,
      repository: this.repository,
      branch: this.branch,
      model: this.source.model,
      turns: this.turnCount,
      scope: this.source.scopeNotice,
    };
  }

  onChange(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  notice(text: string): void {
    this.entries.push({ role: "notice", text, settled: true });
    this.emit();
  }

  get running(): boolean {
    return this.controller !== undefined;
  }

  /**
   * Submit a prompt and stream its reply.
   *
   * Refuses while a reply is running rather than queueing, because a queued
   * prompt that lands after an interruption is a prompt the user did not mean
   * to send.
   */
  async submit(prompt: string): Promise<void> {
    if (this.controller !== undefined) return;
    if (prompt.trim().length === 0) return;

    this.entries.push({ role: "you", text: prompt, settled: true });
    // An empty assistant entry from the start, so the interface shows a caret
    // rather than nothing while the first chunk is in flight. It is withdrawn
    // if the turn opens with reasoning or a tool call instead of text.
    const opening: CoderEntry = { role: "assistant", text: "", settled: false };
    this.entries.push(opening);

    /** The entry each streaming chunk kind is currently appending to. */
    let text: CoderEntry | undefined = opening;
    let reasoning: CoderEntry | undefined;

    const settle = (entry: CoderEntry | undefined) => {
      if (entry !== undefined) entry.settled = true;
    };
    const withdrawOpening = () => {
      const at = this.entries.indexOf(opening);
      if (at >= 0 && opening.text.length === 0) this.entries.splice(at, 1);
    };

    const controller = new AbortController();
    this.controller = controller;
    // Counted here rather than on completion. A turn that is happening is a
    // turn, and counting it only once it settled is what made the status line
    // read `0 replies` under a reply the reader was watching arrive.
    this.turnCount += 1;
    this.emit();

    try {
      for await (const chunk of this.source.reply(prompt, controller.signal)) {
        if (controller.signal.aborted) break;

        if (chunk.type === "text") {
          if (text === undefined) {
            settle(reasoning);
            reasoning = undefined;
            text = { role: "assistant", text: "", settled: false };
            this.entries.push(text);
          }
          text.text += chunk.value;
        } else if (chunk.type === "reasoning") {
          if (reasoning === undefined) {
            if (text === opening) withdrawOpening();
            settle(text);
            text = undefined;
            reasoning = { role: "reasoning", text: "", settled: false };
            this.entries.push(reasoning);
          }
          reasoning.text += chunk.value;
        } else if (chunk.type === "tool_call") {
          if (text === opening) withdrawOpening();
          settle(text);
          settle(reasoning);
          text = undefined;
          reasoning = undefined;
          this.entries.push({
            role: "tool",
            text: chunk.name,
            settled: false,
            tool: {
              callId: chunk.callId,
              name: chunk.name,
              arguments: chunk.arguments,
              output: undefined,
              error: undefined,
              status: "running",
            },
          });
        } else {
          this.applyToolResult(chunk);
        }

        this.emit();
      }

      if (controller.signal.aborted) {
        // Keep the partial text. Cancellation is a state transition, not a
        // failure, so what the agent already said stays on the transcript.
        const last = text ?? reasoning;
        if (last !== undefined && last.text.length > 0) last.text += "\n\n[interrupted]";
      }
    } catch (cause) {
      // A failed turn ends the turn, not the session. The reason belongs on the
      // transcript where the prompt that caused it is still visible, and any
      // text the source produced before failing is kept.
      const message = cause instanceof Error ? cause.message : String(cause);
      if (text !== undefined && text.text.length === 0) {
        this.entries.splice(this.entries.indexOf(text), 1);
        text = undefined;
      }
      this.entries.push({ role: "notice", text: message, settled: true });
    } finally {
      for (const entry of this.entries) {
        if (entry.settled) continue;
        entry.settled = true;
        // A tool call the turn never resolved has no outcome to report.
        if (entry.tool?.status === "running") entry.tool.status = "failed";
      }
      this.controller = undefined;
      this.emit();
    }
  }

  /** Interrupt the running reply. No effect when nothing is running. */
  interrupt(): boolean {
    if (this.controller === undefined) return false;
    this.controller.abort();
    return true;
  }

  private applyToolResult(chunk: Extract<ReplyChunk, { type: "tool_result" }>): void {
    for (let index = this.entries.length - 1; index >= 0; index -= 1) {
      const tool = this.entries[index]?.tool;
      if (tool === undefined || tool.callId !== chunk.callId) continue;
      tool.output = chunk.output;
      tool.error = chunk.error;
      tool.status = chunk.error === undefined ? "succeeded" : "failed";
      const entry = this.entries[index];
      if (entry !== undefined) entry.settled = true;
      return;
    }
  }

  private emit(): void {
    for (const listener of this.listeners) listener();
  }
}

/** A renderer must not be able to mutate the transcript through its snapshot. */
function copyEntry(entry: CoderEntry): CoderEntry {
  return entry.tool === undefined ? { ...entry } : { ...entry, tool: { ...entry.tool } };
}
