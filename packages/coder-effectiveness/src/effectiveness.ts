/**
 * The metric: cost per accepted outcome, and the conditions under which this
 * suite will refuse to state one.
 *
 * DEFINITION. Cost per accepted outcome is the run's TOTAL cost divided by the
 * number of outcomes a verifier accepted — the cost of the failures included.
 * That is the same definition the benchmark report already uses
 * (`apps/openagents.com/workers/api/src/inference/benchmark/report.ts`,
 * `costPerAcceptedOutcomeMsat`), and it is the one worth reporting: you pay
 * for the trials that failed, so an agent that halves its success rate doubles
 * what an accepted outcome costs you even if each individual attempt got
 * cheaper. Dividing accepted-run cost by accepted runs would hide exactly the
 * regression the gate exists to catch.
 *
 * THREE WAYS THE NUMBER IS REFUSED, each a finding rather than a zero:
 *
 * - `no_accepted_outcomes`: nothing was accepted. The denominator is zero and
 *   the cost of an accepted outcome is undefined, not infinite and not free.
 * - `cost_unknown`: no trial could be priced at all — the whole run was on an
 *   unpriced lane such as `gpt-5.6-luna`, or on the local lane, or the coder
 *   reported no token counts.
 * - `cost_partial`: SOME trials priced and others did not. This is the case a
 *   careless implementation gets wrong: summing the priced trials and dividing
 *   by every accepted outcome yields a real-looking number that understates
 *   the truth by however much the unpriced trials cost. A partial numerator
 *   over a full denominator is not a cheaper run, so the number is withheld
 *   and the coverage is printed instead.
 *
 * When a number IS produced it still carries {@link RateBasis}. Today every
 * catalog rate is an operator placeholder, so every number this suite can
 * currently produce is arithmetically sound and economically provisional, and
 * it says so on every surface it reaches.
 */

import type { GradedRun, TrialRecord } from "./harbor-job.ts";
import {
  CODER_RATE_CATALOG,
  CODER_RATE_CATALOG_VERSION,
  type CostDisposition,
  type ModelRateRow,
  priceUsage,
  type RateBasis,
} from "./pricing.ts";

/** One trial with the cost the catalog could or could not put on it. */
export interface TrialCost {
  readonly task: string;
  readonly outcome: TrialRecord["outcome"];
  readonly usd: number | null;
  readonly disposition: CostDisposition;
  readonly reason: string;
}

/** How completely a run's cost is known. */
export type CostCoverage = "known" | "partial" | "unknown";

export interface CostAggregate {
  /** The sum over trials that could be priced, or `null` when none could. */
  readonly totalUsd: number | null;
  readonly coverage: CostCoverage;
  readonly pricedTrials: number;
  readonly unpricedTrials: number;
  /**
   * The weakest basis among the rates used. A run mixing confirmed and
   * placeholder rates is a placeholder-priced run.
   */
  readonly rateBasis: RateBasis | null;
  /** Distinct reasons trials went unpriced, for the report to print. */
  readonly unpricedReasons: ReadonlyArray<string>;
}

export type CostPerAcceptedOutcomeDisposition =
  | "known"
  | "no_accepted_outcomes"
  | "cost_unknown"
  | "cost_partial";

export interface CostPerAcceptedOutcome {
  readonly usd: number | null;
  readonly disposition: CostPerAcceptedOutcomeDisposition;
  readonly rateBasis: RateBasis | null;
  readonly reason: string;
}

export interface EffectivenessReport {
  readonly suite: string;
  readonly lane: string;
  readonly runDigest: string;
  /**
   * The staged text surfaces the run composed from, by content digest, or
   * `null` when the trials announced none or disagreed.
   */
  readonly surfaceDigests: Readonly<Record<string, string>> | null;
  readonly jobId: string | null;
  readonly models: ReadonlyArray<string>;
  readonly agentVersions: ReadonlyArray<string>;
  readonly rateCatalogVersion: string;

  readonly trialsTotal: number;
  readonly accepted: number;
  readonly rejected: number;
  readonly ungraded: number;
  /** Trials a verifier actually judged: the success-rate denominator. */
  readonly graded: number;
  /** Accepted over graded. `null` when no verifier ran, which is not a 0% run. */
  readonly successRate: number | null;
  readonly ungradedRatio: number;

  readonly cost: CostAggregate;
  readonly costPerAcceptedOutcome: CostPerAcceptedOutcome;

  readonly promptTokens: number | null;
  readonly completionTokens: number | null;
  readonly cachedInputTokens: number;
  readonly toolCalls: number | null;
  readonly wallClockSeconds: number | null;

  readonly perTrial: ReadonlyArray<TrialCost>;
}

