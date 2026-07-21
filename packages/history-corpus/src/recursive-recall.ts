import { Duration, Effect, Option, Result, Schema as S } from "effect";
import { AiError, LanguageModel } from "effect/unstable/ai";
import type { HistoryCorpusEntry } from "./corpus.ts";

/**
 * Recursive recall engine (RLM-04, #9140) — Effect-native.
 *
 * This module reimplements the recursive-language-model recall loop from the
 * upstream `rlms` Python package IN EFFECT, per owner direction (no Python
 * runner, no sandbox-mounted interpreter, no `rlms` dependency). The upstream
 * reference read for loop semantics is
 * `alexzhang13/rlm` at commit `72d6940142ddfb84ee6be573dc999a37e633e671`
 * (`rlm/core/rlm.py`): a root model iterates over a mounted context, issues
 * bounded sub-calls, and on any cap hit prefers returning the best partial
 * answer over raising. The audit is
 * `docs/rlm/2026-07-21-rlm-integration-audit-and-roadmap.md` (§1.2 mechanics,
 * §1.4 cost honesty, §5.1 placement, §5.3 re-entry, §5.4 partial-answer UX).
 *
 * Differences from upstream, by design:
 *
 * - The paper's REPL (arbitrary Python execution over `context`) is replaced
 *   by a TYPED OPERATION vocabulary over `HistoryCorpusEntry` arrays — grep,
 *   cursor slice, time slice, turn summary, sub-call, answer. The root model
 *   emits exactly one JSON operation per iteration; the operation is decoded
 *   fail-closed with Effect Schema. An undecodable operation consumes an
 *   iteration and the model is asked again; repeated consecutive failures
 *   fail honestly as `contract_violation`.
 * - `Subcall` mirrors `rlm_query`/`llm_query`: it recurses over an explicit
 *   corpus span with a fresh bounded loop at depth + 1. Depth, sub-call
 *   count, tokens, and the deadline are GLOBAL budgets shared by the whole
 *   recursion tree.
 * - Usage accounting is exact-only: token counts come from the injected
 *   `LanguageModel` responses (`effect/unstable/ai`), never synthesized.
 *
 * NO live spend happens in this module's tests: the `LanguageModel` service
 * is injected, and the hermetic suite provides a scripted layer. An owner-run
 * live smoke (small corpus, a real provider layer, receipts recorded into the
 * usage ledger) is a separate owner-executed step, not part of this packet.
 */

export const RECURSIVE_RECALL_SCHEMA_ID = "openagents.history_corpus.recursive_recall.v1" as const;

// ---------------------------------------------------------------------------
// Caps — every run carries hard caps (audit §1.4: cost tails make uncapped
// execution unacceptable).
// ---------------------------------------------------------------------------

/**
 * Hard caps for one recall run. All caps are global across the recursion
 * tree, not per-depth. `maxDepth` defaults to 1 (root may issue plain
 * sub-calls; sub-calls may not recurse further), matching the paper default
 * and the audit §5.1 depth policy.
 */
export const RecursiveRecallCaps = S.Struct({
  /** Maximum recursion depth. Depth 0 = no sub-calls. Default 1. */
  maxDepth: S.optionalKey(S.Number),
  /** Maximum root-loop iterations per loop (root and each sub-loop). */
  maxIterations: S.Number,
  /** Wall-clock deadline for the whole run, enforced with `Effect.timeout`. */
  timeoutMs: S.Number,
  /** Maximum total tokens (input + output) across every model call. */
  maxTokens: S.Number,
  /** Maximum total sub-calls across the whole recursion tree. */
  maxSubcalls: S.Number,
});
export interface RecursiveRecallCaps extends S.Schema.Type<typeof RecursiveRecallCaps> {}

export const recursiveRecallDefaultMaxDepth = 1;

// ---------------------------------------------------------------------------
// Result contract — Completed | Partial | Failed, partial-answer honesty
// mandatory.
// ---------------------------------------------------------------------------

/** A citation into the corpus cursor scheme (`turnId` + `sequence` range). */
export const RecursiveRecallCitation = S.Struct({
  turnId: S.String,
  sequenceStart: S.Number,
  sequenceEnd: S.Number,
});
export interface RecursiveRecallCitation extends S.Schema.Type<typeof RecursiveRecallCitation> {}

