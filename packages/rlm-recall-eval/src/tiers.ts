/**
 * Tier runners for the dense-recall matrix.
 *
 * Tier D and every Tier S variant run through the PUBLISHED
 * `@openagentsinc/rlm` engine (`makeRlm`, the inline corpus source Layer,
 * `defaultRlmBudget`, the program/result Schemas). No eval-side engine is
 * forked. The `direct`, `bounded_window`, and `provider_compaction` baselines
 * are deliberately NON-engine strategies built for comparison; they hold the
 * same fixed reader capability so the contrast is retrieval strategy, not model
 * IQ.
 *
 * Tier semantics mirror the desktop Tier S consumer
 * (`apps/openagents-desktop/src/history-recall-semantic.ts`): Tier D is the
 * free deterministic default; Tier S is admitted, depth-bounded, and citation
 * gated.
 */

import { Effect } from "effect";
import {
  defaultRlmBudget,
  defaultRlmDeterministicLimits,
  makeRlm,
  RLM_REQUEST_SCHEMA_ID,
  rlmInlineCorpusSourceLayer,
  type RlmBudget,
  type RlmCitation,
  type RlmError,
  type RlmTerminalResult,
} from "@openagentsinc/rlm";

import { priceUsage, type CostResult } from "./price-catalog.ts";
import {
  makeScriptedModel,
  newUsageSink,
  readerAnswer,
  sinkCompleteness,
  UNKNOWN_ANSWER,
  modeledTokens,
  type UsageSink,
} from "./scripted-models.ts";
import {
  answerContains,
  classifyOutcome,
  scoreCitations,
  type TierId,
  type TierRunResult,
} from "./scoring.ts";
import type { PlantedQuestion, SyntheticTranscript } from "./transcripts.ts";

/** Modeled sequential latency per model call (scripted units, not wall-clock). */
export const LATENCY_UNITS_PER_CALL = 1000;

/** Bounded-window baseline: only the last N turns are visible. */
export const BOUNDED_WINDOW_SIZE = 32;

/** Provider-compaction baseline: last N turns verbatim, older turns compacted. */
export const COMPACTION_VERBATIM_TAIL = 16;

/** Strategy profile pin recorded in every scripted run. */
export const EVAL_STRATEGY_REF = "openagents.rlm-recall-eval.scripted.v1" as const;

export interface TierRunOptions {
  readonly transcript: SyntheticTranscript;
  readonly question: PlantedQuestion;
  /** Scored model id from the versioned price catalog. */
  readonly modelId: string;
  /** When false, model calls omit usage (exercises the honesty path). */
  readonly reportUsage?: boolean;
}

// ---------------------------------------------------------------------------
// Shared helpers.
// ---------------------------------------------------------------------------

const citationsOf = (result: RlmTerminalResult): ReadonlyArray<RlmCitation> =>
  "citations" in result ? result.citations : [];

const citedRefsOf = (citations: ReadonlyArray<RlmCitation>): ReadonlyArray<string> => {
  const refs: Array<string> = [];
  for (const c of citations) {
    refs.push(c.entryRefStart);
    if (c.entryRefEnd !== undefined) refs.push(c.entryRefEnd);
  }
  return refs;
};

const usageFromSink = (
  sink: UsageSink,
): {
  inputTokens: number | null;
  outputTokens: number | null;
  completeness: "complete" | "partial" | "unavailable";
} => {
  const completeness = sinkCompleteness(sink);
  if (completeness !== "complete") {
    return { inputTokens: null, outputTokens: null, completeness };
  }
  return { inputTokens: sink.inputTokens, outputTokens: sink.outputTokens, completeness };
};

// ---------------------------------------------------------------------------
// Tier D — deterministic grep through the engine (no model, no spend).
// ---------------------------------------------------------------------------

const deterministicEvidence = (
  result: RlmTerminalResult,
): { evidence: string; citedRefs: ReadonlyArray<string>; findingCount: number } => {
  const output =
    result._tag === "Completed"
      ? result.output
      : result._tag === "Partial"
        ? result.bestOutput
        : undefined;
  if (output === undefined || output._tag !== "DeterministicFindings") {
    return { evidence: "", citedRefs: citedRefsOf(citationsOf(result)), findingCount: 0 };
  }
  const evidence = output.findings.map((f) => f.excerpt).join("\n");
  const citedRefs = citedRefsOf(citationsOf(result));
  return { evidence, citedRefs, findingCount: output.findings.length };
};

