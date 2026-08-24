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
 *
 * Delegated children are deliberately not transcript entries. They outlive the
 * entry that launched them and they change after it settles, so they live in a
 * `CoderTaskRegistry` and reach renderers through `snapshot().tasks`. The
 * session owns the join between the two: it turns a `/delegate` line into
 * launches and reports each child's outcome back onto the transcript.
 */

import type { DelegationOutcome, DelegationRequest } from "./coder-delegate.js";
import { parseDelegateCommand } from "./coder-delegate.js";
import { exportTrajectory } from "./coder-export.js";
import { VERSION } from "./version.js";
import type { CoderTask, CoderTaskId, CoderTaskRegistry } from "./coder-tasks.js";
import type { CoderTool } from "./coder-tools.js";

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
      /**
       * What the turn cost, reported once at the end of it.
       *
       * A turn may take several LLM calls -- a model that asks for tools and
       * then answers -- so this is their total, with the count, rather than the
       * last call's figures presented as the turn's.
       */
      readonly type: "usage";
      readonly promptTokens?: number | undefined;
      readonly completionTokens?: number | undefined;
      readonly calls?: number | undefined;
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

/**
 * A failure, with the reason underneath it.
 *
 * Node reports a failed request as `fetch failed` and puts what actually
 * happened — the refused connection, the reset socket — in `cause`. Reporting
 * only the top of that chain tells a reader nothing they can act on, which is
 * how a session came to show `fetch failed` and nothing else.
 */
const describeFailure = (cause: unknown): string => {
  const seen = new Set<unknown>();
  const parts: string[] = [];
  let current = cause;

  while (current !== undefined && current !== null && !seen.has(current)) {
    seen.add(current);
    const text = current instanceof Error ? current.message : String(current);
    if (text.length > 0 && !parts.includes(text)) parts.push(text);
    current = current instanceof Error ? (current as { cause?: unknown }).cause : undefined;
  }

  return parts.length === 0 ? "The turn failed." : parts.join(": ");
};

/** What a turn cost, on the entry that closed it. */
export interface CoderMetrics {
  readonly promptTokens?: number | undefined;
  readonly completionTokens?: number | undefined;
  /** How many LLM calls the figures aggregate. */
  readonly calls?: number | undefined;
}

/** One entry in the transcript. */
export interface CoderEntry {
  readonly role: "you" | "assistant" | "notice" | "tool" | "reasoning";
  /**
   * When this entry was opened, in epoch milliseconds.
   *
   * Kept because a trajectory is a sequence of timed steps: `/export` has to
   * say when each turn happened, and reconstructing that from the order alone
   * would be inventing it.
   */
  readonly at: number;
  /** Rendered text. Streaming entries grow while chunks arrive. */
  text: string;
  /** False while chunks are still arriving, so a renderer can show a caret. */
  settled: boolean;
  /** Present on a `tool` entry only. */
  readonly tool?: CoderToolCall;
  /** Set on the entry a turn ended on, when the source reported the cost. */
  metrics?: CoderMetrics;
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
   * What the source may still spend, or undefined for a source that spends
   * nothing. A budget first shown when it runs out is a budget that already
   * cost somebody the work it was funding, so the status line carries it from
   * the first frame.
   */
  readonly budget: string | undefined;
  /**
   * Delegated children, oldest first. Empty when nothing was delegated, and
   * then no renderer draws a fleet at all.
   */
  readonly tasks: ReadonlyArray<CoderTask>;
}

/** What the session needs in order to delegate. Absent means it cannot. */
export interface CoderDelegation {
  readonly registry: CoderTaskRegistry;
  /** Usually a `DelegateFleet`. Narrow on purpose, so tests can stand in. */
  readonly fleet: { submit(request: DelegationRequest): Promise<DelegationOutcome> };
  /** Shown when the reader asks for help, and in the launch notice. */
  readonly label: string;
}