/** Exact usage accounting for one run. Token counts come from the model. */
export const RecursiveRecallUsage = S.Struct({
  inputTokens: S.Number,
  outputTokens: S.Number,
  totalTokens: S.Number,
  subcalls: S.Number,
});
export interface RecursiveRecallUsage extends S.Schema.Type<typeof RecursiveRecallUsage> {}

export const recursiveRecallPartialReasons = [
  "timeout",
  "iteration_cap",
  "token_cap",
  "subcall_cap",
] as const;
export type RecursiveRecallPartialReason = (typeof recursiveRecallPartialReasons)[number];

export const recursiveRecallFailureClasses = [
  "runtime_unavailable",
  "contract_violation",
  "leaf_error",
] as const;
export type RecursiveRecallFailureClass = (typeof recursiveRecallFailureClasses)[number];

/**
 * The typed result union. `Partial` mirrors the upstream best-partial
 * behavior: on a cap hit the run returns whatever the buffers hold and says
 * so. `Failed` carries a bounded failure class, never a raw provider error.
 */
export const RecursiveRecallResult = S.Union([
  S.TaggedStruct("Completed", {
    answer: S.String,
    citations: S.Array(RecursiveRecallCitation),
    usage: RecursiveRecallUsage,
    iterations: S.Number,
    depthUsed: S.Number,
  }),
  S.TaggedStruct("Partial", {
    reason: S.Literals(recursiveRecallPartialReasons),
    bestAnswer: S.optionalKey(S.String),
    usage: RecursiveRecallUsage,
    iterations: S.Number,
    depthUsed: S.Number,
  }),
  S.TaggedStruct("Failed", {
    failureClass: S.Literals(recursiveRecallFailureClasses),
    detail: S.String,
    usage: RecursiveRecallUsage,
  }),
]);
export type RecursiveRecallResult = typeof RecursiveRecallResult.Type;

// ---------------------------------------------------------------------------
// Operation vocabulary — the typed replacement for the paper's REPL. The
// deterministic operations mirror the Tier-D traversal vocabulary (grep,
// cursor slice, time slice, turn summary) over `HistoryCorpusEntry` arrays.
// ---------------------------------------------------------------------------

/** An inclusive entry-index span over the ordered corpus array. */
export const RecursiveRecallSpan = S.Struct({
  startIndex: S.Number,
  endIndex: S.Number,
});
export interface RecursiveRecallSpan extends S.Schema.Type<typeof RecursiveRecallSpan> {}

/**
 * One operation per iteration. The model must emit exactly one JSON object
 * matching one of these shapes; anything else is an undecodable operation.
 */
export const RecursiveRecallOp = S.Union([
  /** Case-insensitive literal substring search over entry text + tool names. */
  S.TaggedStruct("Grep", {
    pattern: S.String,
    maxMatches: S.optionalKey(S.Number),
  }),
  /** Entries of one turn within an inclusive sequence range. */
  S.TaggedStruct("CursorSlice", {
    turnId: S.String,
    sequenceStart: S.Number,
    sequenceEnd: S.Number,
  }),
  /** Entries within an inclusive ISO-8601 `observedAt` range. */
  S.TaggedStruct("TimeSlice", {
    fromObservedAt: S.String,
    toObservedAt: S.String,
  }),
  /** Structural summary of one turn (kind counts, sequence range, snippet). */
  S.TaggedStruct("TurnSummary", { turnId: S.String }),
  /** Recurse over a corpus span with a fresh bounded loop at depth + 1. */
  S.TaggedStruct("Subcall", {
    question: S.String,
    span: RecursiveRecallSpan,
  }),
  /** Final answer with citations into the corpus cursor scheme. */
  S.TaggedStruct("Answer", {
    text: S.String,
    citations: S.Array(RecursiveRecallCitation),
  }),
]);
export type RecursiveRecallOp = typeof RecursiveRecallOp.Type;

const decodeOpExit = S.decodeUnknownExit(RecursiveRecallOp);

/**
 * Parse one model response into an operation. Fail-closed: code fences are
 * stripped, the whole remaining text must parse as JSON and decode as one
 * `RecursiveRecallOp`; anything else is `null`.
 */
export const parseRecursiveRecallOp = (text: string): RecursiveRecallOp | null => {
  let candidate = text.trim();
  const fenced = candidate.match(/^```[A-Za-z]*\n([\s\S]*?)\n?```$/);
  if (fenced?.[1] !== undefined) candidate = fenced[1].trim();
  let parsed: unknown;
  try {
    parsed = JSON.parse(candidate);
  } catch {
    return null;
  }
  const decoded = decodeOpExit(parsed);
  return decoded._tag === "Success" ? decoded.value : null;
};

