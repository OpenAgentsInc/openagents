/**
 * The floors a graded run is scored against, and the third verdict that keeps
 * an unpriced lane from passing a cost floor by default.
 *
 * A criterion is `passed`, `failed`, or `unverifiable`. The third one is the
 * point. If a thresholds file declares a cost ceiling and the run happened on
 * `gpt-5.6-luna`, the honest answer is not "under budget" — nothing was
 * measured. Reporting that as a pass would make the gate quietest exactly when
 * the lane is least accountable, which is the failure mode the suite is
 * supposed to remove. So an unmeasurable criterion makes the whole gate
 * `unverifiable`, and the CLI exits 2 rather than 0, so no scheduled job can
 * read silence as green.
 *
 * The same rule covers placeholder rates. The catalog rates are marked
 * provisional by the config that holds them, so scoring a dollar ceiling
 * against them is scoring against a guess. That is `unverifiable` too, unless
 * the thresholds file opts in with `acceptPlaceholderRates: true` — which is
 * a reasonable thing to do for a relative regression check, and an unreasonable
 * thing to do quietly.
 */

import { Schema as S } from "effect";

import type { EffectivenessReport } from "./effectiveness.ts";

export const EffectivenessThresholdsSchema = S.Struct({
  /** Names this floor set, so a report can say what it was scored against. */
  id: S.String,
  /** Fewest verifier-judged trials for the run to be worth scoring at all. */
  minGradedTrials: S.Number,
  /** Lowest acceptable accepted-over-graded rate, 0..1. */
  minSuccessRate: S.Number,
  /** Most of the run that may go ungraded before the run is unreadable, 0..1. */
  maxUngradedRatio: S.Number,
  /** Optional dollar ceiling on cost per accepted outcome. */
  maxCostPerAcceptedOutcomeUsd: S.optional(S.Number),
  /**
   * Score the cost ceiling against rates the catalog marks provisional.
   * Absent or false, a placeholder-priced run leaves the cost criterion
   * unverifiable rather than passing it.
   */
  acceptPlaceholderRates: S.optional(S.Boolean),
});

export type EffectivenessThresholds = typeof EffectivenessThresholdsSchema.Type;

const decodeThresholds = S.decodeUnknownSync(EffectivenessThresholdsSchema);

/** Parse a thresholds document, rejecting values outside their ranges. */
export const parseThresholds = (value: unknown): EffectivenessThresholds => {
  const thresholds = decodeThresholds(value);
  assertFraction("minSuccessRate", thresholds.minSuccessRate);
  assertFraction("maxUngradedRatio", thresholds.maxUngradedRatio);
  if (thresholds.minGradedTrials < 0 || !Number.isInteger(thresholds.minGradedTrials)) {
    throw new Error("minGradedTrials must be a non-negative integer");
  }
  if (
    thresholds.maxCostPerAcceptedOutcomeUsd !== undefined &&
    !(thresholds.maxCostPerAcceptedOutcomeUsd > 0)
  ) {
    throw new Error("maxCostPerAcceptedOutcomeUsd must be greater than zero");
  }
  return thresholds;
};

const assertFraction = (name: string, value: number): void => {
  if (!(value >= 0 && value <= 1)) {
    throw new Error(`${name} must be between 0 and 1, got ${String(value)}`);
  }
};

export type CriterionVerdict = "passed" | "failed" | "unverifiable";

export interface ThresholdCriterion {
  readonly name: string;
  readonly verdict: CriterionVerdict;
  readonly detail: string;
}

export interface ThresholdGate {
  readonly thresholdsId: string;
  /**
   * `failed` beats `unverifiable` beats `passed`: a measured breach is a
   * breach whatever else could not be measured.
   */
  readonly status: CriterionVerdict;
  readonly criteria: ReadonlyArray<ThresholdCriterion>;
}

/** Score a report against its floors. Pure. */
export const evaluateThresholds = (
  report: EffectivenessReport,
  thresholds: EffectivenessThresholds,
): ThresholdGate => {
  const criteria: Array<ThresholdCriterion> = [
    gradedTrialsCriterion(report, thresholds),
    successRateCriterion(report, thresholds),
    ungradedRatioCriterion(report, thresholds),
  ];
  const cost = costCriterion(report, thresholds);
  if (cost !== null) criteria.push(cost);

  return {
    thresholdsId: thresholds.id,
    status: criteria.some((criterion) => criterion.verdict === "failed")
      ? "failed"
      : criteria.some((criterion) => criterion.verdict === "unverifiable")
        ? "unverifiable"
        : "passed",
    criteria,
  };
};

const gradedTrialsCriterion = (
  report: EffectivenessReport,
  thresholds: EffectivenessThresholds,
): ThresholdCriterion => ({
  name: `graded_trials>=${String(thresholds.minGradedTrials)}`,
  verdict: report.graded >= thresholds.minGradedTrials ? "passed" : "failed",
  detail: `${String(report.graded)} of ${String(report.trialsTotal)} trials were graded`,
});

const successRateCriterion = (
  report: EffectivenessReport,
  thresholds: EffectivenessThresholds,
): ThresholdCriterion => {
  const name = `success_rate>=${thresholds.minSuccessRate.toFixed(3)}`;
  if (report.successRate === null) {
    return {
      name,
      verdict: "unverifiable",
      detail: "no verifier ran, so the run has no success rate rather than a zero one",
    };
  }
  return {
    name,
    verdict: report.successRate >= thresholds.minSuccessRate ? "passed" : "failed",
    detail: `success rate ${report.successRate.toFixed(3)} over ${String(report.graded)} graded trials`,
  };
};

const ungradedRatioCriterion = (
  report: EffectivenessReport,
  thresholds: EffectivenessThresholds,
): ThresholdCriterion => ({
  name: `ungraded_ratio<=${thresholds.maxUngradedRatio.toFixed(3)}`,
  verdict: report.ungradedRatio <= thresholds.maxUngradedRatio ? "passed" : "failed",
  detail: `${String(report.ungraded)} of ${String(report.trialsTotal)} trials went ungraded (${report.ungradedRatio.toFixed(3)})`,
});

/** `null` when the thresholds file declares no cost ceiling. */
const costCriterion = (
  report: EffectivenessReport,
  thresholds: EffectivenessThresholds,
): ThresholdCriterion | null => {
  const ceiling = thresholds.maxCostPerAcceptedOutcomeUsd;
  if (ceiling === undefined) return null;

  const name = `cost_per_accepted_outcome<=$${ceiling.toFixed(4)}`;
  const cost = report.costPerAcceptedOutcome;

  if (cost.usd === null) {
    return {
      name,
      verdict: "unverifiable",
      detail: `cost per accepted outcome is unknown (${cost.disposition}): ${cost.reason}`,
    };
  }
  if (cost.rateBasis === "operator_placeholder" && thresholds.acceptPlaceholderRates !== true) {
    return {
      name,
      verdict: "unverifiable",
      detail: `cost per accepted outcome is $${cost.usd.toFixed(4)} but every rate behind it is an operator placeholder; set acceptPlaceholderRates to score against provisional rates`,
    };
  }
  return {
    name,
    verdict: cost.usd <= ceiling ? "passed" : "failed",
    detail: `cost per accepted outcome $${cost.usd.toFixed(4)} against a $${ceiling.toFixed(4)} ceiling`,
  };
};
