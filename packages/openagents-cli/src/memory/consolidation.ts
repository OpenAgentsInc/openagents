// Vendored from packages/agent-experience-memory/src/consolidation.ts by scripts/vendor-memory.mjs — do not edit here.
// The drift guard (test/vendored-memory-drift.test.ts) fails when this copy
// no longer matches the canonical source.
import { Schema as S } from "effect";

import type { GlobalPattern } from "./pattern.js";
import { FactRef, OwnerScopeId, PatternRef, ProjectScopeId } from "./refs.js";
import { canonicalStringify } from "./canonical.js";
import { sha256Hex } from "./sha256.js";
import {
  cosineSimilarity,
} from "./ranking.js";
import { guardEngramContent } from "./engram.js";

/**
 * Background autonomous consolidation — "agent dreaming" / heuristic synthesis
 * (issue #224).
 *
 * Consolidation is an OFFLINE, background pass over already-redacted episodic
 * material. It never runs inside a turn, never reads a raw prompt or trajectory,
 * and never blocks recall: a host schedules it when idle, exactly like a freeze.
 *
 * The pass has three stages:
 *
 * 1. CLUSTER — related episodic engrams are grouped by cosine similarity of
 *    their embeddings (single-linkage with a bounded threshold), so no center,
 *    no iteration count, and no randomness is involved and equal inputs always
 *    give equal clusters.
 * 2. SYNTHESIZE — each cluster of MIN_CLUSTER_SIZE or more episodes becomes one
 *    synthesized heuristic engram: a redacted heuristic sentence distilled by a
 *    pure, deterministic summarizer (shared-token distillation, most recent
 *    first). No model provider is consulted, so the pass cannot fail open and
 *    cannot leak: the synthesizer only ever recombines text that already passed
 *    the engram redaction boundary at capture time.
 * 3. PROMOTE — a synthesis whose support meets the confidence floor is promoted
 *    into a `GlobalPattern` (the reviewed AFS-10 distilled layer), carrying its
 *    supporting success/failure fact references.
 *
 * Every produced value passes `guardEngramContent` again before it can be
 * signed, because synthesis recombines redacted fragments and the boundary is
 * per-value, not per-source.
 */

/** A single episodic memory offered to the consolidation pass. */
export interface EpisodicEngram {
  readonly ref: string;
  readonly text: string;
  /** Capture-time embedding; may be absent for a text-only episode. */
  readonly embedding?: ReadonlyArray<number>;
  /** Epoch milliseconds of the episode observation, used for recency order. */
  readonly observedAtMs: number;
  readonly admission?: "admitted" | "candidate" | "rejected";
}

export const CONSOLIDATION_MIN_CLUSTER_SIZE = 2 as const;
export const CONSOLIDATION_SIMILARITY_THRESHOLD = 0.5 as const;
export const CONSOLIDATION_CONFIDENCE_FLOOR = 0.6 as const;
/** Upper bound on episodes in one cluster considered by the synthesizer. */
export const CONSOLIDATION_MAX_EPISODES_PER_CLUSTER = 16 as const;

/** The synthesized heuristic engram — the output of one dream cycle. */
export const SynthesizedHeuristic = S.Struct({
  schema: S.Literal("openagents.heuristic_synth.v1"),
  synthId: S.String.check(S.isPattern(/^synth:[a-f0-9]{64}$/)),
  ownerScope: OwnerScopeId,
  projectScope: ProjectScopeId,
  /** The redacted heuristic sentence. */
  heuristic: S.String.check(S.isMinLength(1), S.isMaxLength(1000)),
  /** The episode refs that support the heuristic, most recent first. */
  sourceRefs: S.Array(FactRef),
  /**
   * Support strength: |supporting| / |episodes| inside the synthesizing
   * cluster, in [0, 1].
   */
  confidence: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  synthesizedAt: S.String.check(
    S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
  ),
});
export type SynthesizedHeuristic = typeof SynthesizedHeuristic.Type;

