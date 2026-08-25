/**
 * Delegation for `openagents coder`: run many coding agents at once.
 *
 * One console cannot do fifteen things, so it hands each of them to a child
 * coding agent and watches all of them. This module is the seam between the two
 * halves that makes that safe to build on:
 *
 * - A `DelegateHarness` is anything that can run a prompt to completion while
 *   reporting normalized events. `OpencodeHarness` is the first one, and it
 *   drives the `opencode` CLI in its JSON event mode. A fake harness in the
 *   tests drives the same interface, so scheduling, cancellation, and rendering
 *   are testable without a model.
 * - A `DelegateFleet` owns the concurrency cap, the queue, and the writes into
 *   `CoderTaskRegistry`. Nothing else starts children.
 *
 * Three decisions here are worth the words:
 *
 * A refusal is a result, not an exception. Hitting the cap, an unknown harness,
 * and a missing worktree are all ordinary outcomes of asking for massive
 * fan-out, and a caller that has to catch exceptions to find out cannot report
 * them per child.
 *
 * The child's raw event stream is written to a file as it arrives. That file is
 * the child's transcript, and it exists whether or not anyone was watching,
 * which is the difference between a fleet you can review afterwards and a fleet
 * that only existed on screen.
 *
 * The parser is a pure function over one line. A harness that changes its event
 * shape then breaks one small tested function rather than the scheduler.
 */

import type { ChildProcess } from "node:child_process";
import { execFileSync, spawn } from "node:child_process";
import { appendFileSync, createWriteStream, mkdirSync } from "node:fs";

import { runDevinAcp } from "./coder-devin-acp.js";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import type {
  CoderTaskId,
  CoderTaskRegistry,
  CoderToolActivity,
  CoderToolActivityMeta,
} from "./coder-tasks.js";

/** What the console asks for. One shape whether it wants one child or fifteen. */
export interface DelegationRequest {
  /** Three to five words. This is what every compact surface shows. */
  readonly description: string;
  readonly prompt: string;
  /** Where the child works. Defaults to the console's own directory. */
  readonly cwd?: string | undefined;
  /** False to await the child inline; true to leave it running. */
  readonly background?: boolean | undefined;
}

/**
 * What a launch produced.
 *
 * `refused` carries a stable code so a caller can react to the reason rather
 * than to prose, and text so a person or a model reading it knows what to do
 * instead.
 */
export type DelegationOutcome =
  | { readonly status: "completed"; readonly taskId: CoderTaskId; readonly result: string }
  | { readonly status: "failed"; readonly taskId: CoderTaskId; readonly error: string }
  | { readonly status: "stopped"; readonly taskId: CoderTaskId }
  | { readonly status: "refused"; readonly code: RefusalCode; readonly reason: string };

export type RefusalCode = "fleet_full" | "empty_prompt" | "harness_unavailable";

/** A `/delegate` line the console typed, once understood. */
export interface DelegateCommand {
  /** How many children to launch with this prompt. */
  readonly count: number;
  readonly prompt: string;
  readonly description: string;
}

/** How many children one `/delegate` line may ask for. */
export const MAX_DELEGATE_COUNT = 32;

/**
 * Read a `/delegate` line.
 *
 * The grammar is `/delegate [<n>x] <prompt>`, so `/delegate 4x add tests to the
 * parser` launches four children on the same prompt. The count is a separate
 * leading token rather than a flag because the console is a chat box, not a
 * shell, and `--agents 4` in the middle of an English sentence reads as part of
 * the prompt.
 *
 * Returns undefined for anything that is not a delegate line, so an ordinary
 * prompt is unaffected.
 */
export function parseDelegateCommand(text: string): DelegateCommand | undefined {
  const match = /^\/delegate(?:\s+([\s\S]*))?$/.exec(text.trim());
  if (match === null) return undefined;

  let rest = (match[1] ?? "").trim();
  let count = 1;
  const fanout = /^(\d{1,3})x\s+([\s\S]+)$/.exec(rest);
  if (fanout !== null) {
    count = Math.min(MAX_DELEGATE_COUNT, Math.max(1, Number(fanout[1])));
    rest = (fanout[2] ?? "").trim();
  }

  return { count, prompt: rest, description: describePrompt(rest) };
}

/**
 * A short label for a prompt.
 *
 * Every compact surface shows this and nothing else, so it has to be short
 * enough to sit in a column: the first few words, which is what a person would
 * have typed as a title anyway.
 */
export function describePrompt(prompt: string): string {
  const words = prompt.replace(/\s+/g, " ").trim().split(" ").filter(Boolean);
  if (words.length === 0) return "delegated task";
  return words.slice(0, 5).join(" ");
}

/** A child's activity, normalized away from any one harness's event shape. */
export type DelegateEvent =
  | { readonly type: "session"; readonly sessionId: string }
  | {
      readonly type: "tool";
      readonly callId: string;
      readonly name: string;
      readonly target: string | undefined;
      /** Optional display metadata derived from the tool's input and output. */
      readonly meta?: CoderToolActivityMeta;
    }
  | { readonly type: "text"; readonly value: string }
  | { readonly type: "tokens"; readonly input: number; readonly output: number }
  | { readonly type: "error"; readonly message: string };

/** What the fleet needs from a way of running children. */
export interface DelegateHarness {
  /** Shown in the fleet block, for example `opencode`. */
  readonly agent: string;
  /** The child model, shown beside the agent. */
  readonly model: string;
  /**
   * Run `prompt` in `cwd`, yielding events as they happen.
   *
   * The harness must return when the child is done, throw when it could not be
   * run, and stop promptly when `signal` aborts. Everything else — retries,
   * permissions, provider credentials — is the harness's business.
   */
  run(
    input: {
      readonly prompt: string;
      readonly cwd: string;
      readonly transcriptPath: string;
      /**
       * A session to continue rather than start.
       *
       * Set only on a retry, and only when the first attempt got far enough to
       * report one. Resuming is what makes a retry safe: a child that ran
       * twenty-five tools before its provider dropped has already edited files,
       * and starting it again from the prompt would redo all of it.
       */
      readonly resumeSessionId?: string | undefined;
    },
    signal: AbortSignal,
  ): AsyncIterable<DelegateEvent>;
}

/**
 * Read one line of `opencode run --format json` output.
 *
 * Returns undefined for blank lines, for lines that are not JSON, and for event
 * kinds the fleet does not track. A harness that adds an event kind must not be
 * able to stop a fleet, so anything unrecognized is dropped rather than raised.
 */
export function parseOpencodeEvent(line: string): DelegateEvent | undefined {
  const trimmed = line.trim();
  if (trimmed.length === 0 || !trimmed.startsWith("{")) return undefined;

  let event: Record<string, unknown>;
  try {
    event = JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    return undefined;
  }

  const type = typeof event["type"] === "string" ? (event["type"] as string) : undefined;
  const part = isRecord(event["part"]) ? event["part"] : undefined;

  if (type === "tool_use" && part !== undefined) {
    const name = stringField(part, "tool") ?? "tool";
    const callId = stringField(part, "callID") ?? `${name}-${String(event["timestamp"] ?? "")}`;
    const state = isRecord(part["state"]) ? part["state"] : undefined;
    const { target, ...rest } = toolTarget(state);
    return { type: "tool", callId, name, target, ...rest };
  }

  if (type === "text" && part !== undefined) {
    const value = stringField(part, "text");
    return value === undefined ? undefined : { type: "text", value };
  }

  if (type === "step_finish" && part !== undefined) {
    const tokens = isRecord(part["tokens"]) ? part["tokens"] : undefined;
    if (tokens === undefined) return undefined;
    return {
      type: "tokens",
      input: numberField(tokens, "input") ?? 0,
      output: numberField(tokens, "output") ?? 0,
    };
  }

  if (type === "error") {
    return { type: "error", message: describeHarnessError(event) };
  }

  const sessionId = stringField(event, "sessionID");
  if (sessionId !== undefined && type === "step_start") {
    return { type: "session", sessionId };
  }

  return undefined;
}

/**
 * The sentence behind a harness error event.
 *
 * opencode nests the sentence: the event carries an `error` with a `name` and a
 * `data` holding the `message` and a support `ref`. Reading only `message` off
 * the outer object — which is what this did — found nothing, so the fleet fell
 * back to the exit code and every failure on screen read `exited with code 1`,
 * which says nothing about the provider refusal, the missing credential, or the
 * unreachable endpoint that actually happened.
 */
