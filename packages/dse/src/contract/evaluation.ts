import { Schema as S } from "effect";

import {
  CandidateId,
  DatasetRevisionId,
  DseUsageTruth,
  ExampleId,
  Sha256Hex,
  SignatureId,
} from "./refs.js";
import { DatasetSplitName } from "./dataset.js";

/**
 * Metrics, reward bundles, and evaluation reports.
 *
 * A metric has quality and resource components. A reward bundle combines named
 * components — format validity, task metric, tool failure, evidence quality, and
 * cost — with weights into one bounded score. An evaluation report records the
 * per-example and aggregate results, the honest usage truth, and a digest for
 * audit.
 */

export const EVALUATION_REPORT_SCHEMA_LITERAL = "openagents.dse.evaluation_report.v1" as const;

/** A single named metric component. Quality rises with better output; resource is a penalty. */
export const MetricComponent = S.Struct({
  name: S.String.check(S.isMinLength(1), S.isMaxLength(128)),
  kind: S.Literals(["quality", "resource"]),
  value: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  weight: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
});
export type MetricComponent = typeof MetricComponent.Type;

/** The per-example score, its components, and its decode observations. */
export const ExampleScore = S.Struct({
  exampleId: ExampleId,
  quality: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  resource: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  score: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  components: S.Array(MetricComponent).check(S.isMaxLength(16)),
  formatValid: S.Boolean,
  decodeRepaired: S.Boolean,
});
export type ExampleScore = typeof ExampleScore.Type;

export const EvaluationReport = S.Struct({
  schema: S.Literal(EVALUATION_REPORT_SCHEMA_LITERAL),
  signatureId: SignatureId,
  candidateId: CandidateId,
  datasetRevisionId: DatasetRevisionId,
  split: DatasetSplitName,
  metricId: S.String.check(S.isMinLength(1), S.isMaxLength(128)),
  perExample: S.Array(ExampleScore).check(S.isMinLength(1)),
  aggregateQuality: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  aggregateResource: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  aggregateScore: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  usageTruth: DseUsageTruth,
  digest: Sha256Hex,
});
export type EvaluationReport = typeof EvaluationReport.Type;

/**
 * The runtime metric. It scores one decoded example. The package never trusts a
 * metric to bound itself: every returned component value is clamped to 0..1 and
 * the aggregate is a weighted mean. A metric is offline and deterministic.
 */
export interface Metric<O> {
  readonly metricId: string;
  readonly score: (args: {
    readonly expected: O | null;
    readonly actual: O | null;
    readonly formatValid: boolean;
  }) => ReadonlyArray<MetricComponent>;
}

const clamp01 = (value: number): number => (value < 0 ? 0 : value > 1 ? 1 : value);

/**
 * Combine metric components into a bounded quality, resource, and overall score.
 * Correctness (quality) has precedence: the overall score is the weighted
 * quality mean, reduced by the weighted resource penalty, and never rises above
 * the quality mean. Provider-token savings cannot buy back a wrong answer.
 */
export const rewardBundle = (
  components: ReadonlyArray<MetricComponent>,
): { readonly quality: number; readonly resource: number; readonly score: number } => {
  const quality = components.filter((component) => component.kind === "quality");
  const resource = components.filter((component) => component.kind === "resource");
  const weighted = (list: ReadonlyArray<MetricComponent>): number => {
    const totalWeight = list.reduce((sum, component) => sum + component.weight, 0);
    if (totalWeight === 0) return 0;
    return clamp01(
      list.reduce((sum, component) => sum + clamp01(component.value) * component.weight, 0) /
        totalWeight,
    );
  };
  const qualityScore = weighted(quality);
  const resourcePenalty = weighted(resource);
  const score = clamp01(qualityScore * (1 - 0.5 * resourcePenalty));
  return { quality: qualityScore, resource: resourcePenalty, score };
};