const decodeSynth = S.decodeUnknownSync(SynthesizedHeuristic);
const decodePatternRef = S.decodeUnknownSync(PatternRef);
const decodeFactRef = S.decodeUnknownSync(FactRef);

const tokenize = (text: string): ReadonlyArray<string> =>
  text
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((token) => token.length > 1);

/** Deterministic pairwise affinity between two episodes, in [0, 1]. */
const episodeAffinity = (
  left: EpisodicEngram,
  right: EpisodicEngram,
): number => {
  if (left.embedding !== undefined && right.embedding !== undefined) {
    // Cosine is in [-1, 1]; rescale to [0, 1] so a fixed threshold reads the
    // same way for text-overlap ties.
    return (cosineSimilarity(left.embedding, right.embedding) + 1) / 2;
  }
  const leftTokens = new Set(tokenize(left.text));
  const rightTokens = new Set(tokenize(right.text));
  if (leftTokens.size === 0 || rightTokens.size === 0) return 0;
  let shared = 0;
  for (const token of leftTokens) {
    if (rightTokens.has(token)) shared += 1;
  }
  return (2 * shared) / (leftTokens.size + rightTokens.size); // Dice coefficient
};

export type EpisodeCluster = Readonly<{
  readonly members: ReadonlyArray<EpisodicEngram>;
}>;

/**
 * Cluster episodes by single-linkage above the similarity threshold. Order is
 * fully determined by input order: seed order follows first appearance, member
 * order preserves input order, so equal inputs always give equal clusters.
 */
export const clusterEpisodes = (
  episodes: ReadonlyArray<EpisodicEngram>,
  options: Readonly<{ threshold?: number }> = {},
): ReadonlyArray<EpisodeCluster> => {
  const threshold = options.threshold ?? CONSOLIDATION_SIMILARITY_THRESHOLD;
  const clusters: Array<Array<EpisodicEngram>> = [];
  const assignment = new Map<string, number>();
  for (const episode of episodes) {
    let home: number | undefined;
    for (let index = 0; index < clusters.length; index += 1) {
      const cluster = clusters[index];
      if (
        cluster !== undefined &&
        cluster.some((member) => episodeAffinity(member, episode) >= threshold)
      ) {
        home = index;
        break;
      }
    }
    if (home === undefined) {
      clusters.push([episode]);
      assignment.set(episode.ref, clusters.length - 1);
      continue;
    }
    clusters[home]!.push(episode);
    assignment.set(episode.ref, home);
  }
  return clusters.map((members) => ({ members }) as const);
};

/**
 * Distill a cluster into one heuristic sentence without a model provider:
 * rank tokens by document frequency across the cluster's episodes (ties break
 * on first appearance, then lexicographic), take the top few, and join them
 * with the shared action verbs when present. The output is built only from
 * words that appear in the (already redacted) episode text.
 */
const HEURISTIC_TOKEN_BUDGET = 8;

export const synthesizeHeuristicText = (
  members: ReadonlyArray<EpisodicEngram>,
): string => {
  const frequency = new Map<string, number>();
  const firstSeen = new Map<string, number>();
  for (const member of members) {
    for (const token of tokenize(member.text)) {
      frequency.set(token, (frequency.get(token) ?? 0) + 1);
      if (!firstSeen.has(token)) firstSeen.set(token, members.indexOf(member));
    }
  }
  const ranked = [...frequency.entries()]
    .sort(([leftToken, leftCount], [rightToken, rightCount]) => {
      if (rightCount !== leftCount) return rightCount - leftCount;
      const leftIndex = firstSeen.get(leftToken) ?? Number.MAX_SAFE_INTEGER;
      const rightIndex = firstSeen.get(rightToken) ?? Number.MAX_SAFE_INTEGER;
      if (leftIndex !== rightIndex) return leftIndex - rightIndex;
      return leftToken < rightToken ? -1 : leftToken > rightToken ? 1 : 0;
    })
    .slice(0, HEURISTIC_TOKEN_BUDGET)
    .map(([token]) => token);
  return ranked.length > 0 ? ranked.join(" ") : "";
};

const isoTimestamp = (epochMs: number): string => new Date(epochMs).toISOString();