// ---------------------------------------------------------------------------
// Deterministic operations over HistoryCorpusEntry arrays.
// ---------------------------------------------------------------------------

const defaultGrepMaxMatches = 20;
const snippetLength = 160;

const snippet = (text: string | undefined): string => {
  if (text === undefined) return "";
  const oneLine = text.replace(/\s+/g, " ").trim();
  return oneLine.length <= snippetLength ? oneLine : `${oneLine.slice(0, snippetLength)}…`;
};

/** Render one corpus entry as a bounded observation line. */
export const renderRecursiveRecallEntryLine = (index: number, entry: HistoryCorpusEntry): string => {
  const tool = entry.toolName === undefined ? "" : ` tool=${entry.toolName}`;
  return `[${index}] ${entry.turnId}#${entry.sequence} ${entry.kind}${tool} :: ${snippet(entry.text)}`;
};

/** Case-insensitive literal substring grep over entry text and tool names. */
export const grepRecursiveRecallCorpus = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
  pattern: string,
  maxMatches: number = defaultGrepMaxMatches,
): ReadonlyArray<{ readonly index: number; readonly entry: HistoryCorpusEntry }> => {
  const needle = pattern.toLowerCase();
  const bound = Math.max(0, Math.floor(maxMatches));
  const matches: Array<{ readonly index: number; readonly entry: HistoryCorpusEntry }> = [];
  if (needle.length === 0) return matches;
  for (let index = 0; index < entries.length && matches.length < bound; index++) {
    const entry = entries[index];
    if (entry === undefined) continue;
    const haystack = `${entry.text ?? ""} ${entry.toolName ?? ""}`.toLowerCase();
    if (haystack.includes(needle)) matches.push({ entry, index });
  }
  return matches;
};

/** Entries of one turn within an inclusive sequence range, with indexes. */
export const cursorSliceRecursiveRecallCorpus = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
  turnId: string,
  sequenceStart: number,
  sequenceEnd: number,
): ReadonlyArray<{ readonly index: number; readonly entry: HistoryCorpusEntry }> => {
  const matches: Array<{ readonly index: number; readonly entry: HistoryCorpusEntry }> = [];
  for (let index = 0; index < entries.length; index++) {
    const entry = entries[index];
    if (entry === undefined) continue;
    if (entry.turnId === turnId && entry.sequence >= sequenceStart && entry.sequence <= sequenceEnd) {
      matches.push({ entry, index });
    }
  }
  return matches;
};

/** Entries within an inclusive ISO-8601 `observedAt` range, with indexes. */
export const timeSliceRecursiveRecallCorpus = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
  fromObservedAt: string,
  toObservedAt: string,
): ReadonlyArray<{ readonly index: number; readonly entry: HistoryCorpusEntry }> => {
  const matches: Array<{ readonly index: number; readonly entry: HistoryCorpusEntry }> = [];
  for (let index = 0; index < entries.length; index++) {
    const entry = entries[index];
    if (entry === undefined) continue;
    if (entry.observedAt >= fromObservedAt && entry.observedAt <= toObservedAt) {
      matches.push({ entry, index });
    }
  }
  return matches;
};

export interface RecursiveRecallTurnSummary {
  readonly turnId: string;
  readonly entryCount: number;
  readonly sequenceStart: number | null;
  readonly sequenceEnd: number | null;
  readonly kindCounts: ReadonlyArray<readonly [kind: string, count: number]>;
  readonly firstTextSnippet: string;
}

