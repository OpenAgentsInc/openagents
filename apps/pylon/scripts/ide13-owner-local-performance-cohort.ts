import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

import { Schema } from "effect";

import { IdePortableEvidenceMetricSchema } from "../../openagents-desktop/src/ide/portable-evidence-contract.ts";
import {
  type Ide13OwnerLocalRealCohortReceipt,
  runIde13OwnerLocalRealCohort,
} from "./ide13-owner-local-real-cohort.js";

const GIT_SHA = /^[a-f0-9]{40}$/u;
const DEFAULT_REPETITIONS = 10;
const MINIMUM_REPETITIONS = 5;
const MAXIMUM_REPETITIONS = 30;

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);

const SampleSchema = Schema.Struct({
  sample: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  artifactSha256: Schema.String.check(Schema.isPattern(/^[a-f0-9]{64}$/u)),
  artifactBytes: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  metrics: Schema.Array(IdePortableEvidenceMetricSchema),
});

export const Ide13OwnerLocalPerformanceCohortReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-owner-local-performance.v1"),
  generatedAt: Schema.String,
  candidateCommitSha: Schema.String.check(Schema.isPattern(GIT_SHA)),
  baseCommitSha: Schema.String.check(Schema.isPattern(GIT_SHA)),
  cohortRef: Schema.Literal("cohort.ide13.owner-local.real.1"),
  evidenceClass: Schema.Literal("real_local"),
  repetitions: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(MINIMUM_REPETITIONS),
    Schema.isLessThanOrEqualTo(MAXIMUM_REPETITIONS),
  ),
  percentileMethod: Schema.Literal("nearest-rank-floor-over-ascending-samples"),
  samples: Schema.Array(SampleSchema).check(Schema.isMinLength(MINIMUM_REPETITIONS)),
  metrics: Schema.Array(IdePortableEvidenceMetricSchema),
  receiptRef: Ref,
});

export interface Ide13OwnerLocalPerformanceCohortReceipt extends Schema.Schema.Type<
  typeof Ide13OwnerLocalPerformanceCohortReceiptSchema
> {}

type Metric = Ide13OwnerLocalRealCohortReceipt["cohort"]["metrics"][number];

const decodeReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalPerformanceCohortReceiptSchema);

const metricIdentity = (metric: Metric): string => `${metric.metric}:${metric.phase ?? "all"}`;

const percentile = (values: ReadonlyArray<number>, rank: number): number =>
  values.toSorted((left, right) => left - right)[Math.floor((values.length - 1) * rank)] ?? 0;

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

