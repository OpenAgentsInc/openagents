import { readFileSync, statSync } from "node:fs";

import { parseOpencodeEvent } from "./coder-delegate.js";

/**
 * A child's own transcript, read back for the interface to show.
 *
 * Every harness writes one as its child runs — the fleet hands each child a
 * path and the harness appends to it, so a child that is killed still leaves
 * everything it had done behind. This reads that file so the reader can open a
 * child and watch it work rather than waiting for the one paragraph it returns
 * at the end.
 *
 * Two shapes arrive here and neither is negotiable. The self-hosted harness
 * writes its own small records (`session`, `tool`, `tool_result`, `text`);
 * `opencode` writes its own event stream, which `parseOpencodeEvent` already
 * knows how to read because the fleet reads it live. Both are normalized to
 * the same handful of entries, so the screen renders one thing.
 *
 * A file is re-read when it has grown and reused when it has not. A child is
 * appended to several times a second and the screen repaints at least as
 * often; re-parsing a finished child's whole transcript on every frame is work
 * that buys nothing.
 */

export type ChildEntry =
  | { readonly kind: "started"; readonly model: string; readonly cwd: string }
  | { readonly kind: "tool"; readonly name: string; readonly target: string | undefined }
  | { readonly kind: "output"; readonly text: string }
  | { readonly kind: "text"; readonly text: string }
  | { readonly kind: "error"; readonly text: string };

interface Cached {
  readonly size: number;
  readonly entries: ReadonlyArray<ChildEntry>;
}

/**
 * What has been read, by path, bounded.
 *
 * A session's children are few, but nothing here prunes on its own and a cache
 * that only grows is one that eventually holds every transcript of a long
 * session in memory. The oldest entry goes when the cap is reached; re-reading
 * it costs one parse.
 */
const CACHE_LIMIT = 32;
const cache = new Map<string, Cached>();

const remember = (path: string, entry: Cached): void => {
  cache.delete(path);
  cache.set(path, entry);
  if (cache.size > CACHE_LIMIT) {
    const oldest = cache.keys().next();
    if (!oldest.done) cache.delete(oldest.value);
  }
};

/**
 * The child's transcript at this path, or an empty list.
 *
 * Never throws. A child that has not written yet, a path that was removed, and
 * a line that is half-written because the harness is mid-append are all the
 * same thing to a reader watching a running child: less than there will be in
 * a moment.
 */
export const readChildTranscript = (path: string | undefined): ReadonlyArray<ChildEntry> => {
  if (path === undefined) return [];

  let size: number;
  try {
    size = statSync(path).size;
  } catch {
    return [];
  }

  const seen = cache.get(path);
  if (seen !== undefined && seen.size === size) return seen.entries;

  let contents: string;
  try {
    contents = readFileSync(path, "utf8");
  } catch {
    return seen?.entries ?? [];
  }

  const entries = contents.split("\n").flatMap((line) => {
    const entry = parseLine(line);
    return entry === undefined ? [] : [entry];
  });

  remember(path, { size, entries });
  return entries;
};

const parseLine = (line: string): ChildEntry | undefined => {
  const trimmed = line.trim();
  if (trimmed.length === 0 || !trimmed.startsWith("{")) return undefined;

  let record: Record<string, unknown>;
  try {
    record = JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    // A half-written last line, because the harness is appending as this
    // reads. It will parse on the next frame.
    return undefined;
  }

  const own = fromSelfHarness(record);
  if (own !== undefined) return own;

  const acp = fromAcp(record);
  if (acp !== undefined) return acp;

  // Anything else is a harness with its own event stream, and `opencode`'s is
  // the one the fleet already reads live.
  const event = parseOpencodeEvent(trimmed);
  if (event === undefined) return undefined;

  switch (event.type) {
    case "tool":
      return { kind: "tool", name: event.name, target: event.target };
    case "text":
      return { kind: "text", text: event.value };
    case "error":
      return { kind: "error", text: event.message };
    default:
      return undefined;
  }
};

/**
 * A Devin child, which writes the ACP protocol it spoke.
 *
 * The harness records every JSON-RPC message so a reader can open a running
 * child and watch it work. Most of those messages are not worth showing — the
 * handshake, the mode, dozens of thought fragments a token at a time — so this
 * takes the three that are: what it ran, what came back, and what it said.
 */
const fromAcp = (message: Record<string, unknown>): ChildEntry | undefined => {
  if (message["method"] !== "session/update") return undefined;

  const update = nested(nested(message["params"])["update"]);
  const kind = update["sessionUpdate"];

  if (kind === "tool_call") {
    // Devin's `title` is already the phrase a person would read — "Ran ls",
    // "Read src/a.ts" — so it is the target rather than a name to look up.
    const name = text(update["kind"]) ?? "tool";
    return { kind: "tool", name, target: text(update["title"]) };
  }

  if (kind === "tool_call_update") {
    const said = firstContentText(update["content"]);
    return said === undefined ? undefined : { kind: "output", text: said };
  }

  if (kind === "agent_message_chunk") {
    const said = text(nested(update["content"])["text"]);
    return said === undefined ? undefined : { kind: "text", text: said };
  }

  return undefined;
};

/** The first piece of text inside an ACP content array, if there is one. */
const firstContentText = (content: unknown): string | undefined => {
  if (!Array.isArray(content)) return undefined;
  for (const part of content) {
    const inner = nested(nested(part)["content"]);
    const said = text(inner["text"]);
    if (said !== undefined) return said;
  }
  return undefined;
};

const nested = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

const fromSelfHarness = (record: Record<string, unknown>): ChildEntry | undefined => {
  const type = record["type"];

  if (type === "session") {
    return {
      kind: "started",
      model: text(record["model"]) ?? "unknown",
      cwd: text(record["cwd"]) ?? "",
    };
  }

  if (type === "tool") {
    const name = text(record["name"]);
    if (name === undefined) return undefined;
    return { kind: "tool", name, target: target(record["arguments"]) };
  }

  if (type === "tool_result") {
    const output = text(record["output"]);
    return output === undefined ? undefined : { kind: "output", text: output };
  }

  if (type === "text") {
    const value = text(record["value"]);
    return value === undefined ? undefined : { kind: "text", text: value };
  }

  return undefined;
};

/** The one argument worth showing beside a tool's name, if there is one. */
const target = (args: unknown): string | undefined => {
  if (typeof args !== "object" || args === null) return undefined;
  const record = args as Record<string, unknown>;
  for (const key of ["command", "path", "file", "pattern", "query"]) {
    const value = record[key];
    if (typeof value === "string" && value.length > 0) return value;
  }
  return undefined;
};

const text = (value: unknown): string | undefined =>
  typeof value === "string" && value.length > 0 ? value : undefined;
