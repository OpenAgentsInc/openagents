import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsRepoStudyPacket,
  decodeOpenAgentsRepoStudyPacket,
  openAgentsRepoStudyPacketHash,
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
  const parent = await mkdtemp(join(tmpdir(), "openagents-study-packet-"));
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

function buildPacket(root: string, backroom: string) {
  return Effect.runPromise(
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

describe("OpenAgents repo study packet ingest", () => {
  test("emits a deterministic digest-pinned study packet across repeated runs", async () => {
    await withFixture(async ({ root, backroom }) => {
      const first = await buildPacket(root, backroom);
      const second = await buildPacket(root, backroom);

      expect(first.packetHash).toBe(second.packetHash);
      expect(first.packetRef).toBe(second.packetRef);
      expect(first.packetHash).toBe(openAgentsRepoStudyPacketHash(first));
      expect(first.corpusManifestHash).toBe(second.corpusManifestHash);
      expect(first.evidenceSpans.map((span) => span.spanHash)).toEqual(
        second.evidenceSpans.map((span) => span.spanHash),
      );
    });
  });

  test("includes code, commit-history, rationale, backroom, and evidence-span entries", async () => {
    await withFixture(async ({ root, backroom }) => {
      const packet = await buildPacket(root, backroom);
      const sectionKinds = new Set(packet.sections.map((section) => section.kind));
      const rationaleKinds = new Set(packet.rationaleSources.map((source) => source.kind));
      const evidencePaths = new Set(packet.evidenceSpans.map((span) => span.evidence.path));

      expect(packet.commitHistory).toHaveLength(2);
      expect(sectionKinds).toEqual(
        new Set([
          "source_map",
          "invariant_map",
          "typed_ref_glossary",
          "trap_catalog",
          "test_command_catalog",
          "edit_playbook",
          "retained_failure_fixture",
        ]),
      );
      expect(rationaleKinds.has("backroom_archive")).toBe(true);
      expect(packet.rationaleSources.find((source) => source.kind === "backroom_archive")?.availability).toBe(
        "available",
      );
      expect(evidencePaths.has("AGENTS.md")).toBe(true);
      expect(evidencePaths.has("INVARIANTS.md")).toBe(true);
      expect(evidencePaths.has("packages/probe/packages/runtime/src/benchmark/repo-corpus-manifest.ts")).toBe(true);
      expect(packet.sections.some((section) => section.corpusEntryPaths.includes("packages/tassadar-executor/README.md"))).toBe(
        true,
      );
    });
  });

  test("decodes and rejects mutated packet hashes", async () => {
    await withFixture(async ({ root, backroom }) => {
      const packet = await buildPacket(root, backroom);

      await expect(Effect.runPromise(decodeOpenAgentsRepoStudyPacket(packet))).resolves.toMatchObject({
        packetHash: packet.packetHash,
      });
      await expect(
        Effect.runPromise(
          decodeOpenAgentsRepoStudyPacket({
            ...packet,
            packetHash: "sha256:wrong",
          }),
        ),
      ).rejects.toMatchObject({
        _tag: "ProbeBenchmarkContractError",
        path: "repoStudyPacket.packetHash",
      });
    });
  });
});
