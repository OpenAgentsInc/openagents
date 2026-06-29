import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsRepoCorpusManifest,
  decodeOpenAgentsRepoStudyArtifactIndex,
  generateOpenAgentsRepoStudyArtifact,
  OPENAGENTS_REPO_STUDY_ARTIFACT_INDEX_SCHEMA_REF,
  openAgentsRepoStudyArtifactIndexHash,
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
  "packages/probe/packages/runtime/src/benchmark/openagents-study-graph.ts": "export const graph = true;\n",
  "packages/probe/packages/runtime/src/benchmark/openagents-study-packet.ts": "export const packet = true;\n",
  "packages/probe/packages/runtime/src/benchmark/repo-corpus-manifest.ts": "export const manifest = true;\n",
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
  const parent = await mkdtemp(join(tmpdir(), "openagents-study-artifact-"));
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

function generate(root: string, backroom: string) {
  return Effect.runPromise(
    generateOpenAgentsRepoStudyArtifact({
      backroomRootDir: backroom,
      commit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      commitHistory,
      rootDir: root,
    }),
  );
}

describe("OpenAgents repo study artifact", () => {
  test("generates a verification-backed, source-grounded artifact over the real tree shape", async () => {
    await withFixture(async ({ root, backroom }) => {
      const artifact = await generate(root, backroom);

      expect(artifact.index.schemaRef).toBe(OPENAGENTS_REPO_STUDY_ARTIFACT_INDEX_SCHEMA_REF);
      expect(artifact.index.sourceBoundary).toBe("public_refs_only");
      expect(artifact.index.repo).toBe("OpenAgentsInc/openagents");
      expect(artifact.index.indexHash).toBe(openAgentsRepoStudyArtifactIndexHash(artifact.index));

      // Identity carries the underlying artifact digests.
      expect(artifact.index.packetHash).toBe(artifact.packet.packetHash);
      expect(artifact.index.graphHash).toBe(artifact.graph.graphHash);
      expect(artifact.index.corpusManifestHash).toBe(artifact.packet.corpusManifestHash);
      expect(artifact.index.corpusContentHash.startsWith("sha256:")).toBe(true);
      // The content hash is commit-independent: distinct from the commit-embedding
      // manifest hash (so it can stay stable across pure commit drift).
      expect(artifact.index.corpusContentHash).not.toBe(artifact.index.corpusManifestHash);
      expect(artifact.index.verificationHash).toBe(artifact.verification.verificationHash);
      expect(artifact.index.evalReportHash).toBe(artifact.evalReport.reportHash);

      // Verification passes the correctness gate.
      expect(artifact.index.correctnessGatePassed).toBe(true);
      expect(artifact.verification.correctnessGatePassed).toBe(true);
      expect(artifact.index.rejectedClaimCount).toBe(0);
      expect(artifact.index.acceptedClaimCount).toBeGreaterThan(0);

      // Eval harness shows source-grounded lift over baseline.
      expect(artifact.index.evalLift.passRateLiftBps).toBeGreaterThan(0);
      expect(artifact.index.evalLift.rubricScoreLiftBps).toBeGreaterThan(0);

      // Graph counts are recorded.
      expect(artifact.index.nodeCount).toBe(artifact.graph.nodes.length);
      expect(artifact.index.edgeCount).toBe(artifact.graph.edges.length);
      expect(artifact.index.evidenceSpanCount).toBe(artifact.packet.evidenceSpans.length);
    });
  });

  test("is deterministic: regenerating yields a byte-identical index identity", async () => {
    await withFixture(async ({ root, backroom }) => {
      const first = await generate(root, backroom);
      const second = await generate(root, backroom);

      expect(second.index.indexHash).toBe(first.index.indexHash);
      expect(second.packet.packetHash).toBe(first.packet.packetHash);
      expect(second.graph.graphHash).toBe(first.graph.graphHash);
    });
  });

  test("index round-trips through the public-projection decoder", async () => {
    await withFixture(async ({ root, backroom }) => {
      const artifact = await generate(root, backroom);
      const decoded = await Effect.runPromise(
        decodeOpenAgentsRepoStudyArtifactIndex(JSON.parse(JSON.stringify(artifact.index))),
      );

      expect(decoded.indexHash).toBe(artifact.index.indexHash);
    });
  });

  test("a committed study-artifact-index.json on disk does not perturb the corpus identity", async () => {
    await withFixture(async ({ root, backroom }) => {
      const before = await Effect.runPromise(
        buildOpenAgentsRepoCorpusManifest({
          commit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          repo: "OpenAgentsInc/openagents",
          rootDir: root,
        }),
      );

      // Persist the generated index where SA-1 commits it; it must be excluded.
      const artifact = await generate(root, backroom);
      const indexPath = join(
        root,
        "docs/research/machine-studying/openagents-studybench/study-packets/openagents.study-artifact-index.json",
      );
      await writeFile(indexPath, `${JSON.stringify(artifact.index, null, 2)}\n`, "utf8");

      const after = await Effect.runPromise(
        buildOpenAgentsRepoCorpusManifest({
          commit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          repo: "OpenAgentsInc/openagents",
          rootDir: root,
        }),
      );

      expect(after.manifestHash).toBe(before.manifestHash);
      expect(after.entries.some((entry) => entry.path.endsWith(".study-artifact-index.json"))).toBe(false);
    });
  });
});
