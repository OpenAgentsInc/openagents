// Adapter: a Full Auto isolated-host run-thread transcript (`threads.json`) ->
// the internal OpenAgents conversation shape the ATIF converter expects
// (issue: local-conversation -> public /trace/{uuid} ingest).
//
// Full Auto isolated hosts DO NOT use the desktop `KhalaDesktop/conversations.json`
// store (an array of `{ id, messages: [...] }`). They persist run threads at
// `<userData>/threads.json` with a DIFFERENT shape:
//
//   { version, threads: [ { id, title, notes: [ { role, text, timestamp } ] } ] }
//
// This module maps one such thread onto an {@link OpenAgentsConversation} so the
// unchanged `convertOpenAgentsConversationToAtif` converter produces the same
// ATIF a normal conversation would: each note's `text` becomes the message
// `content`/`text`, `role` is preserved, and `title` labels the trajectory.
//
// Read-only: this module only reshapes already-read data. It never mutates the
// store. Redaction + the public-safety tripwire still run downstream unchanged.

import type { OpenAgentsConversation } from "./openagents-conversation-to-atif";

/** One note inside a Full Auto run thread. */
export interface FullAutoThreadNote {
  readonly role?: unknown;
  readonly text?: unknown;
  readonly timestamp?: unknown;
  readonly [k: string]: unknown;
}

/** One Full Auto run thread as stored in `threads.json`. */
export interface FullAutoThread {
  readonly id?: unknown;
  readonly title?: unknown;
  readonly notes?: unknown;
  readonly [k: string]: unknown;
}

/** The parsed `threads.json` document. */
export interface FullAutoThreadsFile {
  readonly version?: unknown;
  readonly threads?: unknown;
  readonly [k: string]: unknown;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);

// ISO 8601 (matches the ATIF step-timestamp requirement in
// packages/atif/src/validate.ts). Full Auto notes store a display-only local
// time-of-day (e.g. "02:26 PM"), which is NOT ISO 8601 and cannot become a
// valid ATIF `timestamp`, so a non-ISO note timestamp is dropped rather than
// carried forward into an invalid trajectory.
const ISO_8601 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;
const isIso8601 = (value: string): boolean =>
  ISO_8601.test(value) && !Number.isNaN(Date.parse(value));

/**
 * Map a Full Auto thread's `notes` onto the internal conversation `messages`
 * shape. `text` becomes both `content` and `text` (the converter reads
 * `content ?? text`); `role` is preserved; a `timestamp` is preserved only when
 * it is already ISO 8601.
 */
export function threadNotesToMessages(
  notes: unknown,
): ReadonlyArray<Record<string, unknown>> {
  if (!Array.isArray(notes)) return [];
  const messages: Record<string, unknown>[] = [];
  for (const raw of notes) {
    if (!isRecord(raw)) continue;
    const note = raw as FullAutoThreadNote;
    const role = typeof note.role === "string" ? note.role : "user";
    const text = typeof note.text === "string" ? note.text : "";
    const message: Record<string, unknown> = { role, content: text, text };
    if (typeof note.timestamp === "string" && isIso8601(note.timestamp)) {
      message.timestamp = note.timestamp;
    }
    messages.push(message);
  }
  return messages;
}

/**
 * Convert one Full Auto run thread into an {@link OpenAgentsConversation}. The
 * result carries the thread id, title, and notes-as-messages so the existing
 * converter can build the identical ATIF trajectory.
 */
export function threadToConversation(thread: FullAutoThread): OpenAgentsConversation {
  const id = typeof thread.id === "string" ? thread.id : undefined;
  const title = typeof thread.title === "string" ? thread.title : undefined;
  return {
    ...(id === undefined ? {} : { id }),
    ...(title === undefined ? {} : { title }),
    messages: threadNotesToMessages(thread.notes),
  };
}

/**
 * Find a run thread by id inside a parsed `threads.json` document and return it
 * as an {@link OpenAgentsConversation}. Ids are compared case-insensitively.
 * Returns `undefined` when the document is not a threads file or has no match.
 */
export function findThreadConversationInThreadsFile(
  parsed: unknown,
  id: string,
): OpenAgentsConversation | undefined {
  if (!isRecord(parsed)) return undefined;
  const threads = (parsed as FullAutoThreadsFile).threads;
  if (!Array.isArray(threads)) return undefined;
  const lowerId = id.toLowerCase();
  for (const raw of threads) {
    if (!isRecord(raw)) continue;
    const thread = raw as FullAutoThread;
    if (typeof thread.id === "string" && thread.id.toLowerCase() === lowerId) {
      return threadToConversation(thread);
    }
  }
  return undefined;
}
