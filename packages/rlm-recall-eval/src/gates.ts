/**
 * Product-admission gates derived from the dense-recall evidence.
 *
 * These gates define EXPLICIT pass/fail criteria for two product decisions the
 * eval must inform but never make on its own:
 *
 * 1. automatic Tier S escalation (letting an insufficient Tier D answer trigger
 *    semantic recall without a per-call user action);
 * 2. depth above one (recursive `RlmMap` beyond a single level).
 *
 * BOTH ARE SEPARATE PRODUCT ADMISSIONS, NOT ENGINE DEFAULTS. This module never
 * flips a switch: `admitted` is always `false`. It only reports whether the
 * stated criteria WOULD pass on the current evidence, so a human admission can
 * be justified or refused. The desktop Tier S consumer keeps semantic recall
 * host-admitted and depth clamped to one regardless of this result.
 */

import type { TierAggregate, TierId } from "./scoring.ts";

export interface GateCriterion {
  readonly name: string;
  readonly passed: boolean;
  readonly detail: string;
}

export interface GateResult {
  readonly gate: "automatic_tier_s_escalation" | "depth_above_one";
  /** ALWAYS false. Enabling is a separate, human product admission. */
  readonly admitted: false;
  /** Whether the stated criteria pass on the current evidence. */
  readonly wouldPass: boolean;
  readonly criteria: ReadonlyArray<GateCriterion>;
}

const byId = (aggregates: ReadonlyArray<TierAggregate>): Map<TierId, TierAggregate> =>
  new Map(aggregates.map((a) => [a.tierId, a]));

/**
 * Automatic Tier S escalation gate.
 *
 * Escalation is only justified if a synthesising semantic tier clearly answers
 * the family Tier D cannot synthesise (pair/conflict), with exact citations,
 * without adding wrong answers, and with KNOWN cost.
 */
export const evaluateEscalationGate = (aggregates: ReadonlyArray<TierAggregate>): GateResult => {
  const map = byId(aggregates);
  const tierD = map.get("tier_d");
  const semantic = map.get("semantic_modelmap");
  const criteria: Array<GateCriterion> = [];

  const tierDPairPartial =
    (tierD?.byFamily.pair.partial ?? 0) > 0 && (tierD?.byFamily.pair.success ?? 0) === 0;
  criteria.push({
    name: "tier_d_insufficient_on_pair",
    passed: tierDPairPartial,
    detail: `Tier D pair success=${String(tierD?.byFamily.pair.success ?? 0)} partial=${String(tierD?.byFamily.pair.partial ?? 0)}`,
  });

  const semanticPairSuccess = semantic?.byFamily.pair.successRate ?? 0;
  criteria.push({
    name: "semantic_pair_success_rate>=1",
    passed: semanticPairSuccess >= 1,
    detail: `semantic_modelmap pair successRate=${semanticPairSuccess.toFixed(3)}`,
  });

  const semanticCoverage = semantic?.citationCoverage.p50 ?? 0;
  const semanticExactness = semantic?.citationExactness.p50 ?? 0;
  criteria.push({
    name: "semantic_exact_citations",
    passed: semanticCoverage >= 1 && semanticExactness >= 1,
    detail: `coverage.p50=${semanticCoverage.toFixed(3)} exactness.p50=${semanticExactness.toFixed(3)}`,
  });

  const semanticIncorrect = semantic?.outcomes.incorrect ?? 0;
  criteria.push({
    name: "semantic_adds_no_wrong_answers",
    passed: semanticIncorrect === 0,
    detail: `semantic_modelmap incorrect=${String(semanticIncorrect)}`,
  });

  const semanticCostKnown = (semantic?.overall.cost.unknownUsageCount ?? 1) === 0;
  criteria.push({
    name: "semantic_cost_is_known",
    passed: semanticCostKnown,
    detail: `semantic_modelmap unknownUsage=${String(semantic?.overall.cost.unknownUsageCount ?? 0)}`,
  });

  return {
    gate: "automatic_tier_s_escalation",
    admitted: false,
    wouldPass: criteria.every((c) => c.passed),
    criteria,
  };
};

/**
 * Depth-above-one gate.
 *
 * Higher depth is only justified if it STRICTLY improves quality over depth one
 * without raising wrong answers or tail cost. The paper is explicit that depth
 * is not monotonically better, so a tie fails the gate.
 */
export const evaluateDepthGate = (aggregates: ReadonlyArray<TierAggregate>): GateResult => {
  const map = byId(aggregates);
  const d1 = map.get("semantic_depth1");
  const d2 = map.get("semantic_depth2");
  const criteria: Array<GateCriterion> = [];

  const d1Success = d1?.outcomes.successRate ?? 0;
  const d2Success = d2?.outcomes.successRate ?? 0;
  criteria.push({
    name: "depth2_strictly_more_successful",
    passed: d2Success > d1Success,
    detail: `depth1 successRate=${d1Success.toFixed(3)} depth2 successRate=${d2Success.toFixed(3)}`,
  });

  const d1Incorrect = d1?.outcomes.incorrect ?? 0;
  const d2Incorrect = d2?.outcomes.incorrect ?? 0;
  criteria.push({
    name: "depth2_adds_no_wrong_answers",
    passed: d2Incorrect <= d1Incorrect,
    detail: `depth1 incorrect=${String(d1Incorrect)} depth2 incorrect=${String(d2Incorrect)}`,
  });

  const d1CostP95 = d1?.overall.cost.known.p95 ?? 0;
  const d2CostP95 = d2?.overall.cost.known.p95 ?? 0;
  criteria.push({
    name: "depth2_tail_cost_not_worse",
    passed: d2CostP95 <= d1CostP95 * 1.0000001,
    detail: `depth1 cost.p95=${d1CostP95.toFixed(8)} depth2 cost.p95=${d2CostP95.toFixed(8)}`,
  });

  return {
    gate: "depth_above_one",
    admitted: false,
    wouldPass: criteria.every((c) => c.passed),
    criteria,
  };
};
