import { readFile } from "node:fs/promises";
import { describe, expect, test } from "bun:test";

const REPORT_URL = new URL(
  "../../../docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.json",
  import.meta.url,
);

interface CandidateArm {
  readonly candidateArmRef: string;
  readonly label: string;
}

interface ModePlan {
  readonly attemptCount: number;
  readonly mode: "answer" | "patch";
  readonly split: "public_retained" | "private_validation";
  readonly taskRefs: ReadonlyArray<string>;
}

interface AttemptRefPattern {
  readonly closeoutBundleRefPattern: string;
  readonly mode: "answer" | "patch";
  readonly probeCloseoutRefPattern: string;
  readonly resourceUsageRefPattern: string;
  readonly rubricScoreRefPattern: string;
  readonly split: "public_retained" | "private_validation";
}

interface Metric {
  readonly attemptCount: number;
  readonly candidateArmRef: string;
  readonly closeoutCompletenessBps: number;
  readonly mode: "answer" | "patch";
  readonly rowCount: number;
  readonly split: "public_retained" | "private_validation";
  readonly testPassRateBps: number | null;
}

interface ComparisonReport {
  readonly attemptRefPatterns: ReadonlyArray<AttemptRefPattern>;
  readonly candidateArms: ReadonlyArray<CandidateArm>;
  readonly closeoutCoverage: {
    readonly expectedAttempts: number;
    readonly missingCloseoutBundleRefs: ReadonlyArray<string>;
    readonly missingProbeCloseoutRefs: ReadonlyArray<string>;
    readonly missingRubricScoreRefs: ReadonlyArray<string>;
    readonly observedAttempts: number;
  };
  readonly metrics: ReadonlyArray<Metric>;
  readonly modePlan: ReadonlyArray<ModePlan>;
  readonly productPromiseBoundary: {
    readonly customerAvailabilityClaim: boolean;
    readonly productReadinessClaim: string;
    readonly publicProductClaimAllowed: boolean;
  };
  readonly publicSafety: {
    readonly customerPrivateSourceIncluded: boolean;
    readonly privateHoldoutRowsUsed: boolean;
    readonly privateValidationRowsCommitted: boolean;
    readonly rawPrivateRowsIncluded: boolean;
    readonly rawRepoArchivesIncluded: boolean;
  };
  readonly schemaRef: string;
  readonly splits: {
    readonly privateValidation: {
      readonly checksumRef: string;
      readonly taskRefs: ReadonlyArray<string>;
    };
    readonly publicRetained: {
      readonly taskRefs: ReadonlyArray<string>;
    };
  };
}

const readReport = async (): Promise<ComparisonReport> =>
  JSON.parse(await readFile(REPORT_URL, "utf8")) as ComparisonReport;

const taskIdFromRef = (taskRef: string): string => {
  const taskId = taskRef.split(".").at(-1);

  if (taskId === undefined || taskId.length === 0) {
    throw new Error(`Invalid task ref: ${taskRef}`);
  }

  return taskId;
};

const fillPattern = (
  pattern: string,
  input: Readonly<{ candidateLabel: string; taskRef: string }>,
): string =>
  pattern
    .replace("{task_id}", taskIdFromRef(input.taskRef))
    .replace("{candidate_label}", input.candidateLabel);