/**
 * Read one line of `claude -p --output-format stream-json --verbose` output.
 *
 * Claude's stream-json is newline-delimited and shaped for its own SDK, not for
 * this fleet. The mapping is deliberately lossy: this fleet only needs to show
 * that the child started, what it is doing, what it said, and how many tokens
 * it used. Everything else — `thinking` blocks, per-tool results, `rate_limit`
 * events, cost in dollars, cache accounting, and message-level `usage` that
 * does not carry a complete `input_tokens`/`output_tokens` pair — is ignored.
 *
 * Mapped:
 *   - `system` `init`       -> `session` (session_id only)
 *   - `assistant` text      -> `text`
 *   - `assistant` tool_use  -> `tool` (callId, name, target from input)
 *   - `result` usage        -> `tokens`
 *   - `result` is_error     -> `error`
 *   - `stream_event` text_delta -> `text`
 *   - `stream_event` tool start -> `tool`
 *   - `error`               -> `error`
 *
 * Not mapped:
 *   - `thinking` blocks
 *   - `user` tool_result messages
 *   - `stream_event` thinking_delta / input_json_delta / message_stop
 *   - `rate_limit_event`
 *   - model, cost, and cache fields when the harness did not request a model
 */
export function parseClaudeEvent(line: string): DelegateEvent | undefined {
  const trimmed = line.trim();
  if (trimmed.length === 0 || !trimmed.startsWith("{")) return undefined;

  let event: Record<string, unknown>;
  try {
    event = JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    return undefined;
  }

  const type = stringField(event, "type");

  if (type === "system" && stringField(event, "subtype") === "init") {
    const sessionId = stringField(event, "session_id");
    if (sessionId !== undefined) return { type: "session", sessionId };
    return undefined;
  }

  if (type === "assistant") {
    const message = isRecord(event["message"]) ? event["message"] : undefined;
    const content = Array.isArray(message?.["content"]) ? (message["content"] as unknown[]) : undefined;
    if (content !== undefined && content.length > 0) {
      const first = isRecord(content[0]) ? content[0] : {};
      const blockType = stringField(first, "type");
      if (blockType === "text") {
        const value = stringField(first, "text");
        if (value !== undefined) return { type: "text", value };
      }
      if (blockType === "tool_use") {
        const callId = stringField(first, "id") ?? "tool";
        const name = stringField(first, "name") ?? "tool";
        const input = isRecord(first["input"]) ? first["input"] : {};
        return { type: "tool", callId, name, target: claudeTarget(input) };
      }
    }

    const usage = isRecord(message?.["usage"]) ? message["usage"] : undefined;
    if (usage !== undefined) {
      const input = numberField(usage, "input_tokens");
      const output = numberField(usage, "output_tokens");
      if (input !== undefined && output !== undefined) return { type: "tokens", input, output };
    }

    return undefined;
  }

  if (type === "result") {
    if (event["is_error"] === true) {
      const message =
        stringField(event, "error") ??
        stringField(event, "result") ??
        "the child agent reported an error";
      return { type: "error", message };
    }

    const usage = isRecord(event["usage"]) ? event["usage"] : undefined;
    if (usage !== undefined) {
      const input = numberField(usage, "input_tokens");
      const output = numberField(usage, "output_tokens");
      if (input !== undefined && output !== undefined) return { type: "tokens", input, output };
    }

    const resultText = stringField(event, "result");
    if (resultText !== undefined && resultText.length > 0) return { type: "text", value: resultText };

    return undefined;
  }

  if (type === "stream_event") {
    const inner = isRecord(event["event"]) ? event["event"] : {};
    const eventType = stringField(inner, "type");

    if (eventType === "content_block_delta") {
      const delta = isRecord(inner["delta"]) ? inner["delta"] : {};
      const deltaType = stringField(delta, "type");
      if (deltaType === "text_delta") {
        const value = stringField(delta, "text");
        if (value !== undefined && value.length > 0) return { type: "text", value };
      }
    }

    if (eventType === "content_block_start") {
      const contentBlock = isRecord(inner["content_block"]) ? inner["content_block"] : {};
      const blockType = stringField(contentBlock, "type");
      if (blockType === "tool_use") {
        const callId = stringField(contentBlock, "id") ?? "tool";
        const name = stringField(contentBlock, "name") ?? "tool";
        const input = isRecord(contentBlock["input"]) ? contentBlock["input"] : {};
        return { type: "tool", callId, name, target: claudeTarget(input) };
      }
      if (blockType === "text") {
        const value = stringField(contentBlock, "text");
        if (value !== undefined && value.length > 0) return { type: "text", value };
      }
    }

    if (eventType === "message_delta") {
      const usage = isRecord(inner["usage"]) ? inner["usage"] : undefined;
      if (usage !== undefined) {
        const input = numberField(usage, "input_tokens");
        const output = numberField(usage, "output_tokens");
        if (input !== undefined && output !== undefined) return { type: "tokens", input, output };
      }
    }

    return undefined;
  }

  if (type === "error") {
    const message =
      stringField(event, "message") ??
      stringField(event, "error") ??
      "the child agent reported an error";
    return { type: "error", message };
  }

  return undefined;
}

/**
 * Read one line of `codex exec --json` output.
 *
 * Codex's exec mode prints a JSONL event stream. The mapping is deliberately
 * lossy: this fleet only needs to show that the child started, what it is
 * doing, what it said, and how many tokens it used. We trust the `type`
 * discriminator and fall back to common field names, so the parser survives
 * minor shape changes. Anything that does not map cleanly is ignored.
 *
 * Mapped:
 *   - `thread.started` (thread_id / id / session_id) -> `session`
 *   - `turn.completed` (usage)                      -> `tokens`
 *   - `item.started` / `item.completed` (text)      -> `text`
 *   - `item.started` / `item.completed` (tool)      -> `tool`
 *   - `error`                                       -> `error`
 *
 * Lossy:
 *   - Item details and output are reduced to a target phrase or a text value.
 *     Rich output, tool result objects, file content, and completion details
 *     are dropped after the target or text is extracted.
 *   - No separate `tool.completed` event exists in `DelegateEvent`, so tool
 *     completion details are ignored.
 *   - Unknown item types and turn lifecycle events are dropped.
 */
export function parseCodexEvent(line: string): DelegateEvent | undefined {
  const trimmed = line.trim();
  if (trimmed.length === 0 || !trimmed.startsWith("{")) return undefined;

  let event: Record<string, unknown>;
  try {
    event = JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    return undefined;
  }

  const type = stringField(event, "type");

  if (type === "thread.started") {
    const sessionId =
      stringField(event, "thread_id") ??
      stringField(event, "id") ??
      stringField(event, "session_id");
    if (sessionId !== undefined) return { type: "session", sessionId };
    return undefined;
  }

  if (
    type === "turn.completed" ||
    type === "thread.completed" ||
    type === "turn.finished" ||
    type === "usage"
  ) {
    const usage = isRecord(event["usage"]) ? event["usage"] : undefined;
    if (usage !== undefined) {
      const input =
        numberField(usage, "input_tokens") ??
        numberField(usage, "input") ??
        numberField(usage, "prompt_tokens");
      const output =
        numberField(usage, "output_tokens") ??
        numberField(usage, "output") ??
        numberField(usage, "completion_tokens");
      if (input !== undefined && output !== undefined) {
        return { type: "tokens", input, output };
      }
    }
    return undefined;
  }

  if (type === "item.started" || type === "item.completed") {
    const item = isRecord(event["item"]) ? event["item"] : event;
    if (isRecord(item)) {
      return parseCodexItem(item);
    }
    return undefined;
  }

  if (type === "error") {
    return { type: "error", message: describeCodexError(event) };
  }

  return undefined;
}

/** The phrase a Codex item is working on, whether it is a tool or a file. */
function codexToolTarget(input: Record<string, unknown>, item: Record<string, unknown>): string | undefined {
  for (const key of [
    "command",
    "file_path",
    "path",
    "file",
    "url",
    "pattern",
    "query",
    "description",
    "content",
    "text",
    "name",
  ]) {
    const value = stringField(input, key);
    if (value !== undefined && value.length > 0) return value;
  }
  const itemFile = stringField(item, "file");
  if (itemFile !== undefined && itemFile.length > 0) return itemFile;
  return undefined;
}

