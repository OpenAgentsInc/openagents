/**
 * `openagents coder --resume`: back into a thread the account already holds.
 *
 * The shape is the Codex one decided in the openagents.com audit of
 * 2026-08-24: bare `--resume` shows a picker over recent threads filtered to
 * the current repository, `--resume <id>` names one directly, `--resume
 * --last` continues the most recent without asking, and `--all` drops the
 * repository filter. `GET /api/v3/threads` is the picker's list and
 * `GET /api/v3/threads/{id}/events` is the transcript it replays, paged
 * through the `after` cursor because the listing caps at fifty and a working
 * session passes fifty events inside an hour.
 *
 * Two reconstructions come out of one event stream, and they are different on
 * purpose:
 *
 * - **The session transcript** (`replayEntries`) is what the interface shows:
 *   one settled entry per recorded fact, in recorded order, so the reader
 *   scrolls the conversation they left.
 * - **The wire transcript** (`replayWire`) is what the model is answered
 *   against: exactly the messages the live turn loop would have accumulated —
 *   user turns as sent, tool exchanges as the paired
 *   `[tool call]`/`[tool result]` turns the proxy accepts, assistant text
 *   whole, and reasoning nowhere, because the live loop never puts a thought
 *   on the wire.
 *
 * Neither replay touches the transcript writer. The events being replayed are
 * the server's own; posting them again would double the record.
 *
 * The repository filter reads the objective. `POST /api/v3/threads` records
 * no structured repository or workspace field — the objective sentence is the
 * only place the opening session names where it ran — so the filter parses
 * back the exact sentence this CLI composes (`openagents coder in <repo> on
 * <branch>`). A thread opened with any other objective has no repository to
 * match and appears only under `--all`.
 */

import { createInterface } from "node:readline";

import type { CoderEntry } from "./coder-session.js";
import { boundedResult, ThreadUnavailable, type WireMessage } from "./coder-thread.js";

const THREADS_PATH = "/api/v3/threads";

/** The server's listing cap. Pages are read at exactly this size. */
const PAGE_LIMIT = 50;

/** One thread as `GET /api/v3/threads` reports it. */
export interface ThreadSummary {
  readonly id: string;
  readonly status: string;
  readonly objective: string;
  readonly eventCount: number;
  readonly startedAt: string | undefined;
  /** Parsed from the objective when this CLI composed it; otherwise absent. */
  readonly repository: string | undefined;
  readonly branch: string | undefined;
}

/** One event as `GET /api/v3/threads/{id}/events` reports it. */
export interface ThreadEvent {
  /** The cursor: a client continues from the last id it read. */
  readonly id: number;
  readonly eventType: string;
  readonly payload: Record<string, unknown>;
  readonly emittedAt: string | undefined;
}

/** The transport seam, so tests hand in a plain function. */
export type ResumeTransport = (input: URL, init?: RequestInit) => Promise<Response>;

export interface ResumeApiOptions {
  readonly origin: string;
  /** The account token. Listing and reading spend nothing. */
  readonly token: string;
  readonly fetch?: ResumeTransport | undefined;
}

/**
 * The repository and branch a thread's objective names, when this CLI named
 * them.
 *
 * This is deterministic parsing of a bounded field this same program wrote —
 * the session opener composes `openagents coder in <repo> on <branch>` — not
 * a guess at free text. Anything else parses to nothing and is simply a
 * thread without a repository.
 */
export function repositoryOf(
  objective: string,
): { readonly repository: string; readonly branch: string } | undefined {
  const match = /^openagents coder in (.+?) on (.+)$/.exec(objective);
  if (match === null) return undefined;
  const [, repository, branch] = match;
  if (repository === undefined || branch === undefined) return undefined;
  return { repository, branch };
}

/**
 * The threads the picker offers, newest first.
 *
 * Filtered to the named repository unless `all`, because a reader resuming
 * work is almost always resuming it where they are standing. Terminal threads
 * stay in the list: the CLI revokes its thread on a clean exit, so an
 * open-only list would usually be empty, and picking a terminal thread gets
 * the refusal that teaches why rather than a listing that hides it.
 */
