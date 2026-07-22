// Resolve a local conversation ID to its on-disk source (issue:
// local-conversation -> public /trace/{uuid} ingest).
//
// Three supported sources, each with a different on-disk shape:
//   - claude     ~/.claude/projects/<slug>/<sessionId>.jsonl        (one file per session)
//   - codex      ~/.codex/sessions/YYYY/MM/DD/rollout-*-<id>.jsonl   (one file per session)
//   - openagents ~/Library/Application Support/<Profile>/KhalaDesktop/conversations.json
//                (the id selects an ARRAY ELEMENT inside one JSON file)
//
// Read-only: this module only locates and reads local files. It never mutates
// them. It returns the raw JSONL text (claude/codex) or the parsed conversation
// object (openagents); conversion + redaction happen downstream.

import { type Dirent, existsSync, readdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import type { OpenAgentsConversation } from "./openagents-conversation-to-atif";
import { findThreadConversationInThreadsFile } from "./threads-to-conversation";

export type ConversationSourceKind = "claude" | "codex" | "openagents";

export const CONVERSATION_SOURCE_KINDS: ReadonlyArray<ConversationSourceKind> = [
  "claude",
  "codex",
  "openagents",
];

export interface ResolveOptions {
  /** Home directory (defaults to os.homedir()). Injectable for tests. */
  readonly home?: string;
  /**
   * An explicit app `userData` directory to ALSO probe for the openagents
   * source. Full Auto isolated hosts store run threads at
   * `<userData>/threads.json` (and may keep a
   * `<userData>/KhalaDesktop/conversations.json`), neither of which lives under
   * the default `~/Library/Application Support` scan. When set, these files are
   * checked BEFORE the profile scan. Leave unset for byte-identical default
   * behavior.
   */
  readonly userData?: string;
}

export type ResolvedConversation =
  | {
      readonly kind: "claude";
      readonly id: string;
      /** Absolute path to the located JSONL file. */
      readonly path: string;
      /** Raw JSONL text. */
      readonly jsonl: string;
    }
  | {
      readonly kind: "codex";
      readonly id: string;
      /** Absolute path to the located JSONL file. */
      readonly path: string;
      /** Raw JSONL text. */
      readonly jsonl: string;
    }
  | {
      readonly kind: "openagents";
      readonly id: string;
      /** Absolute path to the located conversations.json file. */
      readonly path: string;
      /** The parsed conversation object selected by id. */
      readonly conversation: OpenAgentsConversation;
    };

const resolveHome = (options: ResolveOptions): string => options.home ?? homedir();

/** Directory entries of `dir`, or [] when it is missing/unreadable. */
const readEntries = (dir: string): ReadonlyArray<Dirent<string>> => {
  if (!existsSync(dir)) return [];
  try {
    return readdirSync(dir, { withFileTypes: true }) as ReadonlyArray<Dirent<string>>;
  } catch {
    return [];
  }
};

/** Recursively list files under `dir` (missing dir -> []). Bounded, read-only. */
const walkFiles = (dir: string, depth = 6): string[] => {
  if (depth < 0) return [];
  const out: string[] = [];
  for (const entry of readEntries(dir)) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walkFiles(full, depth - 1));
    else if (entry.isFile()) out.push(full);
  }
  return out;
};

// --- Claude Code -----------------------------------------------------------

/** Find `~/.claude/projects/<slug>/<sessionId>.jsonl` for a session id. */
export function findClaudeSession(
  id: string,
  options: ResolveOptions = {},
): { path: string; jsonl: string } | undefined {
  const root = join(resolveHome(options), ".claude", "projects");
  const target = `${id}.jsonl`;
  for (const file of walkFiles(root, 3)) {
    if (file.endsWith(`/${target}`) || file.endsWith(`\\${target}`)) {
      return { path: file, jsonl: readFileSync(file, "utf8") };
    }
  }
  return undefined;
}

// --- Codex -----------------------------------------------------------------

/** Find `~/.codex/sessions/<date>/rollout-*-<id>.jsonl` for a session id. */
export function findCodexSession(
  id: string,
  options: ResolveOptions = {},
): { path: string; jsonl: string } | undefined {
  const root = join(resolveHome(options), ".codex", "sessions");
  const lowerId = id.toLowerCase();
  for (const file of walkFiles(root, 5)) {
    const base = file.split(/[/\\]/).pop() ?? "";
    if (
      base.startsWith("rollout-") &&
      base.endsWith(".jsonl") &&
      base.toLowerCase().includes(lowerId)
    ) {
      return { path: file, jsonl: readFileSync(file, "utf8") };
    }
  }
  return undefined;
}

// --- OpenAgents Desktop ----------------------------------------------------

/** Parse `file` as JSON, returning undefined when missing/unreadable/invalid. */
const parseJsonFile = (file: string): unknown => {
  if (!existsSync(file)) return undefined;
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch {
    return undefined;
  }
};

