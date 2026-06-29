import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  decodeOpenAgentsRepoStudyArtifactFreshness,
  evaluateOpenAgentsRepoStudyFreshness,
  OPENAGENTS_REPO_STUDY_FRESHNESS_SCHEMA_REF,
  type OpenAgentsRepoStudyArtifactIndex,
} from "../src";

// A minimal, schema-valid committed index. `indexHash` is recomputed by helpers
// below so the index passes the SA-1 decoder when needed; these tests exercise
// only the freshness verdict, which compares two already-built indexes.
const baseIndex: OpenAgentsRepoStudyArtifactIndex = {
  acceptedClaimCount: 191,
  commit: "d2ccc8064edd40b532fcaa456bc89b6cbbcb7dc3",
  commitHistoryLimit: 200,
  corpusContentHash: "sha256:c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0",
  corpusEntryCount: 133,
  corpusManifestHash: "sha256:5acead9c68d3057db03ccdd8eb5713ad93f873b9c66637c326ac07b9ca9f4223",
  corpusManifestRef: "openagents_repo_corpus_manifest.5acead9c68d3057d",
  correctnessGatePassed: true,
  edgeCount: 179,
  evalLift: { firstDivergenceStepLift: 4, passRateLiftBps: 10000, rubricScoreLiftBps: 10000 },
  evalReportHash: "sha256:5b318a3bed47c15041eed0b311470e7e7c48420b866dd6fb5f10e05dad7e6343",
  evalReportRef: "openagents_studybench_eval_harness.5b318a3bed47c150",
  evidenceSpanCount: 12,
  graphHash: "sha256:ed78d55d3b82256d09aeaaf49201a97a79bdf26ae22b23f6cc3220d884510e22",
  graphRef: "openagents_repo_studied_knowledge_graph.ed78d55d3b82256d",
  indexHash: "sha256:0a45faea6b6dc258863824f3b449dfce8e1ad85b48b2131515f1b26a5836fa16",
  indexRef: "openagents_repo_study_artifact_index.0a45faea6b6dc258",
  nodeCount: 323,
  packetHash: "sha256:c1380451555450700c6d3f3e54263be79bc6ab80a7c8baad3762057596d0c987",
  packetRef: "openagents_repo_study_packet.c138045155545070",
  rejectedClaimCount: 0,
  repo: "OpenAgentsInc/openagents",
  schemaRef: "openagents.repo_study_artifact_index.v0",
  sourceBoundary: "public_refs_only",
  verificationHash: "sha256:657a150aa6f7d35934ade9fdfb9965e9ef681818ea820e5b6c81c3072400051c",
  verificationRef: "openagents_repo_studied_knowledge_verification.657a150aa6f7d359",
};

function index(overrides: Partial<OpenAgentsRepoStudyArtifactIndex>): OpenAgentsRepoStudyArtifactIndex {
  return { ...baseIndex, ...overrides };
}

function evaluate(input: Parameters<typeof evaluateOpenAgentsRepoStudyFreshness>[0]) {
  return Effect.runPromise(evaluateOpenAgentsRepoStudyFreshness(input));
}