/** The input object for a Codex tool item, wherever the arguments live. */
function codexItemInput(item: Record<string, unknown>): Record<string, unknown> {
  if (isRecord(item["arguments"])) return item["arguments"];
  if (isRecord(item["input"])) return item["input"];
  const fn = item["function"];
  if (isRecord(fn) && isRecord(fn["arguments"])) return fn["arguments"];
  const tool = item["tool"];
  if (isRecord(tool) && isRecord(tool["input"])) return tool["input"];
  const content = item["content"];
  if (isRecord(content)) return content;
  return {};
}

/** Read one Codex item (a tool, a message, or a file) into a fleet event. */
function parseCodexItem(item: Record<string, unknown>): DelegateEvent | undefined {
  const role = stringField(item, "role");
  const itemType = stringField(item, "type") ?? "";
  const isAssistant =
    role === "assistant" || itemType === "assistant" || itemType === "message";

  if (isAssistant) {
    const content = item["content"];
    if (typeof content === "string") return { type: "text", value: content };
    if (isRecord(content)) {
      const text = stringField(content, "text") ?? stringField(content, "content");
      if (text !== undefined) return { type: "text", value: text };
    }
    const output = item["output"];
    if (typeof output === "string") return { type: "text", value: output };
    if (isRecord(output)) {
      const text = stringField(output, "text") ?? stringField(output, "content");
      if (text !== undefined) return { type: "text", value: text };
    }
  }

  const isTool =
    itemType === "function" ||
    itemType === "tool" ||
    itemType === "command" ||
    itemType === "file" ||
    item["function"] !== undefined ||
    item["tool"] !== undefined;
  if (isTool) {
    const callId =
      stringField(item, "id") ??
      stringField(item, "item_id") ??
      stringField(item, "call_id") ??
      "codex_tool";
    const name =
      stringField(item, "name") ??
      (isRecord(item["function"]) ? stringField(item["function"], "name") : undefined) ??
      (isRecord(item["tool"]) ? stringField(item["tool"], "name") : undefined) ??
      itemType;
    const input = codexItemInput(item);
    const target = codexToolTarget(input, item);
    return { type: "tool", callId, name, target };
  }

  return undefined;
}

/** The sentence behind a Codex error event. */
function describeCodexError(event: Record<string, unknown>): string {
  const error = isRecord(event["error"]) ? event["error"] : {};
  return (
    stringField(event, "message") ??
    stringField(error, "message") ??
    stringField(event, "error") ??
    "the child agent reported an error"
  );
}

/** The phrase a Claude tool_use block is working on. */
function claudeTarget(input: Record<string, unknown>): string | undefined {
  for (const key of [
    "command",
    "file_path",
    "path",
    "file",
    "url",
    "pattern",
    "query",
    "description",
    "tool_use_id",
    "content",
    "text",
  ]) {
    const value = stringField(input, key);
    if (value !== undefined && value.length > 0) return value;
  }
  return undefined;
}

function describeHarnessError(event: Record<string, unknown>): string {
  const error = isRecord(event["error"]) ? event["error"] : {};
  const data = isRecord(error["data"]) ? error["data"] : {};
  const sentence =
    stringField(event, "message") ??
    stringField(error, "message") ??
    stringField(data, "message") ??
    stringField(data, "error");
  const name = stringField(error, "name");
  const ref = stringField(data, "ref");

  const parts: string[] = [];
  if (name !== undefined && name !== "Error") parts.push(name);
  parts.push(sentence ?? "the child agent reported an error");
  const text = parts.join(": ");
  return ref === undefined ? text : `${text} (${ref})`;
}

/**
 * What the child was working on, plus any display metadata the harness gave.
 *
 * The harness's own title is preferred because it is what the harness chose to
 * show; the input fields are a fallback for tools that set no title. File
 * ranges come from `offset`/`limit` or `start`/`end`, and search hit counts come
 * from the output fields a real opencode harness uses.
 */
type MutableToolMeta = { -readonly [K in keyof CoderToolActivityMeta]: CoderToolActivityMeta[K] };

function toolTarget(
  state: Record<string, unknown> | undefined,
): { readonly target: string | undefined; readonly meta?: CoderToolActivityMeta } {
  if (state === undefined) return { target: undefined };

  const title = stringField(state, "title");
  const input = isRecord(state["input"]) ? state["input"] : undefined;
  const output = isRecord(state["output"]) ? state["output"] : undefined;

  let target: string | undefined;
  if (title !== undefined && title.length > 0) {
    target = title;
  } else if (input !== undefined) {
    for (const key of ["filePath", "path", "file_path", "file", "command", "pattern", "query", "description", "url"]) {
      const value = stringField(input, key);
      if (value !== undefined && value.length > 0) {
        target = value;
        break;
      }
    }
  }

  if (target === undefined) return { target: undefined };

  const meta: MutableToolMeta = {};

  if (input !== undefined) {
    const offset = numberField(input, "offset");
    const limit = numberField(input, "limit");
    const start = numberField(input, "start");
    const end = numberField(input, "end");
    if (offset !== undefined && limit !== undefined) {
      meta.range = { start: offset, end: offset + limit };
    } else if (start !== undefined && end !== undefined) {
      meta.range = { start, end };
    }
  }

  if (output !== undefined) {
    const size = numberField(output, "size") ?? numberField(output, "length");
    if (size !== undefined) meta.size = size;

    const hits =
      numberField(output, "hits") ??
      numberField(output, "hitCount") ??
      numberField(output, "total") ??
      numberField(output, "totalHits");
    if (hits !== undefined) {
      meta.hitCount = hits;
    } else {
      const matches = Array.isArray(output["matches"]) ? (output["matches"] as ReadonlyArray<unknown>).length : undefined;
      const results = Array.isArray(output["results"]) ? (output["results"] as ReadonlyArray<unknown>).length : undefined;
      if (matches !== undefined) meta.hitCount = matches;
      else if (results !== undefined) meta.hitCount = results;
    }
  }

  return Object.keys(meta).length === 0 ? { target } : { target, meta };
}

export interface OpencodeHarnessOptions {
  /** `provider/model`, for example `vertex-express/gemini-3.7-flash`. */
  readonly model: string;
  /** Defaults to `opencode` on the path. */
  readonly command?: string | undefined;
  /**
   * A config file for the child, passed as `OPENCODE_CONFIG`.
   *
   * This is how a provider the harness is not configured for is supplied
   * without writing a key into the repository: the caller writes a config to a
   * private path and names it here.
   */
  readonly configPath?: string | undefined;
  /**
   * Approve the child's tool use without asking.
   *
   * A delegated child has nobody to ask, so a coding task that has to edit a
   * file or run a command needs this. It is off by default because the
   * decision belongs to whoever launches the fleet, and it should be paired
   * with worktree or sandbox isolation.
   */
  readonly autoApprove?: boolean | undefined;
  readonly env?: Readonly<Record<string, string | undefined>> | undefined;
}

/** How long a stopped child has to leave on its own before it is killed. */
const KILL_GRACE_MS = 3_000;

/**
 * Signal a child and everything it started.
 *
 * The negative pid is the process group, which is why the child is spawned
 * detached. It falls back to the child alone when the group is already gone,
 * because a group whose last member exited between the two calls raises rather
 * than reporting nothing to do.
 */
function killGroup(child: ChildProcess, signal: "SIGTERM" | "SIGKILL"): void {
  const pid = child.pid;
  if (pid === undefined) return;
  try {
    process.kill(-pid, signal);
  } catch {
    try {
      child.kill(signal);
    } catch {
      // Already gone, which is the outcome that was wanted.
    }
  }
}

/**
 * Every process descended from `root`, deepest last.
 *
 * A harness is free to put its own tool processes in groups of their own, and
 * opencode does: signalling the group takes out opencode and leaves whatever a
 * bash tool started running under pid 1. The tree has to be read while the
 * parent is still alive, because reparenting erases the link.
 */
