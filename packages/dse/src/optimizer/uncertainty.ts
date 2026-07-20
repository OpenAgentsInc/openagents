import { Schema as S } from "effect";

import {
  UNCERTAINTY_RECORD_SCHEMA_LITERAL,
  UncertaintyRecord,
  type EvaluationReport,
} from "../contract/index.js";

/**
 * Small-sample uncertainty for a holdout delta (AFS-09 exit check).
 *
 * A compiled artifact must beat its frozen baseline on the holdout, but a tiny
 * holdout can produce a delta that is not meaningful. This helper pins the
 * baseline and candidate holdout scores, the delta, and an uncertainty record.
 * For a sample large enough it reports a normal-approximation confidence
 * interval over the paired per-example score deltas; for a sample too small for
 * a meaningful interval it records an explicit small-sample note instead. It
 * never manufactures confidence a small dataset does not support.
 */

const decodeRecord = S.decodeUnknownSync(UncertaintyRecord);

/** The minimum paired holdout size for a normal-approximation interval. */
export const MIN_NORMAL_APPROX_SAMPLE = 8 as const;

const clampDelta = (value: number): number => (value < -1 ? -1 : value > 1 ? 1 : value);

const pairedDeltas = (
  baseline: EvaluationReport,
  candidate: EvaluationReport,
): ReadonlyArray<number> => {
  const baselineById = new Map(baseline.perExample.map((score) => [score.exampleId, score.score]));
  const deltas: number[] = [];
  for (const score of candidate.perExample) {
    const before = baselineById.get(score.exampleId);
    if (before !== undefined) deltas.push(score.score - before);
  }
  return deltas;
};

const mean = (values: ReadonlyArray<number>): number =>
  values.length === 0 ? 0 : values.reduce((sum, value) => sum + value, 0) / values.length;

/**
 * Build the uncertainty record that accompanies a promotion. The two reports
 * must be the holdout reports for the baseline and the candidate over the same
 * dataset revision.
 */
export const computeUncertainty = (args: {
  readonly signatureId: UncertaintyRecord["signatureId"];
  readonly candidateId: UncertaintyRecord["candidateId"];
  readonly baselineHoldout: EvaluationReport;
  readonly candidateHoldout: EvaluationReport;
}): UncertaintyRecord => {
  const deltas = pairedDeltas(args.baselineHoldout, args.candidateHoldout);
  const sampleSize = Math.max(1, deltas.length);
  const delta = clampDelta(args.candidateHoldout.aggregateScore - args.baselineHoldout.aggregateScore);

  if (deltas.length >= MIN_NORMAL_APPROX_SAMPLE) {
    const m = mean(deltas);
    const variance =
      deltas.reduce((sum, value) => sum + (value - m) * (value - m), 0) / (deltas.length - 1);
    const standardError = Math.sqrt(Math.max(variance, 0) / deltas.length);
    const halfWidth = 1.96 * standardError;
    return decodeRecord({
      schema: UNCERTAINTY_RECORD_SCHEMA_LITERAL,
      signatureId: args.signatureId,
      candidateId: args.candidateId,
      baselineHoldoutScore: args.baselineHoldout.aggregateScore,
      candidateHoldoutScore: args.candidateHoldout.aggregateScore,
      holdoutDelta: delta,
      sampleSize,
      method: "normal_approx_ci",
      ciLow: clampDelta(m - halfWidth),
      ciHigh: clampDelta(m + halfWidth),
      note: `Normal-approximation 95% CI over ${deltas.length} paired holdout deltas.`,
    });
  }

  return decodeRecord({
    schema: UNCERTAINTY_RECORD_SCHEMA_LITERAL,
    signatureId: args.signatureId,
    candidateId: args.candidateId,
    baselineHoldoutScore: args.baselineHoldout.aggregateScore,
    candidateHoldoutScore: args.candidateHoldout.aggregateScore,
    holdoutDelta: delta,
    sampleSize,
    method: "small_sample_note",
    ciLow: delta < 0 ? delta : 0,
    ciHigh: delta > 0 ? delta : 0,
    note:
      `Holdout has ${deltas.length} paired examples, below the ${MIN_NORMAL_APPROX_SAMPLE}-example ` +
      "threshold for a normal-approximation interval; the delta is reported without a confidence " +
      "interval and a larger holdout is required before a strong claim.",
  });
};