/**
 * Run one offline consolidation ("dream") cycle.
 *
 * Episodes are clustered, each qualifying cluster is distilled into one
 * synthesized heuristic, and every candidate passes the strict engram guard
 * before it can be returned. Deterministic: no provider, no clock beyond the
 * supplied `nowMs`, no randomness.
 */
export const consolidateEpisodes = (
  input: Readonly<{
    ownerScope: string;
    projectScope: string;
    episodes: ReadonlyArray<EpisodicEngram>;
    nowMs: number;
    minClusterSize?: number;
    confidenceFloor?: number;
  }>,
): Readonly<{
  heuristics: ReadonlyArray<SynthesizedHeuristic>;
  skippedClusters: number;
  rejectedUnsafe: number;
  clusters: ReadonlyArray<EpisodeCluster>;
}> => {
  const minClusterSize = input.minClusterSize ?? CONSOLIDATION_MIN_CLUSTER_SIZE;
  const confidenceFloor = input.confidenceFloor ?? CONSOLIDATION_CONFIDENCE_FLOOR;
  const clusters = clusterEpisodes(input.episodes);
  const heuristics: Array<SynthesizedHeuristic> = [];
  let skippedClusters = 0;
  let rejectedUnsafe = 0;
  for (const cluster of clusters) {
    if (cluster.members.length < minClusterSize) {
      skippedClusters += 1;
      continue;
    }
    // Most recent first, tie-broken by ref, then bounded.
    const ordered = [...cluster.members]
      .sort((left, right) =>
        right.observedAtMs !== left.observedAtMs
          ? right.observedAtMs - left.observedAtMs
          : left.ref < right.ref
            ? -1
            : left.ref > right.ref
              ? 1
              : 0,
      )
      .slice(0, CONSOLIDATION_MAX_EPISODES_PER_CLUSTER);
    const text = synthesizeHeuristicText(ordered);
    if (text.length === 0) {
      skippedClusters += 1;
      continue;
    }
    const verdict = guardEngramContent(text);
    if (!verdict.storable || verdict.redacted === null || verdict.redacted.trim().length === 0) {
      rejectedUnsafe += 1;
      continue;
    }
    const supporting = ordered.filter((member) => member.admission !== "rejected");
    const confidence = supporting.length / ordered.length;
    if (confidence < confidenceFloor) {
      skippedClusters += 1;
      continue;
    }
    const digest = sha256Hex(canonicalStringify({ heuristic: verdict.redacted, refs: ordered.map((m) => m.ref) }));
    heuristics.push(
      decodeSynth({
        schema: "openagents.heuristic_synth.v1",
        synthId: `synth:${digest}`,
        ownerScope: input.ownerScope,
        projectScope: input.projectScope,
        heuristic: verdict.redacted,
        sourceRefs: ordered.map((member) => decodeFactRef(member.ref)),
        confidence,
        synthesizedAt: isoTimestamp(input.nowMs),
      }),
    );
  }
  return { heuristics, skippedClusters, rejectedUnsafe, clusters } as const;
};

/**
 * Promote a synthesized heuristic into the reviewed distilled global-pattern
 * layer when its support clears the confidence floor. The pattern inherits no
 * access to the private episodes behind it — it carries only their refs.
 */
export const promoteHeuristicToPattern = (
  heuristic: SynthesizedHeuristic,
  applicability: string,
): GlobalPattern => ({
  schema: "openagents.experience_pattern.v1",
  patternRef: decodePatternRef(`pattern:${heuristic.synthId.slice("synth:".length)}`),
  ownerScope: heuristic.ownerScope,
  projectScope: heuristic.projectScope,
  phenomenon: heuristic.heuristic,
  applicability,
  expectedEffect: "act earlier on the recurring situation the heuristic names",
  supportSuccessRefs: heuristic.sourceRefs,
  supportFailureRefs: [],
  confidence: heuristic.confidence,
  observedAt: heuristic.synthesizedAt,
  digest: sha256Hex(canonicalStringify({ phenomenon: heuristic.heuristic })),
});
