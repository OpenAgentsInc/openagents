import { Effect, Schema as S } from "effect";
import type {
  HarnessEventLogError,
  HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract";

import {
  HistoryCorpusEntry,
  HistoryCorpusError,
  type HistoryCorpusManifest,
  type HistoryCorpusPolicy,
  type HistoryCorpusScope,
  historyCorpusCoverageNote,
  historyCorpusEventKindVocabulary,
} from "./corpus.ts";

/**
 * The read seam the builder consumes. `HarnessEventLogStore`
 * (`@openagentsinc/agent-harness-contract`) satisfies this structurally —
 * the builder only reads, it never appends or records boundaries.
 */
export interface HistoryCorpusEventReader {
  readonly read: (params: {
    readonly turnId: string;
    readonly fromCursor: number;
  }) => Effect.Effect<ReadonlyArray<HarnessStreamEvent>, HarnessEventLogError>;
}

/**
 * Neutral thread-note input shape. The desktop `DesktopMessage`
 * (`apps/openagents-desktop/src/chat-contract.ts`) satisfies it structurally;
 * this package never imports the desktop app.
 */
export const NeutralThreadNote = S.Struct({
  key: S.String,
  role: S.Literals(["user", "assistant", "system"]),
  text: S.String,
  timestamp: S.String,
});
export interface NeutralThreadNote extends S.Schema.Type<typeof NeutralThreadNote> {}

/**
 * Neutral thread-snapshot input shape. The desktop `DesktopThread` satisfies
 * it structurally (`createdAt` is optional-or-undefined there, matched here).
 */
export const NeutralThreadSnapshot = S.Struct({
  id: S.String,
  title: S.String,
  createdAt: S.optional(S.String),
  updatedAt: S.String,
  notes: S.Array(NeutralThreadNote),
});
export interface NeutralThreadSnapshot extends S.Schema.Type<typeof NeutralThreadSnapshot> {}

export interface BuildHistoryCorpusInput {
  readonly scope: HistoryCorpusScope;
  /** Durable event-log reader. `HarnessEventLogStore` satisfies this. */
  readonly eventLog?: HistoryCorpusEventReader | undefined;
  /**
   * The turn ids to read from `eventLog`. The store has no turn enumeration,
   * so the caller supplies it. Order does not matter — the builder sorts.
   * Events whose `threadId` is outside the scope are out of scope by
   * definition and are not corpus exclusions.
   */
  readonly turnIds?: ReadonlyArray<string> | undefined;
  /** Thread snapshots to join (display-side gap fill). */
  readonly threads?: ReadonlyArray<NeutralThreadSnapshot> | undefined;
  readonly policy: HistoryCorpusPolicy;
  /** Build timestamp, supplied by the caller — never a wall clock read. */
  readonly builtAt: string;
}

export interface HistoryCorpusBuildResult {
  readonly manifest: HistoryCorpusManifest;
  readonly entries: ReadonlyArray<HistoryCorpusEntry>;
}

/**
 * Thread notes carry raw owner-local chat text, so they enter filtering as
 * `visibility: "private"`, `redactionClass: "private_ref"` — the same class
 * the neutral projection assigns owner-local lane events.
 */
export const threadNoteVisibility = "private" as const;
export const threadNoteRedactionClass = "private_ref" as const;

const decodeEntry = S.decodeUnknownSync(HistoryCorpusEntry);

/** Stable JSONL key order — serialization is independent of object key order. */
const entryKeyOrder = [
  "scopeRef",
  "turnId",
  "sequence",
  "kind",
  "role",
  "text",
  "toolName",
  "observedAt",
  "visibility",
  "redactionClass",
];

/** Serialize corpus entries to JSONL: one entry per line, `\n`-terminated. */
export const corpusEntriesToJsonl = (entries: ReadonlyArray<HistoryCorpusEntry>): string =>
  entries.map((entry) => `${JSON.stringify(entry, entryKeyOrder)}\n`).join("");

const compareStrings = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0);

const compareEntries = (a: HistoryCorpusEntry, b: HistoryCorpusEntry): number =>
  compareStrings(a.scopeRef, b.scopeRef) ||
  compareStrings(a.turnId, b.turnId) ||
  a.sequence - b.sequence;

const sortedUnique = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values)].sort(compareStrings);

const scopeThreadIds = (scope: HistoryCorpusScope): ReadonlyArray<string> => {
  switch (scope._tag) {
    case "Thread":
      return [scope.threadId];
    case "Run":
      return sortedUnique(scope.threadIds);
    case "ThreadSet":
      return sortedUnique(scope.threadIds);
  }
};

const scopeKey = (scope: HistoryCorpusScope): string => {
  switch (scope._tag) {
    case "Thread":
      return `thread.${scope.threadId}`;
    case "Run":
      return `run.${scope.runRef}`;
    case "ThreadSet":
      return `threadset.${sortedUnique(scope.threadIds).join(".")}`;
  }
};

const safeTextOf = (event: HarnessStreamEvent): string | undefined => {
  switch (event.kind) {
    case "text.delta":
    case "reasoning.delta":
      return event.text;
    case "tool.error":
      return event.messageSafe;
    default:
      return undefined;
  }
};

const toolNameOf = (event: HarnessStreamEvent): string | undefined => {
  switch (event.kind) {
    case "tool.input.delta":
    case "tool.input.completed":
    case "tool.call":
    case "tool.result":
    case "tool.error":
      return event.toolName;
    default:
      return undefined;
  }
};