function descendants(root: number): ReadonlyArray<number> {
  let listing: string;
  try {
    listing = execFileSync("ps", ["-A", "-o", "pid=,ppid="], { encoding: "utf8" });
  } catch {
    return [];
  }

  const byParent = new Map<number, Array<number>>();
  for (const line of listing.split("\n")) {
    const [pid, parent] = line.trim().split(/\s+/).map(Number);
    if (pid === undefined || parent === undefined) continue;
    if (!Number.isInteger(pid) || !Number.isInteger(parent)) continue;
    const siblings = byParent.get(parent);
    if (siblings === undefined) byParent.set(parent, [pid]);
    else siblings.push(pid);
  }

  const found: Array<number> = [];
  const walk = (pid: number): void => {
    for (const child of byParent.get(pid) ?? []) {
      if (found.includes(child)) continue;
      found.push(child);
      walk(child);
    }
  };
  walk(root);
  return found;
}

/** Signal a child, its group, and every process either of them started. */
function killTree(child: ChildProcess, signal: "SIGTERM" | "SIGKILL"): void {
  const pid = child.pid;
  const tree = pid === undefined ? [] : descendants(pid);
  killGroup(child, signal);
  for (const descendant of tree) {
    try {
      process.kill(descendant, signal);
    } catch {
      // Already gone, which is the outcome that was wanted.
    }
  }
}

/** Runs children as `opencode run --format json` subprocesses. */
/** How a Devin child is run. */
export interface DevinHarnessOptions {
  /** The binary, so a test can point at a stand-in. Defaults to `devin`. */
  readonly command?: string | undefined;
  /**
   * The permission mode passed through.
   *
   * `dangerous` is this build's name for the unattended mode -- the published
   * documentation calls it "bypass", and passing that is accepted and ignored,
   * so a child would silently fall back to prompting where nobody can answer.
   */
  readonly permissionMode?: string | undefined;
  /** Extra environment for the child. */
  readonly env?: Record<string, string> | undefined;
}

/**
 * Children run by the Devin CLI.
 *
 * A second harness rather than a shell command, so a Devin fan-out is a fleet
 * like any other: it reports through the registry the renderer reads, it can be
 * stopped with the rest, and it does not block the turn that started it. Run
 * through `shell` instead, the same work is one opaque call that freezes the
 * session and shows nothing while it runs.
 *
 * Devin's print mode has no structured output, so a child reports its answer
 * once at the end rather than streaming tool calls the way `opencode --format
 * json` does. The fleet still shows it start, run, and finish, which is the
 * part the reader is waiting on.
 *
 * Its own credentials are used, not this session's grant. That is a different
 * trust and billing boundary from an `opencode` child, and it is the reason the
 * agent is named in the fleet rather than left implicit.
 */
export class DevinHarness implements DelegateHarness {
  readonly agent = "devin";
  readonly model: string;

  constructor(private readonly options: DevinHarnessOptions = {}) {
    // Devin picks its own model from its own configuration, and neither print
    // mode nor ACP reports which. Naming one here would be inventing it.
    this.model = options.permissionMode ?? "dangerous";
  }

  /**
   * Run the child over ACP rather than print mode.
   *
   * `devin -p` writes nothing until it finishes — measured: fourteen seconds of
   * silence for a one-word answer, and `--export` is written at close too. A
   * child doing four minutes of real work reported no tool calls, no tokens,
   * and no transcript, so the fleet showed `Initializing…` for the whole run
   * and a reader could not tell it from a hang. `devin acp` is the same agent
   * streaming the events the fleet already draws.
   */
  async *run(
    input: {
      readonly prompt: string;
      readonly cwd: string;
      readonly transcriptPath: string;
      readonly resumeSessionId?: string | undefined;
    },
    signal: AbortSignal,
  ): AsyncIterable<DelegateEvent> {
    try {
      yield* runDevinAcp(
        { prompt: input.prompt, cwd: input.cwd },
        {
          ...(this.options.command === undefined ? {} : { command: this.options.command }),
          ...(this.options.env === undefined ? {} : { env: this.options.env }),
          // Devin's own word for what this harness has always called a
          // permission mode. `bypass` is its `dangerous`.
          mode: DEVIN_MODES[this.options.permissionMode ?? "dangerous"] ?? "bypass",
          // Appended synchronously as each message arrives, not through a
          // stream: a stream's `end()` does not flush before the next line of
          // this process runs, so a child read the instant it finished showed
          // a transcript missing everything it had just done.
          record: (entry) => {
            try {
              appendFileSync(input.transcriptPath, `${JSON.stringify(entry)}\n`);
            } catch {
              // A transcript that cannot be written must not end the child.
            }
          },
        },
        signal,
      );
    } finally {
      // Nothing to close: every line was already on disk.
    }
  }
}

/**
 * The old permission-mode names, mapped to the session modes ACP offers.
 *
 * `devin -p --permission-mode dangerous` and a `bypass` ACP session are the
 * same posture, and a caller that wrote the old name should not have to learn
 * the new one.
 */
const DEVIN_MODES: Readonly<Record<string, string>> = {
  dangerous: "bypass",
  bypass: "bypass",
  auto: "accept-edits",
  "accept-edits": "accept-edits",
  ask: "ask",
  plan: "plan",
  smart: "smart",
};

/**
 * The models a child is given, in the order they are preferred.
 *
 * Free and grant-free, both on purpose. A thread grant lives an hour, has to be
 * minted, and expires under a console that outlives it — four children once
 * came back `grant_expired` together — while the harness's own catalog costs
 * nothing and needs no credential from us at all.
 *
 * Resolved against what the harness actually lists, so a name that goes away
 * falls through to the next rather than failing a fan-out.
 */
export interface ClaudeCodeHarnessOptions {
  /** The binary. Defaults to `claude`. */
  readonly command?: string | undefined;
  /** The model name passed to `--model`. If unset, the harness reports `not reported`. */
  readonly model?: string | undefined;
  /** The permission mode. Defaults to `auto` for unattended runs. */
  readonly permissionMode?: string | undefined;
  /** A hard cap on turns. Defaults to 50. */
  readonly maxTurns?: number | undefined;
  /** Extra environment for the child. */
  readonly env?: Readonly<Record<string, string | undefined>> | undefined;
}

/**
 * Children run by the Claude Code CLI.
 *
 * Claude Code's `claude -p --output-format stream-json --verbose` is a one-shot
 * headless mode that emits newline-delimited JSON events. This harness drives it
 * the same way the fleet drives opencode and Devin: it streams events, writes
 * the raw transcript, and can be stopped cleanly.
 *
 * It inherits the user's `claude` environment and credentials. It does not read,
 * copy, or forward any Claude credentials from this process.
 */
export class ClaudeCodeHarness implements DelegateHarness {
  readonly agent = "claude";
  readonly model: string;

  constructor(private readonly options: ClaudeCodeHarnessOptions = {}) {
    // Claude's own configuration or the init event may choose the model. Only
    // report one the caller explicitly passed.
    this.model = options.model ?? "not reported";
  }

