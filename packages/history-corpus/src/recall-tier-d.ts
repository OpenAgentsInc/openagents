import { Context, Effect, Layer } from "effect";

import type { HistoryCorpusEntry, HistoryCorpusError, HistoryCorpusScope } from "./corpus.ts";
import type { HistoryCorpusBuildResult } from "./builder.ts";
import {
  HistoryRecallError,
  historyRecallDefaultCaps,
  type HistoryRecallCapName,
  type HistoryRecallCaps,
  type HistoryRecallQuestion,
  type HistoryRecallRequest,
  type HistoryRecallResponse,
  type HistoryRecallSpan,
} from "./recall.ts";

/**
 * Tier D recall implementation (RLM-02, #9138): pure deterministic traversal
 * over an ordered corpus — zero model calls. Budget caps are enforced even
 * for deterministic work: the scan stops at `maxEntriesScanned`, spans stop
 * at `maxSpans`, excerpts are cut at `maxCharsPerSpan`, and every truncation
 * is reported in the response's `honesty` field. Caps never fail a request.
 */

export interface ResolvedHistoryRecallCaps {
  readonly maxSpans: number;
  readonly maxEntriesScanned: number;
  readonly maxCharsPerSpan: number;
}

/** Apply {@link historyRecallDefaultCaps} for absent cap fields. */
export const resolveHistoryRecallCaps = (
  caps?: HistoryRecallCaps | undefined,
): ResolvedHistoryRecallCaps => ({
  maxSpans: caps?.maxSpans ?? historyRecallDefaultCaps.maxSpans,
  maxEntriesScanned: caps?.maxEntriesScanned ?? historyRecallDefaultCaps.maxEntriesScanned,
  maxCharsPerSpan: caps?.maxCharsPerSpan ?? historyRecallDefaultCaps.maxCharsPerSpan,
});

const compareStrings = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0);

/** Bound one excerpt to `maxCharsPerSpan`, recording the cap when it cuts. */
const boundExcerpt = (
  text: string,
  caps: ResolvedHistoryRecallCaps,
  capsHit: Set<HistoryRecallCapName>,
): string => {
  if (text.length <= caps.maxCharsPerSpan) return text;
  capsHit.add("maxCharsPerSpan");
  return text.slice(0, caps.maxCharsPerSpan);
};

/** Excerpt source for one entry: its safe text, else its tool name, else empty. */
const entryExcerptSource = (entry: HistoryCorpusEntry): string =>
  entry.text ?? entry.toolName ?? "";

const entrySpan = (
  entry: HistoryCorpusEntry,
  caps: ResolvedHistoryRecallCaps,
  capsHit: Set<HistoryRecallCapName>,
): HistoryRecallSpan => ({
  scopeRef: entry.scopeRef,
  turnId: entry.turnId,
  sequenceStart: entry.sequence,
  sequenceEnd: entry.sequence,
  excerpt: boundExcerpt(entryExcerptSource(entry), caps, capsHit),
  kind: entry.kind,
});

interface ScanResult {
  readonly spans: ReadonlyArray<HistoryRecallSpan>;
  readonly entriesScanned: number;
}

/**
 * Bounded linear scan for entry-level questions: one span per matching
 * entry, in corpus order. Stops at `maxEntriesScanned` examined entries or
 * when a further match would exceed `maxSpans`, recording the cap either way.
 */
const scanEntryMatches = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
  caps: ResolvedHistoryRecallCaps,
  capsHit: Set<HistoryRecallCapName>,
  matches: (entry: HistoryCorpusEntry) => boolean,
): ScanResult => {
  const spans: Array<HistoryRecallSpan> = [];
  let scanned = 0;
  for (const entry of entries) {
    if (scanned >= caps.maxEntriesScanned) {
      capsHit.add("maxEntriesScanned");
      break;
    }
    scanned += 1;
    if (!matches(entry)) continue;
    if (spans.length >= caps.maxSpans) {
      capsHit.add("maxSpans");
      break;
    }
    spans.push(entrySpan(entry, caps, capsHit));
  }
  return { spans, entriesScanned: scanned };
};

/** Per-turn accumulator for `KeyTurns` and `TurnSummary`. */
interface TurnGroup {
  readonly scopeRef: string;
  readonly turnId: string;
  sequenceStart: number;
  sequenceEnd: number;
  entryCount: number;
  readonly countsByKind: Map<string, number>;
  readonly toolNames: Set<string>;
  firstKind: HistoryCorpusEntry["kind"];
  firstText: string | undefined;
  lastText: string | undefined;
}

/**
 * Bounded linear scan that groups entries by `(scopeRef, turnId)` in
 * first-seen corpus order. `accept` filters which turns accumulate; every
 * examined entry counts as scanned regardless.
 */