describe("OpenAgents repo study freshness", () => {
  test("pure commit drift (HEAD moved, content identical) is FRESH, not stale", async () => {
    // Same corpus manifest, different commit/indexHash — exactly the every-commit
    // case that a naive indexHash gate would (wrongly) call stale.
    const committed = index({});
    const regenerated = index({
      commit: "fd1ba44414b82b0d59002421aa12695e9294bf71",
      indexHash: "sha256:bfbdc106c88c56a8c258dc74064152c9c76ff5e482c49ee31e2227a7985ae69d",
      indexRef: "openagents_repo_study_artifact_index.bfbdc106c88c56a8",
    });

    const freshness = await evaluate({ committed, regenerated, commitsBehind: 7 });

    expect(freshness.status).toBe("fresh");
    expect(freshness.recommendation).toBe("none");
    expect(freshness.contentDrift).toBe(false);
    expect(freshness.commitDrift).toBe(true);
    expect(freshness.commitsBehind).toBe(7);
    expect(freshness.staleSinceCommit).toBe(committed.commit);
    expect(freshness.headCommit).toBe(regenerated.commit);
  });

  test("content drift (admitted source file changed) is STALE with refresh_index", async () => {
    const committed = index({});
    const regenerated = index({
      commit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      corpusContentHash: "sha256:4a4ca3f7e7ec0a25b367f35fd44bf871ee9be1c1ec71f128aa29e2577d22c21d",
      corpusManifestHash: "sha256:1212121212121212121212121212121212121212121212121212121212121212",
      indexHash: "sha256:1111111111111111111111111111111111111111111111111111111111111111",
      indexRef: "openagents_repo_study_artifact_index.1111111111111111",
    });

    const freshness = await evaluate({ committed, regenerated, commitsBehind: 1 });

    expect(freshness.status).toBe("stale");
    expect(freshness.recommendation).toBe("refresh_index");
    expect(freshness.contentDrift).toBe(true);
    expect(freshness.commitDrift).toBe(false);
    expect(freshness.committedCorpusContentHash).toBe(committed.corpusContentHash);
    expect(freshness.regeneratedCorpusContentHash).toBe(regenerated.corpusContentHash);
  });

  test("a red correctness gate is GATE_FAILED with reverify_gate, overriding content drift", async () => {
    const committed = index({});
    const regenerated = index({
      correctnessGatePassed: false,
      corpusContentHash: "sha256:4a4ca3f7e7ec0a25b367f35fd44bf871ee9be1c1ec71f128aa29e2577d22c21d",
      indexHash: "sha256:2222222222222222222222222222222222222222222222222222222222222222",
      indexRef: "openagents_repo_study_artifact_index.2222222222222222",
    });

    const freshness = await evaluate({ committed, regenerated });

    expect(freshness.status).toBe("gate_failed");
    expect(freshness.recommendation).toBe("reverify_gate");
    expect(freshness.correctnessGatePassed).toBe(false);
  });

  test("identical committed and regenerated index is FRESH with no drift", async () => {
    const committed = index({});
    const freshness = await evaluate({ committed, regenerated: committed });

    expect(freshness.status).toBe("fresh");
    expect(freshness.contentDrift).toBe(false);
    expect(freshness.commitDrift).toBe(false);
    expect(freshness.commitsBehind).toBe(0);
    expect(freshness.headCommit).toBe(committed.commit);
  });

  test("the freshness verdict is a deterministic, hash-pinned public projection", async () => {
    const committed = index({});
    const regenerated = index({ commit: "fd1ba44414b82b0d59002421aa12695e9294bf71", indexHash: "sha256:bb" + "0".repeat(62) });

    const first = await evaluate({ committed, regenerated, commitsBehind: 2 });
    const second = await evaluate({ committed, regenerated, commitsBehind: 2 });

    expect(second.freshnessHash).toBe(first.freshnessHash);
    expect(first.schemaRef).toBe(OPENAGENTS_REPO_STUDY_FRESHNESS_SCHEMA_REF);
    expect(first.sourceBoundary).toBe("public_refs_only");

    const decoded = await Effect.runPromise(
      decodeOpenAgentsRepoStudyArtifactFreshness(JSON.parse(JSON.stringify(first))),
    );
    expect(decoded.freshnessHash).toBe(first.freshnessHash);
  });

  test("a tampered freshnessHash is rejected by the decoder", async () => {
    const committed = index({});
    const freshness = await evaluate({ committed, regenerated: committed });
    const tampered = { ...freshness, status: "stale" as const };

    const result = await Effect.runPromiseExit(decodeOpenAgentsRepoStudyArtifactFreshness(tampered));
    expect(result._tag).toBe("Failure");
  });

  test("mismatched repos are rejected", async () => {
    const committed = index({});
    const regenerated = index({ repo: "OpenAgentsInc/other" });

    const result = await Effect.runPromiseExit(
      evaluateOpenAgentsRepoStudyFreshness({ committed, regenerated }),
    );
    expect(result._tag).toBe("Failure");
  });

  test("a negative commitsBehind is rejected", async () => {
    const committed = index({});
    const result = await Effect.runPromiseExit(
      evaluateOpenAgentsRepoStudyFreshness({ committed, regenerated: committed, commitsBehind: -1 }),
    );
    expect(result._tag).toBe("Failure");
  });
});