/** Structural summary of one turn: counts per kind, sequence range, snippet. */
export const summarizeRecursiveRecallTurn = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
  turnId: string,
): RecursiveRecallTurnSummary => {
  const kindCounts = new Map<string, number>();
  let entryCount = 0;
  let sequenceStart: number | null = null;
  let sequenceEnd: number | null = null;
  let firstTextSnippet = "";
  for (const entry of entries) {
    if (entry.turnId !== turnId) continue;
    entryCount++;
    kindCounts.set(entry.kind, (kindCounts.get(entry.kind) ?? 0) + 1);
    if (sequenceStart === null || entry.sequence < sequenceStart) sequenceStart = entry.sequence;
    if (sequenceEnd === null || entry.sequence > sequenceEnd) sequenceEnd = entry.sequence;
    if (firstTextSnippet === "" && entry.text !== undefined && entry.text.trim() !== "") {
      firstTextSnippet = snippet(entry.text);
    }
  }
  return {
    entryCount,
    firstTextSnippet,
    kindCounts: [...kindCounts.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
    sequenceEnd,
    sequenceStart,
    turnId,
  };
};

// ---------------------------------------------------------------------------
// The engine.
// ---------------------------------------------------------------------------

export interface RunRecursiveRecallOptions {
  /** Ordered, cursor-addressed corpus entries (already policy-filtered). */
  readonly corpus: ReadonlyArray<HistoryCorpusEntry>;
  readonly question: string;
  readonly caps: RecursiveRecallCaps;
}

/**
 * Consecutive undecodable-operation bound. Each failure consumes an
 * iteration and re-prompts; this many failures IN A ROW fail the run as
 * `contract_violation` (repeated structural refusal, not a budget problem).
 */
export const recursiveRecallConsecutiveDecodeFailureLimit = 3;

interface MutableRunState {
  inputTokens: number;
  outputTokens: number;
  subcalls: number;
  depthUsed: number;
  rootIterations: number;
  bestAnswer: string | undefined;
}

const usageOf = (state: MutableRunState): RecursiveRecallUsage => ({
  inputTokens: state.inputTokens,
  outputTokens: state.outputTokens,
  subcalls: state.subcalls,
  totalTokens: state.inputTokens + state.outputTokens,
});

const partialOf = (
  state: MutableRunState,
  reason: RecursiveRecallPartialReason,
): RecursiveRecallResult => ({
  _tag: "Partial",
  ...(state.bestAnswer === undefined ? {} : { bestAnswer: state.bestAnswer }),
  depthUsed: state.depthUsed,
  iterations: state.rootIterations,
  reason,
  usage: usageOf(state),
});

const failedOf = (
  state: MutableRunState,
  failureClass: RecursiveRecallFailureClass,
  detail: string,
): RecursiveRecallResult => ({
  _tag: "Failed",
  detail: detail.slice(0, 300),
  failureClass,
  usage: usageOf(state),
});

/** Keep only citations that resolve to at least one real corpus entry. */
export const resolveRecursiveRecallCitations = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
  citations: ReadonlyArray<RecursiveRecallCitation>,
): ReadonlyArray<RecursiveRecallCitation> =>
  citations.filter(citation =>
    entries.some(
      entry =>
        entry.turnId === citation.turnId &&
        entry.sequence >= citation.sequenceStart &&
        entry.sequence <= citation.sequenceEnd,
    ),
  );

const describeCorpus = (entries: ReadonlyArray<HistoryCorpusEntry>): string => {
  if (entries.length === 0) return "The corpus is empty (0 entries).";
  const turnIds: Array<string> = [];
  for (const entry of entries) {
    if (turnIds.at(-1) !== entry.turnId) turnIds.push(entry.turnId);
  }
  const first = entries[0];
  const last = entries[entries.length - 1];
  return (
    `The corpus has ${entries.length} entries across ${turnIds.length} turn spans ` +
    `(first turn ${first?.turnId ?? "?"}, last turn ${last?.turnId ?? "?"}), ` +
    `observedAt ${first?.observedAt ?? "?"} .. ${last?.observedAt ?? "?"}.`
  );
};

const opInstructions =
  "Respond with EXACTLY ONE JSON object (no prose, no code fences) for one operation:\n" +
  '{"_tag":"Grep","pattern":"<substring>","maxMatches":20}\n' +
  '{"_tag":"CursorSlice","turnId":"<turnId>","sequenceStart":0,"sequenceEnd":0}\n' +
  '{"_tag":"TimeSlice","fromObservedAt":"<iso>","toObservedAt":"<iso>"}\n' +
  '{"_tag":"TurnSummary","turnId":"<turnId>"}\n' +
  '{"_tag":"Subcall","question":"<question>","span":{"startIndex":0,"endIndex":0}}\n' +
  '{"_tag":"Answer","text":"<final answer>","citations":[{"turnId":"<turnId>","sequenceStart":0,"sequenceEnd":0}]}';