/** Where reply chunks come from. One implementation today; ACP is the next. */
export interface ReplySource {
  /** The label the status line shows for the reply source. */
  readonly model: string;
  /**
   * The model's identifier, when it differs from the label above.
   *
   * `model` is written for a narrow status bar. A record has to name something
   * a reader could run again, so an export prefers this and falls back to the
   * label when a source has only one name for itself.
   */
  readonly modelId?: string | undefined;
  /**
   * What this source may still spend, already formatted for the status line,
   * or undefined for a source that meters nothing. Read on every snapshot, so
   * a source spending against a ceiling reports the figure it is at rather
   * than the one it opened with.
   */
  readonly budget?: string | undefined;
  /**
   * Move to the next backend and return its new label.
   *
   * A source that has only one backend leaves this undefined, and the interface
   * then offers no key for it. That is deliberate: the bottom bar names a key
   * only where pressing it would do something.
   */
  cycleBackend?(): string;
  /**
   * Declare the tools the model may call.
   *
   * Optional because the tool runtime is the client's, not the transport's: a
   * source reaches it by running the calls a model asks for and reporting them
   * as chunks. A source that cannot do that leaves this undefined, and the
   * session then declares no tools rather than declaring tools nothing runs.
   */
  useTools?(tools: ReadonlyArray<CoderTool>): void;
  /**
   * Take a message mid-turn, to be read at the next step of the running turn.
   *
   * A turn is a loop of model calls, and between two of them is a place where
   * another message can join without stopping anything. That is what steering
   * is: not interrupting the model, and not waiting for it to finish, but
   * putting a sentence where it will be read next.
   *
   * Returns false when the source cannot take one, and the caller then holds it
   * until the turn ends rather than dropping it.
   */
  steer?(text: string): boolean;
  /**
   * The standing context this source sends with every turn, as text.
   *
   * What `/system` shows. A source reports what it actually sends rather than a
   * description of it, so the two cannot drift: a reader checking why a model
   * behaved a certain way is reading the thing the model read. A source that
   * does not compose its own context leaves this undefined.
   */
  describeContext?(): string;
  /**
   * The tools as declared, in the shape a trajectory records them.
   *
   * Optional for the same reason `describeContext` is: a source that declares
   * nothing has nothing to report, and an export then simply omits the field
   * the format already makes optional.
   */
  toolDefinitions?(): ReadonlyArray<Record<string, unknown>>;
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
  /**
   * Prompts typed while a turn was running, in the order they were typed.
   *
   * Their entries are already on the transcript, so a reader sees what they
   * said the moment they said it; only the sending waits.
   */
  private readonly pending: string[] = [];
  private turnCount = 0;
  private unsubscribeTasks: (() => void) | undefined;

  constructor(
    private readonly source: ReplySource,
    private readonly repository: string,
    private readonly branch: string,
    private readonly delegation?: CoderDelegation,
    /**
     * Put in front of the first prompt, and nowhere else.
     *
     * A session told how to approach its work needs that before its first
     * decision. It goes ahead of the first turn rather than into every one:
     * after that it is in the transcript, and paying for it again each turn
     * buys nothing.
     */
    private readonly standing?: string,
  ) {
    // A child reporting progress has to reach the renderer, and the renderer
    // subscribes to the session rather than to the registry, so the session
    // forwards. Without this the fleet block only moved when a chat chunk
    // happened to arrive.
    this.unsubscribeTasks = delegation?.registry.onChange(() => this.emit());
  }

  snapshot(): CoderSnapshot {
    return {
      entries: this.entries.map(copyEntry),
      running: this.controller !== undefined,
      repository: this.repository,
      branch: this.branch,
      model: this.source.model,
      turns: this.turnCount,
      budget: this.source.budget,
      tasks: this.delegation?.registry.list() ?? [],
    };
  }

  /** Whether `/delegate` does anything, which is what the interface reads. */
  get canDelegate(): boolean {
    return this.delegation !== undefined;
  }

  /**
   * Stop every running child.
   *
   * Children are stopped as a group because that is how they were launched and
   * how they are read. Stopping one of fifteen is what the detail view is for.
   */
  stopTasks(): number {
    const registry = this.delegation?.registry;
    if (registry === undefined) return 0;
    const running = registry.list().filter((task) => task.status === "running").length;
    if (running === 0) return 0;
    registry.stopAll();
    this.notice(`Stopped ${String(running)} ${running === 1 ? "child" : "children"}.`);
    return running;
  }

  /** Forget children nothing will look at again. Called on the interface tick. */
  pruneTasks(): void {
    this.delegation?.registry.prune();
  }

  /**
   * Whether this thread can change backend at all.
   *
   * False for a source with nothing to switch to, which is what the interface
   * reads before offering the key.
   */
  get canCycleBackend(): boolean {
    return typeof this.source.cycleBackend === "function";
  }

