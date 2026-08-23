/**
 * Session state for `openagents coder`.
 *
 * This module holds the transcript and the reply source, and knows nothing
 * about how either is drawn. The interface in `coder-ui.ts` and the
 * line-oriented fallback in `coder-plain.ts` both render the same snapshot, so
 * the two cannot disagree about what a session contains.
 *
 * The reply source is a stand-in. The delivered first stage replaces
 * `DummyReplySource` with an ACP client that spawns the agent runtime and
 * streams `session/update` notifications; nothing outside this file changes
 * when it does, because both produce the same chunks through the same
 * interface.
 */

/** One entry in the transcript. */
export interface CoderEntry {
  readonly role: "you" | "assistant" | "notice";
  /** Rendered text. Assistant entries grow while a reply streams. */
  text: string;
  /** False while chunks are still arriving, so a renderer can show a caret. */
  settled: boolean;
}

/** Everything a renderer needs. No renderer reads anything else. */
export interface CoderSnapshot {
  readonly entries: ReadonlyArray<CoderEntry>;
  /** True while a reply is streaming, which disables submission. */
  readonly running: boolean;
  readonly repository: string;
  readonly branch: string;
  readonly model: string;
  /** Replies produced this session, shown where a real grant shows call count. */
  readonly turns: number;
}

/** Where reply chunks come from. One implementation today; ACP is the next. */
export interface ReplySource {
  /** The label the status line shows for the reply source. */
  readonly model: string;
  /**
   * Yield the reply to `prompt` in chunks. Rendering appends each chunk as it
   * arrives, so a slow source shows partial text rather than nothing.
   */
  reply(prompt: string, signal: AbortSignal): AsyncIterable<string>;
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
 * the runtime landed.
 */
export class DummyReplySource implements ReplySource {
  readonly model = "dummy (no agent attached)";

  constructor(private readonly delayMs = 18) {}

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<string> {
    const body = [
      DUMMY_PREAMBLE,
      "",
      `You said: ${prompt.trim()}`,
      "",
      "What works right now: the transcript, the composer, streaming, " +
        "interruption, and the status line.",
      "",
      "What does not: reading files, running commands, and answering the " +
        "question you actually asked.",
    ].join("\n");

    for (const token of tokenize(body)) {
      if (signal.aborted) return;
      await sleep(this.delayMs, signal);
      if (signal.aborted) return;
      yield token;
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
      entries: this.entries.map((entry) => ({ ...entry })),
      running: this.controller !== undefined,
      repository: this.repository,
      branch: this.branch,
      model: this.source.model,
      turns: this.turnCount,
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
    const reply: CoderEntry = { role: "assistant", text: "", settled: false };
    this.entries.push(reply);

    const controller = new AbortController();
    this.controller = controller;
    this.emit();

    try {
      for await (const chunk of this.source.reply(prompt, controller.signal)) {
        if (controller.signal.aborted) break;
        reply.text += chunk;
        this.emit();
      }
      if (controller.signal.aborted && reply.text.length > 0) {
        // Keep the partial text. Cancellation is a state transition, not a
        // failure, so what the agent already said stays on the transcript.
        reply.text += "\n\n[interrupted]";
      }
    } finally {
      reply.settled = true;
      this.controller = undefined;
      this.turnCount += 1;
      this.emit();
    }
  }

  /** Interrupt the running reply. No effect when nothing is running. */
  interrupt(): boolean {
    if (this.controller === undefined) return false;
    this.controller.abort();
    return true;
  }

  private emit(): void {
    for (const listener of this.listeners) listener();
  }
}