const buildPrompt = (params: {
  readonly entries: ReadonlyArray<HistoryCorpusEntry>;
  readonly question: string;
  readonly depth: number;
  readonly maxDepth: number;
  readonly caps: RecursiveRecallCaps;
  readonly iteration: number;
  readonly subcallsUsed: number;
  readonly transcript: ReadonlyArray<string>;
}): string => {
  const lines: Array<string> = [
    "You are a recall engine over a conversation-history corpus.",
    describeCorpus(params.entries),
    opInstructions,
    `Question: ${params.question}`,
  ];
  if (params.transcript.length > 0) {
    lines.push("Prior operations and observations:", ...params.transcript);
  }
  lines.push(
    `Iteration ${params.iteration} of ${params.caps.maxIterations}. ` +
      `Depth ${params.depth} of ${params.maxDepth}. ` +
      `Subcalls used ${params.subcallsUsed} of ${params.caps.maxSubcalls}.`,
  );
  return lines.join("\n");
};

const renderMatches = (
  matches: ReadonlyArray<{ readonly index: number; readonly entry: HistoryCorpusEntry }>,
): string =>
  matches.length === 0
    ? "no matches"
    : matches.map(({ entry, index }) => renderRecursiveRecallEntryLine(index, entry)).join("\n");

const clampSpan = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
  span: RecursiveRecallSpan,
): ReadonlyArray<HistoryCorpusEntry> => {
  const start = Math.max(0, Math.floor(span.startIndex));
  const end = Math.min(entries.length - 1, Math.floor(span.endIndex));
  return start > end ? [] : entries.slice(start, end + 1);
};

/**
 * Run the bounded recursive recall loop.
 *
 * The error channel is `never`: every failure mode is a typed
 * `RecursiveRecallResult` (`Partial` on a cap hit with best-partial honesty,
 * `Failed` with a bounded failure class otherwise). Requires the
 * `LanguageModel` service from `effect/unstable/ai`; hermetic tests inject a
 * scripted layer, live use injects a real provider layer.
 *
 * Cap enforcement points, all inside this loop:
 * - `timeoutMs` — `Effect.timeoutOption` around the whole recursion.
 * - `maxIterations` — per loop (root and each sub-loop).
 * - `maxTokens` — checked after every model call, from exact response usage.
 * - `maxSubcalls` — global across the recursion tree, checked at issue time.
 * - `maxDepth` — a too-deep `Subcall` is refused (consumes the iteration).
 */