  /**
   * Move the next turn to the next backend.
   *
   * Refused while a turn is running. The running turn was submitted with the
   * backend it named and the server has already accepted it, so switching now
   * would change the label without changing the answer being streamed under it
   * — the status line would name a model that did not produce the text on
   * screen. The caller shows the refusal rather than switching silently.
   */
  cycleBackend(): { readonly switched: boolean; readonly label: string | undefined } {
    if (this.source.cycleBackend === undefined) return { switched: false, label: undefined };
    if (this.controller !== undefined) {
      this.notice("A turn is running. The model switches on the next turn.");
      return { switched: false, label: this.source.model };
    }

    const label = this.source.cycleBackend();
    this.notice(`Model switched to ${label}.`);
    return { switched: true, label };
  }

  onChange(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  notice(text: string): void {
    this.entries.push({ role: "notice", text, settled: true, at: Date.now() });
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
    if (prompt.trim().length === 0) return;

    // Delegation is not a turn: it does not go to the model, it does not block
    // the next prompt, and it is allowed while a reply is streaming. That is
    // the point of a fleet — the console keeps working while children run.
    // `/system` is not a turn either: it reads what the session already holds,
    // shows it as a notice, and sends nothing. A reader checking what the model
    // was told should not have to change what the model was told to find out.
    if (/^\/system\s*$/.test(prompt.trim())) {
      this.entries.push({ role: "you", text: prompt, settled: true, at: Date.now() });
      const context = this.source.describeContext?.();
      this.notice(
        context === undefined
          ? "This reply source composes no context of its own, so there is nothing to show."
          : context,
      );
      this.emit();
      return;
    }

    // `/export` is not a turn either: it writes what has already happened.
    if (/^\/export\s*$/.test(prompt.trim())) {
      this.entries.push({ role: "you", text: prompt, settled: true, at: Date.now() });
      try {
        const written = exportTrajectory(this.snapshot(), {
          model: this.source.modelId ?? this.source.model,
          toolDefinitions: this.source.toolDefinitions?.(),
          version: VERSION,
        });
        this.notice(
          `Exported ${String(written.steps)} step${written.steps === 1 ? "" : "s"} as ATIF to ${written.path}` +
            (written.copied ? " (path copied to the clipboard)." : "."),
        );
      } catch (cause) {
        this.notice(
          `The export could not be written: ${cause instanceof Error ? cause.message : String(cause)}`,
        );
      }
      this.emit();
      return;
    }

    const delegate = parseDelegateCommand(prompt);
    if (delegate !== undefined) {
      this.entries.push({ role: "you", text: prompt, settled: true, at: Date.now() });
      this.startDelegation(delegate.count, delegate.prompt, delegate.description);
      this.emit();
      return;
    }

    // A turn is running, so this one waits its place rather than being dropped.
    // Typing while the model works is how a reader steers, and an interface
    // that silently ignores the key is one that cannot be steered at all.
    if (this.controller !== undefined) {
      this.entries.push({ role: "you", text: prompt, settled: true, at: Date.now() });

      // Steering first: a source that runs a loop of model calls can read this
      // at its next step, so the model sees it while it is still working. A
      // source that cannot holds it until the turn ends instead of dropping it.
      if (this.source.steer?.(prompt) === true) {
        this.notice("Steering: the model reads this at its next step.");
        this.emit();
        return;
      }

      this.pending.push(prompt);
      this.notice(
        this.pending.length === 1
          ? "Queued. It goes to the model when this turn ends; press escape to interrupt and send it now."
          : `Queued, ${String(this.pending.length)} waiting.`,
      );
      this.emit();
      return;
    }

    this.entries.push({ role: "you", text: prompt, settled: true, at: Date.now() });
    await this.run(prompt);
  }

  /**
   * Send one prompt and stream its reply.
   *
   * Split from `submit` so a queued prompt, whose entry is already on the
   * transcript, is sent without adding a second one.
   */
  private async run(prompt: string): Promise<void> {
    // An empty assistant entry from the start, so the interface shows a caret
    // rather than nothing while the first chunk is in flight. It is withdrawn
    // if the turn opens with reasoning or a tool call instead of text.
    const opening: CoderEntry = { role: "assistant", text: "", settled: false, at: Date.now() };
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
      // The reader's entry above keeps what they typed; the model receives the
      // standing context ahead of it on the first turn only.
      const sent =
        this.standing === undefined || this.turnCount > 1
          ? prompt
          : `${this.standing}\n\n---\n\n${prompt}`;

      for await (const chunk of this.source.reply(sent, controller.signal)) {
        if (controller.signal.aborted) break;

        if (chunk.type === "text") {
          if (text === undefined) {
            settle(reasoning);
            reasoning = undefined;
            text = { role: "assistant", text: "", settled: false, at: Date.now() };
            this.entries.push(text);
          }
          text.text += chunk.value;
        } else if (chunk.type === "reasoning") {
          if (reasoning === undefined) {
            if (text === opening) withdrawOpening();
            settle(text);
            text = undefined;
            reasoning = { role: "reasoning", text: "", settled: false, at: Date.now() };
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
            at: Date.now(),
            tool: {
              callId: chunk.callId,
              name: chunk.name,
              arguments: chunk.arguments,
              output: undefined,
              error: undefined,
              status: "running",
            },
          });
        } else if (chunk.type === "usage") {
          // Onto the entry the turn ended on, which is the step a reader of the
          // trajectory would attribute the cost to. `calls` says how many LLM
          // calls it aggregates, so a turn that used tools is not read as one.
          const closing = text ?? reasoning ?? this.entries.at(-1);
          if (closing !== undefined) {
            closing.metrics = {
              promptTokens: chunk.promptTokens,
              completionTokens: chunk.completionTokens,
              calls: chunk.calls,
            };
          }
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
      const message = describeFailure(cause);
      if (text !== undefined && text.text.length === 0) {
        this.entries.splice(this.entries.indexOf(text), 1);
        text = undefined;
      }
      this.entries.push({ role: "notice", text: message, settled: true, at: Date.now() });
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

    // Whatever was typed during the turn goes now, in order. Its entry is
    // already on the transcript, so this sends without adding another.
    const next = this.pending.shift();
    if (next !== undefined) await this.run(next);
  }

  /** Interrupt the running reply. No effect when nothing is running. */
  interrupt(): boolean {
    if (this.controller === undefined) return false;
    this.controller.abort();
    return true;
  }

  /** Release the registry subscription. Safe to call twice. */
  close(): void {
    this.unsubscribeTasks?.();
    this.unsubscribeTasks = undefined;
  }

  /**
   * Launch `count` children on one prompt and report each as it lands.
   *
   * Not awaited: the whole reason to delegate is that the console stays usable
   * while the children work, so this returns as soon as they are submitted and
   * every outcome arrives later as a notice. Failures are reported per child
   * rather than as one summary, because with fifteen children the reader needs
   * to know which one.
   */
  private startDelegation(count: number, prompt: string, description: string): void {
    const delegation = this.delegation;
    if (delegation === undefined) {
      this.notice(
        "This session cannot delegate: children spend the session's thread, and this " +
          "session has none. Sign in with `openagents auth login`, " +
          "or start the session with `--child-model provider/model` to run children on a " +
          "provider of your own.",
      );
      return;
    }
    if (prompt.trim().length === 0) {
      this.notice("Usage: /delegate [<n>x] <prompt>. For example `/delegate 3x add tests`.");
      return;
    }

    this.notice(
      `Delegating ${String(count)} ${count === 1 ? "child" : "children"} to ${delegation.label}.`,
    );

    for (let index = 0; index < count; index += 1) {
      const request: DelegationRequest = {
        description,
        prompt,
        background: true,
      };
      void delegation.fleet.submit(request).then((outcome) => this.reportOutcome(outcome));
    }
  }

  private reportOutcome(outcome: DelegationOutcome): void {
    const registry = this.delegation?.registry;
    if (outcome.status === "refused") {
      this.notice(`Delegation refused (${outcome.code}): ${outcome.reason}`);
      return;
    }

    const task = registry?.get(outcome.taskId);
    const label = task === undefined ? outcome.taskId : `${outcome.taskId} ${task.description}`;
    if (outcome.status === "completed") {
      const first = firstLine(outcome.result);
      this.notice(first.length > 0 ? `${label} finished: ${first}` : `${label} finished.`);
    } else if (outcome.status === "failed") {
      this.notice(`${label} failed: ${outcome.error}`);
    } else {
      this.notice(`${label} stopped.`);
    }

    // Reported is read: the notice above is the delivery, so leaving the badge
    // on would ask the reader to go and find what they were just told.
    this.markRead(outcome.taskId);
  }

  private markRead(id: CoderTaskId): void {
    this.delegation?.registry.markRead(id);
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

/** The first non-empty line, which is how a result is announced in one row. */
function firstLine(text: string): string {
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.length > 0) return trimmed;
  }
  return "";
}

/** A renderer must not be able to mutate the transcript through its snapshot. */
function copyEntry(entry: CoderEntry): CoderEntry {
  return entry.tool === undefined ? { ...entry } : { ...entry, tool: { ...entry.tool } };
}