  async *run(
    input: {
      readonly prompt: string;
      readonly cwd: string;
      readonly transcriptPath: string;
      readonly resumeSessionId?: string | undefined;
    },
    signal: AbortSignal,
  ): AsyncIterable<DelegateEvent> {
    const command = this.options.command ?? "claude";

    // `--print` runs the prompt and exits. `--output-format stream-json`
    // requires `--verbose`. `--permission-mode auto` keeps a delegated child
    // from stopping to ask; `--max-turns` is a guard on runaway cost.
    const args = [
      "-p",
      input.prompt,
      "--output-format",
      "stream-json",
      "--verbose",
      "--permission-mode",
      this.options.permissionMode ?? "auto",
    ];
    if (this.options.model !== undefined) {
      args.push("--model", this.options.model);
    }

    // Claude Code print mode cannot resume an existing session by id, so a
    // resumeSessionId is ignored rather than re-running the prompt on a fresh
    // session and duplicating any work already done.
    void input.resumeSessionId;

    mkdirSync(dirname(input.transcriptPath), { recursive: true });
    const transcript = createWriteStream(input.transcriptPath, { flags: "a" });

    const child = spawn(command, args, {
      cwd: input.cwd,
      env: { ...process.env, ...this.options.env },
      stdio: ["ignore", "pipe", "pipe"],
      detached: true,
    });

    const queue: DelegateEvent[] = [];
    let notify: (() => void) | undefined;
    const wake = () => {
      notify?.();
      notify = undefined;
    };

    let stderr = "";
    let stdout = "";
    let pending = "";
    let reported: string | undefined;
    let exited = false;
    let failure: string | undefined;

    const onAbort = () => {
      killTree(child, "SIGTERM");
      const grace = setTimeout(() => killTree(child, "SIGKILL"), KILL_GRACE_MS);
      grace.unref();
      child.once("close", () => clearTimeout(grace));
    };
    signal.addEventListener("abort", onAbort, { once: true });

    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      transcript.write(chunk);
      stdout = `${stdout}${chunk}`.slice(-4000);
      pending += chunk;
      let newline = pending.indexOf("\n");
      while (newline >= 0) {
        const line = pending.slice(0, newline);
        pending = pending.slice(newline + 1);
        const event = parseClaudeEvent(line);
        if (event !== undefined) {
          if (event.type === "error") reported = event.message;
          queue.push(event);
        }
        newline = pending.indexOf("\n");
      }
      wake();
    });

    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk: string) => {
      stderr = `${stderr}${chunk}`.slice(-4000);
    });

    child.on("error", (cause: Error) => {
      failure =
        (cause as NodeJS.ErrnoException).code === "ENOENT"
          ? `The \`${command}\` harness is not on the path.`
          : cause.message;
      exited = true;
      wake();
    });

    child.on("close", (code) => {
      const trailing = parseClaudeEvent(pending);
      if (trailing !== undefined) {
        if (trailing.type === "error") reported = trailing.message;
        queue.push(trailing);
      }
      if (failure === undefined && code !== 0 && !signal.aborted) {
        failure = reported ?? describeExit(code, stderr, stdout);
      }
      exited = true;
      wake();
    });

    try {
      while (true) {
        while (queue.length > 0) {
          const event = queue.shift();
          if (event !== undefined) yield event;
        }
        if (exited) break;
        await new Promise<void>((resolve) => {
          notify = resolve;
        });
      }
    } finally {
      signal.removeEventListener("abort", onAbort);
      transcript.end();
    }

    if (failure !== undefined) throw new Error(failure);
  }
}

/**
 * How a Codex child is run.
 */
export interface CodexHarnessOptions {
  /**
   * The `codex exec --sandbox` policy for the child. Defaults to
   * `workspace-write`: the checkout it was pointed at, and nothing outside it.
   */
  readonly sandbox?: string | undefined;
  /** The binary. Defaults to `codex`. */
  readonly command?: string | undefined;
  /** The model name passed to `-m, --model`. If unset, the harness reports `not reported`. */
  readonly model?: string | undefined;
  /**
   * The approval policy for unattended runs.
   *
   * Codex's exec mode can ask for approval before running commands. A
   * delegated child has nobody to ask, so the default is `never`. Pass another
   * value only when the caller's own policy requires it.
   */
  readonly permissionMode?: string | undefined;
  /** Extra environment for the child. */
  readonly env?: Readonly<Record<string, string | undefined>> | undefined;
}

/**
 * Children run by the OpenAI `codex` CLI in `exec --json` mode.
 *
 * `codex exec --json` emits newline-delimited JSON events. This harness drives
 * it like the others: it streams events, writes the raw transcript, supports
 * resuming with `exec resume`, and can be stopped cleanly.
 *
 * It inherits the user's `codex` environment and credentials. It does not read,
 * copy, or forward any Codex credentials from this process.
 */
export class CodexHarness implements DelegateHarness {
  readonly agent = "codex";
  private readonly options: CodexHarnessOptions;
  private reportedModel: string | undefined;

  constructor(options: CodexHarnessOptions = {}) {
    this.options = options;
  }

  get model(): string {
    return this.options.model ?? this.reportedModel ?? "not reported";
  }

  async *run(
    input: {
      readonly prompt: string;
      readonly cwd: string;
      readonly transcriptPath: string;
      readonly resumeSessionId?: string | undefined;
    },
    signal: AbortSignal,
  ): AsyncIterable<DelegateEvent> {
    const command = this.options.command ?? "codex";

    // `--json` puts Codex in newline-delimited event mode.
    //
    // Sandbox rather than approval mode: `codex exec` has no
    // `--ask-for-approval`, and a delegated child has nobody to ask anyway, so
    // what it needs is a stated boundary rather than a prompt. The default is
    // `workspace-write` — the child may edit the checkout it was pointed at
    // and nothing outside it — and a caller who wants a narrower or wider one
    // passes it. Checked against `codex exec --help` rather than assumed.
    const sandbox = this.options.sandbox ?? "workspace-write";
    const args =
      input.resumeSessionId === undefined
        ? ["exec", "--json", "--sandbox", sandbox]
        : ["exec", "resume", "--json", "--sandbox", sandbox];
    if (this.options.model !== undefined) {
      args.push("-m", this.options.model);
    }
    if (input.resumeSessionId === undefined) {
      args.push(input.prompt);
    } else {
      args.push(input.resumeSessionId, input.prompt);
    }

    mkdirSync(dirname(input.transcriptPath), { recursive: true });
    const transcript = createWriteStream(input.transcriptPath, { flags: "a" });

    const child = spawn(command, args, {
      cwd: input.cwd,
      env: { ...process.env, ...this.options.env },
      stdio: ["ignore", "pipe", "pipe"],
      detached: true,
    });

    const queue: DelegateEvent[] = [];
    let notify: (() => void) | undefined;
    const wake = () => {
      notify?.();
      notify = undefined;
    };

    let stderr = "";
    let stdout = "";
    let pending = "";
    let reported: string | undefined;
    let exited = false;
    let failure: string | undefined;

    const onAbort = () => {
      killTree(child, "SIGTERM");
      const grace = setTimeout(() => killTree(child, "SIGKILL"), KILL_GRACE_MS);
      grace.unref();
      child.once("close", () => clearTimeout(grace));
    };
    signal.addEventListener("abort", onAbort, { once: true });

    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      transcript.write(chunk);
      stdout = `${stdout}${chunk}`.slice(-4000);
      pending += chunk;
      let newline = pending.indexOf("\n");
      while (newline >= 0) {
        const line = pending.slice(0, newline);
        pending = pending.slice(newline + 1);
        // If Codex reports which model is answering, use it for the lane label
        // when the caller did not name one explicitly.
        try {
          const raw = JSON.parse(line) as Record<string, unknown>;
          if (raw["type"] === "thread.started" && this.options.model === undefined) {
            const maybe = stringField(raw, "model");
            if (maybe !== undefined) this.reportedModel = maybe;
          }
        } catch {
          // Not JSON; the line will be handled by parseCodexEvent below.
        }
        const event = parseCodexEvent(line);
        if (event !== undefined) {
          if (event.type === "error") reported = event.message;
          queue.push(event);
        }
        newline = pending.indexOf("\n");
      }
      wake();
    });

    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk: string) => {
      transcript.write(chunk);
      stderr = `${stderr}${chunk}`.slice(-4000);
    });

    child.on("error", (cause: Error) => {
      failure =
        (cause as NodeJS.ErrnoException).code === "ENOENT"
          ? `The \`${command}\` harness is not on the path.`
          : cause.message;
      exited = true;
      wake();
    });

    child.on("close", (code) => {
      const trailing = parseCodexEvent(pending);
      if (trailing !== undefined) {
        if (trailing.type === "error") reported = trailing.message;
        queue.push(trailing);
      }
      if (failure === undefined && code !== 0 && !signal.aborted) {
        failure = reported ?? describeExit(code, stderr, stdout);
      }
      exited = true;
      wake();
    });

    try {
      while (true) {
        while (queue.length > 0) {
          const event = queue.shift();
          if (event !== undefined) yield event;
        }
        if (exited) break;
        await new Promise<void>((resolve) => {
          notify = resolve;
        });
      }
    } finally {
      signal.removeEventListener("abort", onAbort);
      transcript.end();
    }

    if (failure !== undefined) throw new Error(failure);
  }
}

export const FREE_CHILD_MODELS: ReadonlyArray<string> = [
  // Ox Alpha, free and unlimited while it lasts. The slug says neither `ox`
  // nor `alpha`: opencode's own normalization maps `x-preview-f` to `ox-alpha`
  // (`packages/stats/core/src/domain/model-normalization.ts`), and the `-free`
  // entry is the unlimited tier the picker calls "Ox Alpha Free (Unlimited)".
  // Searching the model list for its name finds nothing, which is how a first
  // pass at this list picked a different model entirely.
  "opencode/x-preview-f-free",
];