export const runTierD = (options: TierRunOptions): Effect.Effect<TierRunResult, never> => {
  const { transcript, question } = options;
  const tierId: TierId = "tier_d";
  return Effect.gen(function* () {
    const shape = yield* makeRlm({ admitSemantic: false, model: { refuseSemantic: true } });
    const result = yield* shape.run({
      _tag: "Deterministic",
      schemaId: RLM_REQUEST_SCHEMA_ID,
      runRef: `${tierId}:${question.questionId}`,
      corpus: transcript.corpusInput,
      operation: { _tag: "Grep", pattern: question.grepPattern },
      limits: defaultRlmDeterministicLimits,
    });
    const { evidence, citedRefs } = deterministicEvidence(result);
    const produced = readerAnswer(question.family, question.grepPattern, evidence);
    const abstained = produced === UNKNOWN_ANSWER;
    const contains = answerContains(question.family, produced, question.expectedAnswer);
    const bothSurfaced =
      question.family === "pair" &&
      question.expectedAnswer.split("|").every((v) => evidence.includes(v));
    // Tier D is retrieval-only: it never SYNTHESISES a combined answer.
    const outcome = classifyOutcome({
      family: question.family,
      answerContainsExpected: contains,
      synthesized: false,
      abstained,
      bothPairValuesSurfaced: bothSurfaced,
    });
    const citation = scoreCitations(citedRefs, question.expectedEntryRefs);
    const cost: CostResult = priceUsage(options.modelId, {
      inputTokens: 0,
      outputTokens: 0,
      completeness: "complete",
    });
    return {
      tierId,
      transcriptId: transcript.transcriptId,
      questionId: question.questionId,
      family: question.family,
      historySize: transcript.historySize,
      outcome,
      producedAnswer: produced,
      answerContainsExpected: contains,
      citation,
      modelCalls: 0,
      subcalls: 0,
      tokenInput: 0,
      tokenOutput: 0,
      tokenCompleteness: "complete",
      cost,
      modeledLatencyUnits: 0,
      capsHit: result.honesty?.capsHit ?? [],
      note: "deterministic grep; zero model tokens",
    } satisfies TierRunResult;
  }).pipe(
    Effect.provide(rlmInlineCorpusSourceLayer),
    Effect.catch((error: RlmError) => Effect.succeed(engineFailure(tierId, options, error))),
  );
};

// ---------------------------------------------------------------------------
// Tier S — semantic RLM programs through the engine.
// ---------------------------------------------------------------------------

type ProgramNode = Record<string, unknown>;

const grepNode: ProgramNode = {
  _tag: "CorpusOp",
  nodeRef: "n.grep",
  operator: "Grep",
  params: {},
  inputValueRefs: [],
  outputValueRef: "v.hits",
};

const grepWith = (pattern: string): ProgramNode => ({
  ...grepNode,
  params: { pattern },
});

const programDepth0 = (pattern: string): string =>
  JSON.stringify({
    schemaId: "openagents.ai.rlm_program.v1",
    programRef: "prog.depth0",
    nodes: [
      grepWith(pattern),
      { _tag: "Commit", nodeRef: "n.commit", valueRef: "v.hits", citationValueRefs: [] },
    ],
  });

const programModelMap = (pattern: string): string =>
  JSON.stringify({
    schemaId: "openagents.ai.rlm_program.v1",
    programRef: "prog.modelmap",
    nodes: [
      grepWith(pattern),
      {
        _tag: "Partition",
        nodeRef: "n.part",
        inputValueRef: "v.hits",
        partCount: 2,
        outputValueRef: "v.parts",
      },
      {
        _tag: "ModelMap",
        nodeRef: "n.map",
        inputCollectionRef: "v.parts",
        promptTemplate: "Extract the recorded answer from this evidence: {{item}}",
        outputValueRef: "v.mapped",
        maxConcurrency: 4,
      },
      {
        _tag: "Transform",
        nodeRef: "n.join",
        operator: "TransformJoinText",
        params: {},
        inputValueRefs: ["v.mapped"],
        outputValueRef: "v.joined",
      },
      { _tag: "Commit", nodeRef: "n.commit", valueRef: "v.joined", citationValueRefs: ["v.hits"] },
    ],
  });

