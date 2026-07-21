import { Schema as S } from "effect";
import {
  AgentRuntimeRedactionClass,
  AgentRuntimeVisibility,
  KhalaRuntimeEventKind,
  khalaRuntimeEventKinds,
} from "@openagentsinc/agent-runtime-schema";

/**
 * HistoryCorpus schemas (RLM-01, #9137).
 *
 * A history corpus is a deterministic, cursor-addressed, redaction-aware
 * export of durable conversation history for one scope — one thread, one run,
 * or a thread set. Its sources are the durable harness event log
 * (`HarnessEventLogStore`, HARN-02) and neutral thread snapshots (the desktop
 * thread-store shape). The corpus is the READ artifact for recall traversal
 * (`docs/rlm/2026-07-21-rlm-integration-audit-and-roadmap.md` §4.1, §5.2): it
 * is never compacted, and cursors (`turnId` plus `sequence`) are its citation
 * scheme.
 */

/**
 * The seven core kinds the neutral log projection carries at HEAD
 * (`apps/openagents-desktop/src/harness-projection.ts`). Every other
 * `KhalaRuntimeEventKind` reaches the corpus only where a source actually
 * emitted it. The manifest coverage statement makes this bound explicit.
 */
export const neutralLogCoreKinds: ReadonlyArray<KhalaRuntimeEventKind> = [
  "turn.started",
  "turn.finished",
  "turn.interrupted",
  "text.delta",
  "reasoning.delta",
  "tool.call",
  "tool.result",
];

/**
 * The fixed honesty statement carried by every manifest's `coverage.note`.
 * The corpus must never imply completeness the neutral log does not have.
 */
export const historyCorpusCoverageNote =
  "The neutral harness log at HEAD carries only the seven core kinds: " +
  "turn.started, turn.finished, turn.interrupted, text.delta, reasoning.delta, " +
  "tool.call, tool.result. Plan, meter, question, child, and notice facts do " +
  "not reach the neutral log and are not in this corpus. Thread notes fill " +
  "display-side gaps only where a thread snapshot is supplied.";

/**
 * Corpus entry kind: every neutral event kind, plus the synthetic
 * `thread.note` kind for entries joined from a thread snapshot.
 */
export const HistoryCorpusEntryKind = S.Union([KhalaRuntimeEventKind, S.Literal("thread.note")]);
export type HistoryCorpusEntryKind = typeof HistoryCorpusEntryKind.Type;

export const HistoryCorpusEntryRole = S.Literals(["user", "assistant", "system"]);
export type HistoryCorpusEntryRole = typeof HistoryCorpusEntryRole.Type;

/**
 * One cursor-addressed corpus unit — a bounded, safe projection of one
 * `KhalaRuntimeEvent` or one thread note.
 *
 * Addressing: for event entries, `(turnId, sequence)` is the exact durable
 * cursor of the source event and round-trips to the source store
 * (`read({ turnId, fromCursor: sequence - 1 })` yields the event first).
 * Thread notes get synthetic addressing: `turnId` is the note key (or a
 * `note.<threadId>.<index>` ref when the note has no key) and `sequence` is
 * the note's index in the thread snapshot.
 *
 * Safety: `text` carries only the safe text the source already carries —
 * `text.delta`/`reasoning.delta` text, `tool.error` `messageSafe`, and
 * thread-note text. Refs (prompt refs, result refs, raw sidecar refs) stay
 * refs and are not dereferenced here.
 */
export const HistoryCorpusEntry = S.Struct({
  /** The owning thread id inside the scope. */
  scopeRef: S.String,
  /** Source turn id, or the synthetic thread-note ref. */
  turnId: S.String,
  /** The durable cursor (event `sequence`, or the note index). */
  sequence: S.Number,
  kind: HistoryCorpusEntryKind,
  /** Present only on `thread.note` entries. */
  role: S.optionalKey(HistoryCorpusEntryRole),
  /** Only the safe text the source event or note carries. */
  text: S.optionalKey(S.String),
  /** Present only on tool.* entries. */
  toolName: S.optionalKey(S.String),
  observedAt: S.String,
  visibility: AgentRuntimeVisibility,
  redactionClass: AgentRuntimeRedactionClass,
});
export interface HistoryCorpusEntry extends S.Schema.Type<typeof HistoryCorpusEntry> {}

/** Corpus scope — one thread, one run over threads, or an explicit thread set. */
export const HistoryCorpusScope = S.Union([
  S.TaggedStruct("Thread", { threadId: S.String }),
  S.TaggedStruct("Run", { runRef: S.String, threadIds: S.Array(S.String) }),
  S.TaggedStruct("ThreadSet", { threadIds: S.Array(S.String) }),
]);
export type HistoryCorpusScope = typeof HistoryCorpusScope.Type;

/**
 * Inclusion policy. An event whose `visibility` is not listed is excluded and
 * counted under `excludedByVisibility`. An event that passes visibility but
 * whose `redactionClass` is not listed is excluded and counted under
 * `excludedByRedaction`. Nothing is dropped silently.
 */
export const HistoryCorpusPolicy = S.Struct({
  includeVisibilities: S.Array(AgentRuntimeVisibility),
  includeRedactionClasses: S.Array(AgentRuntimeRedactionClass),
});
export interface HistoryCorpusPolicy extends S.Schema.Type<typeof HistoryCorpusPolicy> {}

/**
 * The coverage statement — which kinds the corpus actually contains, which
 * kinds of the neutral vocabulary it does not, and the fixed honesty note
 * about the seven-kind projection bound.
 */
export const HistoryCorpusCoverage = S.Struct({
  /** Distinct entry kinds present in the corpus, sorted. */
  eventKindsIncluded: S.Array(S.String),
  /** Neutral event kinds absent from the corpus, sorted. */
  eventKindsExcluded: S.Array(S.String),
  note: S.String,
});
export interface HistoryCorpusCoverage extends S.Schema.Type<typeof HistoryCorpusCoverage> {}

/** The exclusions record — visibility/redaction filtering is counted, never silent. */
export const HistoryCorpusExclusions = S.Struct({
  excludedByVisibility: S.Number,
  excludedByRedaction: S.Number,
  policy: HistoryCorpusPolicy,
});
export interface HistoryCorpusExclusions extends S.Schema.Type<typeof HistoryCorpusExclusions> {}

/**
 * The corpus manifest. `builtAt` comes from the build input, never from a
 * wall clock, so identical inputs yield an identical manifest. `byteLength`
 * is the exact UTF-8 byte length of the corpus JSONL serialization.
 */
export const HistoryCorpusManifest = S.Struct({
  corpusRef: S.String,
  scope: HistoryCorpusScope,
  builtAt: S.String,
  entryCount: S.Number,
  byteLength: S.Number,
  coverage: HistoryCorpusCoverage,
  exclusions: HistoryCorpusExclusions,
});
export interface HistoryCorpusManifest extends S.Schema.Type<typeof HistoryCorpusManifest> {}

export class HistoryCorpusError extends S.TaggedErrorClass<HistoryCorpusError>()(
  "HistoryCorpus.Error",
  {
    operation: S.String,
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

/** The full neutral event-kind vocabulary, re-exported for coverage math. */
export const historyCorpusEventKindVocabulary: ReadonlyArray<string> = khalaRuntimeEventKinds;
