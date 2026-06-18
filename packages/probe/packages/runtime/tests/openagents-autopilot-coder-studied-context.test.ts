import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  applyOpenAgentsAutopilotCoderStudiedContextToToolMenuInput,
  buildOpenAgentsAutopilotCoderStudiedContext,
  buildOpenAgentsAutopilotCoderStudiedPlanContext,
  buildOpenAgentsRepoStudiedKnowledgeGraph,
  buildOpenAgentsRepoStudyPacket,
  buildOpenAgentsStudybenchHiddenEditExamSet,
  loadBlueprintSignatureRegistry,
  lookupBlueprintSignatures,
  openAgentsAutopilotCoderStudiedContextHash,
  openAgentsAutopilotCoderStudiedPlanContextHash,
  planProbeToolMenu,
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
  "packages/probe/packages/runtime/src/benchmark/openagents-autopilot-coder-studied-context.ts":
    "export const context = true;\n",
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
  const parent = await mkdtemp(join(tmpdir(), "openagents-autopilot-coder-studied-context-"));
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

describe("OpenAgents Autopilot Coder studied context", () => {
  test("builds a context pack with invariant, introducing commit, audit, and rejected-lineage refs", async () => {
    await withFixture(async ({ root, backroom }) => {
      const { graph, packet } = await buildFixture(root, backroom);
      const context = await Effect.runPromise(
        buildOpenAgentsAutopilotCoderStudiedContext({
          editSitePath: "packages/probe/packages/runtime/src/benchmark/openagents-autopilot-coder-studied-context.ts",
          graph,
          packet,
        }),
      );
      const planContext = await Effect.runPromise(
        buildOpenAgentsAutopilotCoderStudiedPlanContext({
          context,
          existingContextPackRefs: ["context_pack.probe.assignment.base"],
        }),
      );

      expect(context.contextHash).toBe(openAgentsAutopilotCoderStudiedContextHash(context));
      expect(context.contextPackRef).toContain("context_pack.openagents.autopilot_coder.studied_context");
      expect(context.invariantNodeRefs.length).toBeGreaterThan(0);
      expect(context.introducingCommitNodeRefs).toHaveLength(1);
      expect(context.auditNodeRefs.length).toBeGreaterThan(0);
      expect(context.rejectedLineageNodeRefs.length).toBeGreaterThan(0);
      expect(context.sourceAuthorityRefs).toContain("authority.openagents.repo_study.commit_history");
      expect(context.sourceAuthorityRefs).toContain("authority.openagents.repo_study.audit");
      expect(context.sourceAuthorityRefs).toContain("authority.openagents.repo_study.rejected_lineage");
      expect(context.keywordRoutingAllowed).toBe(false);
      expect(context.mutationAuthority).toBe(false);
      expect(planContext.planContextHash).toBe(openAgentsAutopilotCoderStudiedPlanContextHash(planContext));
      expect(planContext.contextPackRefs).toContain("context_pack.probe.assignment.base");
      expect(planContext.contextPackRefs).toContain(context.contextPackRef);
      expect(planContext.readFirstFileRefs).toEqual([context.editSitePath]);
    });
  });

  test("injects the studied context pack into the existing Probe tool-menu plan path", async () => {
    await withFixture(async ({ root, backroom }) => {
      const { graph, packet } = await buildFixture(root, backroom);
      const context = await Effect.runPromise(
        buildOpenAgentsAutopilotCoderStudiedContext({ graph, packet }),
      );
      const registryView = await Effect.runPromise(loadBlueprintSignatureRegistry({ sourceKind: "staticFixture" }));
      const lookup = await Effect.runPromise(
        lookupBlueprintSignatures({
          backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.tool_menu"],
          lookupId: "lookup.openagents.autopilot_coder.studied_context",
          registryView,
          request: {
            actorRef: "actor.openagents.autopilot_coder",
            allowedSurfaces: ["agent_api"],
            backendKind: "apple_fm_bridge",
            contextPackRef: context.contextPackRef,
            programSignatureIds: ["program_signature.probe.tool_menu.project.v1"],
            riskCeiling: "medium",
          },
        }),
      );
      const menuInput = applyOpenAgentsAutopilotCoderStudiedContextToToolMenuInput(
        {
          backendKind: "apple_fm_bridge",
          contextPackRefs: ["context_pack.probe.assignment.base"],
          deniedToolRefs: [],
          lookup,
          menuId: "probe_tool_menu.openagents.autopilot_coder.studied_context",
          sourceAuthorityRefs: ["source_authority.probe.assignment"],
          supportedToolRefs: ["tool.probe.read_file", "tool.probe.code_search", "tool.probe.record_evidence"],
        },
        context,
      );
      const menu = await Effect.runPromise(planProbeToolMenu(menuInput));

      expect(menu.tools.length).toBeGreaterThan(0);
      expect(menu.tools.every((tool) => tool.contextPackRefs.includes(context.contextPackRef))).toBe(true);
      expect(menu.tools.every((tool) =>
        tool.sourceAuthorityRefs.includes("authority.openagents.autopilot_coder.studied_context")
      )).toBe(true);
    });
  });

  test("measures a studied-context-enabled coder beating baseline through the S4 harness", async () => {
    await withFixture(async ({ root, backroom }) => {
      const { graph, packet } = await buildFixture(root, backroom);
      const examSet = await Effect.runPromise(
        buildOpenAgentsStudybenchHiddenEditExamSet({
          graph,
          maxExams: 1,
          packet,
        }),
      );
      const exam = examSet.exams[0]!;
      const context = await Effect.runPromise(
        buildOpenAgentsAutopilotCoderStudiedContext({
          editSitePath: exam.task.expectedFiles[0],
          graph,
          packet,
        }),
      );
      const { report } = await Effect.runPromise(
        runOpenAgentsStudybenchEvalHarness({
          candidates: [
            {
              attempts: [
                {
                  examRef: exam.examRef,
                  firstDivergenceStep: exam.idealTrajectoryRefs.length + 1,
                  satisfiedClaimIds: exam.task.rubric.map((claim) => claim.claim_id),
                  selectedFileRefs: [context.editSitePath],
                  wrongFileReadCount: 0,
                },
              ],
              candidateRef: "candidate.openagents.autopilot_coder.studied_context.v0",
              profile: "studied_substrate",
              studiedSubstrateAvailable: true,
            },
            {
              attempts: [
                {
                  examRef: exam.examRef,
                  firstDivergenceStep: 1,
                  satisfiedClaimIds: [],
                  selectedFileRefs: ["packages/probe/packages/runtime/src/benchmark/studybench.ts"],
                  wrongFileReadCount: 3,
                },
              ],
              candidateRef: "candidate.openagents.autopilot_coder.baseline_grep_and_guess.v0",
              profile: "baseline_grep_and_guess",
              studiedSubstrateAvailable: false,
            },
          ],
          examSet,
          generatedAt: "2026-06-18T00:00:00.000Z",
          graph,
          packet,
        }),
      );
      const studied = report.aggregateScores.find((score) => score.candidateProfile === "studied_substrate")!;
      const baseline = report.aggregateScores.find((score) => score.candidateProfile === "baseline_grep_and_guess")!;

      expect(report.comparison.studiedBeatsBaseline).toBe(true);
      expect(studied.passRateBps).toBeGreaterThan(baseline.passRateBps);
      expect(studied.totalWrongFileReadCount).toBeLessThan(baseline.totalWrongFileReadCount);
      expect(studied.meanFirstDivergenceStep).toBeGreaterThan(baseline.meanFirstDivergenceStep);
    });
  });
});
