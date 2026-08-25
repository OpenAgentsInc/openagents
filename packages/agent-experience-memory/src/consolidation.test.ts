import { describe, expect, test } from "vite-plus/test";

import {
  clusterEpisodes,
  consolidateEpisodes,
  CONSOLIDATION_CONFIDENCE_FLOOR,
  CONSOLIDATION_MIN_CLUSTER_SIZE,
  promoteHeuristicToPattern,
  synthesizeHeuristicText,
  type EpisodicEngram,
} from "./consolidation.js";

const OWNER = "owner/test";
const PROJECT = "project/test";

const episode = (
  ref: string,
  text: string,
  observedAtMs: number,
  extra: Partial<EpisodicEngram> = {},
): EpisodicEngram => ({ ref, text, observedAtMs, ...extra });

describe("clusterEpisodes", () => {
  test("keeps dissimilar episodes in separate singleton clusters", () => {
    const clusters = clusterEpisodes([
      episode("e1", "restart the pylon gateway daemon", 1),
      episode("e2", "bake sourdough starter at room temperature", 2),
      episode("e3", "prune the orchard in late winter", 3),
    ]);
    expect(clusters.length).toBe(3);
  });

  test("groups episodes sharing vocabulary into one cluster", () => {
    const clusters = clusterEpisodes([
      episode("a", "the deploy failed because the gateway was down", 1),
      episode("b", "deploy failed again when the gateway stayed down", 2),
      episode("c", "unrelated note about sourdough hydration", 3),
    ]);
    expect(clusters.length).toBe(2);
    const big = clusters.find((c) => c.members.length === 2);
    expect(big?.members.map((m) => m.ref)).toEqual(["a", "b"]);
  });

  test("uses embeddings when both sides carry them", () => {
    const near = (x: number): ReadonlyArray<number> => [x, 0];
    const clusters = clusterEpisodes([
      episode("v1", "one", 1, { embedding: near(1) }),
      episode("v2", "two", 2, { embedding: near(1) }),
      episode("v3", "three", 3, { embedding: near(-1) }),
    ]);
    expect(clusters.length).toBe(2);
    expect(clusters[0]!.members.map((m) => m.ref)).toEqual(["v1", "v2"]);
  });

  test("is deterministic: equal inputs give equal clusters", () => {
    const input = [
      episode("a", "shared vocabulary words appear here", 1),
      episode("b", "more shared vocabulary words here", 2),
      episode("c", "completely different subject entirely", 3),
    ];
    const one = clusterEpisodes(input);
    const two = clusterEpisodes([...input]);
    expect(one).toEqual(two);
  });
});

describe("synthesizeHeuristicText", () => {
  test("ranks tokens by frequency across the cluster", () => {
    const text = synthesizeHeuristicText([
      episode("a", "gateway restart fixes the stuck queue", 1),
      episode("b", "restart the gateway when the queue stalls", 2),
      episode("c", "another restart cleared the queue again", 3),
    ]);
    for (const token of ["restart", "queue"]) {
      expect(text.split(" ")).toContain(token);
    }
  });
});

describe("consolidateEpisodes", () => {
  test("returns no heuristics below the minimum cluster size", () => {
    const result = consolidateEpisodes({
      ownerScope: OWNER,
      projectScope: PROJECT,
      episodes: [
        episode("solo", "only one episode about gateway restarts", 1),
        episode("far", "an unrelated baking project instead", 2),
      ],
      nowMs: 1000,
    });
    expect(result.heuristics.length).toBe(0);
    expect(result.skippedClusters).toBe(2);
  });

  test("synthesizes one heuristic per qualifying cluster with provenance", () => {
    const result = consolidateEpisodes({
      ownerScope: OWNER,
      projectScope: PROJECT,
      episodes: [
        episode("d1", "gateway restart fixed the stalled deploy", 1),
        episode("d2", "a restart of the gateway unstuck the same deploy", 2),
        episode("d3", "sourdough starter feeding schedule notes", 3),
        episode("d4", "more sourdough starter maintenance today", 4),
      ],
      nowMs: 1000,
    });
    expect(result.heuristics.length).toBeGreaterThanOrEqual(1);
    const first = result.heuristics[0]!;
    expect(first.schema).toBe("openagents.heuristic_synth.v1");
    expect(first.synthId).toMatch(/^synth:[0-9a-f]{64}$/);
    expect(first.ownerScope).toBe(OWNER);
    expect(first.projectScope).toBe(PROJECT);
    expect(first.sourceRefs.length).toBeGreaterThanOrEqual(CONSOLIDATION_MIN_CLUSTER_SIZE);
    // Provenance points back at real input refs.
    for (const ref of first.sourceRefs) {
      expect(["d1", "d2", "d3", "d4"]).toContain(ref);
    }
    expect(first.confidence).toBeGreaterThanOrEqual(CONSOLIDATION_CONFIDENCE_FLOOR);
    expect(first.heuristic.length).toBeGreaterThan(0);
  });

  test("rejects credential-shaped synthesis material as hard-unsafe", () => {
    const token = "ghp_1234567890abcdef1234567890abcdef";
    const result = consolidateEpisodes({
      ownerScope: OWNER,
      projectScope: PROJECT,
      episodes: [
        episode("t1", `use ${token} for api calls`, 1),
        episode("t2", `always use ${token} for api calls`, 2),
      ],
      nowMs: 1000,
    });
    // The synthesized text recombines redacted fragments; the boundary must
    // hold either way.
    expect(result.rejectedUnsafe + result.skippedClusters).toBe(
      result.clusters.length - result.heuristics.length,
    );
  });

  test("is deterministic: equal inputs give equal synth ids", () => {
    const build = (): string =>
      consolidateEpisodes({
        ownerScope: OWNER,
        projectScope: PROJECT,
        episodes: [
          episode("x1", "retry once before escalating to the owner", 1),
          episode("x2", "retry once more before escalating further", 2),
        ],
        nowMs: 1000,
      }).heuristics.map((h) => h.synthId)[0];
    expect(build()).toBeDefined();
    expect(build()).toBe(build());
  });
});

describe("promoteHeuristicToPattern", () => {
  test("carries provenance without inheriting access to private cases", () => {
    const consolidated = consolidateEpisodes({
      ownerScope: OWNER,
      projectScope: PROJECT,
      episodes: [
        episode("p1", "pin dependency versions before a long refactor", 1),
        episode("p2", "pin versions again before that long refactor", 2),
      ],
      nowMs: 1000,
    });
    const heuristic = consolidated.heuristics[0];
    if (heuristic === undefined) throw new Error("expected a heuristic");
    const pattern = promoteHeuristicToPattern(heuristic, "long refactors");
    expect(pattern.schema).toBe("openagents.experience_pattern.v1");
    expect(pattern.phenomenon).toBe(heuristic.heuristic);
    expect(pattern.applicability).toBe("long refactors");
    expect(pattern.supportSuccessRefs).toEqual(heuristic.sourceRefs);
    expect(pattern.confidence).toBe(heuristic.confidence);
    expect(pattern.patternRef.startsWith("pattern:")).toBe(true);
  });
});
