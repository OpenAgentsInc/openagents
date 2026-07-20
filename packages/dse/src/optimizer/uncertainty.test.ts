import { Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  EVALUATION_REPORT_SCHEMA_LITERAL,
  EvaluationReport,
  candidateId,
  datasetRevisionId,
  exampleId,
  signatureId,
} from "../contract/index.js";
import { sha256Hex } from "../internal/sha256.js";
import { MIN_NORMAL_APPROX_SAMPLE, computeUncertainty } from "./uncertainty.js";

const SIG = signatureId("AppleFm/TurnRoute.v1");
const CAND = candidateId(`cand:${sha256Hex("winner")}`);
const REV = datasetRevisionId("dset:apple-fm/route:0123456789abcdef");
const decodeReport = S.decodeUnknownSync(EvaluationReport);

const report = (scores: ReadonlyArray<number>): EvaluationReport => {
  const perExample = scores.map((score, index) => ({
    exampleId: exampleId(`ex:h${index}`),
    quality: score,
    resource: 0.1,
    score,
    components: [{ name: "task", kind: "quality" as const, value: score, weight: 1 }],
    formatValid: true,
    decodeRepaired: false,
  }));
  const aggregate = scores.reduce((sum, value) => sum + value, 0) / scores.length;
  return decodeReport({
    schema: EVALUATION_REPORT_SCHEMA_LITERAL,
    signatureId: SIG,
    candidateId: CAND,
    datasetRevisionId: REV,
    split: "holdout",
    metricId: "route.v1",
    perExample,
    aggregateQuality: aggregate,
    aggregateResource: 0.1,
    aggregateScore: aggregate,
    usageTruth: "estimated",
    digest: sha256Hex(JSON.stringify(scores)),
  });
};

describe("AFS-09 uncertainty record", () => {
  test("a small holdout records an explicit small-sample note, not a fabricated interval", () => {
    const baseline = report([0.4, 0.4]);
    const candidate = report([1, 1]);
    const record = computeUncertainty({
      signatureId: SIG,
      candidateId: CAND,
      baselineHoldout: baseline,
      candidateHoldout: candidate,
    });
    expect(record.method).toBe("small_sample_note");
    expect(record.sampleSize).toBe(2);
    expect(record.holdoutDelta).toBeCloseTo(0.6, 5);
    expect(record.note).toContain(`${MIN_NORMAL_APPROX_SAMPLE}`);
  });

  test("a large holdout records a normal-approximation confidence interval", () => {
    const baseline = report([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]);
    const candidate = report([0.9, 0.9, 0.9, 0.9, 0.9, 0.9, 0.9, 0.9]);
    const record = computeUncertainty({
      signatureId: SIG,
      candidateId: CAND,
      baselineHoldout: baseline,
      candidateHoldout: candidate,
    });
    expect(record.method).toBe("normal_approx_ci");
    expect(record.sampleSize).toBe(8);
    expect(record.holdoutDelta).toBeCloseTo(0.4, 5);
    // A constant delta has zero variance, so the interval collapses on the delta.
    expect(record.ciLow).toBeCloseTo(0.4, 5);
    expect(record.ciHigh).toBeCloseTo(0.4, 5);
  });
});
