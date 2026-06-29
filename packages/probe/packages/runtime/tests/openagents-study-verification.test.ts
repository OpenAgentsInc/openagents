import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsRepoStudiedKnowledgeGraph,
  buildOpenAgentsRepoStudyPacket,
  openAgentsRepoStudiedKnowledgeVerificationHash,
  verifyOpenAgentsRepoStudiedKnowledgeClaims,
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
  const parent = await mkdtemp(join(tmpdir(), "openagents-study-verification-"));
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

describe("OpenAgents studied knowledge verification", () => {
  test("accepts replayed graph-edge and evidence-span claims deterministically", async () => {
    await withFixture(async ({ root, backroom }) => {
      const { graph, packet } = await buildFixture(root, backroom);
      const edge = graph.edges.find((candidate) => candidate.kind === "edit_site_respects_invariant")!;
      const span = packet.evidenceSpans[0]!;
      const first = await Effect.runPromise(
        verifyOpenAgentsRepoStudiedKnowledgeClaims({
          claims: [
            {
              claimRef: "claim.public.study_verification.edge.good",
              edgeRef: edge.ref,
              kind: "edge",
            },
            {
              claimRef: "claim.public.study_verification.span.good",
              kind: "evidence_span",
              spanHash: span.spanHash,
            },
          ],
          generatedAt: "2026-06-18T00:00:00.000Z",
          graph,
          packet,
          rootDir: root,
        }),
      );
      const second = await Effect.runPromise(
        verifyOpenAgentsRepoStudiedKnowledgeClaims({
          claims: [
            {
              claimRef: "claim.public.study_verification.edge.good",
              edgeRef: edge.ref,
              kind: "edge",
            },
            {
              claimRef: "claim.public.study_verification.span.good",
              kind: "evidence_span",
              spanHash: span.spanHash,
            },
          ],
          generatedAt: "2026-06-18T00:00:00.000Z",
          graph,
          packet,
          rootDir: root,
        }),
      );

      expect(first.verificationHash).toBe(second.verificationHash);
      expect(first.verificationHash).toBe(openAgentsRepoStudiedKnowledgeVerificationHash(first));
      expect(first.correctnessGatePassed).toBe(true);
      expect(first.acceptedCount).toBe(2);
      expect(first.rejectedCount).toBe(0);
      expect(first.validatorReviewRequired).toBe(false);
    });
  });

  test("rejects injected-wrong graph-edge and evidence-span claims", async () => {
    await withFixture(async ({ root, backroom }) => {
      const { graph, packet } = await buildFixture(root, backroom);
      const edge = graph.edges.find((candidate) => candidate.kind === "edit_site_respects_invariant")!;
      const span = packet.evidenceSpans[0]!;
      const report = await Effect.runPromise(
        verifyOpenAgentsRepoStudiedKnowledgeClaims({
          claims: [
            {
              claimRef: "claim.public.study_verification.edge.bad",
              claimedToNodeRef: "study_node.missing.deadbeef",
              edgeRef: edge.ref,
              kind: "edge",
            },
            {
              claimRef: "claim.public.study_verification.span.bad",
              claimedHash: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
              kind: "evidence_span",
              spanHash: span.spanHash,
            },
          ],
          generatedAt: "2026-06-18T00:00:00.000Z",
          graph,
          packet,
          rootDir: root,
        }),
      );

      expect(report.correctnessGatePassed).toBe(false);
      expect(report.acceptedCount).toBe(0);
      expect(report.rejectedCount).toBe(2);
      expect(report.results.map((result) => result.status)).toEqual(["rejected", "rejected"]);
      expect(report.results.flatMap((result) => result.blockerRefs)).toContain(
        "blocker.public.study_verification.edge_replay_mismatch",
      );
      expect(report.results.flatMap((result) => result.blockerRefs)).toContain(
        "blocker.public.study_verification.span_replay_mismatch",
      );
    });
  });

  test("routes non-deterministic remainder to validator review", async () => {
    await withFixture(async ({ root, backroom }) => {
      const { graph, packet } = await buildFixture(root, backroom);
      const report = await Effect.runPromise(
        verifyOpenAgentsRepoStudiedKnowledgeClaims({
          claims: [
            {
              claimRef: "claim.public.study_verification.review.remainder",
              kind: "validator_review_remainder",
              validatorReviewRef: "validator_review.public.study_verification.review_001",
            },
          ],
          generatedAt: "2026-06-18T00:00:00.000Z",
          graph,
          packet,
        }),
      );

      expect(report.correctnessGatePassed).toBe(false);
      expect(report.validatorReviewRequired).toBe(true);
      expect(report.results[0]).toMatchObject({
        status: "needs_validator_review",
        validatorReviewRefs: ["validator_review.public.study_verification.review_001"],
      });
    });
  });
});