const scanTurnGroups = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
  caps: ResolvedHistoryRecallCaps,
  capsHit: Set<HistoryRecallCapName>,
  accept: (entry: HistoryCorpusEntry) => boolean,
): { readonly groups: ReadonlyArray<TurnGroup>; readonly entriesScanned: number } => {
  const groups = new Map<string, TurnGroup>();
  let scanned = 0;
  for (const entry of entries) {
    if (scanned >= caps.maxEntriesScanned) {
      capsHit.add("maxEntriesScanned");
      break;
    }
    scanned += 1;
    if (!accept(entry)) continue;
    const key = `${entry.scopeRef} ${entry.turnId}`;
    const group = groups.get(key);
    if (group === undefined) {
      groups.set(key, {
        scopeRef: entry.scopeRef,
        turnId: entry.turnId,
        sequenceStart: entry.sequence,
        sequenceEnd: entry.sequence,
        entryCount: 1,
        countsByKind: new Map([[entry.kind, 1]]),
        toolNames: new Set(entry.toolName === undefined ? [] : [entry.toolName]),
        firstKind: entry.kind,
        firstText: entry.text,
        lastText: entry.text,
      });
    } else {
      group.sequenceStart = Math.min(group.sequenceStart, entry.sequence);
      group.sequenceEnd = Math.max(group.sequenceEnd, entry.sequence);
      group.entryCount += 1;
      group.countsByKind.set(entry.kind, (group.countsByKind.get(entry.kind) ?? 0) + 1);
      if (entry.toolName !== undefined) group.toolNames.add(entry.toolName);
      if (group.firstText === undefined) group.firstText = entry.text;
      if (entry.text !== undefined) group.lastText = entry.text;
    }
  }
  return { groups: [...groups.values()], entriesScanned: scanned };
};

/** `KeyTurns` excerpt: the turn's first text, plus its last when distinct. */
const keyTurnExcerptSource = (group: TurnGroup): string => {
  if (group.firstText === undefined) return "";
  if (group.lastText === undefined || group.lastText === group.firstText) return group.firstText;
  return `${group.firstText} ... ${group.lastText}`;
};

/** `TurnSummary` excerpt: a deterministic structural record, sorted keys. */
const turnSummaryExcerptSource = (group: TurnGroup): string => {
  const kinds = [...group.countsByKind.entries()]
    .sort(([a], [b]) => compareStrings(a, b))
    .map(([kind, count]) => `${kind}=${count}`)
    .join(" ");
  const tools = [...group.toolNames].sort(compareStrings).join(" ");
  const parts = [
    `entries=${group.entryCount}`,
    `cursor=${group.sequenceStart}..${group.sequenceEnd}`,
    `kinds{${kinds}}`,
    `tools[${tools}]`,
  ];
  if (group.firstText !== undefined) parts.push(`first=${JSON.stringify(group.firstText)}`);
  if (group.lastText !== undefined) parts.push(`last=${JSON.stringify(group.lastText)}`);
  return parts.join(" ");
};

const groupSpans = (
  groups: ReadonlyArray<TurnGroup>,
  limit: number,
  caps: ResolvedHistoryRecallCaps,
  capsHit: Set<HistoryRecallCapName>,
  excerptSource: (group: TurnGroup) => string,
): ReadonlyArray<HistoryRecallSpan> => {
  const wanted = Math.min(limit, groups.length);
  const taken = Math.min(wanted, caps.maxSpans);
  if (taken < wanted) capsHit.add("maxSpans");
  return groups.slice(0, taken).map((group) => ({
    scopeRef: group.scopeRef,
    turnId: group.turnId,
    sequenceStart: group.sequenceStart,
    sequenceEnd: group.sequenceEnd,
    excerpt: boundExcerpt(excerptSource(group), caps, capsHit),
    kind: group.firstKind,
  }));
};

/** Safe `RegExp` compilation: an invalid pattern is a typed failure. */
const compileGrepPattern = (
  pattern: string,
  caseSensitive: boolean,
): RegExp | HistoryRecallError => {
  try {
    return new RegExp(pattern, caseSensitive ? "" : "i");
  } catch (cause) {
    return new HistoryRecallError({
      reason: "invalid_pattern",
      detail: `pattern did not compile: ${String(cause)}`,
      cause,
    });
  }
};

