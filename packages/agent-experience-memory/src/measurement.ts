import { Schema as S } from "effect";

/**
 * The offline benefit-measurement harness.
 *
 * AFS-10 is conservative by design: recall is promoted only AFTER a measured
 * benefit, never on assertion. This module computes the acceptance and
 * correction deltas between paired flag-OFF and flag-ON samples, so a promotion
 * decision rests on evidence. It measures; it does not claim. Version one ships
 * with the flag OFF and NO measured live benefit — the verdict below is a
 * structured input to a future promotion decision, not a product claim.
 */
export const MEMORY_BENEFIT_REPORT_SCHEMA_LITERAL = "openagents.experience_memory_benefit.v1" as const;

/**
 * One paired outcome. `accepted` is whether the owner accepted the turn's output
 * without correction; `corrections` is how many owner corrections the turn
 * needed. Both are recorded for the SAME task, once with memory off and once on.
 */
export const BenefitSample = S.Struct({
  taskRef: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  offAccepted: S.Boolean,
  onAccepted: S.Boolean,
  offCorrections: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  onCorrections: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
});
export type BenefitSample = typeof BenefitSample.Type;

export const BenefitVerdict = S.Literals(["improved", "no_material_delta", "regressed", "insufficient_data"]);
export type BenefitVerdict = typeof BenefitVerdict.Type;

export const MemoryBenefitReport = S.Struct({
  schema: S.Literal(MEMORY_BENEFIT_REPORT_SCHEMA_LITERAL),
  sampleCount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  offAcceptRate: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  onAcceptRate: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  acceptDelta: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  offCorrectionMean: S.Number.check(S.isGreaterThanOrEqualTo(0)),
  onCorrectionMean: S.Number.check(S.isGreaterThanOrEqualTo(0)),
  correctionDelta: S.Number,
  verdict: BenefitVerdict,
  /** Always true in version one: no live benefit has been measured yet. */
  structuredToMeasure: S.Boolean,
});
export type MemoryBenefitReport = typeof MemoryBenefitReport.Type;

const decodeReport = S.decodeUnknownSync(MemoryBenefitReport);

/** The minimum paired-sample count below which a verdict is withheld as insufficient. */
export const MIN_BENEFIT_SAMPLES = 8;

/** The acceptance-delta magnitude below which a change is treated as no material delta. */
export const BENEFIT_ACCEPT_EPSILON = 0.02;

const mean = (values: ReadonlyArray<number>): number =>
  values.length === 0 ? 0 : values.reduce((total, value) => total + value, 0) / values.length;

/**
 * Compute the benefit report from paired samples. A positive `acceptDelta` above
 * the epsilon, with no correction regression, reads as `improved`. A too-small
 * sample reads as `insufficient_data`. The function never rounds a null result
 * up to a benefit.
 */
export const computeBenefitReport = (samples: ReadonlyArray<BenefitSample>): MemoryBenefitReport => {
  const sampleCount = samples.length;
  const offAcceptRate = mean(samples.map((sample) => (sample.offAccepted ? 1 : 0)));
  const onAcceptRate = mean(samples.map((sample) => (sample.onAccepted ? 1 : 0)));
  const acceptDelta = onAcceptRate - offAcceptRate;
  const offCorrectionMean = mean(samples.map((sample) => sample.offCorrections));
  const onCorrectionMean = mean(samples.map((sample) => sample.onCorrections));
  const correctionDelta = onCorrectionMean - offCorrectionMean;

  const verdict: BenefitVerdict =
    sampleCount < MIN_BENEFIT_SAMPLES
      ? "insufficient_data"
      : acceptDelta > BENEFIT_ACCEPT_EPSILON && correctionDelta <= 0
        ? "improved"
        : acceptDelta < -BENEFIT_ACCEPT_EPSILON || correctionDelta > 0
          ? "regressed"
          : "no_material_delta";

  return decodeReport({
    schema: MEMORY_BENEFIT_REPORT_SCHEMA_LITERAL,
    sampleCount,
    offAcceptRate,
    onAcceptRate,
    acceptDelta,
    offCorrectionMean,
    onCorrectionMean,
    correctionDelta,
    verdict,
    structuredToMeasure: true,
  });
};