const programRlmMap = (pattern: string): string =>
  JSON.stringify({
    schemaId: "openagents.ai.rlm_program.v1",
    programRef: "prog.rlmmap",
    nodes: [
      grepWith(pattern),
      {
        _tag: "RlmMap",
        nodeRef: "n.rlm",
        inputCollectionRef: "v.hits",
        questionTemplate: "Extract the recorded answer from this evidence: {{item}}",
        outputValueRef: "v.child",
        maxConcurrency: 2,
      },
      {
        _tag: "Transform",
        nodeRef: "n.join",
        operator: "TransformJoinText",
        params: {},
        inputValueRefs: ["v.child"],
        outputValueRef: "v.joined",
      },
      { _tag: "Commit", nodeRef: "n.commit", valueRef: "v.joined", citationValueRefs: ["v.hits"] },
    ],
  });

interface SemanticVariant {
  readonly tierId: TierId;
  readonly programJson: string;
  readonly synthesized: boolean;
  readonly maxDepth: number;
  readonly note?: string;
}

const semanticVariants = (question: PlantedQuestion): ReadonlyArray<SemanticVariant> => [
  {
    tierId: "semantic_depth0",
    programJson: programDepth0(question.grepPattern),
    synthesized: false,
    maxDepth: 1,
    note: "symbolic environment, zero subcalls",
  },
  {
    tierId: "semantic_modelmap",
    programJson: programModelMap(question.grepPattern),
    synthesized: true,
    maxDepth: 1,
    note: "one-shot ModelMap fan-out; depth unchanged",
  },
  {
    tierId: "semantic_depth1",
    programJson: programRlmMap(question.grepPattern),
    synthesized: true,
    maxDepth: 1,
  },
  {
    tierId: "semantic_depth2",
    programJson: programRlmMap(question.grepPattern),
    synthesized: true,
    maxDepth: 2,
    note: "separately admitted higher depth; SDK 0.2.1-rc.2 recursion is single-level, so depth>1 is MODELED, not truly nested",
  },
];

/** Run the one-shot ModelMap semantic variant on its own (used by the honesty probe). */
export const runSemanticModelMap = (options: TierRunOptions): Effect.Effect<TierRunResult, never> =>
  runSemanticVariant(
    {
      tierId: "semantic_modelmap",
      programJson: programModelMap(options.question.grepPattern),
      synthesized: true,
      maxDepth: 1,
    },
    options,
  );

const runSemanticVariant = (
  variant: SemanticVariant,
  options: TierRunOptions,
): Effect.Effect<TierRunResult, never> => {
  const { transcript, question } = options;
  const sink = newUsageSink();
  const budget: RlmBudget = {
    ...defaultRlmBudget,
    maxDepth: variant.maxDepth,
    requireExactUsage: false,
  };
  const model = makeScriptedModel(
    { programJson: variant.programJson },
    {
      question,
      sink,
      reportUsage: options.reportUsage ?? true,
      strategyRef: EVAL_STRATEGY_REF,
    },
  );
  return Effect.gen(function* () {
    const shape = yield* makeRlm({ admitSemantic: true, model });
    const result = yield* shape.run({
      _tag: "Semantic",
      schemaId: RLM_REQUEST_SCHEMA_ID,
      runRef: `${variant.tierId}:${question.questionId}`,
      corpus: transcript.corpusInput,
      question: question.text,
      budget,
      evidence: {
        requireCitations: true,
        minimumCitations: 1,
        invalidCitation: "partial",
        requireCompleteCorpusCoverage: false,
      },
    });
    return scoreSemantic(variant, options, result, sink);
  }).pipe(
    Effect.provide(rlmInlineCorpusSourceLayer),
    Effect.catch((error: RlmError) =>
      Effect.succeed(engineFailure(variant.tierId, options, error, sink)),
    ),
  );
};

