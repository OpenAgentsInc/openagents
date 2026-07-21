/**
 * Pure scoring and aggregation for the dense-recall matrix.
 *
 * Scores answer correctness, citation exactness/coverage, abstention honesty,
 * modeled latency, model-call distribution, token distribution, and cost. All
 * functions are deterministic; distributions report p50/p75/p90/p95/p99 and are
 * stratified by outcome, because (PAPER-AUDIT §5) means alone hide the sharp
 * tails and the divergence between successful and failed trajectories.
 */

import type { CostResult } from "./price-catalog.ts";
import type { DensityFamily } from "./transcripts.ts";

export type TierId =
  | "direct"
  | "tier_d"
  | "semantic_depth0"
  | "semantic_modelmap"
  | "semantic_depth1"
  | "semantic_depth2"
  | "bounded_window"
  | "provider_compaction";

export type Outcome = "success" | "incorrect" | "partial" | "refused";

export interface CitationScore {
  readonly cited: number;
  readonly expected: number;
  readonly matched: number;
  /** Fraction of expected refs that were cited (recall over ground truth). */
  readonly coverage: number;
  /** Fraction of cited refs that are exact ground-truth matches (precision). */
  readonly exactness: number;
}

export interface TierRunResult {
  readonly tierId: TierId;
  readonly transcriptId: string;
  readonly questionId: string;
  readonly family: DensityFamily;
  readonly historySize: number;
  readonly outcome: Outcome;
  readonly producedAnswer: string;
  readonly answerContainsExpected: boolean;
  readonly citation: CitationScore;
  readonly modelCalls: number;
  readonly subcalls: number;
  readonly tokenInput: number | null;
  readonly tokenOutput: number | null;
  readonly tokenCompleteness: "complete" | "partial" | "unavailable";
  readonly cost: CostResult;
  readonly modeledLatencyUnits: number;
  /** Caps hit reported by the RLM engine honesty channel (Tier D / Tier S). */
  readonly capsHit: ReadonlyArray<string>;
  /** Free-form, public-safe note (e.g. why a baseline abstained). */
  readonly note?: string;
}

/** Does the produced answer contain the full expected answer? */
export const answerContains = (
  family: DensityFamily,
  produced: string,
  expected: string,
): boolean => {
  if (family === "pair") {
    return expected.split("|").every((part) => produced.includes(part));
  }
  return produced.includes(expected);
};

export interface OutcomeInput {
  readonly family: DensityFamily;
  readonly answerContainsExpected: boolean;
  /** A leaf/reduce model call combined the evidence into the answer. */
  readonly synthesized: boolean;
  /** The reader could not produce an answer (window miss, per-call limit, ...). */
  readonly abstained: boolean;
  /** For pair tasks: both conflicting values were surfaced as evidence. */
  readonly bothPairValuesSurfaced: boolean;
}

/**
 * Classify an outcome. Honest abstention (`abstained`) is never scored as a
 * wrong answer. Pair (conflict) tasks require SYNTHESIS: a retrieval-only tier
 * that surfaces both spans without combining them is `partial`, not `success`.
 */
export const classifyOutcome = (input: OutcomeInput): Outcome => {
  if (input.abstained) return "refused";
  if (input.family !== "pair") {
    return input.answerContainsExpected ? "success" : "incorrect";
  }
  if (input.answerContainsExpected) {
    return input.synthesized ? "success" : "partial";
  }
  return input.bothPairValuesSurfaced ? "partial" : "incorrect";
};

/** Score citations against ground-truth expected refs. */
export const scoreCitations = (
  citedEntryRefs: ReadonlyArray<string>,
  expectedEntryRefs: ReadonlyArray<string>,
): CitationScore => {
  const cited = new Set(citedEntryRefs);
  const expected = new Set(expectedEntryRefs);
  let matched = 0;
  for (const ref of expected) {
    if (cited.has(ref)) matched += 1;
  }
  const coverage = expected.size === 0 ? 1 : matched / expected.size;
  const exactness = cited.size === 0 ? (expected.size === 0 ? 1 : 0) : matched / cited.size;
  return { cited: cited.size, expected: expected.size, matched, coverage, exactness };
};

// ---------------------------------------------------------------------------
// Distributions and aggregation.
// ---------------------------------------------------------------------------

/** Nearest-rank percentile over an already-sorted ascending array. */
const percentileSorted = (sorted: ReadonlyArray<number>, q: number): number => {
  if (sorted.length === 0) return 0;
  const rank = Math.ceil(q * sorted.length);
  const idx = Math.min(sorted.length - 1, Math.max(0, rank - 1));
  return sorted[idx]!;
};

export interface Distribution {
  readonly count: number;
  readonly min: number;
  readonly mean: number;
  readonly p50: number;
  readonly p75: number;
  readonly p90: number;
  readonly p95: number;
  readonly p99: number;
  readonly max: number;
}

