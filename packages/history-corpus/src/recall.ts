import { Schema as S } from "effect";

import {
  HistoryCorpusEntry,
  HistoryCorpusEntryKind,
  HistoryCorpusManifest,
  HistoryCorpusScope,
} from "./corpus.ts";

/**
 * HistoryRecall contract (RLM-02, #9138).
 *
 * `HistoryRecall` is the typed query capability over a history corpus
 * (`docs/rlm/2026-07-21-rlm-integration-audit-and-roadmap.md` §5.2). Tier D is
 * the deterministic backend: grep, cursor and time slicing, key-turn
 * extraction, and structural per-turn summaries — zero model calls, always
 * available, every answer a cited cursor span.
 *
 * Cap semantics: caps TRUNCATE, they do not fail. A capped answer is still a
 * valid answer — the response's REQUIRED `honesty` field states how much of
 * the corpus was scanned, which caps were hit, and the corpus coverage bound.
 * Only invalid input (an uncompilable grep pattern, an unavailable corpus)
 * fails, with a typed `HistoryRecallError`.
 */

/**
 * Deterministic question kinds. Each is a pure operation over the ordered
 * corpus — no model call is permitted behind any of them.
 */
export const HistoryRecallQuestion = S.Union([
  /**
   * Lexical search over entry text. `pattern` is compiled with `RegExp` —
   * an invalid pattern is a typed `invalid_pattern` failure, never a crash.
   * Case-insensitive unless `caseSensitive` is true.
   */
  S.TaggedStruct("Grep", {
    pattern: S.String,
    caseSensitive: S.optionalKey(S.Boolean),
  }),
  /**
   * Entries whose `sequence` lies in `[fromSequence, toSequence]` (inclusive
   * both ends), optionally restricted to one `turnId`.
   */
  S.TaggedStruct("CursorSlice", {
    turnId: S.optionalKey(S.String),
    fromSequence: S.Number,
    toSequence: S.Number,
  }),
  /**
   * Entries whose `observedAt` lies in `[fromObservedAt, toObservedAt]`
   * (inclusive both ends, ISO-8601 string comparison).
   */
  S.TaggedStruct("TimeSlice", {
    fromObservedAt: S.String,
    toObservedAt: S.String,
  }),
  /**
   * The first `limit` turn boundaries in corpus order — one span per turn
   * covering its full cursor range, excerpting its first and last text.
   * `limit` is part of the question, not a cap: stopping at `limit` turns is
   * not reported as truncation.
   */
  S.TaggedStruct("KeyTurns", { limit: S.Number }),
  /**
   * A structural summary of one turn: event counts by kind, tools used,
   * first and last text, and the exact cursor span.
   */
  S.TaggedStruct("TurnSummary", { turnId: S.String }),
]);
export type HistoryRecallQuestion = typeof HistoryRecallQuestion.Type;

/** The cap names the honesty field can report as hit. */
export const HistoryRecallCapName = S.Literals([
  "maxSpans",
  "maxEntriesScanned",
  "maxCharsPerSpan",
]);
export type HistoryRecallCapName = typeof HistoryRecallCapName.Type;

/**
 * Budget caps. Deterministic work is still budgeted: a huge corpus must not
 * turn a free query into an unbounded traversal. Every field is optional —
 * absent fields take {@link historyRecallDefaultCaps}.
 */
export const HistoryRecallCaps = S.Struct({
  /** Hard bound on answer spans returned. */
  maxSpans: S.optionalKey(S.Number),
  /** Hard bound on corpus entries examined, in corpus order. */
  maxEntriesScanned: S.optionalKey(S.Number),
  /** Hard bound on excerpt length per span, in characters. */
  maxCharsPerSpan: S.optionalKey(S.Number),
});
export interface HistoryRecallCaps extends S.Schema.Type<typeof HistoryRecallCaps> {}

/** Defaults applied for absent cap fields. */
export const historyRecallDefaultCaps = {
  maxSpans: 50,
  maxEntriesScanned: 10_000,
  maxCharsPerSpan: 400,
} as const;

/**
 * Corpus input: either a scope the service resolves through its corpus
 * provider, or a prebuilt corpus carried inline (manifest plus entries).
 */
export const HistoryRecallCorpusInput = S.Union([
  S.TaggedStruct("Scope", { scope: HistoryCorpusScope }),
  S.TaggedStruct("Corpus", {
    manifest: HistoryCorpusManifest,
    entries: S.Array(HistoryCorpusEntry),
  }),
]);
export type HistoryRecallCorpusInput = typeof HistoryRecallCorpusInput.Type;

export const HistoryRecallRequest = S.Struct({
  corpus: HistoryRecallCorpusInput,
  question: HistoryRecallQuestion,
  caps: S.optionalKey(HistoryRecallCaps),
});
export interface HistoryRecallRequest extends S.Schema.Type<typeof HistoryRecallRequest> {}

/**
 * One cited answer span. `(scopeRef, turnId, sequenceStart..sequenceEnd)` is
 * an exact durable cursor range into the corpus — the citation scheme, never
 * paraphrase. `excerpt` is bounded by the `maxCharsPerSpan` cap.
 */
export const HistoryRecallSpan = S.Struct({
  scopeRef: S.String,
  turnId: S.String,
  sequenceStart: S.Number,
  sequenceEnd: S.Number,
  excerpt: S.String,
  kind: HistoryCorpusEntryKind,
});
export interface HistoryRecallSpan extends S.Schema.Type<typeof HistoryRecallSpan> {}

/**
 * REQUIRED honesty record: recall must state what it scanned and what it
 * could not (audit §5.2). `truncated` is true exactly when `capsHit` is
 * non-empty. `coverageNote` carries the corpus manifest coverage note
 * through, so a caller always sees the seven-kind projection bound.
 */
export const HistoryRecallHonesty = S.Struct({
  tier: S.Literal("deterministic"),
  entriesScanned: S.Number,
  entriesTotal: S.Number,
  truncated: S.Boolean,
  /** The caps that truncated this answer, sorted, empty when complete. */
  capsHit: S.Array(HistoryRecallCapName),
  coverageNote: S.String,
});
export interface HistoryRecallHonesty extends S.Schema.Type<typeof HistoryRecallHonesty> {}

/** Cost record. Tier D is deterministic: `modelCalls` is always 0. */
export const HistoryRecallCost = S.Struct({
  modelCalls: S.Number,
});
export interface HistoryRecallCost extends S.Schema.Type<typeof HistoryRecallCost> {}

export const HistoryRecallResponse = S.Struct({
  answers: S.Array(HistoryRecallSpan),
  honesty: HistoryRecallHonesty,
  cost: HistoryRecallCost,
});
export interface HistoryRecallResponse extends S.Schema.Type<typeof HistoryRecallResponse> {}

/**
 * Typed recall failure. Reasons:
 *
 * - `invalid_pattern` — a `Grep` pattern did not compile with `RegExp`.
 * - `corpus_unavailable` — the corpus provider failed to resolve a scope.
 *
 * There is deliberately no `cap_exceeded` failure: caps truncate and are
 * reported in `honesty.capsHit`, they never fail the request.
 */
export class HistoryRecallError extends S.TaggedErrorClass<HistoryRecallError>()(
  "HistoryRecall.Error",
  {
    reason: S.Literals(["invalid_pattern", "corpus_unavailable"]),
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}