/**
 * What a lane is called, and what it resolves to.
 *
 * The names are the ones people use. Ox Alpha's slug is
 * `opencode/x-preview-f-free` — it says neither `ox` nor `alpha`, because
 * opencode's normalization maps `x-preview-f` to `ox-alpha` elsewhere — so a
 * session offered only the slug is a session nobody can ask for Ox Alpha by
 * name. Asked "can you delegate to ox alpha", one answered no while holding
 * exactly that lane under a name it could not connect to the question.
 *
 * A slug still resolves to itself, so nothing that already worked stops.
 */
/**
 * The lane run by this process, on the account's own thread grant.
 *
 * Named rather than aliased to a harness slug, because it is not a model this
 * or any harness offers — it is the parent's own loop, smaller, on the grant
 * the server minted for children.
 */
export const SELF_CHILD_LANE = "openagents";

export const CHILD_LANE_ALIASES: Readonly<Record<string, string>> = {
  // `ox-alpha` means the self-hosted lane now. It is the same model either
  // way — the server routes the child's grant to OpenRouter's
  // `stealth/ox-alpha` — and running it here costs no second agent, no second
  // credential, and no second idea of what a coding agent is. The opencode
  // route to the same model is still reachable, by its own slug.
  "ox-alpha": SELF_CHILD_LANE,
  [SELF_CHILD_LANE]: SELF_CHILD_LANE,
  gemini: SELF_CHILD_LANE,
  claude: "claude",
  codex: "codex",
};

/**
 * Every lane a `delegate` call may name, by the name a caller would use.
 *
 * The enum is the names; `CHILD_LANES` is what each one is. Offered as an enum
 * so a call chooses from what exists rather than from what it remembers.
 */
/**
 * What each lane actually is: who runs the child, and what answers it.
 *
 * The two are independent and the enum reads as one list, which is how a
 * session came to describe `opencode/x-preview-f-free` as a "fast preview /
 * experimental" model. It is not. It is **the same model as `ox-alpha`** —
 * opencode's own normalization maps `x-preview-f` to `ox-alpha` — reached
 * through a different harness on a different credential. A caller choosing
 * between them is choosing who runs the child, not which model thinks.
 *
 * So each lane says both, and says who pays.
 */
export interface ChildLane {
  /** The name a `delegate` call passes as `model`. */
  readonly name: string;
  /** What runs the child and gives it its tools. */
  readonly harness: string;
  /** What answers. */
  readonly model: string;
  /** Where that model is served from, and whose credential pays for it. */
  readonly served: string;
  readonly bestFor: string;
}

export const CHILD_LANES: ReadonlyArray<ChildLane> = [
  {
    name: "ox-alpha",
    harness: "openagents (this process, one `shell` tool)",
    model: "Ox Alpha",
    served:
      "the OpenAgents inference proxy, routed to OpenRouter `stealth/ox-alpha`, on this session's thread grant",
    bestFor: "work whose shape is the question: design, architecture, an open-ended refactor",
  },
  {
    name: "opencode/x-preview-f-free",
    harness: "opencode (a separate CLI on this machine, its own tools)",
    // Named as the same model on purpose. The difference between this lane and
    // the one above is the harness and the credential, and a description that
    // implies two models sends a caller here for the wrong reason.
    model: "Ox Alpha — the same model as `ox-alpha`, under opencode's name for it",
    served: "OpenCode Zen, on this machine's opencode credential",
    bestFor:
      "the same work as `ox-alpha`, when you want opencode's harness and tools instead of ours",
  },
  {
    name: "gemini",
    harness: "opencode (a separate CLI on this machine, its own tools)",
    model: "Gemini 3.7 Flash",
    served: "OpenCode Zen, on this machine's opencode credential",
    bestFor: "fast, straightforward coding and analysis",
  },
  {
    name: "devin",
    harness: "devin (the Devin CLI, its own tools)",
    // Devin picks its own model from its own configuration and print mode does
    // not report which, so naming one here would be inventing it.
    model: "Devin's own, not reported",
    served: "Devin, on its own credentials — it spends nothing of this account's",
    bestFor: "straightforward engineering with a clear shape: a named fix, a test, a migration",
  },
  {
    name: "claude",
    harness: "claude (the Claude Code CLI, its own tools)",
    // Claude Code picks its own model when none is passed, and this lane does
    // not require one. The init event may report a model, but the harness only
    // advertises one the caller asked for.
    model: "not reported",
    served: "Anthropic via Claude Code, on this machine's Claude credentials",
    bestFor: "work that needs Claude's toolset and tolerates its own cost boundary",
  },
  {
    name: "codex",
    harness: "codex (the OpenAI Codex CLI, its own tools)",
    // Codex exec may report a model in `thread.started`; the harness only
    // advertises one the caller asked for.
    model: "not reported",
    served: "OpenAI, on this machine's Codex credentials",
    bestFor: "work that needs the Codex agent and tolerates its own cost boundary",
  },
];

/** One lane as a line a model can read. */
export const describeChildLane = (lane: ChildLane): string =>
  `\`${lane.name}\` — harness: ${lane.harness}; model: ${lane.model}; served by ${lane.served}. ` +
  `Best for ${lane.bestFor}.`;

export const CHILD_MODELS: ReadonlyArray<string> = [
  // Deduplicated: `ox-alpha` and `openagents` are two names for one lane, and
  // offering both in the enum would read as two choices.
  ...new Set(Object.keys(CHILD_LANE_ALIASES)),
  ...FREE_CHILD_MODELS,
  "devin",
  "claude",
];

/**
 * The name a lane is known by, given its slug.
 *
 * So a lane reached by its slug still reports the name a reader would recognise,
 * whichever way it was reached.
 */
export const childLaneName = (lane: string): string =>
  Object.entries(CHILD_LANE_ALIASES).find(([, slug]) => slug === lane)?.[0] ?? lane;

/** The lane a name means, whether it is an alias, a slug, or Devin. */
export const resolveChildLane = (name: string): string | undefined => {
  const asked = name.trim();
  if (asked.length === 0) return undefined;
  if (/^claude(:.+)?$/.test(asked)) return asked;
  if (/^devin(:.+)?$/.test(asked)) return asked;
  if (/^codex(:.+)?$/.test(asked)) return asked;
  const aliased = CHILD_LANE_ALIASES[asked];
  if (aliased !== undefined) return aliased;
  return FREE_CHILD_MODELS.includes(asked) ? asked : undefined;
};

/** Whether this resolved lane is the one this process runs itself. */
export const selfChildLane = (lane: string): boolean => lane === SELF_CHILD_LANE;

/**
 * The first preferred model the harness offers, or undefined when it lists none.
 *
 * A listing that cannot be read says nothing rather than guessing, for the same
 * reason the preflight does: a harness whose subcommand differs must not be
 * able to block a fan-out that would have worked.
 */
export async function firstAvailableChildModel(
  command = "opencode",
  preferred: ReadonlyArray<string> = FREE_CHILD_MODELS,
): Promise<string | undefined> {
  const listed = await new Promise<string>((resolve) => {
    const probe = spawn(command, ["models"], { stdio: ["ignore", "pipe", "ignore"] });
    let out = "";
    probe.stdout.setEncoding("utf8");
    probe.stdout.on("data", (chunk: string) => {
      out += chunk;
    });
    probe.on("error", () => resolve(""));
    probe.on("close", () => resolve(out));
  });
  if (listed.trim().length === 0) return undefined;
  const names = new Set(listed.split("\n").map((line) => line.trim()));
  return preferred.find((candidate) => names.has(candidate));
}

export class OpencodeHarness implements DelegateHarness {
  readonly agent = "opencode";
  readonly model: string;

  constructor(private readonly options: OpencodeHarnessOptions) {
    this.model = options.model;
  }

  /** The preflight, run once and shared by every child of this fleet. */
  private preflight: Promise<string | undefined> | undefined;