const entryFromEvent = (event: HarnessStreamEvent): HistoryCorpusEntry => {
  const text = safeTextOf(event);
  const toolName = toolNameOf(event);
  return decodeEntry({
    scopeRef: event.threadId,
    turnId: event.turnId,
    sequence: event.sequence,
    kind: event.kind,
    ...(text === undefined ? {} : { text }),
    ...(toolName === undefined ? {} : { toolName }),
    observedAt: event.observedAt,
    visibility: event.visibility,
    redactionClass: event.redactionClass,
  });
};

const entryFromNote = (
  threadId: string,
  note: NeutralThreadNote,
  index: number,
): HistoryCorpusEntry =>
  decodeEntry({
    scopeRef: threadId,
    turnId: note.key.trim() === "" ? `note.${threadId}.${index}` : note.key,
    sequence: index,
    kind: "thread.note",
    role: note.role,
    text: note.text,
    observedAt: note.timestamp,
    visibility: threadNoteVisibility,
    redactionClass: threadNoteRedactionClass,
  });

/**
 * Build a history corpus for a scope. PURE and deterministic: identical
 * inputs produce an identical `{ manifest, entries }` — stable ordering by
 * `(scopeRef, turnId, sequence)`, `builtAt` from the input, no clock, no
 * randomness.
 *
 * - Visibility/redaction filtering never drops silently: every excluded
 *   source unit is counted in `manifest.exclusions` (visibility checked
 *   first, then redaction class).
 * - Every event entry's `(turnId, sequence)` round-trips to the source store.
 * - A duplicate corpus address is a build error, never a silent overwrite.
 */
export const buildHistoryCorpus = (
  input: BuildHistoryCorpusInput,
): Effect.Effect<HistoryCorpusBuildResult, HistoryCorpusError> =>
  Effect.gen(function* () {
    const threadIds = scopeThreadIds(input.scope);
    const inScope = new Set(threadIds);

    // 1. Read every supplied turn from the durable log, whole-turn (cursor -1).
    const events: Array<HarnessStreamEvent> = [];
    const eventLog = input.eventLog;
    if (eventLog !== undefined) {
      for (const turnId of sortedUnique(input.turnIds ?? [])) {
        const turnEvents = yield* eventLog.read({ turnId, fromCursor: -1 }).pipe(
          Effect.mapError(
            (cause) =>
              new HistoryCorpusError({
                operation: "read_event_log",
                detail: `turnId=${turnId}`,
                cause,
              }),
          ),
        );
        for (const event of turnEvents) {
          if (inScope.has(event.threadId)) events.push(event);
        }
      }
    }

    // 2. Assemble, filter, count, sort — synchronous and deterministic.
    return yield* Effect.try({
      try: (): HistoryCorpusBuildResult => {
        const includeVisibilities = new Set(input.policy.includeVisibilities);
        const includeRedactionClasses = new Set(input.policy.includeRedactionClasses);
        let excludedByVisibility = 0;
        let excludedByRedaction = 0;
        const entries: Array<HistoryCorpusEntry> = [];

        const admit = (
          visibility: HistoryCorpusPolicy["includeVisibilities"][number],
          redactionClass: HistoryCorpusPolicy["includeRedactionClasses"][number],
        ): boolean => {
          if (!includeVisibilities.has(visibility)) {
            excludedByVisibility += 1;
            return false;
          }
          if (!includeRedactionClasses.has(redactionClass)) {
            excludedByRedaction += 1;
            return false;
          }
          return true;
        };

        for (const event of events) {
          if (admit(event.visibility, event.redactionClass)) {
            entries.push(entryFromEvent(event));
          }
        }

        const threads = [...(input.threads ?? [])]
          .filter((thread) => inScope.has(thread.id))
          .sort((a, b) => compareStrings(a.id, b.id));
        for (const thread of threads) {
          thread.notes.forEach((note, index) => {
            if (admit(threadNoteVisibility, threadNoteRedactionClass)) {
              entries.push(entryFromNote(thread.id, note, index));
            }
          });
        }

        entries.sort(compareEntries);

        // Cursor-addressing integrity: one entry per (scopeRef, turnId, sequence).
        const seen = new Set<string>();
        for (const entry of entries) {
          const address = `${entry.scopeRef} ${entry.turnId} ${entry.sequence}`;
          if (seen.has(address)) {
            throw new HistoryCorpusError({
              operation: "address_entries",
              detail: `duplicate corpus address turnId=${entry.turnId} sequence=${entry.sequence}`,
            });
          }
          seen.add(address);
        }

        const includedKinds = sortedUnique(entries.map((entry) => entry.kind));
        const includedKindSet = new Set(includedKinds);
        const excludedKinds = historyCorpusEventKindVocabulary
          .filter((kind) => !includedKindSet.has(kind))
          .sort(compareStrings);

        const jsonl = corpusEntriesToJsonl(entries);
        const byteLength = new TextEncoder().encode(jsonl).length;

        const manifest: HistoryCorpusManifest = {
          corpusRef: `corpus.${scopeKey(input.scope)}.${input.builtAt}`,
          scope: input.scope,
          builtAt: input.builtAt,
          entryCount: entries.length,
          byteLength,
          coverage: {
            eventKindsIncluded: includedKinds,
            eventKindsExcluded: excludedKinds,
            note: historyCorpusCoverageNote,
          },
          exclusions: {
            excludedByVisibility,
            excludedByRedaction,
            policy: input.policy,
          },
        };

        return { manifest, entries };
      },
      catch: (cause) =>
        cause instanceof HistoryCorpusError
          ? cause
          : new HistoryCorpusError({ operation: "assemble_corpus", cause }),
    });
  });
