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
import type { CoderTask, CoderTaskId, CoderTaskRegistry } from "./coder-tasks.js";

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
  private unsubscribeTasks: (() => void) | undefined;

  constructor(
    private readonly source: ReplySource,
    private readonly repository: string,
    private readonly branch: string,
    private readonly delegation?: CoderDelegation,
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
    if (prompt.trim().length === 0) return;

    // Delegation is not a turn: it does not go to the model, it does not block
    // the next prompt, and it is allowed while a reply is streaming. That is
    // the point of a fleet — the console keeps working while children run.
    const delegate = parseDelegateCommand(prompt);
    if (delegate !== undefined) {
      this.entries.push({ role: "you", text: prompt, settled: true });
      this.startDelegation(delegate.count, delegate.prompt, delegate.description);
      this.emit();
      return;
    }

    if (this.controller !== undefined) return;

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