export function resumableThreads(
  threads: ReadonlyArray<ThreadSummary>,
  repository: string,
  all: boolean,
): ReadonlyArray<ThreadSummary> {
  if (all) return threads;
  return threads.filter((thread) => thread.repository === repository);
}

/** The most recent candidate, which is what `--resume --last` takes. */
export function pickLast(threads: ReadonlyArray<ThreadSummary>): ThreadSummary | undefined {
  return threads[0];
}

/**
 * Refuse a thread that cannot be continued.
 *
 * A terminal thread holds no authority and its transcript is closed — the
 * server refuses both a re-mint and a new event — so resuming one could only
 * ever show history. The refusal names the status, because `cancelled` after
 * a clean exit and `failed` after an error call for different next steps.
 */
export function assertResumable(thread: ThreadSummary): void {
  if (thread.status === "open") return;
  throw new ThreadUnavailable(
    "thread_terminal",
    `Thread ${thread.id} is ${thread.status}: its transcript is closed and it holds no ` +
      "authority to re-grant. Start a new session with `openagents coder` instead.",
  );
}

/** The account's threads, newest first, as the server reports them. */
export async function listThreads(
  options: ResumeApiOptions,
): Promise<ReadonlyArray<ThreadSummary>> {
  const body = await get(
    options,
    new URL(`${THREADS_PATH}?limit=${String(PAGE_LIMIT)}`, options.origin),
    "The account's threads could not be listed",
  );
  const threads = body["threads"];
  if (!Array.isArray(threads)) return [];
  return threads.map((raw) => summaryOf(record(raw)));
}

/** One thread by id, for `--resume <id>` and for the status check. */
export async function fetchThread(
  options: ResumeApiOptions & { readonly threadId: string },
): Promise<ThreadSummary> {
  const body = await get(
    options,
    new URL(`${THREADS_PATH}/${options.threadId}`, options.origin),
    `Thread ${options.threadId} could not be read`,
  );
  return summaryOf(record(body["thread"]));
}

/**
 * The whole transcript, oldest first, through the cursor.
 *
 * Pages of `PAGE_LIMIT`, continued from the last event id read, until a page
 * comes back short. The cap is the server's; a session's history is exactly
 * the thing that outgrows it.
 */
export async function fetchAllEvents(
  options: ResumeApiOptions & { readonly threadId: string },
): Promise<ReadonlyArray<ThreadEvent>> {
  const collected: ThreadEvent[] = [];
  let after: number | undefined;

  for (;;) {
    const cursor = after === undefined ? "" : `&after=${String(after)}`;
    // Pages are read in order and each continues from the one before it.
    // eslint-disable-next-line no-await-in-loop
    const body = await get(
      options,
      new URL(
        `${THREADS_PATH}/${options.threadId}/events?limit=${String(PAGE_LIMIT)}${cursor}`,
        options.origin,
      ),
      `The transcript of thread ${options.threadId} could not be read`,
    );

    const raw = body["events"];
    const page = Array.isArray(raw) ? raw.map((value) => eventOf(record(value))) : [];
    collected.push(...page);

    const lastId = page.at(-1)?.id;
    if (page.length < PAGE_LIMIT || lastId === undefined) return collected;
    after = lastId;
  }
}

/**
 * The session transcript, rebuilt from the durable record.
 *
 * One settled entry per recorded fact, in recorded order. An interrupted
 * turn's answer carries the same `[interrupted]` marker the live interface
 * appended, because a replay that dropped it would show a turn that appears
 * to have finished. Event types outside the vocabulary are skipped rather
 * than refused: the transcript is append-only and a future writer may know
 * words this reader does not.
 */