/** Aggregate a graded run into the effectiveness report. Pure. */
export const summarizeRun = (
  run: GradedRun,
  catalog: Readonly<Record<string, ModelRateRow>> = CODER_RATE_CATALOG,
  rateCatalogVersion: string = CODER_RATE_CATALOG_VERSION,
): EffectivenessReport => {
  const perTrial: Array<TrialCost> = [];
  let totalUsd = 0;
  let pricedTrials = 0;
  let unpricedTrials = 0;
  const unpricedReasons = new Set<string>();
  const rateBases = new Set<RateBasis>();

  for (const trial of run.trials) {
    const cost = priceUsage(
      trial.modelId,
      {
        promptTokens: trial.promptTokens,
        completionTokens: trial.completionTokens,
        cachedInputTokens: trial.cachedInputTokens,
      },
      catalog,
      run.lane,
    );
    if (cost.usd === null) {
      unpricedTrials += 1;
      unpricedReasons.add(cost.reason);
    } else {
      pricedTrials += 1;
      totalUsd += cost.usd;
      if (cost.rateBasis !== null) rateBases.add(cost.rateBasis);
    }
    perTrial.push({
      task: trial.task,
      outcome: trial.outcome,
      usd: cost.usd,
      disposition: cost.disposition,
      reason: cost.reason,
    });
  }

  const accepted = run.trials.filter((trial) => trial.outcome === "accepted").length;
  const rejected = run.trials.filter((trial) => trial.outcome === "rejected").length;
  const ungraded = run.trials.filter((trial) => trial.outcome === "ungraded").length;
  const graded = accepted + rejected;

  const cost: CostAggregate = {
    totalUsd: pricedTrials === 0 ? null : totalUsd,
    coverage: coverageOf(pricedTrials, unpricedTrials),
    pricedTrials,
    unpricedTrials,
    rateBasis: weakestBasis(rateBases),
    unpricedReasons: [...unpricedReasons].toSorted(),
  };

  return {
    suite: run.suite,
    lane: run.lane,
    runDigest: run.runDigest,
    surfaceDigests: run.surfaceDigests,
    jobId: run.jobId,
    models: distinct(run.trials.map((trial) => trial.modelId)),
    agentVersions: distinct(run.trials.map((trial) => trial.agentVersion)),
    rateCatalogVersion,

    trialsTotal: run.trials.length,
    accepted,
    rejected,
    ungraded,
    graded,
    successRate: graded === 0 ? null : accepted / graded,
    ungradedRatio: run.trials.length === 0 ? 0 : ungraded / run.trials.length,

    cost,
    costPerAcceptedOutcome: perAcceptedOutcome(cost, accepted),

    promptTokens: sumOrNull(run.trials.map((trial) => trial.promptTokens)),
    completionTokens: sumOrNull(run.trials.map((trial) => trial.completionTokens)),
    cachedInputTokens: run.trials.reduce((sum, trial) => sum + trial.cachedInputTokens, 0),
    toolCalls: sumOrNull(run.trials.map((trial) => trial.toolCalls)),
    wallClockSeconds: sumOrNull(run.trials.map((trial) => trial.wallClockSeconds)),

    perTrial,
  };
};

const coverageOf = (priced: number, unpriced: number): CostCoverage => {
  if (priced === 0) return "unknown";
  if (unpriced === 0) return "known";
  return "partial";
};

/** A placeholder anywhere makes the whole aggregate a placeholder. */
const weakestBasis = (bases: ReadonlySet<RateBasis>): RateBasis | null => {
  if (bases.has("operator_placeholder")) return "operator_placeholder";
  if (bases.has("operator_confirmed")) return "operator_confirmed";
  return null;
};

const perAcceptedOutcome = (cost: CostAggregate, accepted: number): CostPerAcceptedOutcome => {
  if (accepted === 0) {
    return {
      usd: null,
      disposition: "no_accepted_outcomes",
      rateBasis: cost.rateBasis,
      reason:
        "no outcome was accepted, so the cost of an accepted outcome is undefined rather than zero or infinite",
    };
  }
  if (cost.coverage === "unknown" || cost.totalUsd === null) {
    return {
      usd: null,
      disposition: "cost_unknown",
      rateBasis: cost.rateBasis,
      reason: `no trial in this run could be priced (${cost.unpricedReasons.join("; ")})`,
    };
  }
  if (cost.coverage === "partial") {
    return {
      usd: null,
      disposition: "cost_partial",
      rateBasis: cost.rateBasis,
      reason: `only ${String(cost.pricedTrials)} of ${String(cost.pricedTrials + cost.unpricedTrials)} trials could be priced, and a partial total over every accepted outcome would understate the cost`,
    };
  }
  return {
    usd: cost.totalUsd / accepted,
    disposition: "known",
    rateBasis: cost.rateBasis,
    reason:
      cost.rateBasis === "operator_placeholder"
        ? "priced entirely from operator placeholder rates, so the figure is provisional"
        : "priced entirely from confirmed rates",
  };
};

/** A sum is only known when every term is. One unknown makes the total unknown. */
const sumOrNull = (values: ReadonlyArray<number | null>): number | null => {
  if (values.length === 0) return null;
  let sum = 0;
  for (const value of values) {
    if (value === null) return null;
    sum += value;
  }
  return sum;
};

const distinct = (values: ReadonlyArray<string | null>): ReadonlyArray<string> =>
  [...new Set(values.filter((value): value is string => value !== null))].toSorted();
