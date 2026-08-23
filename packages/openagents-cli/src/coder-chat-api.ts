/**
 * A reply source backed by the account chat API.
 *
 * The CLI never holds a provider key and never talks to a model vendor. It
 * submits a turn and reads the durable event log the server writes, so a coder
 * thread costs exactly what the server metered and leaves the same receipts the
 * web surface leaves.
 *
 * Which model answers is the server's `model` parameter, and every backend
 * answers with the same events, so this file has no branch per backend: the
 * backend is a value it sends and a label it reports.
 *
 * Two properties of the shipped contract shape this file:
 *
 * - The server records one conversation per account (`DATA-002`) and one active
 *   turn per conversation (`TURN-001`). A coder session therefore shares the
 *   account's conversation rather than opening its own, and a second turn while
 *   one is running is refused with `turn_in_progress` rather than queued.
 * - `GET /api/v3/chat/events` returns the conversation's whole event log, not a
 *   stream. This polls it and yields what is new, which is why the reply
 *   appears in pieces rather than at once.
 */

import { type CoderBackend, defaultBackend, nextBackend } from "./coder-backends.js";
import type { ReplyChunk } from "./coder-session.js";

const SUBMIT_PATH = "/api/v3/chat/turns";
const EVENTS_PATH = "/api/v3/chat/events";

const POLL_INTERVAL_MS = 250;
/** Give up rather than poll forever when a turn never reaches a terminal event. */
const TURN_TIMEOUT_MS = 300_000;

export interface ChatApiOptions {
  readonly origin: string;
  readonly token: string;
  /** Reasoning effort the server passes to the provider. */
  readonly reasoning?: string | undefined;
  /** The backend that answers. Defaults to the first in the published list. */
  readonly backend?: CoderBackend | undefined;
}

interface ChatEvent {
  readonly id?: string;
  readonly run_id?: string;
  readonly sequence?: number;
  readonly type?: string;
  readonly payload?: Record<string, unknown>;
  /**
   * The server's own projection of the tool call this event belongs to, which
   * every `tool_call_*` event carries. It already holds pretty-printed
   * arguments, the extracted result, and a structured error, so reading it
   * rather than the raw payload keeps the CLI and the web surface showing the
   * same tool call.
   */
  readonly tool_call?: ToolCallView;
}

interface ToolCallView {
  readonly call_id?: string;
  readonly name?: string;
  readonly arguments?: string;
  readonly output?: string | null;
  readonly error?: { readonly code?: string | null; readonly message?: string | null } | null;
  readonly status?: string;
}

export class ChatApiUnavailable extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "ChatApiUnavailable";
  }
}

/**
 * Submit a turn and yield what the server records, in the order it records it.
 *
 * A turn interleaves reasoning, tool calls, and assistant text. Every one of
 * those becomes a chunk here. An earlier version yielded only `text_delta`,
 * which made a tool call invisible and joined the sentence before it to the
 * sentence after it.
 */
export class ChatApiReplySource {
  private backend: CoderBackend;

  constructor(private readonly options: ChatApiOptions) {
    this.backend = options.backend ?? defaultBackend();
  }

  /** The label the status line shows, which is the current backend's. */
  get model(): string {
    return this.backend.label;
  }

  /** The id the next turn sends as `model`. */
  get backendId(): string {
    return this.backend.id;
  }

  /**
   * Move to the next backend and return its label.
   *
   * This changes only what the next turn asks for. A turn already running was
   * submitted with the backend it named and keeps it, because the server has
   * already accepted that turn and cannot be told to change its mind.
   */
  cycleBackend(): string {
    this.backend = nextBackend(this.backend);
    return this.backend.label;
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    const seen = await this.latestSequence();
    const runId = await this.submit(prompt, signal);
    const startedAt = Date.now();
    const delivered = new Map<string, number>();
    if (runId !== undefined) delivered.set(runId, seen.get(runId) ?? -1);

    while (!signal.aborted) {
      if (Date.now() - startedAt > TURN_TIMEOUT_MS) {
        throw new ChatApiUnavailable(
          "turn_timed_out",
          "The turn produced no terminal event within five minutes.",
        );
      }

      const events = await this.events(signal);
      if (signal.aborted) return;

      // A submit that answered without a run id still identifies its run in the
      // log; take the newest run not already accounted for.
      const target = runId ?? newestRun(events, seen);
      if (target === undefined) {
        await sleep(POLL_INTERVAL_MS, signal);
        continue;
      }

      const floor = delivered.get(target) ?? seen.get(target) ?? -1;
      let highest = floor;
      let finished = false;

      for (const event of events) {
        if (event.run_id !== target) continue;
        const sequence = typeof event.sequence === "number" ? event.sequence : -1;
        if (sequence <= floor) continue;
        highest = Math.max(highest, sequence);

        if (event.type === "text_delta") {
          const value = event.payload?.["value"];
          if (typeof value === "string" && value.length > 0) yield { type: "text", value };
        } else if (event.type === "reasoning_delta") {
          const value = event.payload?.["value"];
          if (typeof value === "string" && value.length > 0) yield { type: "reasoning", value };
        } else if (event.type === "tool_call_started") {
          const call = toolCall(event);
          if (call !== undefined) yield call;
        } else if (event.type === "tool_call_completed" || event.type === "tool_call_failed") {
          const result = toolResult(event);
          if (result !== undefined) yield result;
        } else if (event.type === "response_completed") {
          finished = true;
        } else if (event.type === "response_failed") {
          // The server names the terminal events `response_completed` and
          // `response_failed`, and reports why in `reason` with a stable
          // `code` beside it.
          const reason = event.payload?.["reason"];
          const code = event.payload?.["code"];
          throw new ChatApiUnavailable(
            typeof code === "string" ? code : "turn_failed",
            typeof reason === "string" ? reason : "The turn failed on the server.",
          );
        }
      }

      delivered.set(target, highest);
      if (finished) return;
      await sleep(POLL_INTERVAL_MS, signal);
    }
  }