const answerQuestion = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
  question: HistoryRecallQuestion,
  caps: ResolvedHistoryRecallCaps,
  capsHit: Set<HistoryRecallCapName>,
): ScanResult | HistoryRecallError => {
  switch (question._tag) {
    case "Grep": {
      const regex = compileGrepPattern(question.pattern, question.caseSensitive === true);
      if (regex instanceof HistoryRecallError) return regex;
      return scanEntryMatches(
        entries,
        caps,
        capsHit,
        (entry) => entry.text !== undefined && regex.test(entry.text),
      );
    }
    case "CursorSlice":
      return scanEntryMatches(
        entries,
        caps,
        capsHit,
        (entry) =>
          (question.turnId === undefined || entry.turnId === question.turnId) &&
          entry.sequence >= question.fromSequence &&
          entry.sequence <= question.toSequence,
      );
    case "TimeSlice":
      return scanEntryMatches(
        entries,
        caps,
        capsHit,
        (entry) =>
          entry.observedAt >= question.fromObservedAt && entry.observedAt <= question.toObservedAt,
      );
    case "KeyTurns": {
      const { groups, entriesScanned } = scanTurnGroups(entries, caps, capsHit, () => true);
      return {
        spans: groupSpans(groups, question.limit, caps, capsHit, keyTurnExcerptSource),
        entriesScanned,
      };
    }
    case "TurnSummary": {
      const { groups, entriesScanned } = scanTurnGroups(
        entries,
        caps,
        capsHit,
        (entry) => entry.turnId === question.turnId,
      );
      return {
        spans: groupSpans(groups, groups.length, caps, capsHit, turnSummaryExcerptSource),
        entriesScanned,
      };
    }
  }
};

export interface RecallTierDInput {
  readonly entries: ReadonlyArray<HistoryCorpusEntry>;
  /** The corpus manifest coverage note, carried through to `honesty`. */
  readonly coverageNote: string;
  readonly question: HistoryRecallQuestion;
  readonly caps?: HistoryRecallCaps | undefined;
}

/**
 * Tier D recall over a corpus. PURE and deterministic: identical inputs give
 * an identical response, no clock, no randomness, and `cost.modelCalls` is
 * always 0. Fails only on invalid input (`invalid_pattern`) — caps truncate
 * and are reported in `honesty.capsHit`.
 */
export const recallTierD = (
  input: RecallTierDInput,
): Effect.Effect<HistoryRecallResponse, HistoryRecallError> =>
  Effect.suspend(() => {
    const caps = resolveHistoryRecallCaps(input.caps);
    const capsHit = new Set<HistoryRecallCapName>();
    const result = answerQuestion(input.entries, input.question, caps, capsHit);
    if (result instanceof HistoryRecallError) return Effect.fail(result);
    return Effect.succeed({
      answers: result.spans,
      honesty: {
        tier: "deterministic" as const,
        entriesScanned: result.entriesScanned,
        entriesTotal: input.entries.length,
        truncated: capsHit.size > 0,
        capsHit: [...capsHit].sort(compareStrings),
        coverageNote: input.coverageNote,
      },
      cost: { modelCalls: 0 },
    });
  });

export interface HistoryRecallShape {
  readonly recall: (
    request: HistoryRecallRequest,
  ) => Effect.Effect<HistoryRecallResponse, HistoryRecallError>;
}

/** The `HistoryRecall` service tag (audit §5.2 — one verb, `recall`). */
export class HistoryRecall extends Context.Service<HistoryRecall, HistoryRecallShape>()(
  "@openagentsinc/history-corpus/HistoryRecall",
) {}

/**
 * The corpus provider the Tier D layer resolves `Scope` requests through.
 * `buildHistoryCorpus` composed with the caller's sources satisfies it.
 */
export interface HistoryRecallCorpusProvider {
  readonly corpusForScope: (
    scope: HistoryCorpusScope,
  ) => Effect.Effect<HistoryCorpusBuildResult, HistoryCorpusError>;
}

/** Build the Tier D `HistoryRecallShape` from a corpus provider. */
export const makeHistoryRecallTierD = (
  provider: HistoryRecallCorpusProvider,
): HistoryRecallShape => ({
  recall: (request) => {
    const corpus =
      request.corpus._tag === "Corpus"
        ? Effect.succeed({
            entries: request.corpus.entries,
            coverageNote: request.corpus.manifest.coverage.note,
          })
        : provider.corpusForScope(request.corpus.scope).pipe(
            Effect.map((built) => ({
              entries: built.entries,
              coverageNote: built.manifest.coverage.note,
            })),
            Effect.mapError(
              (cause) =>
                new HistoryRecallError({
                  reason: "corpus_unavailable",
                  detail: `corpus provider failed: ${cause.operation}`,
                  cause,
                }),
            ),
          );
    return corpus.pipe(
      Effect.flatMap(({ entries, coverageNote }) =>
        recallTierD({ entries, coverageNote, question: request.question, caps: request.caps }),
      ),
    );
  },
});

/** Tier D `HistoryRecall` Layer over a corpus provider. */
export const historyRecallTierDLayer = (
  provider: HistoryRecallCorpusProvider,
): Layer.Layer<HistoryRecall> => Layer.succeed(HistoryRecall, makeHistoryRecallTierD(provider));
