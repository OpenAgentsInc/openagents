import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF,
  buildOpenAgentsRepoStudiedKnowledgeGraph,
  buildOpenAgentsRepoStudyPacket,
  decodeOpenAgentsStudybenchEvalHarnessReport,
  openAgentsStudybenchEvalHarnessReportHash,
  openAgentsStudybenchHiddenEditExamSetHash,
  runOpenAgentsStudybenchEvalHarness,
} from "../src";

const requiredFiles = {
  "AGENTS.md": "# OpenAgents Agent Contract\n\n## Scope\n\nRoot guidance.\n",
  "INVARIANTS.md": "# OpenAgents Invariants\n\nRoot invariant ledger.\n",
  "docs/promises/README.md": "# OpenAgents Product Promises\n\nClaim boundary.\n",
  "docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md":
    "# Tassadar Run\n\nCorrect paradigm.\n",
  "docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md":
    "# Tassadar OpenAgents Repo Studying Audit And Roadmap\n\nStudy packet shape.\n",
  "docs/research/machine-studying/openagents-studybench/private-boundary.md":
    "# Private Split Boundary\n\nPublic rows only.\n",
  "docs/research/machine-studying/openagents-studybench/study-packets/openagents-launch-study-packet-v0.md":
    "# OpenAgents Launch Study Packet v0\n\nPacket refs only.\n",
  "docs/research/machine-studying/openagents-studybench/public-retained/openagents-launch-v0.jsonl":
    "{\"id\":\"openagents_launch_0001\"}\n",
  "docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md": "# AFK Autonomous Loop\n\nNever idle.\n",
  "docs/tassadar/README.md": "# Tassadar\n\nDocs.\n",
  "packages/probe/packages/runtime/package.json": "{\"name\":\"@openagentsinc/probe-runtime\"}\n",
  "packages/probe/packages/runtime/src/benchmark/openagents-study-graph.ts":
    "export const graph = true;\n",
  "packages/probe/packages/runtime/src/benchmark/openagents-study-packet.ts":
    "export const packet = true;\n",
  "packages/probe/packages/runtime/src/benchmark/openagents-study-verification.ts":
    "export const verification = true;\n",
  "packages/probe/packages/runtime/src/benchmark/openagents-studybench-eval-harness.ts":
    "export const harness = true;\n",
  "packages/probe/packages/runtime/src/benchmark/repo-corpus-manifest.ts":
    "export const manifest = true;\n",
  "packages/probe/packages/runtime/src/benchmark/studybench.ts": "export const studybench = true;\n",
  "packages/tassadar-executor/README.md": "# Tassadar Executor\n\nReplay.\n",
  "packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.json":
    "{\"comparison\":true}\n",
  "apps/openagents.com/AGENTS.md": "# Agent Development Notes\n\nFoldkit app.\n",
  "apps/openagents.com/INVARIANTS.md": "# INVARIANTS\n\nPublic projection.\n",
};

const commitHistory = [
  {
    commit: "1111111111111111111111111111111111111111",
    committedAt: "2026-06-18T00:00:00.000Z",
    subjectDigest: "sha256:0d8d11f917db95a6686f94cc6680054084eb8cf440bba7e443e98abbb90049b7",
    subjectPreview: "Add repo studying packet ingest",
  },
  {
    commit: "2222222222222222222222222222222222222222",
    committedAt: "2026-06-17T00:00:00.000Z",
    subjectDigest: "sha256:c67508d38645427baff13c33da5e43dc3d02e559db76f22a365fd20d3fa4a7fe",
    subjectPreview: "Archive previous repo rationale",
  },
] as const;