export const runRecursiveRecall = (
  options: RunRecursiveRecallOptions,
): Effect.Effect<RecursiveRecallResult, never, LanguageModel.LanguageModel> => {
  const maxDepth = options.caps.maxDepth ?? recursiveRecallDefaultMaxDepth;
  const state: MutableRunState = {
    bestAnswer: undefined,
    depthUsed: 0,
    inputTokens: 0,
    outputTokens: 0,
    rootIterations: 0,
    subcalls: 0,
  };

  const loop = (
    entries: ReadonlyArray<HistoryCorpusEntry>,
    question: string,
    depth: number,
  ): Effect.Effect<RecursiveRecallResult, never, LanguageModel.LanguageModel> =>
    Effect.gen(function* () {
      const transcript: Array<string> = [];
      let consecutiveDecodeFailures = 0;
      let iterations = 0;

      for (let iteration = 1; iteration <= options.caps.maxIterations; iteration++) {
        iterations = iteration;
        if (depth === 0) state.rootIterations = iteration;

        const prompt = buildPrompt({
          caps: options.caps,
          depth,
          entries,
          iteration,
          maxDepth,
          question,
          subcallsUsed: state.subcalls,
          transcript,
        });

        const outcome = yield* Effect.result(LanguageModel.generateText({ prompt }));
        if (Result.isFailure(outcome)) {
          return failedOf(
            state,
            "runtime_unavailable",
            AiError.isAiError(outcome.failure) ? outcome.failure.message : String(outcome.failure),
          );
        }
        const response = outcome.success;

        state.inputTokens += response.usage.inputTokens.total ?? 0;
        state.outputTokens += response.usage.outputTokens.total ?? 0;
        if (state.inputTokens + state.outputTokens > options.caps.maxTokens) {
          return partialOf(state, "token_cap");
        }

        const text = response.text;
        const op = parseRecursiveRecallOp(text);
        if (op === null) {
          consecutiveDecodeFailures++;
          if (text.trim() !== "") state.bestAnswer = text.trim();
          if (consecutiveDecodeFailures >= recursiveRecallConsecutiveDecodeFailureLimit) {
            return failedOf(
              state,
              "contract_violation",
              `undecodable operation ${consecutiveDecodeFailures} times in a row`,
            );
          }
          transcript.push(
            `iteration ${iteration}: undecodable operation; respond with exactly one JSON operation object.`,
          );
          continue;
        }
        consecutiveDecodeFailures = 0;

        switch (op._tag) {
          case "Answer": {
            return {
              _tag: "Completed",
              answer: op.text,
              citations: resolveRecursiveRecallCitations(entries, op.citations),
              depthUsed: state.depthUsed,
              iterations,
              usage: usageOf(state),
            };
          }
          case "Grep": {
            const matches = grepRecursiveRecallCorpus(
              entries,
              op.pattern,
              op.maxMatches ?? defaultGrepMaxMatches,
            );
            transcript.push(
              `iteration ${iteration}: Grep(${JSON.stringify(op.pattern)}) ->\n${renderMatches(matches)}`,
            );
            continue;
          }
          case "CursorSlice": {
            const matches = cursorSliceRecursiveRecallCorpus(
              entries,
              op.turnId,
              op.sequenceStart,
              op.sequenceEnd,
            );
            transcript.push(
              `iteration ${iteration}: CursorSlice(${op.turnId}#${op.sequenceStart}..${op.sequenceEnd}) ->\n${renderMatches(matches)}`,
            );
            continue;
          }
          case "TimeSlice": {
            const matches = timeSliceRecursiveRecallCorpus(
              entries,
              op.fromObservedAt,
              op.toObservedAt,
            );
            transcript.push(
              `iteration ${iteration}: TimeSlice(${op.fromObservedAt}..${op.toObservedAt}) ->\n${renderMatches(matches)}`,
            );
            continue;
          }
          case "TurnSummary": {
            const summary = summarizeRecursiveRecallTurn(entries, op.turnId);
            transcript.push(
              `iteration ${iteration}: TurnSummary(${op.turnId}) -> entries=${summary.entryCount} ` +
                `sequences=${summary.sequenceStart ?? "-"}..${summary.sequenceEnd ?? "-"} ` +
                `kinds=${summary.kindCounts.map(([kind, count]) => `${kind}:${count}`).join(",")} ` +
                `first=${JSON.stringify(summary.firstTextSnippet)}`,
            );
            continue;
          }
          case "Subcall": {
            if (depth + 1 > maxDepth) {
              transcript.push(
                `iteration ${iteration}: Subcall refused — depth cap reached (maxDepth=${maxDepth}).`,
              );
              continue;
            }
            if (state.subcalls >= options.caps.maxSubcalls) {
              return partialOf(state, "subcall_cap");
            }
            state.subcalls++;
            state.depthUsed = Math.max(state.depthUsed, depth + 1);
            const child = yield* loop(clampSpan(entries, op.span), op.question, depth + 1);
            if (child._tag === "Completed") {
              state.bestAnswer = child.answer;
              transcript.push(
                `iteration ${iteration}: Subcall(${JSON.stringify(op.question)}) -> answer=${JSON.stringify(child.answer)} ` +
                  `citations=${JSON.stringify(child.citations)}`,
              );
              continue;
            }
            if (child._tag === "Partial" && (child.reason === "token_cap" || child.reason === "subcall_cap")) {
              // A GLOBAL budget was exhausted inside the child — stop honestly.
              return child;
            }
            if (child._tag === "Failed") {
              // A structural child failure (contract violation, model outage)
              // fails the run; masking it as an observation would launder it.
              return child;
            }
            transcript.push(
              `iteration ${iteration}: Subcall(${JSON.stringify(op.question)}) -> partial (${child.reason})` +
                (child.bestAnswer === undefined ? "" : ` best=${JSON.stringify(child.bestAnswer)}`),
            );
            continue;
          }
        }
      }

      return partialOf(state, "iteration_cap");
    });

  const run = loop(options.corpus, options.question, 0).pipe(
    Effect.timeoutOption(Duration.millis(options.caps.timeoutMs)),
    Effect.map(outcome =>
      Option.isSome(outcome) ? outcome.value : partialOf(state, "timeout"),
    ),
  );

  return run.pipe(
    Effect.catchDefect(defect => Effect.succeed(failedOf(state, "leaf_error", String(defect)))),
  );
};