  /** Highest sequence per run before submitting, so old events are not replayed. */
  private async latestSequence(): Promise<Map<string, number>> {
    const seen = new Map<string, number>();
    for (const event of await this.events()) {
      if (typeof event.run_id !== "string") continue;
      const sequence = typeof event.sequence === "number" ? event.sequence : -1;
      seen.set(event.run_id, Math.max(seen.get(event.run_id) ?? -1, sequence));
    }
    return seen;
  }

  private async submit(prompt: string, signal: AbortSignal): Promise<string | undefined> {
    const response = await fetch(new URL(SUBMIT_PATH, this.options.origin), {
      method: "POST",
      signal,
      headers: {
        authorization: `Bearer ${this.options.token}`,
        "content-type": "application/json",
        accept: "application/json",
      },
      body: JSON.stringify({
        message: prompt,
        model: this.backend.id,
        ...(this.options.reasoning === undefined ? {} : { reasoning: this.options.reasoning }),
      }),
    });

    const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;

    if (response.status === 401 || response.status === 403) {
      throw new ChatApiUnavailable(
        "scope_missing",
        "This token cannot reach the chat API. Sign in again with the chat:account scope.",
      );
    }
    if (response.status === 409) {
      throw new ChatApiUnavailable(
        "turn_in_progress",
        "The account already has a turn running. One turn runs at a time.",
      );
    }
    if (response.status === 429) {
      throw new ChatApiUnavailable("rate_limited", "The chat API is rate limiting this account.");
    }
    if (response.status < 200 || response.status >= 300) {
      const code = typeof body["error"] === "string" ? body["error"] : `http_${response.status}`;
      throw new ChatApiUnavailable(code, `The chat API refused the turn (${code}).`);
    }

    const turn = body["turn"];
    if (turn !== null && typeof turn === "object") {
      const id = (turn as Record<string, unknown>)["id"];
      if (typeof id === "string") return id;
    }
    return undefined;
  }

  private async events(signal?: AbortSignal): Promise<ReadonlyArray<ChatEvent>> {
    const response = await fetch(new URL(EVENTS_PATH, this.options.origin), {
      ...(signal === undefined ? {} : { signal }),
      headers: {
        authorization: `Bearer ${this.options.token}`,
        accept: "application/json",
      },
    });

    if (response.status === 401 || response.status === 403) {
      throw new ChatApiUnavailable(
        "scope_missing",
        "This token cannot read chat events. Sign in again with the chat:account scope.",
      );
    }
    if (response.status < 200 || response.status >= 300) return [];

    const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;
    const events = body["events"];
    return Array.isArray(events) ? (events as ReadonlyArray<ChatEvent>) : [];
  }
}

/** The start of a tool call, read from the server's projection of it. */
function toolCall(event: ChatEvent): Extract<ReplyChunk, { type: "tool_call" }> | undefined {
  const view = event.tool_call;
  const callId = view?.call_id ?? stringField(event.payload, "call_id");
  if (callId === undefined) return undefined;
  return {
    type: "tool_call",
    callId,
    name: view?.name ?? stringField(event.payload, "name") ?? "tool",
    arguments: view?.arguments ?? stringField(event.payload, "arguments") ?? "",
  };
}

/** The outcome of a tool call. `error` decides whether it succeeded. */
function toolResult(event: ChatEvent): Extract<ReplyChunk, { type: "tool_result" }> | undefined {
  const view = event.tool_call;
  const callId = view?.call_id ?? stringField(event.payload, "call_id");
  if (callId === undefined) return undefined;

  const message = view?.error?.message ?? stringField(event.payload, "error");
  const error = message ?? (event.type === "tool_call_failed" ? "The tool failed." : undefined);
  const output = view?.output ?? stringField(event.payload, "output");

  return { type: "tool_result", callId, output: output ?? undefined, error };
}

function stringField(
  payload: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = payload?.[key];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

/** The newest run in the log that the pre-submit snapshot did not know about. */
function newestRun(
  events: ReadonlyArray<ChatEvent>,
  seen: ReadonlyMap<string, number>,
): string | undefined {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const runId = events[index]?.run_id;
    if (typeof runId === "string" && !seen.has(runId)) return runId;
  }
  return undefined;
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