  /**
   * Check that the harness exists and knows the model, once per fleet.
   *
   * Without this, a model the harness cannot resolve fails inside its provider
   * and is reported as `Unexpected server error`, once per child — fifteen
   * identical sentences naming nothing. `opencode models` costs one process at
   * the start of a fan-out and turns that into the model id and the fact that
   * it is not on the list.
   *
   * A preflight that cannot answer says nothing rather than guessing: a
   * harness that lists no models, or a build whose subcommand differs, must
   * not be able to block a fleet that would have worked.
   */
  private check(command: string): Promise<string | undefined> {
    this.preflight ??= new Promise<string | undefined>((resolve) => {
      const probe = spawn(command, ["models"], {
        env: {
          ...process.env,
          ...this.options.env,
          ...(this.options.configPath === undefined
            ? {}
            : { OPENCODE_CONFIG: this.options.configPath }),
        },
        stdio: ["ignore", "pipe", "ignore"],
      });

      let listing = "";
      probe.stdout.setEncoding("utf8");
      probe.stdout.on("data", (chunk: string) => {
        listing += chunk;
      });

      probe.on("error", (cause: Error) => {
        resolve(
          (cause as NodeJS.ErrnoException).code === "ENOENT"
            ? `The \`${command}\` harness is not installed. Install it with ` +
                "`npm i -g opencode-ai`, or name another with --child-command."
            : `The \`${command}\` harness could not be started: ${cause.message}`,
        );
      });

      probe.on("close", (code) => {
        const models = listing
          .split("\n")
          .map((line) => stripAnsi(line).trim())
          .filter((line) => line.includes("/"));
        if (code !== 0 || models.length === 0 || models.includes(this.model)) {
          resolve(undefined);
          return;
        }
        resolve(
          `The \`${command}\` harness has no model \`${this.model}\`. ` +
            `It offers ${String(models.length)}, including ${models.slice(0, 3).join(", ")}.`,
        );
      });
    });
    return this.preflight;
  }

  async *run(
    input: {
      readonly prompt: string;
      readonly cwd: string;
      readonly transcriptPath: string;
      readonly resumeSessionId?: string | undefined;
    },
    signal: AbortSignal,
  ): AsyncIterable<DelegateEvent> {
    const command = this.options.command ?? "opencode";

    const problem = await this.check(command);
    if (problem !== undefined) {
      yield { type: "error", message: problem };
      throw new Error(problem);
    }

    const args = ["run", "--format", "json", "--model", this.model, "--dir", input.cwd];
    if (this.options.autoApprove === true) args.push("--auto");

    if (input.resumeSessionId === undefined) {
      args.push(input.prompt);
    } else {
      // The work already done stays done. `--continue` picks the session back
      // up with its whole transcript, so the child carries on from where its
      // provider dropped rather than re-reading and re-editing everything.
      args.push("--session", input.resumeSessionId, "--continue");
      args.push(
        "The previous attempt stopped when the model provider became unavailable. " +
          "Continue from where you left off and finish the task.",
      );
    }

    const child = spawn(command, args, {
      cwd: input.cwd,
      env: {
        ...process.env,
        ...this.options.env,
        ...(this.options.configPath === undefined
          ? {}
          : { OPENCODE_CONFIG: this.options.configPath }),
      },
      stdio: ["ignore", "pipe", "pipe"],
      // Its own process group, so stopping a child stops what the child
      // started. A coding agent shells out, and killing only the agent leaves
      // its build or its `sleep` running with nothing left to stop it — with a
      // fan-out of fifteen, every cancelled fleet would leave a pile behind.
      detached: true,
    });

    // The transcript is written as the events arrive, not at the end, so a
    // child that is killed still leaves everything it had done behind.
    const transcript = createWriteStream(input.transcriptPath, { flags: "a" });

    const queue: DelegateEvent[] = [];
    let notify: (() => void) | undefined;
    const wake = () => {
      notify?.();
      notify = undefined;
    };

    let stderr = "";
    /** The tail of stdout, kept for a child that failed without an event. */
    let stdout = "";
    let reported: string | undefined;
    let exited = false;
    let failure: string | undefined;

    const onAbort = () => {
      killTree(child, "SIGTERM");
      // A harness that ignores the term, or a tool that will not stop, still
      // has to go: the reader asked for the child to end, not to be asked.
      const grace = setTimeout(() => killTree(child, "SIGKILL"), KILL_GRACE_MS);
      grace.unref();
      child.once("close", () => clearTimeout(grace));
    };
    signal.addEventListener("abort", onAbort, { once: true });

    let pending = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      transcript.write(chunk);
      stdout = `${stdout}${chunk}`.slice(-4000);
      pending += chunk;
      let newline = pending.indexOf("\n");
      while (newline >= 0) {
        const line = pending.slice(0, newline);
        pending = pending.slice(newline + 1);
        const event = parseOpencodeEvent(line);
        if (event !== undefined) {
          if (event.type === "error") reported = event.message;
          queue.push(event);
        }
        newline = pending.indexOf("\n");
      }
      wake();
    });

    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk: string) => {
      // Kept but not streamed: the harness prints progress noise here, and only
      // the tail matters, and only when the child failed.
      stderr = `${stderr}${chunk}`.slice(-4000);
    });

    child.on("error", (cause: Error) => {
      failure =
        (cause as NodeJS.ErrnoException).code === "ENOENT"
          ? `The \`${command}\` harness is not on the path.`
          : cause.message;
      exited = true;
      wake();
    });

    child.on("close", (code) => {
      const trailing = parseOpencodeEvent(pending);
      if (trailing !== undefined) {
        if (trailing.type === "error") reported = trailing.message;
        queue.push(trailing);
      }
      if (failure === undefined && code !== 0 && !signal.aborted) {
        // What the harness said beats what the shell said. A child that
        // reported `provider refused the key` and then exited 1 has already
        // explained itself, and replacing that with the exit code is how a
        // fleet ends up reporting three identical `code 1` lines that name
        // nothing a reader could fix.
        failure = reported ?? describeExit(code, stderr, stdout);
      }
      exited = true;
      wake();
    });

    try {
      while (true) {
        while (queue.length > 0) {
          const event = queue.shift();
          if (event !== undefined) yield event;
        }
        if (exited) break;
        await new Promise<void>((resolve) => {
          notify = resolve;
        });
      }
    } finally {
      signal.removeEventListener("abort", onAbort);
      transcript.end();
    }

    if (failure !== undefined) throw new Error(failure);
  }
}

/**
 * Why a child ended, in one sentence.
 *
 * Both streams are read, stderr first: a harness that dies before it starts
 * writes there, and one that dies mid-run may have written only structured
 * output that this side could not name. An exit code on its own is the last
 * resort, not the first answer.
 */
function describeExit(code: number | null, stderr: string, stdout = ""): string {
  const exit = code === null ? "was killed" : `exited with code ${code}`;
  const tail = lastLines(stderr) ?? lastLines(stdout);
  return tail === undefined ? `The child ${exit}.` : `The child ${exit}: ${tail}`;
}

function lastLines(text: string): string | undefined {
  const tail = text
    .split("\n")
    .map((line) => stripAnsi(line).trim())
    .filter((line) => line.length > 0)
    .slice(-2)
    .join(" ");
  return tail.length === 0 ? undefined : tail.slice(-400);
}