export function replayEntries(events: ReadonlyArray<ThreadEvent>): ReadonlyArray<CoderEntry> {
  const entries: CoderEntry[] = [];

  for (const event of events) {
    const at = stampOf(event.emittedAt);
    const payload = event.payload;

    if (event.eventType === "turn.user") {
      entries.push({ role: "you", text: text(payload["text"]), settled: true, at });
    } else if (event.eventType === "turn.reasoning") {
      entries.push({ role: "reasoning", text: text(payload["text"]), settled: true, at });
    } else if (event.eventType === "tool.ran") {
      const name = text(payload["tool"]) || "tool";
      const failure = typeof payload["error"] === "string" ? payload["error"] : undefined;
      entries.push({
        role: "tool",
        text: name,
        settled: true,
        at,
        tool: {
          callId: text(payload["call_id"]),
          name,
          arguments: text(payload["arguments"]),
          output: typeof payload["output"] === "string" ? payload["output"] : undefined,
          error: failure,
          status: payload["status"] === "failed" || failure !== undefined ? "failed" : "succeeded",
        },
      });
    } else if (event.eventType === "turn.assistant") {
      const said = text(payload["text"]);
      const interrupted = payload["interrupted"] === true;
      const usage = record(payload["usage"]);
      const entry: CoderEntry = {
        role: "assistant",
        text: interrupted && said.length > 0 ? `${said}\n\n[interrupted]` : said,
        settled: true,
        at,
      };
      if (Object.keys(usage).length > 0) {
        entry.metrics = {
          promptTokens: count(usage["prompt_tokens"]),
          completionTokens: count(usage["completion_tokens"]),
          calls: count(usage["calls"]),
        };
      }
      entries.push(entry);
    }
  }

  return entries;
}

/**
 * The model-facing transcript, rebuilt in the shape the live loop feeds it.
 *
 * `turn.user` is a user message as sent — steered or not, the record holds
 * what reached the wire. `tool.ran` becomes the standard chat exchange: an
 * assistant message carrying the call in `tool_calls`, with the arguments as
 * the raw JSON string the record kept, then a `tool` message named by
 * `tool_call_id` with the result bounded by the same figure the live loop
 * uses, because this transcript is re-sent on every round and the bound is a
 * context-budget decision, not a property of the record. `turn.assistant` is
 * the turn's whole answer. `turn.reasoning` is deliberately absent: the live
 * loop never puts a thought on the wire.
 *
 * One call per assistant message, not one per round: the record does not
 * delimit rounds — a round of two concurrent calls and two rounds of one
 * land as the same two consecutive `tool.ran` events — and its assistant
 * prose is recorded once per turn, so regrouping here would be inventing a
 * structure the record does not hold. The provider only requires that every
 * `tool_calls` message is answered before the next assistant message, and
 * this shape keeps that invariant per call.
 */
export function replayWire(events: ReadonlyArray<ThreadEvent>): ReadonlyArray<WireMessage> {
  const messages: WireMessage[] = [];

  for (const event of events) {
    const payload = event.payload;

    if (event.eventType === "turn.user") {
      messages.push({ role: "user", content: text(payload["text"]) });
    } else if (event.eventType === "tool.ran") {
      const callId = text(payload["call_id"]);
      const outcome =
        typeof payload["output"] === "string"
          ? payload["output"]
          : typeof payload["error"] === "string"
            ? payload["error"]
            : "";
      messages.push({
        role: "assistant",
        content: "",
        tool_calls: [
          {
            id: callId,
            type: "function",
            function: {
              name: text(payload["tool"]) || "tool",
              arguments: text(payload["arguments"]),
            },
          },
        ],
      });
      messages.push({
        role: "tool",
        tool_call_id: callId,
        content: boundedResult(outcome),
      });
    } else if (event.eventType === "turn.assistant") {
      const said = text(payload["text"]);
      if (said.length > 0) messages.push({ role: "assistant", content: said });
    }
  }

  return messages;
}

/** One picker row: enough to choose by, in one line. */
export function describeThread(thread: ThreadSummary, index: number): string {
  const where = thread.repository === undefined ? thread.objective : thread.repository;
  const events = `${String(thread.eventCount)} event${thread.eventCount === 1 ? "" : "s"}`;
  const when =
    thread.startedAt === undefined ? "" : ` ${thread.startedAt.slice(0, 16).replace("T", " ")}`;
  return `${String(index + 1).padStart(3)}. ${thread.id.slice(0, 8)}  ${thread.status.padEnd(9)}  ${events.padEnd(10)}${when}  ${where}`;
}