const scoreSemantic = (
  variant: SemanticVariant,
  options: TierRunOptions,
  result: RlmTerminalResult,
  sink: UsageSink,
): TierRunResult => {
  const { transcript, question } = options;
  const committedText =
    result._tag === "Completed"
      ? result.output._tag === "InlineValue"
        ? result.output.value
        : ""
      : result._tag === "Partial" && result.bestOutput?._tag === "InlineValue"
        ? result.bestOutput.value
        : "";
  const citedRefs = citedRefsOf(citationsOf(result));
  const contains = answerContains(question.family, committedText, question.expectedAnswer);
  const abstained = result._tag === "Refused";
  const bothSurfaced =
    question.family === "pair" &&
    question.expectedAnswer.split("|").every((v) => committedText.includes(v));
  const outcome = classifyOutcome({
    family: question.family,
    answerContainsExpected: contains,
    synthesized: variant.synthesized,
    abstained,
    bothPairValuesSurfaced: bothSurfaced,
  });
  const modelCalls = result.usage?.modelCalls ?? 0;
  const subcalls = result.usage?.subcalls ?? 0;
  const usage = usageFromSink(sink);
  const cost = priceUsage(options.modelId, usage);
  const note = variant.note;
  return {
    tierId: variant.tierId,
    transcriptId: transcript.transcriptId,
    questionId: question.questionId,
    family: question.family,
    historySize: transcript.historySize,
    outcome,
    producedAnswer: committedText.slice(0, 512),
    answerContainsExpected: contains,
    citation: scoreCitations(citedRefs, question.expectedEntryRefs),
    modelCalls,
    subcalls,
    tokenInput: usage.inputTokens,
    tokenOutput: usage.outputTokens,
    tokenCompleteness: usage.completeness,
    cost,
    modeledLatencyUnits: (modelCalls + subcalls) * LATENCY_UNITS_PER_CALL,
    capsHit: result.honesty?.capsHit ?? [],
    ...(note !== undefined ? { note } : {}),
  } satisfies TierRunResult;
};

const engineFailure = (
  tierId: TierId,
  options: TierRunOptions,
  error: RlmError,
  sink?: UsageSink,
): TierRunResult => {
  const usage = sink
    ? usageFromSink(sink)
    : { inputTokens: 0, outputTokens: 0, completeness: "complete" as const };
  return {
    tierId,
    transcriptId: options.transcript.transcriptId,
    questionId: options.question.questionId,
    family: options.question.family,
    historySize: options.transcript.historySize,
    outcome: "refused",
    producedAnswer: UNKNOWN_ANSWER,
    answerContainsExpected: false,
    citation: scoreCitations([], options.question.expectedEntryRefs),
    modelCalls: 0,
    subcalls: 0,
    tokenInput: usage.inputTokens,
    tokenOutput: usage.outputTokens,
    tokenCompleteness: usage.completeness,
    cost: priceUsage(options.modelId, usage),
    modeledLatencyUnits: 0,
    capsHit: [],
    note: `engine refused: ${error.reason}`,
  };
};

// ---------------------------------------------------------------------------
// Non-engine baselines: direct, bounded window, provider compaction.
// ---------------------------------------------------------------------------

interface BaselineOutcome {
  readonly prompt: string;
  readonly abstainedNoCall: boolean;
  readonly note: string;
}