function stripAnsi(text: string): string {
  // eslint-disable-next-line no-control-regex
  return text.replace(/\x1b\[[0-9;]*[A-Za-z]/g, "");
}

import type { CoderDelegationMemory } from "./coder-memory.js";

export interface DelegateFleetOptions {
  /**
   * How many children may run at once.
   *
   * A cap is not a nicety. Each child is a provider client and a process, so
   * an uncapped fan-out is rate-limit errors and a machine that stops
   * responding, and both of those look like the fleet not working.
   */
  readonly maxConcurrent: number;
  /** How many may wait for a slot before further requests are refused. */
  readonly maxQueued?: number | undefined;
  /** Where child transcripts go. Defaults to a private directory under tmp. */
  readonly transcriptDirectory?: string | undefined;
  /** The console's own directory, used when a request names none. */
  readonly cwd?: string | undefined;
  /**
   * The parent's memory, when the session carries one. `inherit` contributes
   * an advisory block to each child prompt; `harvest` records a completed
   * child's answer. Both are best-effort: memory never breaks a delegation.
   */
  readonly memory?: CoderDelegationMemory | undefined;
}

/**
 * The scheduler: the only thing that starts children.
 *
 * `submit` resolves when that child reaches a terminal state, so a caller can
 * await one child, await `Promise.all` of many, or ignore the promise for a
 * background child and read the registry instead.
 */
export class DelegateFleet {
  private readonly transcriptDirectory: string;
  private readonly waiting: Array<() => void> = [];
  private active = 0;
  private queued = 0;
  private sequence = 0;

  constructor(
    private readonly registry: CoderTaskRegistry,
    private readonly harness: DelegateHarness,
    private readonly options: DelegateFleetOptions,
  ) {
    this.transcriptDirectory =
      options.transcriptDirectory ?? join(tmpdir(), "openagents-coder-delegations");
    mkdirSync(this.transcriptDirectory, { recursive: true, mode: 0o700 });
  }

  get activeCount(): number {
    return this.active;
  }

  get queuedCount(): number {
    return this.queued;
  }

  /** Launch one child. Waits for a slot when the fleet is full. */
  async submit(request: DelegationRequest): Promise<DelegationOutcome> {
    if (request.prompt.trim().length === 0) {
      return { status: "refused", code: "empty_prompt", reason: "A child needs a prompt." };
    }

    const maxQueued = this.options.maxQueued ?? this.options.maxConcurrent * 8;
    if (this.active >= this.options.maxConcurrent && this.queued >= maxQueued) {
      return {
        status: "refused",
        code: "fleet_full",
        reason:
          `The fleet is full: ${String(this.active)} running and ` +
          `${String(this.queued)} queued, with a cap of ${String(this.options.maxConcurrent)}. ` +
          "Wait for a child to finish or raise the cap.",
      };
    }

    const cwd = request.cwd ?? this.options.cwd ?? process.cwd();
    // Inherited memory rides the prompt itself, so every harness — a shell
    // child, Devin over ACP, an opencode lane — carries it the same way.
    let inherited = "";
    try {
      inherited = this.options.memory?.inherit(request.prompt) ?? "";
    } catch {
      inherited = "";
    }
    const prompt = inherited.length > 0 ? `${request.prompt}\n\n${inherited}` : request.prompt;
    // Registered before it can queue, so a child waiting for a slot is visible
    // as `pending` rather than as nothing at all.
    const task = this.registry.register({
      id: this.mintId(),
      description: request.description.trim().length > 0 ? request.description : "delegated task",
      prompt,
      agent: this.harness.agent,
      model: this.harness.model,
      cwd,
      background: request.background ?? true,
    });

    if (this.active >= this.options.maxConcurrent) {
      this.queued += 1;
      await new Promise<void>((resolve) => this.waiting.push(resolve));
      this.queued -= 1;
      // A child stopped while it waited must not start now.
      if (this.registry.get(task.id)?.status === "stopped") {
        return { status: "stopped", taskId: task.id };
      }
    }

    this.active += 1;
    try {
      return await this.execute(task.id, { ...request, prompt }, cwd);
    } finally {
      this.active -= 1;
      this.waiting.shift()?.();
    }
  }

  /** Launch several children at once, respecting the cap. */
  submitAll(requests: ReadonlyArray<DelegationRequest>): Promise<ReadonlyArray<DelegationOutcome>> {
    return Promise.all(requests.map((request) => this.submit(request)));
  }

  private async execute(
    id: CoderTaskId,
    request: DelegationRequest,
    cwd: string,
  ): Promise<DelegationOutcome> {
    const transcriptPath = join(this.transcriptDirectory, `${id}.jsonl`);
    const controller = new AbortController();
    this.registry.start(id, controller);
    this.registry.attachTranscript(id, transcriptPath);

    /** Tool calls already counted. A harness reports one call several times. */
    const counted = new Set<string>();
    /** The child's session, once it reports one, so a retry can resume it. */
    let sessionId: string | undefined;
    let text = "";

    for (let attempt = 1; ; attempt += 1) {
      let reported: string | undefined;
      let thrown: string | undefined;

      try {
        for await (const event of this.harness.run(
          {
            prompt: request.prompt,
            cwd,
            transcriptPath,
            ...(sessionId === undefined ? {} : { resumeSessionId: sessionId }),
          },
          controller.signal,
        )) {
          if (controller.signal.aborted) break;
          if (event.type === "session") {
            sessionId = event.sessionId;
          } else if (event.type === "tool") {
            if (counted.has(event.callId)) continue;
            counted.add(event.callId);
            const activity: CoderToolActivity = {
              toolName: event.name,
              target: event.target,
              ...(event.meta === undefined ? {} : { meta: event.meta }),
            };
            this.registry.recordToolUse(id, activity);
          } else if (event.type === "tokens") {
            this.registry.recordTokens(id, { input: event.input, output: event.output });
          } else if (event.type === "text") {
            // Only the final assistant text is the child's answer, and a
            // harness emits one text part per step, so the last one wins.
            text = event.value;
          } else if (event.type === "error") {
            reported = event.message;
          }
        }
      } catch (cause) {
        thrown = cause instanceof Error ? cause.message : String(cause);
      }

      if (controller.signal.aborted) return { status: "stopped", taskId: id };

      const failure = reported ?? thrown;
      if (failure === undefined) {
        this.registry.complete(id, text);
        try {
          this.options.memory?.harvest(id, text);
        } catch {
          // Memory must never turn a completed child into a failed one.
        }
        return { status: "completed", taskId: id, result: text };
      }

      const retry = this.retryDelay(failure, attempt, sessionId, counted.size);
      if (retry === undefined) {
        this.registry.fail(id, failure);
        return { status: "failed", taskId: id, error: failure };
      }

      this.registry.recordToolUse(id, {
        toolName: "retry",
        target: `provider unavailable, attempt ${String(attempt + 1)}`,
      });

      await new Promise((wake) => setTimeout(wake, retry));
      if (controller.signal.aborted) return { status: "stopped", taskId: id };
    }
  }

  /**
   * How long to wait before running this child again, or `undefined` to stop.
   *
   * A provider that went away is the one failure worth retrying: nothing about
   * the request was wrong, and the six minutes of work the child had already
   * done are thrown away with it. Everything else — a refused permission, a
   * missing model, a prompt the child could not carry out — recurs on the
   * second attempt exactly as it did on the first.
   *
   * A retry that cannot resume the child's session is refused once the child
   * has run a tool. That child has edited files, and starting it again from
   * the prompt would apply its work twice; losing the run is the better of two
   * bad outcomes. With a session to continue, the work already done stays
   * done and the retry is safe.
   */
  private retryDelay(
    failure: string,
    attempt: number,
    sessionId: string | undefined,
    toolsRun: number,
  ): number | undefined {
    const attempts = 3;
    if (attempt >= attempts) return undefined;
    if (!transientProviderFailure(failure)) return undefined;
    if (sessionId === undefined && toolsRun > 0) return undefined;

    // Exponential, from two seconds. A provider that has just dropped is
    // usually back within a few, and a child is not a keystroke — waiting eight
    // seconds to save six minutes of work is not a wait anyone notices.
    return 2_000 * 2 ** (attempt - 1);
  }

  private mintId(): CoderTaskId {
    // Time first so ids sort in launch order, then a counter so two children
    // launched in the same millisecond cannot collide.
    this.sequence += 1;
    return `d${Date.now().toString(36)}${this.sequence.toString(36).padStart(2, "0")}`;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" ? value : undefined;
}

function numberField(record: Record<string, unknown>, key: string): number | undefined {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

/**
 * Whether this failure is the provider going away rather than the work being
 * wrong.
 *
 * Matched on the message because that is all a harness gives: `opencode`
 * reports its provider's error as text, and the shape seen in practice is
 * `APIError: Error from provider (Console): Upstream request failed: Endpoint
 * is unavailable.` The list is deliberately narrow. A false positive re-runs a
 * child that was never going to succeed, which costs minutes and money for the
 * same answer.
 */
export function transientProviderFailure(message: string): boolean {
  const text = message.toLowerCase();

  // A refusal that names the request is not the provider being away, however
  // much of the surrounding wording matches.
  if (/\b(invalid|unauthorized|forbidden|not found|unsupported|quota)\b/.test(text)) return false;

  return [
    "endpoint is unavailable",
    "upstream request failed",
    "service unavailable",
    "temporarily unavailable",
    "overloaded",
    "rate limit",
    "too many requests",
    "connection reset",
    "connection refused",
    "socket hang up",
    "econnreset",
    "etimedout",
    "gateway timeout",
    "bad gateway",
    "database is locked",
    "sqlite_busy",
    "502",
    "503",
    "504",
  ].some((needle) => text.includes(needle));
}