/**
 * An answer to the picker, as a candidate index.
 *
 * Pure so the selection is testable without a terminal: a number from 1 to
 * `count` selects, anything else — empty, out of range, not a number —
 * cancels rather than guessing.
 */
export function parsePick(answer: string, candidates: number): number | undefined {
  const trimmed = answer.trim();
  if (!/^\d+$/.test(trimmed)) return undefined;
  const index = Number.parseInt(trimmed, 10) - 1;
  return index >= 0 && index < candidates ? index : undefined;
}

/**
 * Ask which thread to resume. TTY only — the non-interactive forms are
 * `--resume <id>` and `--resume --last`, and the caller enforces that.
 */
export async function pickThread(
  candidates: ReadonlyArray<ThreadSummary>,
  io: { readonly stdin: NodeJS.ReadableStream; readonly stdout: NodeJS.WritableStream },
): Promise<ThreadSummary | undefined> {
  io.stdout.write("Resume which thread?\n\n");
  candidates.forEach((thread, index) => {
    io.stdout.write(`${describeThread(thread, index)}\n`);
  });
  io.stdout.write("\n");

  const readline = createInterface({ input: io.stdin, output: io.stdout });
  try {
    const answer = await new Promise<string>((resolve) => {
      readline.question(`Thread [1-${String(candidates.length)}, enter cancels]: `, resolve);
    });
    const index = parsePick(answer, candidates.length);
    return index === undefined ? undefined : candidates[index];
  } finally {
    readline.close();
  }
}

// ── transport ─────────────────────────────────────────────────────────────

/** One authenticated GET, refused with the server's own code and sentence. */
async function get(
  options: ResumeApiOptions,
  url: URL,
  failure: string,
): Promise<Record<string, unknown>> {
  const transport = options.fetch ?? globalThis.fetch.bind(globalThis);
  const response = await transport(url, {
    headers: {
      authorization: `Bearer ${options.token}`,
      accept: "application/json",
    },
  }).catch((cause: unknown) => {
    throw new ThreadUnavailable(
      "network_refused",
      `The API at ${options.origin} could not be reached: ${String(cause)}`,
    );
  });

  const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;

  if (response.status === 401 || response.status === 403) {
    throw new ThreadUnavailable(
      "scope_missing",
      "This token cannot read the account's threads. Run `openagents auth login` to sign in again.",
      response.status,
    );
  }
  if (response.status === 404) {
    throw new ThreadUnavailable(
      "thread_not_found",
      `${failure}: this account holds no such thread.`,
      response.status,
    );
  }
  if (response.status < 200 || response.status >= 300) {
    const code = typeof body["code"] === "string" ? body["code"] : `http_${response.status}`;
    const message = typeof body["message"] === "string" ? body["message"] : `${failure} (${code}).`;
    throw new ThreadUnavailable(code, message, response.status);
  }

  return body;
}

// ── parsing ───────────────────────────────────────────────────────────────

function summaryOf(raw: Record<string, unknown>): ThreadSummary {
  const objective = text(raw["objective"]);
  const named = repositoryOf(objective);
  return {
    id: text(raw["id"]),
    status: text(raw["status"]) || "unknown",
    objective,
    eventCount: count(raw["event_count"]),
    startedAt: typeof raw["started_at"] === "string" ? raw["started_at"] : undefined,
    repository: named?.repository,
    branch: named?.branch,
  };
}

function eventOf(raw: Record<string, unknown>): ThreadEvent {
  return {
    id: count(raw["id"]),
    eventType: text(raw["event_type"]),
    payload: record(raw["payload"]),
    emittedAt: typeof raw["emitted_at"] === "string" ? raw["emitted_at"] : undefined,
  };
}

function stampOf(emittedAt: string | undefined): number {
  if (emittedAt === undefined) return Date.now();
  const parsed = Date.parse(emittedAt);
  return Number.isFinite(parsed) ? parsed : Date.now();
}

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function count(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}