/** Match `id` against a parsed `conversations.json` (array or `{conversations}`). */
const findConversationInConversationsFile = (
  parsed: unknown,
  id: string,
): OpenAgentsConversation | undefined => {
  const lowerId = id.toLowerCase();
  const list = Array.isArray(parsed)
    ? parsed
    : parsed !== null &&
        typeof parsed === "object" &&
        Array.isArray((parsed as { conversations?: unknown }).conversations)
      ? (parsed as { conversations: unknown[] }).conversations
      : [];
  for (const item of list) {
    if (
      item !== null &&
      typeof item === "object" &&
      typeof (item as { id?: unknown }).id === "string" &&
      (item as { id: string }).id.toLowerCase() === lowerId
    ) {
      return item as OpenAgentsConversation;
    }
  }
  return undefined;
};

/**
 * Probe an explicit `userData` directory for a conversation/thread id. Full
 * Auto isolated hosts keep run threads at `<userData>/threads.json` (adapted
 * through {@link findThreadConversationInThreadsFile}) and may also keep a
 * `<userData>/KhalaDesktop/conversations.json`. threads.json is checked first.
 */
const findOpenAgentsInUserData = (
  userData: string,
  id: string,
): { path: string; conversation: OpenAgentsConversation } | undefined => {
  const threadsFile = join(userData, "threads.json");
  const threadHit = findThreadConversationInThreadsFile(parseJsonFile(threadsFile), id);
  if (threadHit !== undefined) return { path: threadsFile, conversation: threadHit };

  const conversationsFile = join(userData, "KhalaDesktop", "conversations.json");
  const conversationHit = findConversationInConversationsFile(
    parseJsonFile(conversationsFile),
    id,
  );
  if (conversationHit !== undefined) {
    return { path: conversationsFile, conversation: conversationHit };
  }
  return undefined;
};

/**
 * Find a desktop conversation by id across every app profile's
 * `KhalaDesktop/conversations.json`. Ids are compared case-insensitively (the
 * desktop store uses upper-case UUIDs). When `options.userData` is set, that
 * directory's `threads.json` / `conversations.json` is probed FIRST so Full
 * Auto isolated-host run threads resolve directly.
 */
export function findOpenAgentsConversation(
  id: string,
  options: ResolveOptions = {},
): { path: string; conversation: OpenAgentsConversation } | undefined {
  if (options.userData !== undefined) {
    const hit = findOpenAgentsInUserData(options.userData, id);
    if (hit !== undefined) return hit;
  }

  const home = resolveHome(options);
  const supportRoot = join(home, "Library", "Application Support");

  for (const profile of readEntries(supportRoot)) {
    if (!profile.isDirectory()) continue;
    const file = join(supportRoot, profile.name, "KhalaDesktop", "conversations.json");
    if (!existsSync(file)) continue;
    const conversation = findConversationInConversationsFile(parseJsonFile(file), id);
    if (conversation !== undefined) return { path: file, conversation };
  }
  return undefined;
}

// --- Dispatch --------------------------------------------------------------

export class ConversationNotFoundError extends Error {
  constructor(id: string, kind: ConversationSourceKind | "auto") {
    super(
      kind === "auto"
        ? `No local Claude, Codex, or OpenAgents conversation found for id "${id}".`
        : `No local ${kind} conversation found for id "${id}".`,
    );
    this.name = "ConversationNotFoundError";
  }
}

/**
 * Resolve a conversation id to its source. When `kind` is omitted, auto-detect
 * by trying claude, then codex, then openagents (the per-session-file sources
 * first, since they are the highest-fidelity).
 */
export function resolveConversation(
  id: string,
  kind: ConversationSourceKind | "auto" = "auto",
  options: ResolveOptions = {},
): ResolvedConversation {
  const tryClaude = (): ResolvedConversation | undefined => {
    const hit = findClaudeSession(id, options);
    return hit ? { kind: "claude", id, path: hit.path, jsonl: hit.jsonl } : undefined;
  };
  const tryCodex = (): ResolvedConversation | undefined => {
    const hit = findCodexSession(id, options);
    return hit ? { kind: "codex", id, path: hit.path, jsonl: hit.jsonl } : undefined;
  };
  const tryOpenAgents = (): ResolvedConversation | undefined => {
    const hit = findOpenAgentsConversation(id, options);
    return hit
      ? { kind: "openagents", id, path: hit.path, conversation: hit.conversation }
      : undefined;
  };

  const resolved =
    kind === "claude"
      ? tryClaude()
      : kind === "codex"
        ? tryCodex()
        : kind === "openagents"
          ? tryOpenAgents()
          : tryClaude() ?? tryCodex() ?? tryOpenAgents();

  if (resolved === undefined) throw new ConversationNotFoundError(id, kind);
  return resolved;
}
