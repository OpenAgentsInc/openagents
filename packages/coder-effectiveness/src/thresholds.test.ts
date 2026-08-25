/**
 * The gate, including the case that matters most: a cost floor that cannot be
 * measured must not read as a pass.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";

import { summarizeRun } from "./effectiveness.ts";
import { readHarborJob } from "./harbor-job.ts";
import { CODER_RATE_CATALOG_VERSION } from "./pricing.ts";
import { type EffectivenessThresholds, evaluateThresholds, parseThresholds } from "./thresholds.ts";

const fixture = (name: string): string =>
  fileURLToPath(new URL(`../fixtures/${name}`, import.meta.url));

const report = (name: string) =>
  summarizeRun(
    readHarborJob(fixture(name), {
      suite: "tb2-cross-section",
      lane: "proxy",
      rateCatalogVersion: CODER_RATE_CATALOG_VERSION,
    }),
  );

const floors = (overrides: Partial<EffectivenessThresholds> = {}): EffectivenessThresholds =>
  parseThresholds({
    id: "test-floors",
    minGradedTrials: 2,
    minSuccessRate: 0.4,
    maxUngradedRatio: 0.25,
    ...overrides,
  });

const criterion = (
  gate: ReturnType<typeof evaluateThresholds>,
  fragment: string,
): { name: string; verdict: string; detail: string } => {
  const found = gate.criteria.find((entry) => entry.name.includes(fragment));
  if (found === undefined) throw new Error(`no criterion matching ${fragment}`);
  return found;
};

describe("parseThresholds", () => {
  test("reads the checked-in suite floors", () => {
    const path = fileURLToPath(new URL("../thresholds/tb2-cross-section.json", import.meta.url));
    const thresholds = parseThresholds(JSON.parse(readFileSync(path, "utf8")));

    expect(thresholds.id).toBe("tb2-cross-section");
    expect(thresholds.maxCostPerAcceptedOutcomeUsd).toBeGreaterThan(0);
    // Left off deliberately: every catalog rate is provisional today.
    expect(thresholds.acceptPlaceholderRates).toBeUndefined();
  });

  test("rejects a rate outside 0..1", () => {
    expect(() => floors({ minSuccessRate: 1.5 })).toThrow(/minSuccessRate/u);
    expect(() => floors({ maxUngradedRatio: -0.1 })).toThrow(/maxUngradedRatio/u);
  });

  test("rejects a fractional or negative trial count", () => {
    expect(() => floors({ minGradedTrials: 2.5 })).toThrow(/minGradedTrials/u);
    expect(() => floors({ minGradedTrials: -1 })).toThrow(/minGradedTrials/u);
  });

  test("rejects a cost ceiling of zero", () => {
    expect(() => floors({ maxCostPerAcceptedOutcomeUsd: 0 })).toThrow(
      /maxCostPerAcceptedOutcomeUsd/u,
    );
  });
});

describe("evaluateThresholds", () => {
  test("passes a run that clears every floor", () => {
    const gate = evaluateThresholds(report("priced-lane"), floors());

    expect(gate.status).toBe("passed");
    expect(gate.criteria.every((entry) => entry.verdict === "passed")).toBe(true);
  });

  test("fails a run below the success floor", () => {
    // The regressed lane accepts 1 of 4 graded trials.
    const gate = evaluateThresholds(report("regressed-lane"), floors({ minSuccessRate: 0.4 }));

    expect(gate.status).toBe("failed");
    expect(criterion(gate, "success_rate").verdict).toBe("failed");
  });

  test("fails a run with too few graded trials", () => {
    const gate = evaluateThresholds(report("crashed-verifier"), floors({ minGradedTrials: 3 }));

    expect(gate.status).toBe("failed");
    expect(criterion(gate, "graded_trials").verdict).toBe("failed");
  });

  test("fails a run where too many verifiers crashed", () => {
    const gate = evaluateThresholds(
      report("crashed-verifier"),
      floors({ minGradedTrials: 1, maxUngradedRatio: 0.25 }),
    );

    expect(gate.status).toBe("failed");
    expect(criterion(gate, "ungraded_ratio").verdict).toBe("failed");
  });

  // The rule that keeps an unpriced lane accountable.
  test("leaves a cost floor unverifiable on the unpriced luna lane, never passed", () => {
    const gate = evaluateThresholds(
      report("unpriced-lane"),
      floors({ minSuccessRate: 0.5, maxCostPerAcceptedOutcomeUsd: 2 }),
    );

    const cost = criterion(gate, "cost_per_accepted_outcome");
    expect(cost.verdict).toBe("unverifiable");
    expect(cost.verdict).not.toBe("passed");
    expect(cost.detail).toContain("gpt-5.6-luna");
    expect(gate.status).toBe("unverifiable");
  });

  test("leaves a cost floor unverifiable when only some trials were priced", () => {
    const gate = evaluateThresholds(
      report("mixed-lane"),
      floors({ maxCostPerAcceptedOutcomeUsd: 2 }),
    );

    expect(criterion(gate, "cost_per_accepted_outcome").verdict).toBe("unverifiable");
    expect(gate.status).toBe("unverifiable");
  });

  test("will not score a dollar ceiling against placeholder rates by default", () => {
    // The priced lane has a real number. It is still built from rates the
    // forge config marks provisional, so scoring against it needs consent.
    const gate = evaluateThresholds(
      report("priced-lane"),
      floors({ maxCostPerAcceptedOutcomeUsd: 100 }),
    );

    expect(criterion(gate, "cost_per_accepted_outcome").verdict).toBe("unverifiable");
    expect(gate.status).toBe("unverifiable");
  });

  test("scores the ceiling once the thresholds file opts into placeholder rates", () => {
    const under = evaluateThresholds(
      report("priced-lane"),
      floors({ maxCostPerAcceptedOutcomeUsd: 100, acceptPlaceholderRates: true }),
    );
    const over = evaluateThresholds(
      report("priced-lane"),
      floors({ maxCostPerAcceptedOutcomeUsd: 0.0001, acceptPlaceholderRates: true }),
    );

    expect(criterion(under, "cost_per_accepted_outcome").verdict).toBe("passed");
    expect(under.status).toBe("passed");
    expect(criterion(over, "cost_per_accepted_outcome").verdict).toBe("failed");
    expect(over.status).toBe("failed");
  });

  test("omits the cost criterion when the thresholds file declares no ceiling", () => {
    const gate = evaluateThresholds(report("unpriced-lane"), floors({ minSuccessRate: 0.5 }));

    expect(gate.criteria.some((entry) => entry.name.includes("cost"))).toBe(false);
    expect(gate.status).toBe("passed");
  });

  test("a measured breach outranks an unmeasurable criterion", () => {
    // The regressed lane breaches the success floor and cannot be scored on
    // cost. `failed` is the honest status: something was measured and broke.
    const gate = evaluateThresholds(
      report("regressed-lane"),
      floors({ minSuccessRate: 0.5, maxCostPerAcceptedOutcomeUsd: 2 }),
    );

    expect(criterion(gate, "success_rate").verdict).toBe("failed");
    expect(criterion(gate, "cost_per_accepted_outcome").verdict).toBe("unverifiable");
    expect(gate.status).toBe("failed");
  });

  test("reports no success rate rather than a zero one when no verifier ran", () => {
    const ungradedOnly = summarizeRun({
      jobId: "job-none",
      suite: "tb2-cross-section",
      lane: "proxy",
      runDigest: "effectiveness:test",
      trials: [
        {
          task: "fix-git",
          outcome: "ungraded",
          modelId: "gemini-3.7-flash",
          agentVersion: "0.4.0",
          promptTokens: 1000,
          completionTokens: 100,
          cachedInputTokens: 0,
          toolCalls: 1,
          wallClockSeconds: 10,
          threadId: null,
          exception: "VerifierCrashedError",
        },
      ],
    });
    const gate = evaluateThresholds(ungradedOnly, floors({ minGradedTrials: 0 }));

    expect(criterion(gate, "success_rate").verdict).toBe("unverifiable");
    expect(criterion(gate, "success_rate").detail).toContain("no verifier ran");
  });
});
