import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsRepoStudiedKnowledgeGraph,
  buildOpenAgentsRepoStudyPacket,
  decodeOpenAgentsRepoStudiedKnowledgeGraph,
  openAgentsRepoStudiedKnowledgeEdgeHash,
  openAgentsRepoStudiedKnowledgeGraphHash,
  traverseOpenAgentsRepoStudiedKnowledgeGraph,
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
  const parent = await mkdtemp(join(tmpdir(), "openagents-study-graph-"));
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

async function buildPacket(root: string, backroom: string) {
  return await Effect.runPromise(
    buildOpenAgentsRepoStudyPacket({
      backroomRootDir: backroom,
      commit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      commitHistory,
      generatedAt: "2026-06-18T00:00:00.000Z",
      repo: "OpenAgentsInc/openagents",
      rootDir: root,
    }),
  );
}

describe("OpenAgents studied knowledge graph", () => {
  test("emits a deterministic digest-pinned graph whose edges resolve to nodes", async () => {
    await withFixture(async ({ root, backroom }) => {
      const packet = await buildPacket(root, backroom);
      const first = await Effect.runPromise(
        buildOpenAgentsRepoStudiedKnowledgeGraph({
          generatedAt: "2026-06-18T00:00:00.000Z",
          packet,
        }),
      );
      const second = await Effect.runPromise(
        buildOpenAgentsRepoStudiedKnowledgeGraph({
          generatedAt: "2026-06-18T00:00:00.000Z",
          packet,
        }),
      );
      const nodeRefs = new Set(first.nodes.map((node) => node.ref));
      const nodeKinds = new Set(first.nodes.map((node) => node.kind));

      expect(first.graphHash).toBe(second.graphHash);
      expect(first.graphRef).toBe(second.graphRef);
      expect(first.graphHash).toBe(openAgentsRepoStudiedKnowledgeGraphHash(first));
      expect(nodeKinds).toEqual(new Set(["code", "commit", "doc", "evidence_span", "invariant", "issue", "rationale"]));
      expect(first.edges.every((edge) => nodeRefs.has(edge.fromNodeRef) && nodeRefs.has(edge.toNodeRef))).toBe(true);
      expect(first.edges.some((edge) => edge.kind === "code_warned_by_rejected_lineage")).toBe(true);
      expect(first.edges.some((edge) => edge.kind === "issue_tracks_edit_site")).toBe(true);
    });
  });

  test("traverses from an edit site to invariant, commit, audit, issue, and rejected lineage", async () => {
    await withFixture(async ({ root, backroom }) => {
      const packet = await buildPacket(root, backroom);
      const graph = await Effect.runPromise(
        buildOpenAgentsRepoStudiedKnowledgeGraph({
          generatedAt: "2026-06-18T00:00:00.000Z",
          packet,
        }),
      );
      const traversal = await Effect.runPromise(
        traverseOpenAgentsRepoStudiedKnowledgeGraph(graph, {
          path: "packages/probe/packages/runtime/src/benchmark/openagents-study-packet.ts",
        }),
      );

      expect(traversal.graphHash).toBe(graph.graphHash);
      expect(traversal.invariantNodeRefs.length).toBeGreaterThan(0);
      expect(traversal.commitNodeRefs.length).toBe(1);
      expect(traversal.auditNodeRefs.length).toBeGreaterThan(0);
      expect(traversal.issueNodeRefs.length).toBe(1);
      expect(traversal.rejectedLineageNodeRefs.length).toBe(1);
      expect(traversal.traversedEdgeRefs.length).toBeGreaterThanOrEqual(5);
    });
  });

  test("rejects graph edges that no longer resolve", async () => {
    await withFixture(async ({ root, backroom }) => {
      const packet = await buildPacket(root, backroom);
      const graph = await Effect.runPromise(
        buildOpenAgentsRepoStudiedKnowledgeGraph({
          generatedAt: "2026-06-18T00:00:00.000Z",
          packet,
        }),
      );
      const firstEdge = graph.edges[0]!;
      const mutatedEdgeBody = {
        fromNodeRef: "study_node.missing.deadbeef",
        kind: firstEdge.kind,
        rationaleRef: firstEdge.rationaleRef,
        sourceEvidenceNodeRefs: firstEdge.sourceEvidenceNodeRefs,
        toNodeRef: firstEdge.toNodeRef,
      };
      const mutatedEdgeHash = openAgentsRepoStudiedKnowledgeEdgeHash(mutatedEdgeBody);
      const mutatedGraphBody = {
        ...graph,
        edges: [
          {
            ...mutatedEdgeBody,
            edgeHash: mutatedEdgeHash,
            ref: `study_edge.${firstEdge.kind}.${mutatedEdgeHash.replace(/^sha256:/, "").slice(0, 16)}`,
          },
          ...graph.edges.slice(1),
        ],
        graphHash: "sha256:pending",
      };
      const mutatedGraph = {
        ...mutatedGraphBody,
        graphHash: openAgentsRepoStudiedKnowledgeGraphHash(mutatedGraphBody),
      };

      await expect(
        Effect.runPromise(decodeOpenAgentsRepoStudiedKnowledgeGraph(mutatedGraph)),
      ).rejects.toMatchObject({
        _tag: "ProbeBenchmarkContractError",
        path: "studyGraph.edges[0].fromNodeRef",
      });
    });
  });
});