async function withFixture<A>(run: (input: { root: string; backroom: string }) => Promise<A>): Promise<A> {
  const parent = await mkdtemp(join(tmpdir(), "openagents-studybench-eval-harness-"));
  const root = join(parent, "openagents");
  const backroom = join(parent, "backroom");

  try {
    for (const [path, content] of Object.entries(requiredFiles)) {
      const absolutePath = join(root, path);
      await mkdir(dirname(absolutePath), { recursive: true });
      await writeFile(absolutePath, content, "utf8");
    }

    await mkdir(backroom, { recursive: true });
    await writeFile(join(backroom, "README.md"), "# Backroom\n\nArchive and reference repository.\n", "utf8");

    return await run({ root, backroom });
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
}

async function buildFixture(root: string, backroom: string) {
  const packet = await Effect.runPromise(
    buildOpenAgentsRepoStudyPacket({
      backroomRootDir: backroom,
      commit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      commitHistory,
      generatedAt: "2026-06-18T00:00:00.000Z",
      repo: "OpenAgentsInc/openagents",
      rootDir: root,
    }),
  );
  const graph = await Effect.runPromise(
    buildOpenAgentsRepoStudiedKnowledgeGraph({
      generatedAt: "2026-06-18T00:00:00.000Z",
      packet,
    }),
  );

  return { graph, packet };
}

describe("OpenAgents StudyBench eval harness", () => {
  test("runs hidden-edit exams over S1/S2 substrate and distinguishes studied from baseline", async () => {
    await withFixture(async ({ root, backroom }) => {
      const { graph, packet } = await buildFixture(root, backroom);
      const first = await Effect.runPromise(
        runOpenAgentsStudybenchEvalHarness({
          generatedAt: "2026-06-18T00:00:00.000Z",
          graph,
          maxExams: 2,
          packet,
        }),
      );
      const second = await Effect.runPromise(
        runOpenAgentsStudybenchEvalHarness({
          generatedAt: "2026-06-18T00:00:00.000Z",
          graph,
          maxExams: 2,
          packet,
        }),
      );
      const studied = first.report.aggregateScores.find((score) => score.candidateProfile === "studied_substrate")!;
      const baseline = first.report.aggregateScores.find((score) => score.candidateProfile === "baseline_grep_and_guess")!;

      expect(first.examSet.examSetHash).toBe(second.examSet.examSetHash);
      expect(first.examSet.examSetHash).toBe(openAgentsStudybenchHiddenEditExamSetHash(first.examSet));
      expect(first.report.reportHash).toBe(second.report.reportHash);
      expect(first.report.reportHash).toBe(openAgentsStudybenchEvalHarnessReportHash(first.report));
      expect(first.report.attemptScores).toHaveLength(first.examSet.exams.length * 2);
      expect(first.report.comparison.studiedBeatsBaseline).toBe(true);
      expect(first.report.comparison.distinguishingMetricRefs).toContain(
        "metric.openagents.studybench.pass_at_fixed_budget_lift",
      );
      expect(studied.passRateBps).toBeGreaterThan(baseline.passRateBps);
      expect(studied.totalWrongFileReadCount).toBeLessThan(baseline.totalWrongFileReadCount);
      expect(studied.meanFirstDivergenceStep).toBeGreaterThan(baseline.meanFirstDivergenceStep);
    });
  });

  test("emits StudyBench claim and rubric score records without exposing private task bodies in the report", async () => {
    await withFixture(async ({ root, backroom }) => {
      const { graph, packet } = await buildFixture(root, backroom);
      const { report } = await Effect.runPromise(
        runOpenAgentsStudybenchEvalHarness({
          generatedAt: "2026-06-18T00:00:00.000Z",
          graph,
          maxExams: 1,
          packet,
        }),
      );
      const reportJson = JSON.stringify(report);
      const attempt = report.attemptScores[0]!;

      expect(attempt.claimScores.every((score) => score.schemaRef === PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF)).toBe(
        true,
      );
      expect(attempt.rubricScore.schemaRef).toBe(PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF);
      expect(attempt.retainedFailureRefs.length).toBeGreaterThan(0);
      expect(attempt.studyGraphTraversalRef).toContain("study_traversal.openagents");
      expect(reportJson).not.toContain("\"gold_answer\":");
      expect(reportJson).not.toContain("Use the study packet and graph traversal");
      expect(reportJson).not.toContain("rubric\":[");
    });
  });

  test("rejects mutated public report hashes", async () => {
    await withFixture(async ({ root, backroom }) => {
      const { graph, packet } = await buildFixture(root, backroom);
      const { report } = await Effect.runPromise(
        runOpenAgentsStudybenchEvalHarness({
          generatedAt: "2026-06-18T00:00:00.000Z",
          graph,
          maxExams: 1,
          packet,
        }),
      );

      await expect(
        Effect.runPromise(
          decodeOpenAgentsStudybenchEvalHarnessReport({
            ...report,
            reportHash: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
          }),
        ),
      ).rejects.toMatchObject({
        _tag: "ProbeBenchmarkContractError",
        path: "studybenchEvalHarnessReport.reportHash",
      });
    });
  });
});
