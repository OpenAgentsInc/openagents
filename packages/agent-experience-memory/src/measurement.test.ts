import { describe, expect, test } from "vite-plus/test";

import {
  BENEFIT_ACCEPT_EPSILON,
  computeBenefitReport,
  MIN_BENEFIT_SAMPLES,
  type BenefitSample,
} from "./measurement.js";

const sample = (
  taskRef: string,
  offAccepted: boolean,
  onAccepted: boolean,
  offCorrections: number,
  onCorrections: number,
): BenefitSample => ({ taskRef, offAccepted, onAccepted, offCorrections, onCorrections });

describe("benefit measurement", () => {
  test("a too-small sample yields insufficient_data, never a manufactured benefit", () => {
    const report = computeBenefitReport([sample("t1", false, true, 1, 0)]);
    expect(report.sampleCount).toBe(1);
    expect(report.verdict).toBe("insufficient_data");
    expect(report.structuredToMeasure).toBe(true);
  });

  test("a clear acceptance gain with no correction regression reads as improved", () => {
    const samples = Array.from({ length: MIN_BENEFIT_SAMPLES }, (_unused, index) =>
      sample(`t${index}`, index < 2, true, 2, 0),
    );
    const report = computeBenefitReport(samples);
    expect(report.acceptDelta).toBeGreaterThan(BENEFIT_ACCEPT_EPSILON);
    expect(report.correctionDelta).toBeLessThanOrEqual(0);
    expect(report.verdict).toBe("improved");
  });

  test("no material change reads as no_material_delta", () => {
    const samples = Array.from({ length: MIN_BENEFIT_SAMPLES }, (_unused, index) =>
      sample(`t${index}`, true, true, 1, 1),
    );
    const report = computeBenefitReport(samples);
    expect(report.acceptDelta).toBe(0);
    expect(report.verdict).toBe("no_material_delta");
  });

  test("a correction regression reads as regressed even with flat acceptance", () => {
    const samples = Array.from({ length: MIN_BENEFIT_SAMPLES }, (_unused, index) =>
      sample(`t${index}`, true, true, 0, 3),
    );
    const report = computeBenefitReport(samples);
    expect(report.correctionDelta).toBeGreaterThan(0);
    expect(report.verdict).toBe("regressed");
  });
});