describe("OpenAgents StudyBench MVP-14 comparison report", () => {
  test("covers public-retained, private-validation, and patch-mode attempts with closeout refs", async () => {
    const report = await readReport();

    expect(report.schemaRef).toBe("probe.studybench_comparison_report.v0");
    expect(report.candidateArms.map((arm) => arm.label)).toEqual([
      "baseline_no_packet",
      "study_packet",
      "gepa_packet",
    ]);
    expect(report.splits.publicRetained.taskRefs).toHaveLength(10);
    expect(report.splits.privateValidation.taskRefs).toHaveLength(5);
    expect(report.splits.privateValidation.checksumRef).toMatch(/^checksum\./);

    const patchPlan = report.modePlan.find((plan) => plan.mode === "patch");
    expect(patchPlan?.taskRefs).toHaveLength(2);

    const plannedAttempts = report.modePlan.reduce(
      (total, plan) => total + plan.attemptCount,
      0,
    );
    const derivedAttempts = report.modePlan.flatMap((plan) =>
      plan.taskRefs.flatMap((taskRef) =>
        report.candidateArms.map((arm) => {
          const pattern = report.attemptRefPatterns.find(
            (candidatePattern) =>
              candidatePattern.mode === plan.mode &&
              candidatePattern.split === plan.split,
          );

          if (pattern === undefined) {
            throw new Error(`Missing attempt pattern for ${plan.mode}/${plan.split}`);
          }

          return {
            closeoutBundleRef: fillPattern(pattern.closeoutBundleRefPattern, {
              candidateLabel: arm.label,
              taskRef,
            }),
            probeCloseoutRef: fillPattern(pattern.probeCloseoutRefPattern, {
              candidateLabel: arm.label,
              taskRef,
            }),
            resourceUsageRef: fillPattern(pattern.resourceUsageRefPattern, {
              candidateLabel: arm.label,
              taskRef,
            }),
            rubricScoreRef: fillPattern(pattern.rubricScoreRefPattern, {
              candidateLabel: arm.label,
              taskRef,
            }),
          };
        }),
      ),
    );

    expect(plannedAttempts).toBe(45);
    expect(derivedAttempts).toHaveLength(report.closeoutCoverage.expectedAttempts);
    expect(report.closeoutCoverage.observedAttempts).toBe(45);
    expect(report.closeoutCoverage.missingCloseoutBundleRefs).toEqual([]);
    expect(report.closeoutCoverage.missingProbeCloseoutRefs).toEqual([]);
    expect(report.closeoutCoverage.missingRubricScoreRefs).toEqual([]);
    expect(
      derivedAttempts.every(
        (attempt) =>
          attempt.closeoutBundleRef.startsWith("closeout_bundle.probe.") &&
          attempt.probeCloseoutRef.startsWith("probe_closeout.probe.") &&
          attempt.rubricScoreRef.startsWith("rubric_score.probe.") &&
          attempt.resourceUsageRef.startsWith("resource_usage.probe."),
      ),
    ).toBe(true);
  });

  test("keeps answer-mode and patch-mode metrics separate", async () => {
    const report = await readReport();
    const metricKeys = new Set(
      report.metrics.map(
        (metric) => `${metric.candidateArmRef}:${metric.mode}:${metric.split}`,
      ),
    );

    for (const arm of report.candidateArms) {
      expect(metricKeys.has(`${arm.candidateArmRef}:answer:public_retained`)).toBe(true);
      expect(metricKeys.has(`${arm.candidateArmRef}:answer:private_validation`)).toBe(true);
      expect(metricKeys.has(`${arm.candidateArmRef}:patch:public_retained`)).toBe(true);
    }

    expect(
      report.metrics
        .filter((metric) => metric.mode === "answer")
        .every((metric) => metric.testPassRateBps === null),
    ).toBe(true);
    expect(
      report.metrics
        .filter((metric) => metric.mode === "patch")
        .every((metric) => typeof metric.testPassRateBps === "number"),
    ).toBe(true);
    expect(
      report.metrics.every(
        (metric) =>
          metric.closeoutCompletenessBps === 10_000 &&
          metric.attemptCount === metric.rowCount,
      ),
    ).toBe(true);
  });

  test("does not expose private material or claim product readiness", async () => {
    const rawReport = await readFile(REPORT_URL, "utf8");
    const report = JSON.parse(rawReport) as ComparisonReport;

    expect(rawReport).not.toContain("gold_answer");
    expect(rawReport).not.toContain("hidden_rubric");
    expect(rawReport).not.toContain("hidden_gold_answer");
    expect(rawReport).not.toContain("private_holdout");
    expect(rawReport).not.toContain("raw_repo_archive");
    expect(rawReport).not.toContain("customer_private_source");
    expect(rawReport).not.toContain("/Users/");

    expect(report.publicSafety).toMatchObject({
      customerPrivateSourceIncluded: false,
      privateHoldoutRowsUsed: false,
      privateValidationRowsCommitted: false,
      rawPrivateRowsIncluded: false,
      rawRepoArchivesIncluded: false,
    });
    expect(report.productPromiseBoundary).toMatchObject({
      customerAvailabilityClaim: false,
      productReadinessClaim: "blocked",
      publicProductClaimAllowed: false,
    });
  });
});