export const distribution = (values: ReadonlyArray<number>): Distribution => {
  if (values.length === 0) {
    return { count: 0, min: 0, mean: 0, p50: 0, p75: 0, p90: 0, p95: 0, p99: 0, max: 0 };
  }
  const sorted = [...values].sort((a, b) => a - b);
  const sum = sorted.reduce((s, v) => s + v, 0);
  return {
    count: sorted.length,
    min: sorted[0]!,
    mean: sum / sorted.length,
    p50: percentileSorted(sorted, 0.5),
    p75: percentileSorted(sorted, 0.75),
    p90: percentileSorted(sorted, 0.9),
    p95: percentileSorted(sorted, 0.95),
    p99: percentileSorted(sorted, 0.99),
    max: sorted[sorted.length - 1]!,
  };
};

export interface OutcomeCounts {
  readonly total: number;
  readonly success: number;
  readonly incorrect: number;
  readonly partial: number;
  readonly refused: number;
  readonly successRate: number;
  readonly answeredCorrectlyOrAbstained: number;
}

export const countOutcomes = (results: ReadonlyArray<TierRunResult>): OutcomeCounts => {
  let success = 0;
  let incorrect = 0;
  let partial = 0;
  let refused = 0;
  for (const r of results) {
    if (r.outcome === "success") success += 1;
    else if (r.outcome === "incorrect") incorrect += 1;
    else if (r.outcome === "partial") partial += 1;
    else refused += 1;
  }
  const total = results.length;
  return {
    total,
    success,
    incorrect,
    partial,
    refused,
    successRate: total === 0 ? 0 : success / total,
    // Honesty-positive: correct OR honestly abstained (never a wrong answer).
    answeredCorrectlyOrAbstained: success + refused,
  };
};

export interface CostAggregate {
  /** USD distribution over runs with KNOWN cost only. */
  readonly known: Distribution;
  readonly totalKnownUsd: number;
  readonly knownCount: number;
  readonly unknownUsageCount: number;
  readonly unknownModelCount: number;
  readonly listPlaceholderCount: number;
  readonly verifiedCount: number;
}

export const aggregateCost = (results: ReadonlyArray<TierRunResult>): CostAggregate => {
  const known: Array<number> = [];
  let unknownUsage = 0;
  let unknownModel = 0;
  let listPlaceholder = 0;
  let verified = 0;
  for (const r of results) {
    if (r.cost.disposition === "known" && r.cost.usd !== null) {
      known.push(r.cost.usd);
      if (r.cost.costBasis === "verified") verified += 1;
      else if (r.cost.costBasis === "list_placeholder") listPlaceholder += 1;
    } else if (r.cost.disposition === "unknown_usage") {
      unknownUsage += 1;
    } else {
      unknownModel += 1;
    }
  }
  return {
    known: distribution(known),
    totalKnownUsd: known.reduce((s, v) => s + v, 0),
    knownCount: known.length,
    unknownUsageCount: unknownUsage,
    unknownModelCount: unknownModel,
    listPlaceholderCount: listPlaceholder,
    verifiedCount: verified,
  };
};

export interface StratifiedDistributions {
  /** Model calls, latency, tokens, cost distributions per outcome stratum. */
  readonly modelCalls: Distribution;
  readonly latency: Distribution;
  readonly totalTokens: Distribution;
  readonly cost: CostAggregate;
}

const stratify = (results: ReadonlyArray<TierRunResult>): StratifiedDistributions => ({
  modelCalls: distribution(results.map((r) => r.modelCalls + r.subcalls)),
  latency: distribution(results.map((r) => r.modeledLatencyUnits)),
  totalTokens: distribution(
    results
      .filter((r) => r.tokenInput !== null && r.tokenOutput !== null)
      .map((r) => (r.tokenInput ?? 0) + (r.tokenOutput ?? 0)),
  ),
  cost: aggregateCost(results),
});

export interface TierAggregate {
  readonly tierId: TierId;
  readonly outcomes: OutcomeCounts;
  readonly citationCoverage: Distribution;
  readonly citationExactness: Distribution;
  readonly overall: StratifiedDistributions;
  /** Distributions stratified by outcome (success/incorrect/partial/refused). */
  readonly byOutcome: Readonly<Record<Outcome, StratifiedDistributions>>;
  readonly byFamily: Readonly<Record<DensityFamily, OutcomeCounts>>;
}

export const aggregateTier = (
  tierId: TierId,
  results: ReadonlyArray<TierRunResult>,
): TierAggregate => {
  const outcomes: ReadonlyArray<Outcome> = ["success", "incorrect", "partial", "refused"];
  const families: ReadonlyArray<DensityFamily> = ["constant", "linear", "pair"];
  const byOutcome = Object.fromEntries(
    outcomes.map((o) => [o, stratify(results.filter((r) => r.outcome === o))]),
  ) as Record<Outcome, StratifiedDistributions>;
  const byFamily = Object.fromEntries(
    families.map((f) => [f, countOutcomes(results.filter((r) => r.family === f))]),
  ) as Record<DensityFamily, OutcomeCounts>;
  return {
    tierId,
    outcomes: countOutcomes(results),
    citationCoverage: distribution(results.map((r) => r.citation.coverage)),
    citationExactness: distribution(results.map((r) => r.citation.exactness)),
    overall: stratify(results),
    byOutcome,
    byFamily,
  };
};