export const runIde13OwnerLocalPerformanceCohort = async (
  input: Readonly<{
    candidateCommitSha?: string;
    outputPath?: string;
    repetitions?: number;
    repositoryRoot?: string;
  }> = {},
): Promise<Ide13OwnerLocalPerformanceCohortReceipt> => {
  const repetitions = input.repetitions ?? DEFAULT_REPETITIONS;
  if (
    !Number.isInteger(repetitions) ||
    repetitions < MINIMUM_REPETITIONS ||
    repetitions > MAXIMUM_REPETITIONS
  ) {
    throw new Error("owner-local performance repetitions must be an integer from 5 through 30");
  }

  const runs: Ide13OwnerLocalRealCohortReceipt[] = [];
  for (let sample = 1; sample <= repetitions; sample += 1) {
    runs.push(
      await runIde13OwnerLocalRealCohort({
        ...(input.candidateCommitSha === undefined
          ? {}
          : { candidateCommitSha: input.candidateCommitSha }),
        ...(input.repositoryRoot === undefined ? {} : { repositoryRoot: input.repositoryRoot }),
      }),
    );
  }

  const first = runs[0];
  if (first === undefined) throw new Error("owner-local performance cohort produced no samples");
  if (
    runs.some(
      (run) =>
        run.cohort.candidateCommitSha !== first.cohort.candidateCommitSha ||
        run.cohort.baseCommitSha !== first.cohort.baseCommitSha ||
        run.cohort.cohortRef !== first.cohort.cohortRef ||
        run.cohort.evidenceClass !== "real_local",
    )
  ) {
    throw new Error("owner-local performance cohort sample identity changed during the run");
  }

  const expectedIdentities = first.cohort.metrics.map(metricIdentity).toSorted();
  if (
    runs.some(
      (run) =>
        JSON.stringify(run.cohort.metrics.map(metricIdentity).toSorted()) !==
        JSON.stringify(expectedIdentities),
    )
  ) {
    throw new Error("owner-local performance metric matrix changed during the run");
  }

  const samples = runs.map((run, index) => ({
    sample: index + 1,
    artifactSha256: run.cohort.artifact.sha256 ?? "",
    artifactBytes: run.cohort.artifact.bytes ?? 0,
    metrics: run.cohort.metrics,
  }));
  const metrics = first.cohort.metrics.map((template) => {
    const values = runs.map((run) => {
      const value = run.cohort.metrics.find(
        (candidate) => metricIdentity(candidate) === metricIdentity(template),
      );
      if (value === undefined) throw new Error("owner-local performance metric sample is absent");
      return value.p50;
    });
    const p50 = percentile(values, 0.5);
    const p95 = percentile(values, 0.95);
    const p99 = percentile(values, 0.99);
    return {
      ...template,
      repetitions,
      p50,
      p95,
      p99,
      passed: p95 <= template.thresholdP95 && p99 <= template.thresholdP99,
      receiptRef: `receipt.ide13.owner-local.performance.${sha256(
        `${first.cohort.candidateCommitSha}:${metricIdentity(template)}:${values.join(",")}`,
      ).slice(0, 32)}`,
    } satisfies Metric;
  });
  if (metrics.some((metric) => !metric.passed)) {
    throw new Error("owner-local performance cohort metric threshold failed");
  }

  const receipt = decodeReceipt(
    {
      schemaVersion: "openagents.desktop.ide-portable-owner-local-performance.v1",
      generatedAt: new Date().toISOString(),
      candidateCommitSha: first.cohort.candidateCommitSha,
      baseCommitSha: first.cohort.baseCommitSha,
      cohortRef: "cohort.ide13.owner-local.real.1",
      evidenceClass: "real_local",
      repetitions,
      percentileMethod: "nearest-rank-floor-over-ascending-samples",
      samples,
      metrics,
      receiptRef: `receipt.ide13.owner-local.performance.${sha256(
        JSON.stringify({ candidateCommitSha: first.cohort.candidateCommitSha, samples }),
      ).slice(0, 32)}`,
    },
    { onExcessProperty: "error" },
  );
  if (input.outputPath !== undefined) {
    await mkdir(dirname(input.outputPath), { recursive: true });
    await writeFile(input.outputPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
  }
  return receipt;
};

if (import.meta.main) {
  const repositoryRoot = resolve(join(import.meta.dirname, "../../.."));
  const outputPath = resolve(
    repositoryRoot,
    "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-performance.json",
  );
  const candidateCommitSha = process.env.OPENAGENTS_IDE13_CANDIDATE_COMMIT_SHA;
  const repetitionsText = process.env.OPENAGENTS_IDE13_PERFORMANCE_REPETITIONS;
  const receipt = await runIde13OwnerLocalPerformanceCohort({
    ...(candidateCommitSha === undefined ? {} : { candidateCommitSha }),
    ...(repetitionsText === undefined ? {} : { repetitions: Number(repetitionsText) }),
    outputPath,
    repositoryRoot,
  });
  process.stdout.write(
    `${JSON.stringify({
      candidateCommitSha: receipt.candidateCommitSha,
      receiptRef: receipt.receiptRef,
      repetitions: receipt.repetitions,
    })}\n`,
  );
  process.exit(0);
}