const scoreBaseline = (
  tierId: TierId,
  options: TierRunOptions,
  b: BaselineOutcome,
): TierRunResult => {
  const { transcript, question } = options;
  const reportUsage = options.reportUsage ?? true;
  const produced = b.abstainedNoCall
    ? UNKNOWN_ANSWER
    : readerAnswer(question.family, question.grepPattern, b.prompt);
  const abstained = produced === UNKNOWN_ANSWER;
  const contains = answerContains(question.family, produced, question.expectedAnswer);
  const bothSurfaced =
    question.family === "pair" &&
    !b.abstainedNoCall &&
    question.expectedAnswer.split("|").every((v) => b.prompt.includes(v));
  const outcome = classifyOutcome({
    family: question.family,
    answerContainsExpected: contains,
    synthesized: true,
    abstained,
    bothPairValuesSurfaced: bothSurfaced,
  });
  // One model call unless the call was never issued (per-call limit exceeded).
  const modelCalls = b.abstainedNoCall ? 0 : 1;
  const completeness: "complete" | "partial" | "unavailable" =
    modelCalls === 0 ? "complete" : reportUsage ? "complete" : "unavailable";
  const tokenInput = modelCalls === 0 ? 0 : reportUsage ? modeledTokens(b.prompt) : null;
  const tokenOutput = modelCalls === 0 ? 0 : reportUsage ? modeledTokens(produced) : null;
  const cost = priceUsage(options.modelId, {
    inputTokens: tokenInput,
    outputTokens: tokenOutput,
    completeness,
  });
  return {
    tierId,
    transcriptId: transcript.transcriptId,
    questionId: question.questionId,
    family: question.family,
    historySize: transcript.historySize,
    outcome,
    producedAnswer: produced,
    answerContainsExpected: contains,
    // Baselines emit prose, not exact digest-anchored citations.
    citation: scoreCitations([], question.expectedEntryRefs),
    modelCalls,
    subcalls: 0,
    tokenInput,
    tokenOutput,
    tokenCompleteness: completeness,
    cost,
    modeledLatencyUnits: modelCalls * LATENCY_UNITS_PER_CALL,
    capsHit: b.abstainedNoCall ? ["maxPromptTokensPerCall"] : [],
    note: b.note,
  };
};

export const runDirect = (options: TierRunOptions): Effect.Effect<TierRunResult, never> => {
  const { transcript } = options;
  const headroom = defaultRlmBudget.maxPromptTokensPerCall;
  const overHeadroom = transcript.approxTotalTokens > headroom;
  const prompt = overHeadroom ? "" : transcript.entries.map((e) => e.text ?? "").join("\n");
  return Effect.succeed(
    scoreBaseline("direct", options, {
      prompt,
      abstainedNoCall: overHeadroom,
      note: overHeadroom
        ? "per-call prompt headroom exceeded; whole corpus does not fit one call"
        : "whole corpus placed in a single model call",
    }),
  );
};

export const runBoundedWindow = (options: TierRunOptions): Effect.Effect<TierRunResult, never> => {
  const { transcript } = options;
  const start = Math.max(0, transcript.entries.length - BOUNDED_WINDOW_SIZE);
  const prompt = transcript.entries
    .slice(start)
    .map((e) => e.text ?? "")
    .join("\n");
  return Effect.succeed(
    scoreBaseline("bounded_window", options, {
      prompt,
      abstainedNoCall: false,
      note: `last ${String(BOUNDED_WINDOW_SIZE)} turns visible only`,
    }),
  );
};

export const runProviderCompaction = (
  options: TierRunOptions,
): Effect.Effect<TierRunResult, never> => {
  const { transcript } = options;
  const cut = Math.max(0, transcript.entries.length - COMPACTION_VERBATIM_TAIL);
  // Older turns are lossily compacted: their precise locators are DROPPED.
  const compacted = transcript.entries
    .slice(0, cut)
    .map((_e, i) => `Turn ${String(i)}: (compacted summary; precise details omitted)`)
    .join("\n");
  const verbatim = transcript.entries
    .slice(cut)
    .map((e) => e.text ?? "")
    .join("\n");
  return Effect.succeed(
    scoreBaseline("provider_compaction", options, {
      prompt: `${compacted}\n${verbatim}`,
      abstainedNoCall: false,
      note: `provider compaction: last ${String(COMPACTION_VERBATIM_TAIL)} turns verbatim, older turns summarised lossily`,
    }),
  );
};

// ---------------------------------------------------------------------------
// Run every tier for one question.
// ---------------------------------------------------------------------------

export const runAllTiersForQuestion = (
  options: TierRunOptions,
): Effect.Effect<ReadonlyArray<TierRunResult>, never> =>
  Effect.gen(function* () {
    const results: Array<TierRunResult> = [];
    results.push(yield* runDirect(options));
    results.push(yield* runTierD(options));
    for (const variant of semanticVariants(options.question)) {
      results.push(yield* runSemanticVariant(variant, options));
    }
    results.push(yield* runBoundedWindow(options));
    results.push(yield* runProviderCompaction(options));
    return results;
  });
